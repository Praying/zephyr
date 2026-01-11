//! Bitget-specific types and message formats.
//!
//! This module contains types for parsing Bitget WebSocket and REST API responses.

use serde::{Deserialize, Serialize};

/// Bitget market type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BitgetMarket {
    /// Spot market
    #[default]
    Spot,
    /// USDT-margined perpetual futures (UMCBL)
    UsdtFutures,
    /// Coin-margined perpetual futures (DMCBL)
    CoinFutures,
}

impl BitgetMarket {
    /// Returns the WebSocket base URL for this market.
    #[must_use]
    pub const fn ws_base_url(&self) -> &'static str {
        // All markets use the same WebSocket URL
        "wss://ws.bitget.com/v2/ws/public"
    }

    /// Returns the private WebSocket base URL.
    #[must_use]
    pub const fn ws_private_url(&self) -> &'static str {
        // All markets use the same private WebSocket URL
        "wss://ws.bitget.com/v2/ws/private"
    }

    /// Returns the REST API base URL.
    #[must_use]
    pub const fn rest_base_url(&self) -> &'static str {
        "https://api.bitget.com"
    }

    /// Returns the testnet WebSocket base URL.
    #[must_use]
    pub const fn testnet_ws_base_url(&self) -> &'static str {
        // All markets use the same testnet WebSocket URL
        "wss://ws.bitget.com/v2/ws/public"
    }

    /// Returns the testnet REST API base URL.
    #[must_use]
    pub const fn testnet_rest_base_url(&self) -> &'static str {
        "https://api.bitget.com"
    }

    /// Returns the product type string for Bitget API.
    #[must_use]
    pub const fn product_type(&self) -> &'static str {
        match self {
            Self::Spot => "SPOT",
            Self::UsdtFutures => "USDT-FUTURES",
            Self::CoinFutures => "COIN-FUTURES",
        }
    }

    /// Returns the instrument type for WebSocket subscriptions.
    #[must_use]
    pub const fn inst_type(&self) -> &'static str {
        match self {
            Self::Spot => "SPOT",
            Self::UsdtFutures => "USDT-FUTURES",
            Self::CoinFutures => "COIN-FUTURES",
        }
    }
}

/// Bitget WebSocket subscription request.
#[derive(Debug, Clone, Serialize)]
pub struct BitgetSubscription {
    /// Operation type
    pub op: String,
    /// Subscription arguments
    pub args: Vec<BitgetSubscriptionArg>,
}

/// Bitget subscription argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitgetSubscriptionArg {
    /// Instrument type (SPOT, USDT-FUTURES, COIN-FUTURES)
    pub inst_type: String,
    /// Channel name
    pub channel: String,
    /// Instrument ID (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inst_id: Option<String>,
}

impl BitgetSubscription {
    /// Creates a new subscription request.
    #[must_use]
    pub fn subscribe(args: Vec<BitgetSubscriptionArg>) -> Self {
        Self {
            op: "subscribe".to_string(),
            args,
        }
    }

    /// Creates a new unsubscription request.
    #[must_use]
    pub fn unsubscribe(args: Vec<BitgetSubscriptionArg>) -> Self {
        Self {
            op: "unsubscribe".to_string(),
            args,
        }
    }
}

/// Bitget WebSocket login request.
#[derive(Debug, Clone, Serialize)]
pub struct BitgetLogin {
    /// Operation type
    pub op: String,
    /// Login arguments
    pub args: Vec<BitgetLoginArg>,
}

/// Bitget login argument.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BitgetLoginArg {
    /// API key
    pub api_key: String,
    /// Passphrase
    pub passphrase: String,
    /// Timestamp (seconds)
    pub timestamp: String,
    /// Signature
    pub sign: String,
}

impl BitgetLogin {
    /// Creates a new login request.
    #[must_use]
    pub fn new(api_key: String, passphrase: String, timestamp: String, sign: String) -> Self {
        Self {
            op: "login".to_string(),
            args: vec![BitgetLoginArg {
                api_key,
                passphrase,
                timestamp,
                sign,
            }],
        }
    }
}

