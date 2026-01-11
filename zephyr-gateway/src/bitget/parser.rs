//! Bitget market data parser implementation.
//!
//! Implements the [`MarketDataParser`] trait for Bitget WebSocket streams.

#![allow(clippy::too_many_lines)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::unnecessary_literal_bound)]

use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, info, warn};

use zephyr_core::data::{KlineData, KlinePeriod, OrderBook, OrderBookLevel, TickData};
use zephyr_core::error::NetworkError;
use zephyr_core::traits::{MarketDataParser, ParserCallback, ParserConfig};
use zephyr_core::types::{Amount, FundingRate, MarkPrice, Price, Quantity, Symbol, Timestamp};

use crate::ws::{WebSocketCallback, WebSocketClient, WebSocketConfig, WebSocketMessage};

use super::types::*;

/// Bitget market data parser.
///
/// Parses WebSocket streams from Bitget exchange and converts them
/// to standardized [`TickData`] and [`KlineData`] structures.
///
/// # Supported Channels
///
/// - `ticker` - Ticker data (best bid/ask, last price)
/// - `trade` - Trade data
/// - `candle<period>` - Candlestick data
/// - `books5` / `books15` - Order book depth
/// - `mark-price` - Mark price (derivatives)
/// - `funding-rate` - Funding rate (perpetual futures)
///
/// # Example
///
/// ```ignore
/// use zephyr_gateway::bitget::BitgetParser;
/// use zephyr_core::traits::{ParserConfig, MarketDataParser};
///
/// let mut parser = BitgetParser::new();
/// let config = ParserConfig::builder()
///     .endpoint("wss://ws.bitget.com/v2/ws/public")
///     .exchange("bitget")
///     .build();
///
/// parser.init(&config).await?;
/// parser.connect().await?;
/// parser.subscribe(&[Symbol::new("BTC-USDT")?]).await?;
/// ```
pub struct BitgetParser {
    /// Market type (spot, futures)
    market: BitgetMarket,
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
}

impl BitgetParser {
    /// Creates a new Bitget parser for spot market.
    #[must_use]
    pub fn new() -> Self {
        Self::with_market(BitgetMarket::Spot)
    }

    /// Creates a new Bitget parser for USDT futures.
    #[must_use]
    pub fn new_futures() -> Self {
        Self::with_market(BitgetMarket::UsdtFutures)
    }

