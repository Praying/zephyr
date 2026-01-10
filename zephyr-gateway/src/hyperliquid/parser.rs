//! Hyperliquid market data parser implementation.
//!
//! Implements the [`MarketDataParser`] trait for Hyperliquid WebSocket streams.
//! Hyperliquid is a decentralized perpetual exchange with its own L1 blockchain.

use async_trait::async_trait;
use parking_lot::RwLock;
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, info, warn};

use zephyr_core::data::{KlineData, KlinePeriod, OrderBook, OrderBookLevel, TickData};
use zephyr_core::error::NetworkError;
use zephyr_core::traits::{MarketDataParser, ParserCallback, ParserConfig};
use zephyr_core::types::{Amount, Price, Quantity, Symbol, Timestamp};

use crate::ws::{WebSocketCallback, WebSocketClient, WebSocketConfig, WebSocketMessage};

use super::types::*;

/// Hyperliquid market data parser.
///
/// Parses WebSocket streams from Hyperliquid DEX and converts them
/// to standardized [`TickData`] and [`KlineData`] structures.
///
/// # Supported Streams
///
/// - `allMids` - All mid prices
/// - `l2Book` - L2 order book
/// - `trades` - Trade stream
/// - `candle` - Kline/candlestick data
///
/// # Example
///
/// ```ignore
/// use zephyr_gateway::hyperliquid::HyperliquidParser;
/// use zephyr_core::traits::{ParserConfig, MarketDataParser};
///
/// let mut parser = HyperliquidParser::new();
/// let config = ParserConfig::builder()
///     .endpoint("wss://api.hyperliquid.xyz/ws")
///     .exchange("hyperliquid")
///     .build();
///
/// parser.init(&config).await?;
/// parser.connect().await?;
/// parser.subscribe(&[Symbol::new("BTC-USD")?]).await?;
/// ```
pub struct HyperliquidParser {
    /// Market type (perpetual, spot)
    market: HyperliquidMarket,
    /// WebSocket client
    ws_client: Option<WebSocketClient>,
    /// Parser configuration
    config: Option<ParserConfig>,
    /// User callback
    callback: Option<Arc<dyn ParserCallback>>,
    /// Subscribed symbols
    subscribed_symbols: Arc<RwLock<HashSet<Symbol>>>,
    /// Connection state
    connected: Arc<AtomicBool>,
    /// Asset index mapping (coin name -> index)
    asset_indices: Arc<RwLock<HashMap<String, u32>>>,
    /// Last mid prices cache
    last_mids: Arc<RwLock<HashMap<String, Price>>>,
    /// Use testnet
    testnet: bool,
}

impl HyperliquidParser {
    /// Creates a new Hyperliquid parser for perpetual market.
    #[must_use]
    pub fn new() -> Self {
        Self::with_market(HyperliquidMarket::Perpetual)
    }

    /// Creates a new Hyperliquid parser for spot market.
    #[must_use]
    pub fn new_spot() -> Self {
        Self::with_market(HyperliquidMarket::Spot)
    }

    /// Creates a new Hyperliquid parser with specified market type.
    #[must_use]
    pub fn with_market(market: HyperliquidMarket) -> Self {
        Self {
            market,
            ws_client: None,
            config: None,
            callback: None,
            subscribed_symbols: Arc::new(RwLock::new(HashSet::new())),
            connected: Arc::new(AtomicBool::new(false)),
            asset_indices: Arc::new(RwLock::new(HashMap::new())),
            last_mids: Arc::new(RwLock::new(HashMap::new())),
            testnet: false,
        }
    }

    /// Sets whether to use testnet.
    #[must_use]
    pub fn with_testnet(mut self, testnet: bool) -> Self {
        self.testnet = testnet;
        self
    }

    /// Returns the market type.
    #[must_use]
    pub fn market(&self) -> HyperliquidMarket {
        self.market
    }

    /// Returns whether using testnet.
    #[must_use]
    pub fn is_testnet(&self) -> bool {
        self.testnet
    }

    /// Converts a symbol to Hyperliquid format (e.g., "BTC-USD" -> "BTC").
    fn to_hyperliquid_coin(symbol: &Symbol) -> String {
        // Hyperliquid uses just the base asset name (e.g., "BTC", "ETH")
        let s = symbol.as_str();
        if let Some(pos) = s.find('-') {
            s[..pos].to_string()
        } else {
            s.to_string()
        }
    }