/// Bitget WebSocket response wrapper.
#[derive(Debug, Clone, Deserialize)]
pub struct BitgetWsResponse<T> {
    /// Event type (for subscription responses)
    #[serde(default)]
    pub event: Option<String>,
    /// Response code
    #[serde(default)]
    pub code: Option<String>,
    /// Response message
    #[serde(default)]
    pub msg: Option<String>,
    /// Action type (snapshot, update)
    #[serde(default)]
    pub action: Option<String>,
    /// Subscription argument
    #[serde(default)]
    pub arg: Option<BitgetSubscriptionArg>,
    /// Data payload
    #[serde(default)]
    pub data: Option<Vec<T>>,
    /// Timestamp
    #[serde(default)]
    pub ts: Option<String>,
}

/// Bitget ticker data (from ticker channel).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitgetTicker {
    /// Instrument ID
    pub inst_id: String,
    /// Last traded price
    pub last_pr: String,
    /// Best ask price
    pub ask_pr: String,
    /// Best bid price
    pub bid_pr: String,
    /// Best ask size
    #[serde(default)]
    pub ask_sz: Option<String>,
    /// Best bid size
    #[serde(default)]
    pub bid_sz: Option<String>,
    /// Open price (24h)
    #[serde(default)]
    pub open24h: Option<String>,
    /// High price (24h)
    #[serde(default)]
    pub high24h: Option<String>,
    /// Low price (24h)
    #[serde(default)]
    pub low24h: Option<String>,
    /// Base volume (24h)
    #[serde(default)]
    pub base_volume: Option<String>,
    /// Quote volume (24h)
    #[serde(default)]
    pub quote_volume: Option<String>,
    /// Timestamp
    pub ts: String,
    /// Open interest (for futures)
    #[serde(default)]
    pub open_utc: Option<String>,
    /// Change (24h)
    #[serde(default)]
    pub change_utc24h: Option<String>,
}

/// Bitget trade data (from trade channel).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitgetTrade {
    /// Instrument ID
    pub inst_id: String,
    /// Trade ID
    pub trade_id: String,
    /// Trade price
    pub price: String,
    /// Trade size
    pub size: String,
    /// Trade side (buy/sell)
    pub side: String,
    /// Timestamp
    pub ts: String,
}

/// Bitget candlestick data (from candle channel).
#[derive(Debug, Clone, Deserialize)]
pub struct BitgetCandle(pub Vec<String>);

impl BitgetCandle {
    /// Returns the timestamp.
    pub fn timestamp(&self) -> Option<&str> {
        self.0.first().map(String::as_str)
    }

    /// Returns the open price.
    pub fn open(&self) -> Option<&str> {
        self.0.get(1).map(String::as_str)
    }

    /// Returns the high price.
    pub fn high(&self) -> Option<&str> {
        self.0.get(2).map(String::as_str)
    }

    /// Returns the low price.
    pub fn low(&self) -> Option<&str> {
        self.0.get(3).map(String::as_str)
    }

    /// Returns the close price.
    pub fn close(&self) -> Option<&str> {
        self.0.get(4).map(String::as_str)
    }

    /// Returns the volume.
    pub fn volume(&self) -> Option<&str> {
        self.0.get(5).map(String::as_str)
    }

    /// Returns the quote volume.
    pub fn quote_volume(&self) -> Option<&str> {
        self.0.get(6).map(String::as_str)
    }
}

/// Bitget order book data (from books channel).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitgetOrderBook {
    /// Asks [price, size]
    pub asks: Vec<Vec<String>>,
    /// Bids [price, size]
    pub bids: Vec<Vec<String>>,
    /// Timestamp
    pub ts: String,
    /// Checksum (for verification)
    #[serde(default)]
    pub checksum: Option<i64>,
}

/// Bitget mark price data (from mark-price channel).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitgetMarkPrice {
    /// Instrument ID
    pub inst_id: String,
    /// Mark price
    pub mark_price: String,
    /// Timestamp
    pub ts: String,
}

/// Bitget funding rate data (from funding-rate channel).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitgetFundingRate {
    /// Instrument ID
    pub inst_id: String,
    /// Current funding rate
    pub funding_rate: String,
    /// Next funding time
    #[serde(default)]
    pub next_funding_time: Option<String>,
}

/// Generic Bitget WebSocket message.
pub type BitgetWsMessage = BitgetWsResponse<serde_json::Value>;

// ============================================================================
// REST API Types
// ============================================================================

/// Bitget order side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BitgetOrderSide {
    /// Buy order
    Buy,
    /// Sell order
    Sell,
}

/// Bitget order type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BitgetOrderType {
    /// Limit order
    Limit,
    /// Market order
    Market,
}

