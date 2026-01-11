//! Binance-specific types and message formats.
//!
//! This module contains types for parsing Binance WebSocket and REST API responses.

#![allow(clippy::match_same_arms)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::must_use_candidate)]

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Binance market type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BinanceMarket {
    /// Spot market
    #[default]
    Spot,
    /// USDT-margined futures
    UsdtFutures,
    /// Coin-margined futures
    CoinFutures,
}

impl BinanceMarket {
    /// Returns the WebSocket base URL for this market.
    #[must_use]
    pub const fn ws_base_url(&self) -> &'static str {
        match self {
            Self::Spot => "wss://stream.binance.com:9443/ws",
            Self::UsdtFutures => "wss://fstream.binance.com/ws",
            Self::CoinFutures => "wss://dstream.binance.com/ws",
        }
    }

    /// Returns the REST API base URL for this market.
    #[must_use]
    pub const fn rest_base_url(&self) -> &'static str {
        match self {
            Self::Spot => "https://api.binance.com",
            Self::UsdtFutures => "https://fapi.binance.com",
            Self::CoinFutures => "https://dapi.binance.com",
        }
    }

    /// Returns the testnet WebSocket base URL for this market.
    #[must_use]
    pub const fn testnet_ws_base_url(&self) -> &'static str {
        match self {
            Self::Spot => "wss://testnet.binance.vision/ws",
            Self::UsdtFutures => "wss://stream.binancefuture.com/ws",
            Self::CoinFutures => "wss://dstream.binancefuture.com/ws",
        }
    }

    /// Returns the testnet REST API base URL for this market.
    #[must_use]
    pub const fn testnet_rest_base_url(&self) -> &'static str {
        match self {
            Self::Spot => "https://testnet.binance.vision",
            Self::UsdtFutures => "https://testnet.binancefuture.com",
            Self::CoinFutures => "https://testnet.binancefuture.com",
        }
    }
}

/// Binance WebSocket subscription request.
#[derive(Debug, Clone, Serialize)]
pub struct BinanceSubscription {
    /// Request method
    pub method: String,
    /// Stream names to subscribe
    pub params: Vec<String>,
    /// Request ID
    pub id: u64,
}

impl BinanceSubscription {
    /// Creates a new subscription request.
    #[must_use]
    pub fn subscribe(streams: Vec<String>, id: u64) -> Self {
        Self {
            method: "SUBSCRIBE".to_string(),
            params: streams,
            id,
        }
    }

    /// Creates a new unsubscription request.
    #[must_use]
    pub fn unsubscribe(streams: Vec<String>, id: u64) -> Self {
        Self {
            method: "UNSUBSCRIBE".to_string(),
            params: streams,
            id,
        }
    }
}

/// Binance WebSocket subscription response.
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceSubscriptionResponse {
    /// Result (null on success)
    pub result: Option<serde_json::Value>,
    /// Request ID
    pub id: u64,
}

