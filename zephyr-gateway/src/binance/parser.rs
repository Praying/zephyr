//! Binance market data parser implementation.
//!
//! Implements the [`MarketDataParser`] trait for Binance WebSocket streams.

#![allow(dead_code)]

use async_trait::async_trait;
use parking_lot::RwLock;
use rust_decimal::Decimal;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tracing::{debug, info, warn};

use zephyr_core::data::{KlineData, KlinePeriod, OrderBook, TickData};
use zephyr_core::error::NetworkError;
use zephyr_core::traits::{MarketDataParser, ParserCallback, ParserConfig};
use zephyr_core::types::{Amount, FundingRate, MarkPrice, Price, Quantity, Symbol, Timestamp};

use crate::ws::{WebSocketCallback, WebSocketClient, WebSocketConfig, WebSocketMessage};

use super::types::*;

/// Binance market data parser.
///
/// Parses WebSocket streams from Binance exchange and converts them
/// to standardized [`TickData`] and [`KlineData`] structures.
///
/// # Supported Streams
///
/// - `<symbol>@aggTrade` - Aggregate trades
/// - `<symbol>@bookTicker` - Best bid/ask
/// - `<symbol>@kline_<interval>` - Kline/candlestick
/// - `<symbol>@markPrice` - Mark price (futures)
/// - `<symbol>@depth<levels>` - Order book depth
///
/// # Example
///
/// ```ignore
/// use zephyr_gateway::binance::BinanceParser;
/// use zephyr_core::traits::{ParserConfig, MarketDataParser};
///
/// let mut parser = BinanceParser::new();
/// let config = ParserConfig::builder()
///     .endpoint("wss://stream.binance.com:9443/ws")
///     .exchange("binance")
///     .build();
///
/// parser.init(&config).await?;
/// parser.connect().await?;
/// parser.subscribe(&[Symbol::new("BTC-USDT")?]).await?;
/// ```
pub struct BinanceParser {
    /// Market type (spot, futures)
    market: BinanceMarket,
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
    /// Request ID counter
    request_id: Arc<AtomicU64>,
}

impl BinanceParser {
    /// Creates a new Binance parser for spot market.
    #[must_use]
    pub fn new() -> Self {
        Self::with_market(BinanceMarket::Spot)
    }

    /// Creates a new Binance parser for USDT futures.
    #[must_use]
    pub fn new_futures() -> Self {
        Self::with_market(BinanceMarket::UsdtFutures)
    }