/// Bitget order status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BitgetOrderStatus {
    /// Order initialized
    Init,
    /// Order new (pending)
    New,
    /// Order partially filled
    #[serde(rename = "partial-fill")]
    PartiallyFilled,
    /// Order fully filled
    #[serde(rename = "full-fill")]
    Filled,
    /// Order canceled
    Cancelled,
    /// Order live
    Live,
}

/// Bitget position side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BitgetPositionSide {
    /// Long position
    Long,
    /// Short position
    Short,
}

/// Bitget margin mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BitgetMarginMode {
    /// Cross margin
    Crossed,
    /// Isolated margin
    Isolated,
}

/// Bitget time in force.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BitgetTimeInForce {
    /// Good till cancel
    Gtc,
    /// Fill or kill
    Fok,
    /// Immediate or cancel
    Ioc,
    /// Post only
    #[serde(rename = "post_only")]
    PostOnly,
}

/// Bitget REST API response wrapper.
#[derive(Debug, Clone, Deserialize)]
pub struct BitgetApiResponse<T> {
    /// Response code ("00000" for success)
    pub code: String,
    /// Response message
    pub msg: String,
    /// Request time
    #[serde(default, rename = "requestTime")]
    pub request_time: Option<i64>,
    /// Data payload
    pub data: Option<T>,
}

impl<T> BitgetApiResponse<T> {
    /// Returns true if the response indicates success.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.code == "00000"
    }
}

/// Bitget order response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitgetOrderResponse {
    /// Order ID
    pub order_id: String,
    /// Client order ID
    #[serde(default)]
    pub client_oid: Option<String>,
}

/// Bitget order details.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitgetOrderDetails {
    /// Instrument ID
    pub inst_id: String,
    /// Order ID
    pub order_id: String,
    /// Client order ID
    #[serde(default)]
    pub client_oid: Option<String>,
    /// Price
    #[serde(default)]
    pub price: Option<String>,
    /// Size
    pub size: String,
    /// Order type
    pub order_type: BitgetOrderType,
    /// Side
    pub side: BitgetOrderSide,
    /// Status
    pub status: BitgetOrderStatus,
    /// Filled size
    #[serde(default)]
    pub filled_qty: Option<String>,
    /// Average fill price
    #[serde(default)]
    pub price_avg: Option<String>,
    /// Fee
    #[serde(default)]
    pub fee: Option<String>,
    /// Fee currency
    #[serde(default)]
    pub fee_ccy: Option<String>,
    /// Creation time
    #[serde(default)]
    pub c_time: Option<String>,
    /// Update time
    #[serde(default)]
    pub u_time: Option<String>,
    /// Reduce only
    #[serde(default)]
    pub reduce_only: Option<String>,
    /// Time in force
    #[serde(default)]
    pub force: Option<BitgetTimeInForce>,
}

/// Bitget position details.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitgetPosition {
    /// Instrument ID
    pub inst_id: String,
    /// Margin mode
    pub margin_mode: BitgetMarginMode,
    /// Position side
    pub hold_side: BitgetPositionSide,
    /// Position quantity
    pub total: String,
    /// Available quantity
    pub available: String,
    /// Average price
    pub average_open_price: String,
    /// Unrealized `PnL`
    pub unrealized_pl: String,
    /// Realized `PnL`
    #[serde(default)]
    pub realized_pl: Option<String>,
    /// Leverage
    pub leverage: String,
    /// Liquidation price
    #[serde(default)]
    pub liquidation_price: Option<String>,
    /// Mark price
    #[serde(default)]
    pub mark_price: Option<String>,
    /// Margin
    #[serde(default)]
    pub margin: Option<String>,
    /// Margin ratio
    #[serde(default)]
    pub margin_ratio: Option<String>,
    /// Update time
    #[serde(default)]
    pub u_time: Option<String>,
    /// Creation time
    #[serde(default)]
    pub c_time: Option<String>,
}

/// Bitget account balance.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitgetBalance {
    /// Currency
    pub coin: String,
    /// Available balance
    pub available: String,
    /// Frozen balance
    #[serde(default)]
    pub frozen: Option<String>,
    /// Locked balance
    #[serde(default)]
    pub locked: Option<String>,
    /// Equity
    #[serde(default)]
    pub equity: Option<String>,
    /// Unrealized `PnL`
    #[serde(default)]
    pub upl: Option<String>,
}