/// Binance aggregate trade (tick) message.
///
/// Stream: `<symbol>@aggTrade`
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceAggTrade {
    /// Event type
    #[serde(rename = "e")]
    pub event_type: String,
    /// Event time
    #[serde(rename = "E")]
    pub event_time: i64,
    /// Symbol
    #[serde(rename = "s")]
    pub symbol: String,
    /// Aggregate trade ID
    #[serde(rename = "a")]
    pub agg_trade_id: i64,
    /// Price
    #[serde(rename = "p")]
    pub price: String,
    /// Quantity
    #[serde(rename = "q")]
    pub quantity: String,
    /// First trade ID
    #[serde(rename = "f")]
    pub first_trade_id: i64,
    /// Last trade ID
    #[serde(rename = "l")]
    pub last_trade_id: i64,
    /// Trade time
    #[serde(rename = "T")]
    pub trade_time: i64,
    /// Is buyer maker
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

/// Binance book ticker message (best bid/ask).
///
/// Stream: `<symbol>@bookTicker`
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceBookTicker {
    /// Event type (for futures)
    #[serde(rename = "e", default)]
    pub event_type: Option<String>,
    /// Update ID
    #[serde(rename = "u")]
    pub update_id: i64,
    /// Symbol
    #[serde(rename = "s")]
    pub symbol: String,
    /// Best bid price
    #[serde(rename = "b")]
    pub bid_price: String,
    /// Best bid quantity
    #[serde(rename = "B")]
    pub bid_quantity: String,
    /// Best ask price
    #[serde(rename = "a")]
    pub ask_price: String,
    /// Best ask quantity
    #[serde(rename = "A")]
    pub ask_quantity: String,
    /// Event time (for futures)
    #[serde(rename = "E", default)]
    pub event_time: Option<i64>,
    /// Transaction time (for futures)
    #[serde(rename = "T", default)]
    pub transaction_time: Option<i64>,
}

/// Binance kline/candlestick message.
///
/// Stream: `<symbol>@kline_<interval>`
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceKlineEvent {
    /// Event type
    #[serde(rename = "e")]
    pub event_type: String,
    /// Event time
    #[serde(rename = "E")]
    pub event_time: i64,
    /// Symbol
    #[serde(rename = "s")]
    pub symbol: String,
    /// Kline data
    #[serde(rename = "k")]
    pub kline: BinanceKline,
}

/// Binance kline data.
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceKline {
    /// Kline start time
    #[serde(rename = "t")]
    pub start_time: i64,
    /// Kline close time
    #[serde(rename = "T")]
    pub close_time: i64,
    /// Symbol
    #[serde(rename = "s")]
    pub symbol: String,
    /// Interval
    #[serde(rename = "i")]
    pub interval: String,
    /// First trade ID
    #[serde(rename = "f")]
    pub first_trade_id: i64,
    /// Last trade ID
    #[serde(rename = "L")]
    pub last_trade_id: i64,
    /// Open price
    #[serde(rename = "o")]
    pub open: String,
    /// Close price
    #[serde(rename = "c")]
    pub close: String,
    /// High price
    #[serde(rename = "h")]
    pub high: String,
    /// Low price
    #[serde(rename = "l")]
    pub low: String,
    /// Base asset volume
    #[serde(rename = "v")]
    pub volume: String,
    /// Number of trades
    #[serde(rename = "n")]
    pub num_trades: i64,
    /// Is this kline closed?
    #[serde(rename = "x")]
    pub is_closed: bool,
    /// Quote asset volume
    #[serde(rename = "q")]
    pub quote_volume: String,
    /// Taker buy base asset volume
    #[serde(rename = "V")]
    pub taker_buy_volume: String,
    /// Taker buy quote asset volume
    #[serde(rename = "Q")]
    pub taker_buy_quote_volume: String,
}

/// Binance mark price message (futures).
///
/// Stream: `<symbol>@markPrice` or `!markPrice@arr`
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceMarkPrice {
    /// Event type
    #[serde(rename = "e")]
    pub event_type: String,
    /// Event time
    #[serde(rename = "E")]
    pub event_time: i64,
    /// Symbol
    #[serde(rename = "s")]
    pub symbol: String,
    /// Mark price
    #[serde(rename = "p")]
    pub mark_price: String,
    /// Index price
    #[serde(rename = "i")]
    pub index_price: String,
    /// Estimated settle price (only for delivery futures)
    #[serde(rename = "P", default)]
    pub estimated_settle_price: Option<String>,
    /// Funding rate
    #[serde(rename = "r")]
    pub funding_rate: String,
    /// Next funding time
    #[serde(rename = "T")]
    pub next_funding_time: i64,
}

/// Binance depth update message.
///
/// Stream: `<symbol>@depth<levels>` or `<symbol>@depth`
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceDepthUpdate {
    /// Event type
    #[serde(rename = "e")]
    pub event_type: String,
    /// Event time
    #[serde(rename = "E")]
    pub event_time: i64,
    /// Symbol
    #[serde(rename = "s")]
    pub symbol: String,
    /// First update ID
    #[serde(rename = "U")]
    pub first_update_id: i64,
    /// Final update ID
    #[serde(rename = "u")]
    pub final_update_id: i64,
    /// Bids to update
    #[serde(rename = "b")]
    pub bids: Vec<[String; 2]>,
    /// Asks to update
    #[serde(rename = "a")]
    pub asks: Vec<[String; 2]>,
}

/// Binance partial depth snapshot.
///
/// Stream: `<symbol>@depth<levels>@<speed>ms`
#[derive(Debug, Clone, Deserialize)]
pub struct BinancePartialDepth {
    /// Last update ID
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: i64,
    /// Bids
    pub bids: Vec<[String; 2]>,
    /// Asks
    pub asks: Vec<[String; 2]>,
}

/// Combined stream wrapper.
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceCombinedStream<T> {
    /// Stream name
    pub stream: String,
    /// Data payload
    pub data: T,
}

/// Generic Binance WebSocket message.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum BinanceWsMessage {
    /// Subscription response
    SubscriptionResponse(BinanceSubscriptionResponse),
    /// Aggregate trade
    AggTrade(BinanceAggTrade),
    /// Book ticker
    BookTicker(BinanceBookTicker),
    /// Kline event
    Kline(BinanceKlineEvent),
    /// Mark price
    MarkPrice(BinanceMarkPrice),
    /// Depth update
    DepthUpdate(BinanceDepthUpdate),
    /// Partial depth
    PartialDepth(BinancePartialDepth),
    /// Combined stream
    Combined(BinanceCombinedStream<serde_json::Value>),
    /// Unknown message
    Unknown(serde_json::Value),
}