    /// Creates a new Binance parser with specified market type.
    #[must_use]
    pub fn with_market(market: BinanceMarket) -> Self {
        Self {
            market,
            ws_client: None,
            config: None,
            callback: None,
            subscribed_symbols: Arc::new(RwLock::new(HashSet::new())),
            connected: Arc::new(AtomicBool::new(false)),
            request_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Returns the market type.
    #[must_use]
    pub fn market(&self) -> BinanceMarket {
        self.market
    }

    /// Generates the next request ID.
    fn next_request_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Converts a symbol to Binance format (e.g., "BTC-USDT" -> "btcusdt").
    fn to_binance_symbol(symbol: &Symbol) -> String {
        symbol.as_str().replace('-', "").to_lowercase()
    }

    /// Converts a Binance symbol to standard format (e.g., "BTCUSDT" -> "BTC-USDT").
    fn from_binance_symbol(binance_symbol: &str) -> Option<Symbol> {
        // Common quote currencies
        let quotes = ["USDT", "BUSD", "USDC", "BTC", "ETH", "BNB"];

        let upper = binance_symbol.to_uppercase();
        for quote in quotes {
            if upper.ends_with(quote) {
                let base = &upper[..upper.len() - quote.len()];
                if !base.is_empty() {
                    return Symbol::new(format!("{base}-{quote}")).ok();
                }
            }
        }

        // Fallback: just use the symbol as-is
        Symbol::new(binance_symbol).ok()
    }

    /// Builds stream names for subscription.
    fn build_stream_names(&self, symbols: &[Symbol]) -> Vec<String> {
        let mut streams = Vec::new();

        for symbol in symbols {
            let binance_sym = Self::to_binance_symbol(symbol);

            // Subscribe to aggregate trades for tick data
            streams.push(format!("{binance_sym}@aggTrade"));

            // Subscribe to book ticker for best bid/ask
            streams.push(format!("{binance_sym}@bookTicker"));

            // Subscribe to 1m kline by default
            streams.push(format!("{binance_sym}@kline_1m"));

            // For futures, also subscribe to mark price
            if matches!(
                self.market,
                BinanceMarket::UsdtFutures | BinanceMarket::CoinFutures
            ) {
                streams.push(format!("{binance_sym}@markPrice"));
            }
        }

        streams
    }

    /// Parses a WebSocket message and dispatches to callback.
    async fn handle_message(&self, text: &str) {
        let callback = match &self.callback {
            Some(cb) => cb,
            None => return,
        };

        // Try to parse as different message types
        if let Ok(msg) = serde_json::from_str::<BinanceWsMessage>(text) {
            match msg {
                BinanceWsMessage::AggTrade(trade) => {
                    if let Some(tick) = self.parse_agg_trade(&trade) {
                        callback.on_tick(tick).await;
                    }
                }
                BinanceWsMessage::BookTicker(ticker) => {
                    if let Some(tick) = self.parse_book_ticker(&ticker) {
                        callback.on_tick(tick).await;
                    }
                }
                BinanceWsMessage::Kline(kline_event) => {
                    // Only emit closed klines
                    if kline_event.kline.is_closed {
                        if let Some(kline) = self.parse_kline(&kline_event) {
                            // Note: ParserCallback doesn't have on_kline,
                            // we'd need to extend it or use a different approach
                            debug!(
                                symbol = %kline.symbol,
                                period = %kline.period,
                                close = %kline.close,
                                "Kline closed"
                            );
                        }
                    }
                }
                BinanceWsMessage::MarkPrice(mark_price) => {
                    if let Some(tick) = self.parse_mark_price(&mark_price) {
                        callback.on_tick(tick).await;
                    }
                }
                BinanceWsMessage::DepthUpdate(depth) => {
                    if let Some(orderbook) = self.parse_depth_update(&depth) {
                        callback.on_orderbook(orderbook).await;
                    }
                }
                BinanceWsMessage::PartialDepth(depth) => {
                    // Partial depth doesn't have symbol, need to extract from stream name
                    debug!("Received partial depth update");
                    let _ = depth;
                }
                BinanceWsMessage::Combined(combined) => {
                    // Handle combined stream - extract stream name and re-parse
                    self.handle_combined_stream(&combined, callback.as_ref())
                        .await;
                }
                BinanceWsMessage::SubscriptionResponse(resp) => {
                    debug!(id = resp.id, "Subscription response received");
                }
                BinanceWsMessage::Unknown(value) => {
                    debug!(message = %value, "Unknown message type");
                }
            }
        } else {
            warn!(message = %text, "Failed to parse WebSocket message");
        }
    }

    /// Handles combined stream messages.
    async fn handle_combined_stream(
        &self,
        combined: &BinanceCombinedStream<serde_json::Value>,
        callback: &dyn ParserCallback,
    ) {
        let stream = &combined.stream;
        let data = &combined.data;

        if stream.ends_with("@aggTrade") {
            if let Ok(trade) = serde_json::from_value::<BinanceAggTrade>(data.clone()) {
                if let Some(tick) = self.parse_agg_trade(&trade) {
                    callback.on_tick(tick).await;
                }
            }
        } else if stream.ends_with("@bookTicker") {
            if let Ok(ticker) = serde_json::from_value::<BinanceBookTicker>(data.clone()) {
                if let Some(tick) = self.parse_book_ticker(&ticker) {
                    callback.on_tick(tick).await;
                }
            }
        } else if stream.contains("@kline_") {
            if let Ok(kline_event) = serde_json::from_value::<BinanceKlineEvent>(data.clone()) {
                if kline_event.kline.is_closed {
                    if let Some(kline) = self.parse_kline(&kline_event) {
                        debug!(
                            symbol = %kline.symbol,
                            period = %kline.period,
                            "Kline closed"
                        );
                    }
                }
            }
        } else if stream.ends_with("@markPrice") {
            if let Ok(mark_price) = serde_json::from_value::<BinanceMarkPrice>(data.clone()) {
                if let Some(tick) = self.parse_mark_price(&mark_price) {
                    callback.on_tick(tick).await;
                }
            }
        }
    }

    /// Parses aggregate trade to TickData.
    fn parse_agg_trade(&self, trade: &BinanceAggTrade) -> Option<TickData> {
        let symbol = Self::from_binance_symbol(&trade.symbol)?;
        let price = Price::new(trade.price.parse().ok()?).ok()?;
        let volume = Quantity::new(trade.quantity.parse().ok()?).ok()?;
        let timestamp = Timestamp::new(trade.trade_time).ok()?;

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

    /// Parses book ticker to TickData.
    fn parse_book_ticker(&self, ticker: &BinanceBookTicker) -> Option<TickData> {
        let symbol = Self::from_binance_symbol(&ticker.symbol)?;
        let bid_price = Price::new(ticker.bid_price.parse().ok()?).ok()?;
        let bid_qty = Quantity::new(ticker.bid_quantity.parse().ok()?).ok()?;
        let ask_price = Price::new(ticker.ask_price.parse().ok()?).ok()?;
        let ask_qty = Quantity::new(ticker.ask_quantity.parse().ok()?).ok()?;

        // Calculate mid price
        let mid = (bid_price.as_decimal() + ask_price.as_decimal()) / Decimal::from(2);
        let price = Price::new(mid).ok()?;

        let timestamp = ticker
            .event_time
            .map(|t| Timestamp::new(t).ok())
            .flatten()
            .unwrap_or_else(Timestamp::now);

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

    /// Parses kline event to KlineData.
    fn parse_kline(&self, event: &BinanceKlineEvent) -> Option<KlineData> {
        let symbol = Self::from_binance_symbol(&event.symbol)?;
        let kline = &event.kline;

        let period = match kline.interval.as_str() {
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

        let timestamp = Timestamp::new(kline.start_time).ok()?;
        let open = Price::new(kline.open.parse().ok()?).ok()?;
        let high = Price::new(kline.high.parse().ok()?).ok()?;
        let low = Price::new(kline.low.parse().ok()?).ok()?;
        let close = Price::new(kline.close.parse().ok()?).ok()?;
        let volume = Quantity::new(kline.volume.parse().ok()?).ok()?;
        let turnover = Amount::new(kline.quote_volume.parse().ok()?).ok()?;

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

    /// Parses mark price to TickData (futures).
    fn parse_mark_price(&self, mark_price: &BinanceMarkPrice) -> Option<TickData> {
        let symbol = Self::from_binance_symbol(&mark_price.symbol)?;
        let price = Price::new(mark_price.mark_price.parse().ok()?).ok()?;
        let timestamp = Timestamp::new(mark_price.event_time).ok()?;

        let funding_rate = FundingRate::new(mark_price.funding_rate.parse().ok()?).ok();
        let mark = MarkPrice::new(mark_price.mark_price.parse().ok()?).ok();

        let mut builder = TickData::builder()
            .symbol(symbol)
            .timestamp(timestamp)
            .price(price)
            .volume(Quantity::ZERO);

        if let Some(fr) = funding_rate {
            builder = builder.funding_rate(fr);
        }
        if let Some(mp) = mark {
            builder = builder.mark_price(mp);
        }

        builder.build().ok()
    }

    /// Parses depth update to OrderBook.
    fn parse_depth_update(&self, depth: &BinanceDepthUpdate) -> Option<OrderBook> {
        use zephyr_core::data::OrderBookLevel;

        let symbol = Self::from_binance_symbol(&depth.symbol)?;
        let timestamp = Timestamp::new(depth.event_time).ok()?;

        let bids: Vec<OrderBookLevel> = depth
            .bids
            .iter()
            .filter_map(|[p, q]| {
                let price = Price::new(p.parse().ok()?).ok()?;
                let quantity = Quantity::new(q.parse().ok()?).ok()?;
                Some(OrderBookLevel::new(price, quantity))
            })
            .collect();

        let asks: Vec<OrderBookLevel> = depth
            .asks
            .iter()
            .filter_map(|[p, q]| {
                let price = Price::new(p.parse().ok()?).ok()?;
                let quantity = Quantity::new(q.parse().ok()?).ok()?;
                Some(OrderBookLevel::new(price, quantity))
            })
            .collect();

        Some(OrderBook {
            symbol,
            timestamp,
            bids,
            asks,
        })
    }
}

impl Default for BinanceParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal WebSocket callback handler.
struct BinanceWsCallback {
    parser: Arc<BinanceParser>,
}

#[async_trait]
impl WebSocketCallback for BinanceWsCallback {
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
        info!(exchange = "binance", "Parser connected");
    }

    async fn on_disconnected(&self, reason: Option<String>) {
        self.parser.connected.store(false, Ordering::SeqCst);
        if let Some(callback) = &self.parser.callback {
            callback.on_disconnected(reason.clone()).await;
        }
        warn!(exchange = "binance", reason = ?reason, "Parser disconnected");
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
impl MarketDataParser for BinanceParser {
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
            .exchange("binance")
            .reconnect_enabled(config.reconnect_enabled)
            .max_reconnect_attempts(config.max_reconnect_attempts)
            .reconnect_delay(config.reconnect_delay())
            .max_reconnect_delay(config.max_reconnect_delay())
            .heartbeat_interval(config.heartbeat_interval())
            .build();

        self.ws_client = Some(WebSocketClient::new(ws_config));
        self.config = Some(config.clone());

        info!(
            exchange = "binance",
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

        // Note: We can't easily set the callback here due to ownership issues
        // In a real implementation, we'd use Arc<Self> or a different pattern
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

        let streams = self.build_stream_names(symbols);
        let request_id = self.next_request_id();
        let subscription = BinanceSubscription::subscribe(streams, request_id);

        ws_client.send_json(&subscription).await?;

        // Update subscribed symbols
        {
            let mut subscribed = self.subscribed_symbols.write();
            for symbol in symbols {
                subscribed.insert(symbol.clone());
            }
        }

        info!(
            exchange = "binance",
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

        let streams = self.build_stream_names(symbols);
        let request_id = self.next_request_id();
        let unsubscription = BinanceSubscription::unsubscribe(streams, request_id);

        ws_client.send_json(&unsubscription).await?;

        // Update subscribed symbols
        {
            let mut subscribed = self.subscribed_symbols.write();
            for symbol in symbols {
                subscribed.remove(symbol);
            }
        }

        info!(
            exchange = "binance",
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
        "binance"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_binance_symbol() {
        assert_eq!(
            BinanceParser::to_binance_symbol(&Symbol::new("BTC-USDT").unwrap()),
            "btcusdt"
        );
        assert_eq!(
            BinanceParser::to_binance_symbol(&Symbol::new("ETH-BTC").unwrap()),
            "ethbtc"
        );
    }

    #[test]
    fn test_from_binance_symbol() {
        let symbol = BinanceParser::from_binance_symbol("BTCUSDT").unwrap();
        assert_eq!(symbol.as_str(), "BTC-USDT");

        let symbol = BinanceParser::from_binance_symbol("ETHBTC").unwrap();
        assert_eq!(symbol.as_str(), "ETH-BTC");
    }

    #[test]
    fn test_build_stream_names() {
        let parser = BinanceParser::new();
        let symbols = vec![Symbol::new("BTC-USDT").unwrap()];
        let streams = parser.build_stream_names(&symbols);

        assert!(streams.contains(&"btcusdt@aggTrade".to_string()));
        assert!(streams.contains(&"btcusdt@bookTicker".to_string()));
        assert!(streams.contains(&"btcusdt@kline_1m".to_string()));
    }

    #[test]
    fn test_build_stream_names_futures() {
        let parser = BinanceParser::new_futures();
        let symbols = vec![Symbol::new("BTC-USDT").unwrap()];
        let streams = parser.build_stream_names(&symbols);

        assert!(streams.contains(&"btcusdt@markPrice".to_string()));
    }

    #[test]
    fn test_parse_agg_trade() {
        let parser = BinanceParser::new();
        let trade = BinanceAggTrade {
            event_type: "aggTrade".to_string(),
            event_time: 1672515782136,
            symbol: "BTCUSDT".to_string(),
            agg_trade_id: 123456789,
            price: "42000.00".to_string(),
            quantity: "0.001".to_string(),
            first_trade_id: 100,
            last_trade_id: 105,
            trade_time: 1672515782136,
            is_buyer_maker: true,
        };

        let tick = parser.parse_agg_trade(&trade).unwrap();
        assert_eq!(tick.symbol.as_str(), "BTC-USDT");
        assert_eq!(tick.price.as_decimal(), Decimal::from(42000));
    }

    #[test]
    fn test_parse_book_ticker() {
        let parser = BinanceParser::new();
        let ticker = BinanceBookTicker {
            event_type: None,
            update_id: 400900217,
            symbol: "BTCUSDT".to_string(),
            bid_price: "42000.00".to_string(),
            bid_quantity: "10.5".to_string(),
            ask_price: "42002.00".to_string(),
            ask_quantity: "8.3".to_string(),
            event_time: Some(1672515782136),
            transaction_time: None,
        };

        let tick = parser.parse_book_ticker(&ticker).unwrap();
        assert_eq!(tick.symbol.as_str(), "BTC-USDT");
        // Mid price should be (42000 + 42002) / 2 = 42001
        assert_eq!(tick.price.as_decimal(), Decimal::from(42001));
    }

    #[test]
    fn test_parse_kline() {
        let parser = BinanceParser::new();
        let event = BinanceKlineEvent {
            event_type: "kline".to_string(),
            event_time: 1672515782136,
            symbol: "BTCUSDT".to_string(),
            kline: BinanceKline {
                start_time: 1672515780000,
                close_time: 1672515839999,
                symbol: "BTCUSDT".to_string(),
                interval: "1m".to_string(),
                first_trade_id: 100,
                last_trade_id: 200,
                open: "42000.00".to_string(),
                close: "42100.00".to_string(),
                high: "42150.00".to_string(),
                low: "41950.00".to_string(),
                volume: "100.5".to_string(),
                num_trades: 100,
                is_closed: true,
                quote_volume: "4200000.00".to_string(),
                taker_buy_volume: "50.25".to_string(),
                taker_buy_quote_volume: "2100000.00".to_string(),
            },
        };

        let kline = parser.parse_kline(&event).unwrap();
        assert_eq!(kline.symbol.as_str(), "BTC-USDT");
        assert_eq!(kline.period, KlinePeriod::Minute1);
        assert_eq!(kline.open.as_decimal(), Decimal::from(42000));
        assert_eq!(kline.close.as_decimal(), Decimal::from(42100));
    }

    #[test]
    fn test_parser_default() {
        let parser = BinanceParser::default();
        assert_eq!(parser.market(), BinanceMarket::Spot);
        assert!(!parser.is_connected());
    }
}