/// Bitget account details.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitgetAccount {
    /// Margin coin
    pub margin_coin: String,
    /// Locked balance
    #[serde(default)]
    pub locked: Option<String>,
    /// Available balance
    pub available: String,
    /// Cross max available
    #[serde(default)]
    pub cross_max_available: Option<String>,
    /// Fixed max available
    #[serde(default)]
    pub fixed_max_available: Option<String>,
    /// Max transferable
    #[serde(default)]
    pub max_transfer_out: Option<String>,
    /// Equity
    #[serde(default)]
    pub equity: Option<String>,
    /// USD equity
    #[serde(default)]
    pub usd_equity: Option<String>,
    /// BTC equity
    #[serde(default)]
    pub btc_equity: Option<String>,
    /// Unrealized `PnL`
    #[serde(default)]
    pub unrealized_pl: Option<String>,
}

/// Bitget cancel order response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BitgetCancelResponse {
    /// Order ID
    pub order_id: String,
    /// Client order ID
    #[serde(default)]
    pub client_oid: Option<String>,
}

/// Bitget API error response.
#[derive(Debug, Clone, Deserialize)]
pub struct BitgetApiError {
    /// Error code
    pub code: String,
    /// Error message
    pub msg: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_urls() {
        assert_eq!(
            BitgetMarket::Spot.ws_base_url(),
            "wss://ws.bitget.com/v2/ws/public"
        );
        assert_eq!(
            BitgetMarket::UsdtFutures.rest_base_url(),
            "https://api.bitget.com"
        );
    }

    #[test]
    fn test_product_type() {
        assert_eq!(BitgetMarket::Spot.product_type(), "SPOT");
        assert_eq!(BitgetMarket::UsdtFutures.product_type(), "USDT-FUTURES");
        assert_eq!(BitgetMarket::CoinFutures.product_type(), "COIN-FUTURES");
    }

    #[test]
    fn test_subscription_serialize() {
        let sub = BitgetSubscription::subscribe(vec![BitgetSubscriptionArg {
            inst_type: "SPOT".to_string(),
            channel: "ticker".to_string(),
            inst_id: Some("BTCUSDT".to_string()),
        }]);
        let json = serde_json::to_string(&sub).unwrap();
        assert!(json.contains("subscribe"));
        assert!(json.contains("ticker"));
        assert!(json.contains("BTCUSDT"));
    }

    #[test]
    fn test_ticker_deserialize() {
        let json = r#"{
            "instId": "BTCUSDT",
            "lastPr": "42000.00",
            "askPr": "42001.00",
            "bidPr": "41999.00",
            "ts": "1_672_515_782_136"
        }"#;

        let ticker: BitgetTicker = serde_json::from_str(json).unwrap();
        assert_eq!(ticker.inst_id, "BTCUSDT");
        assert_eq!(ticker.last_pr, "42000.00");
    }

    #[test]
    fn test_trade_deserialize() {
        let json = r#"{
            "instId": "BTCUSDT",
            "tradeId": "123_456_789",
            "price": "42000.00",
            "size": "0.001",
            "side": "buy",
            "ts": "1_672_515_782_136"
        }"#;

        let trade: BitgetTrade = serde_json::from_str(json).unwrap();
        assert_eq!(trade.inst_id, "BTCUSDT");
        assert_eq!(trade.price, "42000.00");
    }

    #[test]
    fn test_api_response_success() {
        let json = r#"{
            "code": "00000",
            "msg": "success",
            "data": {"orderId": "123"}
        }"#;

        let resp: BitgetApiResponse<serde_json::Value> = serde_json::from_str(json).unwrap();
        assert!(resp.is_success());
    }

    #[test]
    fn test_candle_accessors() {
        let candle = BitgetCandle(vec![
            "1672515780000".to_string(),
            "42000.00".to_string(),
            "42150.00".to_string(),
            "41950.00".to_string(),
            "42100.00".to_string(),
            "100.5".to_string(),
            "4200000.00".to_string(),
        ]);

        assert_eq!(candle.timestamp(), Some("1672515780000"));
        assert_eq!(candle.open(), Some("42000.00"));
        assert_eq!(candle.high(), Some("42150.00"));
        assert_eq!(candle.low(), Some("41950.00"));
        assert_eq!(candle.close(), Some("42100.00"));
        assert_eq!(candle.volume(), Some("100.5"));
    }
}