    /// Converts a Hyperliquid coin to standard symbol format (e.g., "BTC" -> "BTC-USD").
    fn from_hyperliquid_coin(coin: &str) -> Option<Symbol> {
        // Hyperliquid perpetuals are USD-denominated
        Symbol::new(format!("{coin}-USD")).ok()
    }

    /// Parses a WebSocket message and dispatches to callback.
    async fn handle_message(&self, text: &str) {
        let callback = match &self.callback {
            Some(cb) => cb,
            None => return,
        };

        // Try to parse as WebSocket response
        if let Ok(response) = serde_json::from_str::<HyperliquidWsResponse>(text) {
            match response.channel.as_str() {
                "allMids" => {
                    if let Ok(mids) = serde_json::from_value::<HyperliquidAllMids>(response.data) {
                        self.handle_all_mids(&mids, callback.as_ref()).await;
                    }
                }
                "l2Book" => {
                    if let Ok(book) = serde_json::from_value::<HyperliquidL2Book>(response.data) {
                        if let Some(orderbook) = self.parse_l2_book(&book) {
                            callback.on_orderbook(orderbook).await;
                        }
                    }
                }
                "trades" => {
                    if let Ok(trades) =
                        serde_json::from_value::<Vec<HyperliquidTrade>>(response.data)
                    {
                        for trade in &trades {
                            if let Some(tick) = self.parse_trade(trade) {
                                callback.on_tick(tick).await;
                            }
                        }
                    }
                }
                "candle" => {
                    if let Ok(candles) =
                        serde_json::from_value::<Vec<HyperliquidCandle>>(response.data)
                    {
                        for candle in &candles {
                            if let Some(kline) = self.parse_candle(candle, &response.channel) {
                                debug!(
                                    symbol = %kline.symbol,
                                    period = %kline.period,
                                    close = %kline.close,
                                    "Kline received"
                                );
                            }
                        }
                    }
                }
                _ => {
                    debug!(channel = %response.channel, "Unknown channel");
                }
            }
        } else {
            // Try to parse as subscription confirmation or error
            debug!(message = %text, "Unhandled message");
        }
    }

    /// Handles all mids update.
    async fn handle_all_mids(&self, mids: &HyperliquidAllMids, callback: &dyn ParserCallback) {
        let timestamp = Timestamp::now();

        // Collect ticks to send outside the lock
        let ticks_to_send: Vec<TickData> = {
            let subscribed = self.subscribed_symbols.read();
            mids.mids
                .iter()
                .filter_map(|(coin, price_str)| {
                    let price_dec = price_str.parse::<Decimal>().ok()?;
                    let price = Price::new(price_dec).ok()?;

                    // Update cache
                    self.last_mids.write().insert(coin.clone(), price);

                    // Check if we're subscribed to this symbol
                    let symbol = Self::from_hyperliquid_coin(coin)?;
                    if subscribed.contains(&symbol) {
                        TickData::builder()
                            .symbol(symbol)
                            .timestamp(timestamp)
                            .price(price)
                            .volume(Quantity::ZERO)
                            .build()
                            .ok()
                    } else {
                        None
                    }
                })
                .collect()
        };

        // Send ticks outside the lock
        for tick in ticks_to_send {
            callback.on_tick(tick).await;
        }
    }

    /// Parses L2 order book to OrderBook.
    fn parse_l2_book(&self, book: &HyperliquidL2Book) -> Option<OrderBook> {
        let symbol = Self::from_hyperliquid_coin(&book.coin)?;
        let timestamp = Timestamp::new(book.time).ok()?;

        // Hyperliquid returns levels as [[bids], [asks]]
        let (bids, asks) = if book.levels.len() >= 2 {
            let bids: Vec<OrderBookLevel> = book.levels[0]
                .iter()
                .filter_map(|level| {
                    let price = Price::new(level.price.parse().ok()?).ok()?;
                    let quantity = Quantity::new(level.size.parse().ok()?).ok()?;
                    Some(OrderBookLevel::new(price, quantity))
                })
                .collect();

            let asks: Vec<OrderBookLevel> = book.levels[1]
                .iter()
                .filter_map(|level| {
                    let price = Price::new(level.price.parse().ok()?).ok()?;
                    let quantity = Quantity::new(level.size.parse().ok()?).ok()?;
                    Some(OrderBookLevel::new(price, quantity))
                })
                .collect();

            (bids, asks)
        } else {
            (Vec::new(), Vec::new())
        };

        Some(OrderBook {
            symbol,
            timestamp,
            bids,
            asks,
        })
    }