// ============================================================================
// REST API Types
// ============================================================================

/// Binance order side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BinanceOrderSide {
    /// Buy order
    Buy,
    /// Sell order
    Sell,
}

/// Binance order type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BinanceOrderType {
    /// Limit order
    Limit,
    /// Market order
    Market,
    /// Stop loss order
    StopLoss,
    /// Stop loss limit order
    StopLossLimit,
    /// Take profit order
    TakeProfit,
    /// Take profit limit order
    TakeProfitLimit,
    /// Limit maker order
    LimitMaker,
}

/// Binance order status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BinanceOrderStatus {
    /// New order
    New,
    /// Partially filled
    PartiallyFilled,
    /// Filled
    Filled,
    /// Canceled
    Canceled,
    /// Pending cancel
    PendingCancel,
    /// Rejected
    Rejected,
    /// Expired
    Expired,
    /// Expired in match
    ExpiredInMatch,
}

/// Binance time in force.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BinanceTimeInForce {
    /// Good till cancel
    Gtc,
    /// Immediate or cancel
    Ioc,
    /// Fill or kill
    Fok,
    /// Good till crossing (post only)
    Gtx,
    /// Good till date
    Gtd,
}

/// Binance position side (futures).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BinancePositionSide {
    /// Both (one-way mode)
    Both,
    /// Long
    Long,
    /// Short
    Short,
}

/// Binance margin type (futures).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BinanceMarginType {
    /// Isolated margin
    Isolated,
    /// Cross margin
    Cross,
}

/// Binance new order response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinanceOrderResponse {
    /// Symbol
    pub symbol: String,
    /// Order ID
    pub order_id: i64,
    /// Order list ID
    #[serde(default)]
    pub order_list_id: Option<i64>,
    /// Client order ID
    pub client_order_id: String,
    /// Transaction time
    #[serde(default)]
    pub transact_time: Option<i64>,
    /// Price
    #[serde(default)]
    pub price: Option<String>,
    /// Original quantity
    #[serde(default)]
    pub orig_qty: Option<String>,
    /// Executed quantity
    #[serde(default)]
    pub executed_qty: Option<String>,
    /// Cumulative quote quantity
    #[serde(default)]
    pub cummulative_quote_qty: Option<String>,
    /// Status
    pub status: BinanceOrderStatus,
    /// Time in force
    #[serde(default)]
    pub time_in_force: Option<BinanceTimeInForce>,
    /// Order type
    #[serde(rename = "type")]
    pub order_type: BinanceOrderType,
    /// Side
    pub side: BinanceOrderSide,
    /// Update time (futures)
    #[serde(default)]
    pub update_time: Option<i64>,
    /// Average price (futures)
    #[serde(default)]
    pub avg_price: Option<String>,
    /// Position side (futures)
    #[serde(default)]
    pub position_side: Option<BinancePositionSide>,
    /// Reduce only (futures)
    #[serde(default)]
    pub reduce_only: Option<bool>,
}