    /// Creates a new Bitget parser with specified market type.
    #[must_use]
    pub fn with_market(market: BitgetMarket) -> Self {
        Self {
            market,
            ws_client: None,
            config: None,
            callback: None,
            subscribed_symbols: Arc::new(RwLock::new(HashSet::new())),
            connected: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns the market type.
    #[must_use]
    pub fn market(&self) -> BitgetMarket {
        self.market
    }

    /// Converts a symbol to Bitget format (e.g., "BTC-USDT" -> "BTCUSDT").
    fn to_bitget_symbol(symbol: &Symbol) -> String {
        symbol.as_str().replace('-', "").to_uppercase()
    }

    /// Converts a Bitget symbol to standard format (e.g., "BTCUSDT" -> "BTC-USDT").
    #[allow(dead_code)]
    fn from_bitget_symbol(bitget_symbol: &str) -> Option<Symbol> {
        // Common quote currencies
        let quotes = ["USDT", "USDC", "BTC", "ETH"];

        let upper = bitget_symbol.to_uppercase();
        for quote in quotes {
            if upper.ends_with(quote) {
                let base = &upper[..upper.len() - quote.len()];
                if !base.is_empty() {
                    return Symbol::new(format!("{base}-{quote}")).ok();
                }
            }
        }

        // Fallback: just use the symbol as-is
        Symbol::new(bitget_symbol).ok()
    }

    /// Builds subscription arguments for the given symbols.
    fn build_subscription_args(&self, symbols: &[Symbol]) -> Vec<BitgetSubscriptionArg> {
        let mut args = Vec::new();
        let inst_type = self.market.inst_type().to_string();

        for symbol in symbols {
            let inst_id = Self::to_bitget_symbol(symbol);

            // Subscribe to ticker for tick data
            args.push(BitgetSubscriptionArg {
                inst_type: inst_type.clone(),
                channel: "ticker".to_string(),
                inst_id: Some(inst_id.clone()),
            });

            // Subscribe to trades
            args.push(BitgetSubscriptionArg {
                inst_type: inst_type.clone(),
                channel: "trade".to_string(),
                inst_id: Some(inst_id.clone()),
            });

            // Subscribe to 1m candles
            args.push(BitgetSubscriptionArg {
                inst_type: inst_type.clone(),
                channel: "candle1m".to_string(),
                inst_id: Some(inst_id.clone()),
            });

            // Subscribe to order book (5 levels)
            args.push(BitgetSubscriptionArg {
                inst_type: inst_type.clone(),
                channel: "books5".to_string(),
                inst_id: Some(inst_id.clone()),
            });

            // For futures, also subscribe to mark price and funding rate
            if matches!(
                self.market,
                BitgetMarket::UsdtFutures | BitgetMarket::CoinFutures
            ) {
                args.push(BitgetSubscriptionArg {
                    inst_type: inst_type.clone(),
                    channel: "mark-price".to_string(),
                    inst_id: Some(inst_id.clone()),
                });

                args.push(BitgetSubscriptionArg {
                    inst_type: inst_type.clone(),
                    channel: "funding-rate".to_string(),
                    inst_id: Some(inst_id),
                });
            }
        }

        args
    }

    /// Parses a WebSocket message and dispatches to callback.
    #[allow(dead_code)]
    async fn handle_message(&self, text: &str) {
        let callback = match &self.callback {
            Some(cb) => cb,
            None => return,
        };

        // Try to parse as generic response first
        if let Ok(response) = serde_json::from_str::<BitgetWsResponse<serde_json::Value>>(text) {
            // Check if it's an event response (subscribe/unsubscribe/login)
            if let Some(event) = &response.event {
                match event.as_str() {
                    "subscribe" => {
                        debug!(channel = ?response.arg, "Subscription confirmed");
                    }
                    "unsubscribe" => {
                        debug!(channel = ?response.arg, "Unsubscription confirmed");
                    }
                    "login" => {
                        if response.code.as_deref() == Some("0") {
                            debug!("Login successful");
                        } else {
                            warn!(code = ?response.code, msg = ?response.msg, "Login failed");
                        }
                    }
                    "error" => {
                        warn!(code = ?response.code, msg = ?response.msg, "WebSocket error");
                    }
                    _ => {
                        debug!(event = %event, "Unknown event");
                    }
                }
                return;
            }

            // Check if it's data push
            if let Some(arg) = &response.arg {
                self.handle_data_push(
                    &arg.channel,
                    &arg.inst_id,
                    &response.data,
                    callback.as_ref(),
                )
                .await;
            }
        } else {
            warn!(message = %text, "Failed to parse WebSocket message");
        }
    }

    /// Handles data push messages based on channel.
    #[allow(dead_code)]
    async fn handle_data_push(
        &self,
        channel: &str,
        inst_id: &Option<String>,
        data: &Option<Vec<serde_json::Value>>,
        callback: &dyn ParserCallback,
    ) {
        let data = match data {
            Some(d) => d,
            None => return,
        };

        match channel {
            "ticker" => {
                for item in data {
                    if let Ok(ticker) = serde_json::from_value::<BitgetTicker>(item.clone()) {
                        if let Some(tick) = self.parse_ticker(&ticker) {
                            callback.on_tick(tick).await;
                        }
                    }
                }
            }
            "trade" => {
                for item in data {
                    if let Ok(trade) = serde_json::from_value::<BitgetTrade>(item.clone()) {
                        if let Some(tick) = self.parse_trade(&trade) {
                            callback.on_tick(tick).await;
                        }
                    }
                }
            }
            c if c.starts_with("candle") => {
                let period = self.parse_candle_period(c);
                let symbol = inst_id.as_ref().and_then(|s| Self::from_bitget_symbol(s));

                for item in data {
                    if let Ok(candle) = serde_json::from_value::<BitgetCandle>(item.clone()) {
                        if let (Some(sym), Some(per)) = (&symbol, period) {
                            if let Some(kline) = self.parse_candle(&candle, sym.clone(), per) {
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
            }
            "books5" | "books15" | "books" => {
                let symbol = inst_id.as_ref().and_then(|s| Self::from_bitget_symbol(s));

                for item in data {
                    if let Ok(book) = serde_json::from_value::<BitgetOrderBook>(item.clone()) {
                        if let Some(sym) = &symbol {
                            if let Some(orderbook) = self.parse_orderbook(&book, sym.clone()) {
                                callback.on_orderbook(orderbook).await;
                            }
                        }
                    }
                }
            }
            "mark-price" => {
                for item in data {
                    if let Ok(mark) = serde_json::from_value::<BitgetMarkPrice>(item.clone()) {
                        if let Some(tick) = self.parse_mark_price(&mark) {
                            callback.on_tick(tick).await;
                        }
                    }
                }
            }
            "funding-rate" => {
                for item in data {
                    if let Ok(funding) = serde_json::from_value::<BitgetFundingRate>(item.clone()) {
                        if let Some(tick) = self.parse_funding_rate(&funding) {
                            callback.on_tick(tick).await;
                        }
                    }
                }
            }
            _ => {
                debug!(channel = %channel, "Unknown channel");
            }
        }
    }

    /// Parses ticker data to TickData.
    #[allow(dead_code)]
    fn parse_ticker(&self, ticker: &BitgetTicker) -> Option<TickData> {
        let symbol = Self::from_bitget_symbol(&ticker.inst_id)?;
        let price = Price::new(ticker.last_pr.parse().ok()?).ok()?;
        let timestamp = Timestamp::new(ticker.ts.parse().ok()?).ok()?;

        let bid_price = Price::new(ticker.bid_pr.parse().ok()?).ok()?;
        let ask_price = Price::new(ticker.ask_pr.parse().ok()?).ok()?;

        let bid_qty = ticker
            .bid_sz
            .as_ref()
            .and_then(|s| s.parse().ok())
            .and_then(|d| Quantity::new(d).ok())
            .unwrap_or(Quantity::ZERO);
        let ask_qty = ticker
            .ask_sz
            .as_ref()
            .and_then(|s| s.parse().ok())
            .and_then(|d| Quantity::new(d).ok())
            .unwrap_or(Quantity::ZERO);

        Some(
            TickData::builder()
                .symbol(symbol)
                .timestamp(timestamp)
                .price(price)
                .volume(Quantity::ZERO)
                .bid_price(bid_price)
                .bid_quantity(bid_qty)
                .ask_price(ask_price)
                .ask_quantity(ask_qty)
                .build()
                .ok()?,
        )
    }

    /// Parses trade data to TickData.
    #[allow(dead_code)]
    fn parse_trade(&self, trade: &BitgetTrade) -> Option<TickData> {
        let symbol = Self::from_bitget_symbol(&trade.inst_id)?;
        let price = Price::new(trade.price.parse().ok()?).ok()?;
        let volume = Quantity::new(trade.size.parse().ok()?).ok()?;
        let timestamp = Timestamp::new(trade.ts.parse().ok()?).ok()?;

        Some(
            TickData::builder()
                .symbol(symbol)
                .timestamp(timestamp)
                .price(price)
                .volume(volume)
                .build()
                .ok()?,
        )
    }

    /// Parses candle period from channel name.
    #[allow(dead_code)]
    fn parse_candle_period(&self, channel: &str) -> Option<KlinePeriod> {
        let period_str = channel.strip_prefix("candle")?;
        match period_str {
            "1m" => Some(KlinePeriod::Minute1),
            "5m" => Some(KlinePeriod::Minute5),
            "15m" => Some(KlinePeriod::Minute15),
            "30m" => Some(KlinePeriod::Minute30),
            "1H" | "1h" => Some(KlinePeriod::Hour1),
            "4H" | "4h" => Some(KlinePeriod::Hour4),
            "1D" | "1d" => Some(KlinePeriod::Day1),
            "1W" | "1w" => Some(KlinePeriod::Week1),
            _ => None,
        }
    }

    /// Parses candle data to KlineData.
    #[allow(dead_code)]
    fn parse_candle(
        &self,
        candle: &BitgetCandle,
        symbol: Symbol,
        period: KlinePeriod,
    ) -> Option<KlineData> {
        let timestamp = Timestamp::new(candle.timestamp()?.parse().ok()?).ok()?;
        let open = Price::new(candle.open()?.parse().ok()?).ok()?;
        let high = Price::new(candle.high()?.parse().ok()?).ok()?;
        let low = Price::new(candle.low()?.parse().ok()?).ok()?;
        let close = Price::new(candle.close()?.parse().ok()?).ok()?;
        let volume = Quantity::new(candle.volume()?.parse().ok()?).ok()?;
        let turnover = candle
            .quote_volume()
            .and_then(|s| s.parse().ok())
            .and_then(|d| Amount::new(d).ok())
            .unwrap_or(Amount::ZERO);

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

    /// Parses order book data to OrderBook.
    #[allow(dead_code)]
    fn parse_orderbook(&self, book: &BitgetOrderBook, symbol: Symbol) -> Option<OrderBook> {
        let timestamp = Timestamp::new(book.ts.parse().ok()?).ok()?;

        let bids: Vec<OrderBookLevel> = book
            .bids
            .iter()
            .filter_map(|level| {
                if level.len() >= 2 {
                    let price = Price::new(level[0].parse().ok()?).ok()?;
                    let quantity = Quantity::new(level[1].parse().ok()?).ok()?;
                    Some(OrderBookLevel::new(price, quantity))
                } else {
                    None
                }
            })
            .collect();

        let asks: Vec<OrderBookLevel> = book
            .asks
            .iter()
            .filter_map(|level| {
                if level.len() >= 2 {
                    let price = Price::new(level[0].parse().ok()?).ok()?;
                    let quantity = Quantity::new(level[1].parse().ok()?).ok()?;
                    Some(OrderBookLevel::new(price, quantity))
                } else {
                    None
                }
            })
            .collect();

        Some(OrderBook {
            symbol,
            timestamp,
            bids,
            asks,
        })
    }

    /// Parses mark price to TickData.
    #[allow(dead_code)]
    fn parse_mark_price(&self, mark: &BitgetMarkPrice) -> Option<TickData> {
        let symbol = Self::from_bitget_symbol(&mark.inst_id)?;
        let price = Price::new(mark.mark_price.parse().ok()?).ok()?;
        let timestamp = Timestamp::new(mark.ts.parse().ok()?).ok()?;
        let mark_price = MarkPrice::new(mark.mark_price.parse().ok()?).ok();

        let mut builder = TickData::builder()
            .symbol(symbol)
            .timestamp(timestamp)
            .price(price)
            .volume(Quantity::ZERO);

        if let Some(mp) = mark_price {
            builder = builder.mark_price(mp);
        }

        builder.build().ok()
    }

    /// Parses funding rate to TickData.
    #[allow(dead_code)]
    fn parse_funding_rate(&self, funding: &BitgetFundingRate) -> Option<TickData> {
        let symbol = Self::from_bitget_symbol(&funding.inst_id)?;
        let funding_rate = FundingRate::new(funding.funding_rate.parse().ok()?).ok()?;
        let timestamp = Timestamp::now();

        TickData::builder()
            .symbol(symbol)
            .timestamp(timestamp)
            .price(Price::ZERO)
            .volume(Quantity::ZERO)
            .funding_rate(funding_rate)
            .build()
            .ok()
    }
}

impl Default for BitgetParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal WebSocket callback handler.
#[allow(dead_code)]
struct BitgetWsCallback {
    parser: Arc<BitgetParser>,
}

#[async_trait]
impl WebSocketCallback for BitgetWsCallback {
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
        info!(exchange = "bitget", "Parser connected");
    }

    async fn on_disconnected(&self, reason: Option<String>) {
        self.parser.connected.store(false, Ordering::SeqCst);
        if let Some(callback) = &self.parser.callback {
            callback.on_disconnected(reason.clone()).await;
        }
        warn!(exchange = "bitget", reason = ?reason, "Parser disconnected");
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
impl MarketDataParser for BitgetParser {
    async fn init(&mut self, config: &ParserConfig) -> Result<(), NetworkError> {
        // Determine endpoint based on market type if not specified
        let endpoint = if config.endpoint.is_empty() {
            self.market.ws_base_url().to_string()
        } else {
            config.endpoint.clone()
        };

        // Build WebSocket config
        let ws_config = WebSocketConfig::builder()
            .url(&endpoint)
            .exchange("bitget")
            .reconnect_enabled(config.reconnect_enabled)
            .max_reconnect_attempts(config.max_reconnect_attempts)
            .reconnect_delay(config.reconnect_delay())
            .max_reconnect_delay(config.max_reconnect_delay())
            .heartbeat_interval(config.heartbeat_interval())
            .build();

        self.ws_client = Some(WebSocketClient::new(ws_config));
        self.config = Some(config.clone());

        info!(
            exchange = "bitget",
            market = ?self.market,
            endpoint = %endpoint,
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

        let args = self.build_subscription_args(symbols);
        let subscription = BitgetSubscription::subscribe(args);

        ws_client.send_json(&subscription).await?;

        // Update subscribed symbols
        {
            let mut subscribed = self.subscribed_symbols.write();
            for symbol in symbols {
                subscribed.insert(symbol.clone());
            }
        }

        info!(
            exchange = "bitget",
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

        let args = self.build_subscription_args(symbols);
        let unsubscription = BitgetSubscription::unsubscribe(args);

        ws_client.send_json(&unsubscription).await?;

        // Update subscribed symbols
        {
            let mut subscribed = self.subscribed_symbols.write();
            for symbol in symbols {
                subscribed.remove(symbol);
            }
        }

        info!(
            exchange = "bitget",
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
        "bitget"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn test_to_bitget_symbol() {
        assert_eq!(
            BitgetParser::to_bitget_symbol(&Symbol::new("BTC-USDT").unwrap()),
            "BTCUSDT"
        );
        assert_eq!(
            BitgetParser::to_bitget_symbol(&Symbol::new("ETH-BTC").unwrap()),
            "ETHBTC"
        );
    }

    #[test]
    fn test_from_bitget_symbol() {
        let symbol = BitgetParser::from_bitget_symbol("BTCUSDT").unwrap();
        assert_eq!(symbol.as_str(), "BTC-USDT");

        let symbol = BitgetParser::from_bitget_symbol("ETHBTC").unwrap();
        assert_eq!(symbol.as_str(), "ETH-BTC");
    }

    #[test]
    fn test_build_subscription_args() {
        let parser = BitgetParser::new();
        let symbols = vec![Symbol::new("BTC-USDT").unwrap()];
        let args = parser.build_subscription_args(&symbols);

        // Should have ticker, trade, candle1m, books5 for spot
        assert!(args.iter().any(|a| a.channel == "ticker"));
        assert!(args.iter().any(|a| a.channel == "trade"));
        assert!(args.iter().any(|a| a.channel == "candle1m"));
        assert!(args.iter().any(|a| a.channel == "books5"));
    }

    #[test]
    fn test_build_subscription_args_futures() {
        let parser = BitgetParser::new_futures();
        let symbols = vec![Symbol::new("BTC-USDT").unwrap()];
        let args = parser.build_subscription_args(&symbols);

        // Should also have mark-price and funding-rate for futures
        assert!(args.iter().any(|a| a.channel == "mark-price"));
        assert!(args.iter().any(|a| a.channel == "funding-rate"));
    }

    #[test]
    fn test_parse_ticker() {
        let parser = BitgetParser::new();
        let ticker = BitgetTicker {
            inst_id: "BTCUSDT".to_string(),
            last_pr: "42000.00".to_string(),
            ask_pr: "42001.00".to_string(),
            bid_pr: "41999.00".to_string(),
            ask_sz: Some("10.5".to_string()),
            bid_sz: Some("8.3".to_string()),
            open24h: None,
            high24h: None,
            low24h: None,
            base_volume: None,
            quote_volume: None,
            ts: "1_672_515_782_136".to_string(),
            open_utc: None,
            change_utc24h: None,
        };

        let tick = parser.parse_ticker(&ticker).unwrap();
        assert_eq!(tick.symbol.as_str(), "BTC-USDT");
        assert_eq!(tick.price.as_decimal(), Decimal::from(42000));
    }

    #[test]
    fn test_parse_trade() {
        let parser = BitgetParser::new();
        let trade = BitgetTrade {
            inst_id: "BTCUSDT".to_string(),
            trade_id: "123_456_789".to_string(),
            price: "42000.00".to_string(),
            size: "0.001".to_string(),
            side: "buy".to_string(),
            ts: "1_672_515_782_136".to_string(),
        };

        let tick = parser.parse_trade(&trade).unwrap();
        assert_eq!(tick.symbol.as_str(), "BTC-USDT");
        assert_eq!(tick.price.as_decimal(), Decimal::from(42000));
    }

    #[test]
    fn test_parse_candle_period() {
        let parser = BitgetParser::new();

        assert_eq!(
            parser.parse_candle_period("candle1m"),
            Some(KlinePeriod::Minute1)
        );
        assert_eq!(
            parser.parse_candle_period("candle5m"),
            Some(KlinePeriod::Minute5)
        );
        assert_eq!(
            parser.parse_candle_period("candle1H"),
            Some(KlinePeriod::Hour1)
        );
        assert_eq!(
            parser.parse_candle_period("candle1D"),
            Some(KlinePeriod::Day1)
        );
        assert_eq!(parser.parse_candle_period("invalid"), None);
    }

    #[test]
    fn test_parse_mark_price() {
        let parser = BitgetParser::new_futures();
        let mark = BitgetMarkPrice {
            inst_id: "BTCUSDT".to_string(),
            mark_price: "42000.00".to_string(),
            ts: "1_672_515_782_136".to_string(),
        };

        let tick = parser.parse_mark_price(&mark).unwrap();
        assert_eq!(tick.symbol.as_str(), "BTC-USDT");
        assert!(tick.mark_price.is_some());
    }

    #[test]
    fn test_parser_default() {
        let parser = BitgetParser::default();
        assert_eq!(parser.market(), BitgetMarket::Spot);
        assert!(!parser.is_connected());
    }
}