    /// Parses trade to TickData.
    fn parse_trade(&self, trade: &HyperliquidTrade) -> Option<TickData> {
        let symbol = Self::from_hyperliquid_coin(&trade.coin)?;
        let price = Price::new(trade.price.parse().ok()?).ok()?;
        let volume = Quantity::new(trade.size.parse().ok()?).ok()?;
        let timestamp = Timestamp::new(trade.time).ok()?;

        TickData::builder()
            .symbol(symbol)
            .timestamp(timestamp)
            .price(price)
            .volume(volume)
            .build()
            .ok()
    }

    /// Parses candle to KlineData.
    fn parse_candle(&self, candle: &HyperliquidCandle, channel: &str) -> Option<KlineData> {
        // Extract coin and interval from channel (format: "candle:BTC:1m")
        let parts: Vec<&str> = channel.split(':').collect();
        let coin = parts.get(1)?;
        let interval = parts.get(2).unwrap_or(&"1m");

        let symbol = Self::from_hyperliquid_coin(coin)?;
        let period = match *interval {
            "1m" => KlinePeriod::Minute1,
            "5m" => KlinePeriod::Minute5,
            "15m" => KlinePeriod::Minute15,
            "30m" => KlinePeriod::Minute30,
            "1h" => KlinePeriod::Hour1,
            "4h" => KlinePeriod::Hour4,
            "1d" => KlinePeriod::Day1,
            "1w" => KlinePeriod::Week1,
            _ => return None,
        };

        let timestamp = Timestamp::new(candle.timestamp).ok()?;
        let open = Price::new(candle.open.parse().ok()?).ok()?;
        let high = Price::new(candle.high.parse().ok()?).ok()?;
        let low = Price::new(candle.low.parse().ok()?).ok()?;
        let close = Price::new(candle.close.parse().ok()?).ok()?;
        let volume = Quantity::new(candle.volume.parse().ok()?).ok()?;
        // Hyperliquid doesn't provide turnover directly, calculate from volume * close
        let turnover = Amount::new(volume.as_decimal() * close.as_decimal()).ok()?;

        KlineData::builder()
            .symbol(symbol)
            .timestamp(timestamp)
            .period(period)
            .open(open)
            .high(high)
            .low(low)
            .close(close)
            .volume(volume)
            .turnover(turnover)
            .build()
            .ok()
    }

    /// Gets the asset index for a coin.
    pub fn get_asset_index(&self, coin: &str) -> Option<u32> {
        self.asset_indices.read().get(coin).copied()
    }

    /// Sets the asset index mapping.
    pub fn set_asset_indices(&self, indices: HashMap<String, u32>) {
        *self.asset_indices.write() = indices;
    }
}

impl Default for HyperliquidParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal WebSocket callback handler.
struct HyperliquidWsCallback {
    parser: Arc<HyperliquidParser>,
}

#[async_trait]
impl WebSocketCallback for HyperliquidWsCallback {
    async fn on_message(&self, message: WebSocketMessage) {
        if let WebSocketMessage::Text(text) = message {
            self.parser.handle_message(&text).await;
        }
    }

    async fn on_connected(&self) {
        self.parser.connected.store(true, Ordering::SeqCst);
        if let Some(callback) = &self.parser.callback {
            callback.on_connected().await;
        }
        info!(exchange = "hyperliquid", "Parser connected");
    }

    async fn on_disconnected(&self, reason: Option<String>) {
        self.parser.connected.store(false, Ordering::SeqCst);
        if let Some(callback) = &self.parser.callback {
            callback.on_disconnected(reason.clone()).await;
        }
        warn!(exchange = "hyperliquid", reason = ?reason, "Parser disconnected");
    }

    async fn on_error(&self, error: NetworkError) {
        if let Some(callback) = &self.parser.callback {
            callback.on_error(error).await;
        }
    }

    async fn on_reconnecting(&self, attempt: u32, max_attempts: u32) {
        if let Some(callback) = &self.parser.callback {
            callback.on_reconnecting(attempt, max_attempts).await;
        }
    }
}

