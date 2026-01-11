//! OKX market data parser implementation.
//!
//! Implements the [`MarketDataParser`] trait for OKX WebSocket streams.

#![allow(clippy::collapsible_if)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::unused_self)]
#![allow(clippy::needless_question_mark)]
#![allow(clippy::or_fun_call)]
#![allow(clippy::unnecessary_literal_bound)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::ref_option)]
#![allow(dead_code)]

use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, info, warn};

use zephyr_core::data::{KlineData, KlinePeriod, TickData};
use zephyr_core::error::NetworkError;
use zephyr_core::traits::{MarketDataParser, ParserCallback, ParserConfig};
use zephyr_core::types::{Amount, FundingRate, MarkPrice, Price, Quantity, Symbol, Timestamp};

use crate::ws::{WebSocketCallback, WebSocketClient, WebSocketConfig, WebSocketMessage};

use super::types::*;

/// OKX market data parser.
///
/// Parses WebSocket streams from OKX exchange and converts them
/// to standardized [`TickData`] and [`KlineData`] structures.
///
/// # Supported Channels
///
/// - `tickers` - Ticker data (best bid/ask, last price)
/// - `trades` - Trade data
/// - `candle<period>` - Candlestick data
/// - `books5` / `books` - Order book depth
/// - `mark-price` - Mark price (derivatives)
/// - `funding-rate` - Funding rate (perpetual swaps)
///
/// # Example
///
/// ```ignore
/// use zephyr_gateway::okx::OkxParser;
/// use zephyr_core::traits::{ParserConfig, MarketDataParser};
///
/// let mut parser = OkxParser::new();
/// let config = ParserConfig::builder()
///     .endpoint("wss://ws.okx.com:8443/ws/v5/public")
///     .exchange("okx")
///     .build();
///
/// parser.init(&config).await?;
/// parser.connect().await?;
/// parser.subscribe(&[Symbol::new("BTC-USDT")?]).await?;
/// ```
pub struct OkxParser {
    /// Market type (spot, swap)
    market: OkxMarket,
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

impl OkxParser {
    /// Creates a new OKX parser for spot market.
    #[must_use]
    pub fn new() -> Self {
        Self::with_market(OkxMarket::Spot)
    }

    /// Creates a new OKX parser for perpetual swaps.
    #[must_use]
    pub fn new_swap() -> Self {
        Self::with_market(OkxMarket::Swap)
    }

    /// Creates a new OKX parser with specified market type.
    #[must_use]
    pub fn with_market(market: OkxMarket) -> Self {
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
    pub fn market(&self) -> OkxMarket {
        self.market
    }

    /// Converts a symbol to OKX format (e.g., "BTC-USDT" stays as "BTC-USDT").
    /// OKX uses hyphenated format natively.
    fn to_okx_symbol(symbol: &Symbol) -> String {
        symbol.as_str().to_uppercase()
    }

    /// Converts an OKX symbol to standard format.
    /// OKX already uses hyphenated format, so minimal conversion needed.
    fn from_okx_symbol(okx_symbol: &str) -> Option<Symbol> {
        Symbol::new(okx_symbol.to_uppercase()).ok()
    }

    /// Builds subscription arguments for the given symbols.
    fn build_subscription_args(&self, symbols: &[Symbol]) -> Vec<OkxSubscriptionArg> {
        let mut args = Vec::new();

        for symbol in symbols {
            let inst_id = Self::to_okx_symbol(symbol);

            // Subscribe to tickers for tick data
            args.push(OkxSubscriptionArg {
                channel: "tickers".to_string(),
                inst_type: None,
                inst_id: Some(inst_id.clone()),
            });

            // Subscribe to trades
            args.push(OkxSubscriptionArg {
                channel: "trades".to_string(),
                inst_type: None,
                inst_id: Some(inst_id.clone()),
            });

            // Subscribe to 1m candles
            args.push(OkxSubscriptionArg {
                channel: "candle1m".to_string(),
                inst_type: None,
                inst_id: Some(inst_id.clone()),
            });

            // Subscribe to order book (5 levels)
            args.push(OkxSubscriptionArg {
                channel: "books5".to_string(),
                inst_type: None,
                inst_id: Some(inst_id.clone()),
            });

            // For derivatives, also subscribe to mark price and funding rate
            if matches!(
                self.market,
                OkxMarket::Swap | OkxMarket::CoinSwap | OkxMarket::Futures
            ) {
                args.push(OkxSubscriptionArg {
                    channel: "mark-price".to_string(),
                    inst_type: None,
                    inst_id: Some(inst_id.clone()),
                });

                if matches!(self.market, OkxMarket::Swap | OkxMarket::CoinSwap) {
                    args.push(OkxSubscriptionArg {
                        channel: "funding-rate".to_string(),
                        inst_type: None,
                        inst_id: Some(inst_id),
                    });
                }
            }
        }

        args
    }

    /// Parses a WebSocket message and dispatches to callback.
    async fn handle_message(&self, text: &str) {
        let callback = match &self.callback {
            Some(cb) => cb,
            None => return,
        };

        // Try to parse as generic response first
        if let Ok(response) = serde_json::from_str::<OkxWsResponse<serde_json::Value>>(text) {
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
                self.handle_data_push(&arg.channel, &response.data, callback.as_ref())
                    .await;
            }
        } else {
            warn!(message = %text, "Failed to parse WebSocket message");
        }
    }