/// Binance cancel order response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinanceCancelResponse {
    /// Symbol
    pub symbol: String,
    /// Order ID
    pub order_id: i64,
    /// Original client order ID
    #[serde(default)]
    pub orig_client_order_id: Option<String>,
    /// Client order ID
    pub client_order_id: String,
    /// Status
    pub status: BinanceOrderStatus,
}

/// Binance position information (futures).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinancePosition {
    /// Symbol
    pub symbol: String,
    /// Position amount
    pub position_amt: String,
    /// Entry price
    pub entry_price: String,
    /// Break even price
    #[serde(default)]
    pub break_even_price: Option<String>,
    /// Mark price
    pub mark_price: String,
    /// Unrealized profit
    pub un_realized_profit: String,
    /// Liquidation price
    pub liquidation_price: String,
    /// Leverage
    pub leverage: String,
    /// Max notional value
    #[serde(default)]
    pub max_notional_value: Option<String>,
    /// Margin type
    pub margin_type: BinanceMarginType,
    /// Isolated margin
    #[serde(default)]
    pub isolated_margin: Option<String>,
    /// Is auto add margin
    #[serde(default)]
    pub is_auto_add_margin: Option<String>,
    /// Position side
    pub position_side: BinancePositionSide,
    /// Notional
    #[serde(default)]
    pub notional: Option<String>,
    /// Isolated wallet
    #[serde(default)]
    pub isolated_wallet: Option<String>,
    /// Update time
    pub update_time: i64,
}

/// Binance account balance (futures).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinanceBalance {
    /// Account alias
    #[serde(default)]
    pub account_alias: Option<String>,
    /// Asset
    pub asset: String,
    /// Balance
    pub balance: String,
    /// Cross wallet balance
    #[serde(default)]
    pub cross_wallet_balance: Option<String>,
    /// Cross unrealized PnL
    #[serde(default)]
    pub cross_un_pnl: Option<String>,
    /// Available balance
    pub available_balance: String,
    /// Max withdraw amount
    #[serde(default)]
    pub max_withdraw_amount: Option<String>,
    /// Margin available
    #[serde(default)]
    pub margin_available: Option<bool>,
    /// Update time
    #[serde(default)]
    pub update_time: Option<i64>,
}

/// Binance account information (futures).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinanceFuturesAccount {
    /// Fee tier
    #[serde(default)]
    pub fee_tier: Option<i32>,
    /// Can trade
    pub can_trade: bool,
    /// Can deposit
    pub can_deposit: bool,
    /// Can withdraw
    pub can_withdraw: bool,
    /// Update time
    pub update_time: i64,
    /// Multi assets margin
    #[serde(default)]
    pub multi_assets_margin: Option<bool>,
    /// Total initial margin
    pub total_initial_margin: String,
    /// Total maintenance margin
    pub total_maint_margin: String,
    /// Total wallet balance
    pub total_wallet_balance: String,
    /// Total unrealized profit
    pub total_unrealized_profit: String,
    /// Total margin balance
    pub total_margin_balance: String,
    /// Total position initial margin
    #[serde(default)]
    pub total_position_initial_margin: Option<String>,
    /// Total open order initial margin
    #[serde(default)]
    pub total_open_order_initial_margin: Option<String>,
    /// Total cross wallet balance
    #[serde(default)]
    pub total_cross_wallet_balance: Option<String>,
    /// Total cross unrealized PnL
    #[serde(default)]
    pub total_cross_un_pnl: Option<String>,
    /// Available balance
    pub available_balance: String,
    /// Max withdraw amount
    #[serde(default)]
    pub max_withdraw_amount: Option<String>,
    /// Assets
    #[serde(default)]
    pub assets: Vec<BinanceBalance>,
    /// Positions
    #[serde(default)]
    pub positions: Vec<BinancePosition>,
}