#[async_trait]
impl MarketDataParser for HyperliquidParser {
    async fn init(&mut self, config: &ParserConfig) -> Result<(), NetworkError> {
        // Determine endpoint based on testnet setting if not specified
        let endpoint = if config.endpoint.is_empty() {
            if self.testnet {
                self.market.testnet_ws_base_url().to_string()
            } else {
                self.market.ws_base_url().to_string()
            }
        } else {
            config.endpoint.clone()
        };

        // Build WebSocket config
        let ws_config = WebSocketConfig::builder()
            .url(&endpoint)
            .exchange("hyperliquid")
            .reconnect_enabled(config.reconnect_enabled)
            .max_reconnect_attempts(config.max_reconnect_attempts)
            .reconnect_delay(config.reconnect_delay())
            .max_reconnect_delay(config.max_reconnect_delay())
            .heartbeat_interval(config.heartbeat_interval())
            .build();

        self.ws_client = Some(WebSocketClient::new(ws_config));
        self.config = Some(config.clone());

        info!(
            exchange = "hyperliquid",
            market = ?self.market,
            endpoint = %endpoint,
            testnet = self.testnet,
            "Parser initialized"
        );

        Ok(())
    }

    async fn connect(&mut self) -> Result<(), NetworkError> {
        let ws_client = self
            .ws_client
            .as_mut()
            .ok_or(NetworkError::ConnectionFailed {
                reason: "Parser not initialized".to_string(),
            })?;

        ws_client.connect().await?;
        self.connected.store(true, Ordering::SeqCst);

        // Subscribe to allMids by default for price updates
        let all_mids_sub = HyperliquidSubscription::subscribe(HyperliquidSubParams::AllMids);
        ws_client.send_json(&all_mids_sub).await?;

        if let Some(callback) = &self.callback {
            callback.on_connected().await;
        }

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), NetworkError> {
        if let Some(ws_client) = self.ws_client.as_mut() {
            ws_client.disconnect().await;
        }
        self.connected.store(false, Ordering::SeqCst);

        if let Some(callback) = &self.callback {
            callback
                .on_disconnected(Some("Client disconnected".to_string()))
                .await;
        }

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    async fn subscribe(&mut self, symbols: &[Symbol]) -> Result<(), NetworkError> {
        let ws_client = self
            .ws_client
            .as_ref()
            .ok_or(NetworkError::ConnectionFailed {
                reason: "Parser not initialized".to_string(),
            })?;

        if !self.is_connected() {
            return Err(NetworkError::ConnectionClosed {
                reason: "Not connected".to_string(),
            });
        }

        for symbol in symbols {
            let coin = Self::to_hyperliquid_coin(symbol);

            // Subscribe to L2 book
            let l2_sub = HyperliquidSubscription::subscribe(HyperliquidSubParams::L2Book {
                coin: coin.clone(),
            });
            ws_client.send_json(&l2_sub).await?;

            // Subscribe to trades
            let trades_sub = HyperliquidSubscription::subscribe(HyperliquidSubParams::Trades {
                coin: coin.clone(),
            });
            ws_client.send_json(&trades_sub).await?;

            // Subscribe to 1m candles
            let candle_sub = HyperliquidSubscription::subscribe(HyperliquidSubParams::Candle {
                coin: coin.clone(),
                interval: "1m".to_string(),
            });
            ws_client.send_json(&candle_sub).await?;
        }

        // Update subscribed symbols
        {
            let mut subscribed = self.subscribed_symbols.write();
            for symbol in symbols {
                subscribed.insert(symbol.clone());
            }
        }

        info!(
            exchange = "hyperliquid",
            symbols = ?symbols,
            "Subscribed to symbols"
        );

        Ok(())
    }

    async fn unsubscribe(&mut self, symbols: &[Symbol]) -> Result<(), NetworkError> {
        let ws_client = self
            .ws_client
            .as_ref()
            .ok_or(NetworkError::ConnectionFailed {
                reason: "Parser not initialized".to_string(),
            })?;

        if !self.is_connected() {
            return Err(NetworkError::ConnectionClosed {
                reason: "Not connected".to_string(),
            });
        }

        for symbol in symbols {
            let coin = Self::to_hyperliquid_coin(symbol);

            // Unsubscribe from L2 book
            let l2_unsub = HyperliquidSubscription::unsubscribe(HyperliquidSubParams::L2Book {
                coin: coin.clone(),
            });
            ws_client.send_json(&l2_unsub).await?;

            // Unsubscribe from trades
            let trades_unsub = HyperliquidSubscription::unsubscribe(HyperliquidSubParams::Trades {
                coin: coin.clone(),
            });
            ws_client.send_json(&trades_unsub).await?;

            // Unsubscribe from candles
            let candle_unsub = HyperliquidSubscription::unsubscribe(HyperliquidSubParams::Candle {
                coin: coin.clone(),
                interval: "1m".to_string(),
            });
            ws_client.send_json(&candle_unsub).await?;
        }

        // Update subscribed symbols
        {
            let mut subscribed = self.subscribed_symbols.write();
            for symbol in symbols {
                subscribed.remove(symbol);
            }
        }

        info!(
            exchange = "hyperliquid",
            symbols = ?symbols,
            "Unsubscribed from symbols"
        );

        Ok(())
    }

    fn set_callback(&mut self, callback: Box<dyn ParserCallback>) {
        self.callback = Some(Arc::from(callback));
    }

    fn subscribed_symbols(&self) -> Vec<Symbol> {
        self.subscribed_symbols.read().iter().cloned().collect()
    }

    fn exchange(&self) -> &str {
        "hyperliquid"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_hyperliquid_coin() {
        assert_eq!(
            HyperliquidParser::to_hyperliquid_coin(&Symbol::new("BTC-USD").unwrap()),
            "BTC"
        );
        assert_eq!(
            HyperliquidParser::to_hyperliquid_coin(&Symbol::new("ETH-USD").unwrap()),
            "ETH"
        );
    }

    #[test]
    fn test_from_hyperliquid_coin() {
        let symbol = HyperliquidParser::from_hyperliquid_coin("BTC").unwrap();
        assert_eq!(symbol.as_str(), "BTC-USD");

        let symbol = HyperliquidParser::from_hyperliquid_coin("ETH").unwrap();
        assert_eq!(symbol.as_str(), "ETH-USD");
    }

    #[test]
    fn test_parser_default() {
        let parser = HyperliquidParser::default();
        assert_eq!(parser.market(), HyperliquidMarket::Perpetual);
        assert!(!parser.is_connected());
        assert!(!parser.is_testnet());
    }

    #[test]
    fn test_parser_with_testnet() {
        let parser = HyperliquidParser::new().with_testnet(true);
        assert!(parser.is_testnet());
    }

    #[test]
    fn test_parse_trade() {
        let parser = HyperliquidParser::new();
        let trade = HyperliquidTrade {
            coin: "BTC".to_string(),
            side: "B".to_string(),
            price: "42000.0".to_string(),
            size: "0.1".to_string(),
            hash: "0x123".to_string(),
            time: 1672515782136,
            tid: 12345,
        };

        let tick = parser.parse_trade(&trade).unwrap();
        assert_eq!(tick.symbol.as_str(), "BTC-USD");
        assert_eq!(tick.price.as_decimal(), Decimal::from(42000));
    }

    #[test]
    fn test_parse_l2_book() {
        let parser = HyperliquidParser::new();
        let book = HyperliquidL2Book {
            coin: "BTC".to_string(),
            time: 1672515782136,
            levels: vec![
                vec![
                    HyperliquidBookLevel {
                        price: "41999.0".to_string(),
                        size: "1.5".to_string(),
                        num_orders: 3,
                    },
                    HyperliquidBookLevel {
                        price: "41998.0".to_string(),
                        size: "2.0".to_string(),
                        num_orders: 5,
                    },
                ],
                vec![
                    HyperliquidBookLevel {
                        price: "42001.0".to_string(),
                        size: "1.0".to_string(),
                        num_orders: 2,
                    },
                    HyperliquidBookLevel {
                        price: "42002.0".to_string(),
                        size: "3.0".to_string(),
                        num_orders: 4,
                    },
                ],
            ],
        };

        let orderbook = parser.parse_l2_book(&book).unwrap();
        assert_eq!(orderbook.symbol.as_str(), "BTC-USD");
        assert_eq!(orderbook.bids.len(), 2);
        assert_eq!(orderbook.asks.len(), 2);
        assert_eq!(orderbook.bids[0].price.as_decimal(), Decimal::from(41999));
        assert_eq!(orderbook.asks[0].price.as_decimal(), Decimal::from(42001));
    }

    #[test]
    fn test_asset_indices() {
        let parser = HyperliquidParser::new();
        let mut indices = HashMap::new();
        indices.insert("BTC".to_string(), 0);
        indices.insert("ETH".to_string(), 1);
        parser.set_asset_indices(indices);

        assert_eq!(parser.get_asset_index("BTC"), Some(0));
        assert_eq!(parser.get_asset_index("ETH"), Some(1));
        assert_eq!(parser.get_asset_index("SOL"), None);
    }
}