    /// Handles data push messages based on channel.
    async fn handle_data_push(
        &self,
        channel: &str,
        data: &Option<Vec<serde_json::Value>>,
        callback: &dyn ParserCallback,
    ) {
        let data = match data {
            Some(d) => d,
            None => return,
        };

        match channel {
            "tickers" => {
                for item in data {
                    if let Ok(ticker) = serde_json::from_value::<OkxTicker>(item.clone()) {
                        if let Some(tick) = self.parse_ticker(&ticker) {
                            callback.on_tick(tick).await;
                        }
                    }
                }
            }
            "trades" => {
                for item in data {
                    if let Ok(trade) = serde_json::from_value::<OkxTrade>(item.clone()) {
                        if let Some(tick) = self.parse_trade(&trade) {
                            callback.on_tick(tick).await;
                        }
                    }
                }
            }
            c if c.starts_with("candle") => {
                // Candle channels are like "candle1m", "candle5m", etc.
                let period = self.parse_candle_period(c);
                for item in data {
                    // OKX candle data is an array, not an object
                    if let Ok(candle_arr) = serde_json::from_value::<Vec<String>>(item.clone()) {
                        if let Some(kline) = self.parse_candle_array(&candle_arr, period) {
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
            "books5" | "books" | "books50-l2-tbt" | "bbo-tbt" => {
                for item in data {
                    if let Ok(book) = serde_json::from_value::<OkxOrderBook>(item.clone()) {
                        // Need to get inst_id from the arg, but we don't have it here
                        // For now, skip order book parsing
                        debug!(
                            bids = book.bids.len(),
                            asks = book.asks.len(),
                            "Order book update"
                        );
                    }
                }
            }
            "mark-price" => {
                for item in data {
                    if let Ok(mark) = serde_json::from_value::<OkxMarkPrice>(item.clone()) {
                        if let Some(tick) = self.parse_mark_price(&mark) {
                            callback.on_tick(tick).await;
                        }
                    }
                }
            }
            "funding-rate" => {
                for item in data {
                    if let Ok(funding) = serde_json::from_value::<OkxFundingRate>(item.clone()) {
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
    fn parse_ticker(&self, ticker: &OkxTicker) -> Option<TickData> {
        let symbol = Self::from_okx_symbol(&ticker.inst_id)?;
        let price = Price::new(ticker.last.parse().ok()?).ok()?;
        let volume = Quantity::new(ticker.last_sz.parse().ok()?).ok()?;
        let timestamp = Timestamp::new(ticker.ts.parse().ok()?).ok()?;

        let bid_price = Price::new(ticker.bid_px.parse().ok()?).ok()?;
        let bid_qty = Quantity::new(ticker.bid_sz.parse().ok()?).ok()?;
        let ask_price = Price::new(ticker.ask_px.parse().ok()?).ok()?;
        let ask_qty = Quantity::new(ticker.ask_sz.parse().ok()?).ok()?;

        Some(
            TickData::builder()
                .symbol(symbol)
                .timestamp(timestamp)
                .price(price)
                .volume(volume)
                .bid_price(bid_price)
                .bid_quantity(bid_qty)
                .ask_price(ask_price)
                .ask_quantity(ask_qty)
                .build()
                .ok()?,
        )
    }

    /// Parses trade data to TickData.
    fn parse_trade(&self, trade: &OkxTrade) -> Option<TickData> {
        let symbol = Self::from_okx_symbol(&trade.inst_id)?;
        let price = Price::new(trade.px.parse().ok()?).ok()?;
        let volume = Quantity::new(trade.sz.parse().ok()?).ok()?;
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
    fn parse_candle_period(&self, channel: &str) -> Option<KlinePeriod> {
        let period_str = channel.strip_prefix("candle")?;
        match period_str {
            "1m" => Some(KlinePeriod::Minute1),
            "5m" => Some(KlinePeriod::Minute5),
            "15m" => Some(KlinePeriod::Minute15),
            "30m" => Some(KlinePeriod::Minute30),
            "1H" => Some(KlinePeriod::Hour1),
            "4H" => Some(KlinePeriod::Hour4),
            "1D" | "1Dutc" => Some(KlinePeriod::Day1),
            "1W" | "1Wutc" => Some(KlinePeriod::Week1),
            _ => None,
        }
    }

    /// Parses candle array to KlineData.
    /// OKX candle format: [ts, o, h, l, c, vol, volCcy, volCcyQuote, confirm]
    fn parse_candle_array(&self, arr: &[String], period: Option<KlinePeriod>) -> Option<KlineData> {
        if arr.len() < 9 {
            return None;
        }

        let _period = period?;
        let _timestamp = Timestamp::new(arr[0].parse().ok()?).ok()?;
        let _open = Price::new(arr[1].parse().ok()?).ok()?;
        let _high = Price::new(arr[2].parse().ok()?).ok()?;
        let _low = Price::new(arr[3].parse().ok()?).ok()?;
        let _close = Price::new(arr[4].parse().ok()?).ok()?;
        let _volume = Quantity::new(arr[5].parse().ok()?).ok()?;
        let _turnover = Amount::new(arr[6].parse().ok()?).ok()?;

        // Note: We don't have the symbol here from the candle data alone
        // In a real implementation, we'd need to track this from the subscription
        // For now, return None as we can't construct a complete KlineData
        None
    }

    /// Parses mark price to TickData.
    fn parse_mark_price(&self, mark: &OkxMarkPrice) -> Option<TickData> {
        let symbol = Self::from_okx_symbol(&mark.inst_id)?;
        let price = Price::new(mark.mark_px.parse().ok()?).ok()?;
        let timestamp = Timestamp::new(mark.ts.parse().ok()?).ok()?;
        let mark_price = MarkPrice::new(mark.mark_px.parse().ok()?).ok();

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
    fn parse_funding_rate(&self, funding: &OkxFundingRate) -> Option<TickData> {
        let symbol = Self::from_okx_symbol(&funding.inst_id)?;
        let funding_rate = FundingRate::new(funding.funding_rate.parse().ok()?).ok()?;
        let timestamp = Timestamp::new(funding.funding_time.parse().ok()?).ok()?;

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

impl Default for OkxParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal WebSocket callback handler.
struct OkxWsCallback {
    parser: Arc<OkxParser>,
}

#[async_trait]
impl WebSocketCallback for OkxWsCallback {
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
        info!(exchange = "okx", "Parser connected");
    }

    async fn on_disconnected(&self, reason: Option<String>) {
        self.parser.connected.store(false, Ordering::SeqCst);
        if let Some(callback) = &self.parser.callback {
            callback.on_disconnected(reason.clone()).await;
        }
        warn!(exchange = "okx", reason = ?reason, "Parser disconnected");
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
impl MarketDataParser for OkxParser {
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
            .exchange("okx")
            .reconnect_enabled(config.reconnect_enabled)
            .max_reconnect_attempts(config.max_reconnect_attempts)
            .reconnect_delay(config.reconnect_delay())
            .max_reconnect_delay(config.max_reconnect_delay())
            .heartbeat_interval(config.heartbeat_interval())
            .build();

        self.ws_client = Some(WebSocketClient::new(ws_config));
        self.config = Some(config.clone());

        info!(
            exchange = "okx",
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
        let subscription = OkxSubscription::subscribe(args);

        ws_client.send_json(&subscription).await?;

        // Update subscribed symbols
        {
            let mut subscribed = self.subscribed_symbols.write();
            for symbol in symbols {
                subscribed.insert(symbol.clone());
            }
        }

        info!(
            exchange = "okx",
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
        let unsubscription = OkxSubscription::unsubscribe(args);

        ws_client.send_json(&unsubscription).await?;

        // Update subscribed symbols
        {
            let mut subscribed = self.subscribed_symbols.write();
            for symbol in symbols {
                subscribed.remove(symbol);
            }
        }

        info!(
            exchange = "okx",
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
        "okx"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[test]
    fn test_to_okx_symbol() {
        assert_eq!(
            OkxParser::to_okx_symbol(&Symbol::new("BTC-USDT").unwrap()),
            "BTC-USDT"
        );
        assert_eq!(
            OkxParser::to_okx_symbol(&Symbol::new("eth-btc").unwrap()),
            "ETH-BTC"
        );
    }

    #[test]
    fn test_from_okx_symbol() {
        let symbol = OkxParser::from_okx_symbol("BTC-USDT").unwrap();
        assert_eq!(symbol.as_str(), "BTC-USDT");

        let symbol = OkxParser::from_okx_symbol("ETH-BTC").unwrap();
        assert_eq!(symbol.as_str(), "ETH-BTC");
    }

    #[test]
    fn test_build_subscription_args() {
        let parser = OkxParser::new();
        let symbols = vec![Symbol::new("BTC-USDT").unwrap()];
        let args = parser.build_subscription_args(&symbols);

        // Should have tickers, trades, candle1m, books5 for spot
        assert!(args.iter().any(|a| a.channel == "tickers"));
        assert!(args.iter().any(|a| a.channel == "trades"));
        assert!(args.iter().any(|a| a.channel == "candle1m"));
        assert!(args.iter().any(|a| a.channel == "books5"));
    }

    #[test]
    fn test_build_subscription_args_swap() {
        let parser = OkxParser::new_swap();
        let symbols = vec![Symbol::new("BTC-USDT-SWAP").unwrap()];
        let args = parser.build_subscription_args(&symbols);

        // Should also have mark-price and funding-rate for swaps
        assert!(args.iter().any(|a| a.channel == "mark-price"));
        assert!(args.iter().any(|a| a.channel == "funding-rate"));
    }

    #[test]
    fn test_parse_ticker() {
        let parser = OkxParser::new();
        let ticker = OkxTicker {
            inst_type: "SPOT".to_string(),
            inst_id: "BTC-USDT".to_string(),
            last: "42000.00".to_string(),
            last_sz: "0.001".to_string(),
            ask_px: "42001.00".to_string(),
            ask_sz: "10.5".to_string(),
            bid_px: "41999.00".to_string(),
            bid_sz: "8.3".to_string(),
            open24h: "41000.00".to_string(),
            high24h: "43000.00".to_string(),
            low24h: "40000.00".to_string(),
            vol_ccy24h: "1000000.00".to_string(),
            vol24h: "25.5".to_string(),
            ts: "1_672_515_782_136".to_string(),
            sod_utc0: None,
            sod_utc8: None,
        };

        let tick = parser.parse_ticker(&ticker).unwrap();
        assert_eq!(tick.symbol.as_str(), "BTC-USDT");
        assert_eq!(tick.price.as_decimal(), Decimal::from(42000));
    }

    #[test]
    fn test_parse_trade() {
        let parser = OkxParser::new();
        let trade = OkxTrade {
            inst_id: "BTC-USDT".to_string(),
            trade_id: "123_456_789".to_string(),
            px: "42000.00".to_string(),
            sz: "0.001".to_string(),
            side: "buy".to_string(),
            ts: "1_672_515_782_136".to_string(),
        };

        let tick = parser.parse_trade(&trade).unwrap();
        assert_eq!(tick.symbol.as_str(), "BTC-USDT");
        assert_eq!(tick.price.as_decimal(), Decimal::from(42000));
    }

    #[test]
    fn test_parse_candle_period() {
        let parser = OkxParser::new();

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
        let parser = OkxParser::new_swap();
        let mark = OkxMarkPrice {
            inst_type: "SWAP".to_string(),
            inst_id: "BTC-USDT-SWAP".to_string(),
            mark_px: "42000.00".to_string(),
            ts: "1_672_515_782_136".to_string(),
        };

        let tick = parser.parse_mark_price(&mark).unwrap();
        assert_eq!(tick.symbol.as_str(), "BTC-USDT-SWAP");
        assert!(tick.mark_price.is_some());
    }

    #[test]
    fn test_parser_default() {
        let parser = OkxParser::default();
        assert_eq!(parser.market(), OkxMarket::Spot);
        assert!(!parser.is_connected());
    }
}