/// Binance API error response.
#[derive(Debug, Clone, Deserialize)]
pub struct BinanceApiError {
    /// Error code
    pub code: i32,
    /// Error message
    pub msg: String,
}

/// Helper to parse decimal from string.
pub fn parse_decimal(s: &str) -> Option<Decimal> {
    s.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_urls() {
        assert_eq!(
            BinanceMarket::Spot.ws_base_url(),
            "wss://stream.binance.com:9443/ws"
        );
        assert_eq!(
            BinanceMarket::UsdtFutures.rest_base_url(),
            "https://fapi.binance.com"
        );
    }

    #[test]
    fn test_subscription_serialize() {
        let sub = BinanceSubscription::subscribe(vec!["btcusdt@aggTrade".to_string()], 1);
        let json = serde_json::to_string(&sub).unwrap();
        assert!(json.contains("SUBSCRIBE"));
        assert!(json.contains("btcusdt@aggTrade"));
    }

    #[test]
    fn test_agg_trade_deserialize() {
        let json = r#"{
            "e": "aggTrade",
            "E": 1_672_515_782_136,
            "s": "BTCUSDT",
            "a": 123_456_789,
            "p": "42000.00",
            "q": "0.001",
            "f": 100,
            "l": 105,
            "T": 1_672_515_782_136,
            "m": true
        }"#;

        let trade: BinanceAggTrade = serde_json::from_str(json).unwrap();
        assert_eq!(trade.symbol, "BTCUSDT");
        assert_eq!(trade.price, "42000.00");
    }

    #[test]
    fn test_kline_deserialize() {
        let json = r#"{
            "e": "kline",
            "E": 1_672_515_782_136,
            "s": "BTCUSDT",
            "k": {
                "t": 1672515780000,
                "T": 1672515839999,
                "s": "BTCUSDT",
                "i": "1m",
                "f": 100,
                "L": 200,
                "o": "42000.00",
                "c": "42100.00",
                "h": "42150.00",
                "l": "41950.00",
                "v": "100.5",
                "n": 100,
                "x": false,
                "q": "4200000.00",
                "V": "50.25",
                "Q": "2100000.00"
            }
        }"#;

        let kline: BinanceKlineEvent = serde_json::from_str(json).unwrap();
        assert_eq!(kline.symbol, "BTCUSDT");
        assert_eq!(kline.kline.open, "42000.00");
        assert!(!kline.kline.is_closed);
    }

    #[test]
    fn test_book_ticker_deserialize() {
        let json = r#"{
            "u": 400900217,
            "s": "BTCUSDT",
            "b": "42000.00",
            "B": "10.5",
            "a": "42001.00",
            "A": "8.3"
        }"#;

        let ticker: BinanceBookTicker = serde_json::from_str(json).unwrap();
        assert_eq!(ticker.symbol, "BTCUSDT");
        assert_eq!(ticker.bid_price, "42000.00");
    }

    #[test]
    fn test_order_response_deserialize() {
        let json = r#"{
            "symbol": "BTCUSDT",
            "orderId": 12345,
            "clientOrderId": "test123",
            "status": "NEW",
            "type": "LIMIT",
            "side": "BUY"
        }"#;

        let order: BinanceOrderResponse = serde_json::from_str(json).unwrap();
        assert_eq!(order.symbol, "BTCUSDT");
        assert_eq!(order.order_id, 12345);
        assert_eq!(order.status, BinanceOrderStatus::New);
    }
}
