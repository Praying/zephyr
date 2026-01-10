//! OKX-specific types and message formats.
//!
//! This module contains types for parsing OKX WebSocket and REST API responses.

#![allow(clippy::doc_markdown)]
#![allow(clippy::match_same_arms)]

use serde::{Deserialize, Serialize};

/// OKX market type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OkxMarket {
    /// Spot market
    #[default]
    Spot,
    /// USDT-margined perpetual swap
    Swap,
    /// Coin-margined perpetual swap
    CoinSwap,
    /// Futures (delivery)
    Futures,
}

impl OkxMarket {
    /// Returns the WebSocket base URL for this market.
    #[must_use]
    pub const fn ws_base_url(&self) -> &'static str {
        // OKX uses the same WebSocket endpoint for all markets
        "wss://ws.okx.com:8443/ws/v5/public"
    }

    /// Returns the private WebSocket base URL.
    #[must_use]
    pub const fn ws_private_url(&self) -> &'static str {
        "wss://ws.okx.com:8443/ws/v5/private"
    }

    /// Returns the REST API base URL.
    #[must_use]
    pub const fn rest_base_url(&self) -> &'static str {
        "https://www.okx.com"
    }

    /// Returns the testnet WebSocket base URL.
    #[must_use]
    pub const fn testnet_ws_base_url(&self) -> &'static str {
        "wss://wspap.okx.com:8443/ws/v5/public?brokerId=9999"
    }

    /// Returns the testnet private WebSocket base URL.
    #[must_use]
    pub const fn testnet_ws_private_url(&self) -> &'static str {
        "wss://wspap.okx.com:8443/ws/v5/private?brokerId=9999"
    }

    /// Returns the testnet REST API base URL.
    #[must_use]
    pub const fn testnet_rest_base_url(&self) -> &'static str {
        "https://www.okx.com"
    }

    /// Returns the instrument type string for OKX API.
    #[must_use]
    pub const fn inst_type(&self) -> &'static str {
        match self {
            Self::Spot => "SPOT",
            Self::Swap => "SWAP",
            Self::CoinSwap => "SWAP",
            Self::Futures => "FUTURES",
        }
    }
}

/// OKX WebSocket subscription request.
#[derive(Debug, Clone, Serialize)]
pub struct OkxSubscription {
    /// Operation type
    pub op: String,
    /// Subscription arguments
    pub args: Vec<OkxSubscriptionArg>,
}

/// OKX subscription argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OkxSubscriptionArg {
    /// Channel name
    pub channel: String,
    /// Instrument type (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inst_type: Option<String>,
    /// Instrument ID (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inst_id: Option<String>,
}

impl OkxSubscription {
    /// Creates a new subscription request.
    #[must_use]
    pub fn subscribe(args: Vec<OkxSubscriptionArg>) -> Self {
        Self {
            op: "subscribe".to_string(),
            args,
        }
    }

    /// Creates a new unsubscription request.
    #[must_use]
    pub fn unsubscribe(args: Vec<OkxSubscriptionArg>) -> Self {
        Self {
            op: "unsubscribe".to_string(),
            args,
        }
    }
}

/// OKX WebSocket login request.
#[derive(Debug, Clone, Serialize)]
pub struct OkxLogin {
    /// Operation type
    pub op: String,
    /// Login arguments
    pub args: Vec<OkxLoginArg>,
}

/// OKX login argument.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OkxLoginArg {
    /// API key
    pub api_key: String,
    /// Passphrase
    pub passphrase: String,
    /// Timestamp
    pub timestamp: String,
    /// Signature
    pub sign: String,
}

impl OkxLogin {
    /// Creates a new login request.
    #[must_use]
    pub fn new(api_key: String, passphrase: String, timestamp: String, sign: String) -> Self {
        Self {
            op: "login".to_string(),
            args: vec![OkxLoginArg {
                api_key,
                passphrase,
                timestamp,
                sign,
            }],
        }
    }
}

/// OKX WebSocket response wrapper.
#[derive(Debug, Clone, Deserialize)]
pub struct OkxWsResponse<T> {
    /// Event type (for subscription responses)
    #[serde(default)]
    pub event: Option<String>,
    /// Response code
    #[serde(default)]
    pub code: Option<String>,
    /// Response message
    #[serde(default)]
    pub msg: Option<String>,
    /// Subscription argument (for subscription responses)
    #[serde(default)]
    pub arg: Option<OkxSubscriptionArg>,
    /// Data payload
    #[serde(default)]
    pub data: Option<Vec<T>>,
    /// Connection ID
    #[serde(default, rename = "connId")]
    pub conn_id: Option<String>,
}

/// OKX ticker data (from tickers channel).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OkxTicker {
    /// Instrument type
    pub inst_type: String,
    /// Instrument ID
    pub inst_id: String,
    /// Last traded price
    pub last: String,
    /// Last traded size
    pub last_sz: String,
    /// Best ask price
    pub ask_px: String,
    /// Best ask size
    pub ask_sz: String,
    /// Best bid price
    pub bid_px: String,
    /// Best bid size
    pub bid_sz: String,
    /// Open price (24h)
    pub open24h: String,
    /// High price (24h)
    pub high24h: String,
    /// Low price (24h)
    pub low24h: String,
    /// Volume (24h) in base currency
    pub vol_ccy24h: String,
    /// Volume (24h) in contracts
    pub vol24h: String,
    /// Timestamp
    pub ts: String,
    /// Open interest (for derivatives)
    #[serde(default)]
    pub sod_utc0: Option<String>,
    /// Open interest (for derivatives)
    #[serde(default)]
    pub sod_utc8: Option<String>,
}

/// OKX trade data (from trades channel).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OkxTrade {
    /// Instrument ID
    pub inst_id: String,
    /// Trade ID
    pub trade_id: String,
    /// Trade price
    pub px: String,
    /// Trade size
    pub sz: String,
    /// Trade side
    pub side: String,
    /// Timestamp
    pub ts: String,
}

/// OKX candlestick data (from candle channel).
#[derive(Debug, Clone, Deserialize)]
pub struct OkxCandle {
    /// Timestamp
    #[serde(rename = "0")]
    pub ts: String,
    /// Open price
    #[serde(rename = "1")]
    pub open: String,
    /// High price
    #[serde(rename = "2")]
    pub high: String,
    /// Low price
    #[serde(rename = "3")]
    pub low: String,
    /// Close price
    #[serde(rename = "4")]
    pub close: String,
    /// Volume in base currency
    #[serde(rename = "5")]
    pub vol: String,
    /// Volume in quote currency
    #[serde(rename = "6")]
    pub vol_ccy: String,
    /// Volume in quote currency (for derivatives)
    #[serde(rename = "7")]
    pub vol_ccy_quote: String,
    /// Confirm flag (0: not confirmed, 1: confirmed)
    #[serde(rename = "8")]
    pub confirm: String,
}

/// OKX order book data (from books channel).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OkxOrderBook {
    /// Asks [price, size, deprecated, number of orders]
    pub asks: Vec<Vec<String>>,
    /// Bids [price, size, deprecated, number of orders]
    pub bids: Vec<Vec<String>>,
    /// Timestamp
    pub ts: String,
    /// Checksum (for verification)
    #[serde(default)]
    pub checksum: Option<i64>,
    /// Previous sequence ID
    #[serde(default)]
    pub prev_seq_id: Option<i64>,
    /// Sequence ID
    #[serde(default)]
    pub seq_id: Option<i64>,
}

/// OKX mark price data (from mark-price channel).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OkxMarkPrice {
    /// Instrument type
    pub inst_type: String,
    /// Instrument ID
    pub inst_id: String,
    /// Mark price
    pub mark_px: String,
    /// Timestamp
    pub ts: String,
}

/// OKX funding rate data (from funding-rate channel).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OkxFundingRate {
    /// Instrument type
    pub inst_type: String,
    /// Instrument ID
    pub inst_id: String,
    /// Current funding rate
    pub funding_rate: String,
    /// Next funding rate
    pub next_funding_rate: String,
    /// Funding time
    pub funding_time: String,
    /// Next funding time
    pub next_funding_time: String,
}

/// Generic OKX WebSocket message.
///
/// Note: OKX uses a unified response format, so we parse as generic JSON
/// and then dispatch based on the channel in the `arg` field.
pub type OkxWsMessage = OkxWsResponse<serde_json::Value>;

// ============================================================================
// REST API Types
// ============================================================================

/// OKX order side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OkxOrderSide {
    /// Buy order
    Buy,
    /// Sell order
    Sell,
}

/// OKX order type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OkxOrderType {
    /// Market order
    Market,
    /// Limit order
    Limit,
    /// Post-only order
    #[serde(rename = "post_only")]
    PostOnly,
    /// Fill or kill
    Fok,
    /// Immediate or cancel
    Ioc,
    /// Optimal limit order (market with slippage protection)
    #[serde(rename = "optimal_limit_ioc")]
    OptimalLimitIoc,
}

/// OKX order status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OkxOrderStatus {
    /// Order canceled
    Canceled,
    /// Order live (pending)
    Live,
    /// Order partially filled
    #[serde(rename = "partially_filled")]
    PartiallyFilled,
    /// Order filled
    Filled,
    /// Order rejected (for algo orders)
    #[serde(rename = "mmp_canceled")]
    MmpCanceled,
}

/// OKX position side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OkxPositionSide {
    /// Long position
    Long,
    /// Short position
    Short,
    /// Net position (one-way mode)
    Net,
}

/// OKX margin mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OkxMarginMode {
    /// Cross margin
    Cross,
    /// Isolated margin
    Isolated,
}

/// OKX trade mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OkxTradeMode {
    /// Cross margin
    Cross,
    /// Isolated margin
    Isolated,
    /// Cash (spot)
    Cash,
}

/// OKX REST API response wrapper.
#[derive(Debug, Clone, Deserialize)]
pub struct OkxApiResponse<T> {
    /// Response code ("0" for success)
    pub code: String,
    /// Response message
    pub msg: String,
    /// Data payload
    pub data: Vec<T>,
}

impl<T> OkxApiResponse<T> {
    /// Returns true if the response indicates success.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.code == "0"
    }
}

/// OKX order response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OkxOrderResponse {
    /// Client order ID
    pub cl_ord_id: String,
    /// Order ID
    pub ord_id: String,
    /// Tag
    #[serde(default)]
    pub tag: Option<String>,
    /// Response code
    pub s_code: String,
    /// Response message
    pub s_msg: String,
}

/// OKX order details.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OkxOrderDetails {
    /// Instrument type
    pub inst_type: String,
    /// Instrument ID
    pub inst_id: String,
    /// Order ID
    pub ord_id: String,
    /// Client order ID
    pub cl_ord_id: String,
    /// Tag
    #[serde(default)]
    pub tag: Option<String>,
    /// Price
    pub px: String,
    /// Size
    pub sz: String,
    /// Order type
    pub ord_type: OkxOrderType,
    /// Side
    pub side: OkxOrderSide,
    /// Position side
    pub pos_side: OkxPositionSide,
    /// Trade mode
    pub td_mode: OkxTradeMode,
    /// Accumulated fill quantity
    pub acc_fill_sz: String,
    /// Average fill price
    #[serde(default)]
    pub avg_px: Option<String>,
    /// State
    pub state: OkxOrderStatus,
    /// Leverage
    #[serde(default)]
    pub lever: Option<String>,
    /// Fee
    #[serde(default)]
    pub fee: Option<String>,
    /// Fee currency
    #[serde(default)]
    pub fee_ccy: Option<String>,
    /// Rebate
    #[serde(default)]
    pub rebate: Option<String>,
    /// Rebate currency
    #[serde(default)]
    pub rebate_ccy: Option<String>,
    /// PnL
    #[serde(default)]
    pub pnl: Option<String>,
    /// Category
    #[serde(default)]
    pub category: Option<String>,
    /// Reduce only
    #[serde(default)]
    pub reduce_only: Option<String>,
    /// Creation time
    pub c_time: String,
    /// Update time
    pub u_time: String,
}

/// OKX position details.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OkxPosition {
    /// Instrument type
    pub inst_type: String,
    /// Instrument ID
    pub inst_id: String,
    /// Margin mode
    pub mgn_mode: OkxMarginMode,
    /// Position ID
    pub pos_id: String,
    /// Position side
    pub pos_side: OkxPositionSide,
    /// Position quantity
    pub pos: String,
    /// Base currency balance (for spot)
    #[serde(default)]
    pub base_bal: Option<String>,
    /// Quote currency balance (for spot)
    #[serde(default)]
    pub quote_bal: Option<String>,
    /// Average price
    pub avg_px: String,
    /// Unrealized PnL
    pub upl: String,
    /// Unrealized PnL ratio
    pub upl_ratio: String,
    /// Leverage
    #[serde(default)]
    pub lever: Option<String>,
    /// Liquidation price
    #[serde(default)]
    pub liq_px: Option<String>,
    /// Mark price
    #[serde(default)]
    pub mark_px: Option<String>,
    /// Initial margin
    #[serde(default)]
    pub imr: Option<String>,
    /// Maintenance margin
    #[serde(default)]
    pub mmr: Option<String>,
    /// Margin
    #[serde(default)]
    pub margin: Option<String>,
    /// Margin ratio
    #[serde(default)]
    pub mgn_ratio: Option<String>,
    /// Interest
    #[serde(default)]
    pub interest: Option<String>,
    /// Notional value
    #[serde(default)]
    pub notional_usd: Option<String>,
    /// ADL indicator
    #[serde(default)]
    pub adl: Option<String>,
    /// Creation time
    pub c_time: String,
    /// Update time
    pub u_time: String,
}

/// OKX account balance.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OkxBalance {
    /// Currency
    pub ccy: String,
    /// Equity
    pub eq: String,
    /// Cash balance
    pub cash_bal: String,
    /// Available equity
    pub avail_eq: String,
    /// Frozen balance
    #[serde(default)]
    pub frozen_bal: Option<String>,
    /// Order frozen
    #[serde(default)]
    pub ord_frozen: Option<String>,
    /// Unrealized PnL
    #[serde(default)]
    pub upl: Option<String>,
    /// Update time
    pub u_time: String,
}

/// OKX account details.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OkxAccount {
    /// Total equity in USD
    pub total_eq: String,
    /// Isolated margin equity in USD
    #[serde(default)]
    pub iso_eq: Option<String>,
    /// Adjusted equity in USD
    #[serde(default)]
    pub adj_eq: Option<String>,
    /// Order frozen
    #[serde(default)]
    pub ord_froz: Option<String>,
    /// Initial margin requirement
    #[serde(default)]
    pub imr: Option<String>,
    /// Maintenance margin requirement
    #[serde(default)]
    pub mmr: Option<String>,
    /// Margin ratio
    #[serde(default)]
    pub mgn_ratio: Option<String>,
    /// Notional value in USD
    #[serde(default)]
    pub notional_usd: Option<String>,
    /// Account details by currency
    pub details: Vec<OkxBalance>,
    /// Update time
    pub u_time: String,
}

/// OKX cancel order response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OkxCancelResponse {
    /// Client order ID
    pub cl_ord_id: String,
    /// Order ID
    pub ord_id: String,
    /// Response code
    pub s_code: String,
    /// Response message
    pub s_msg: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_urls() {
        assert_eq!(
            OkxMarket::Spot.ws_base_url(),
            "wss://ws.okx.com:8443/ws/v5/public"
        );
        assert_eq!(OkxMarket::Swap.rest_base_url(), "https://www.okx.com");
    }

    #[test]
    fn test_inst_type() {
        assert_eq!(OkxMarket::Spot.inst_type(), "SPOT");
        assert_eq!(OkxMarket::Swap.inst_type(), "SWAP");
        assert_eq!(OkxMarket::Futures.inst_type(), "FUTURES");
    }

    #[test]
    fn test_subscription_serialize() {
        let sub = OkxSubscription::subscribe(vec![OkxSubscriptionArg {
            channel: "tickers".to_string(),
            inst_type: None,
            inst_id: Some("BTC-USDT".to_string()),
        }]);
        let json = serde_json::to_string(&sub).unwrap();
        assert!(json.contains("subscribe"));
        assert!(json.contains("tickers"));
        assert!(json.contains("BTC-USDT"));
    }

    #[test]
    fn test_ticker_deserialize() {
        let json = r#"{
            "instType": "SPOT",
            "instId": "BTC-USDT",
            "last": "42000.00",
            "lastSz": "0.001",
            "askPx": "42001.00",
            "askSz": "10.5",
            "bidPx": "41999.00",
            "bidSz": "8.3",
            "open24h": "41000.00",
            "high24h": "43000.00",
            "low24h": "40000.00",
            "volCcy24h": "1000000.00",
            "vol24h": "25.5",
            "ts": "1672515782136"
        }"#;

        let ticker: OkxTicker = serde_json::from_str(json).unwrap();
        assert_eq!(ticker.inst_id, "BTC-USDT");
        assert_eq!(ticker.last, "42000.00");
    }

    #[test]
    fn test_trade_deserialize() {
        let json = r#"{
            "instId": "BTC-USDT",
            "tradeId": "123456789",
            "px": "42000.00",
            "sz": "0.001",
            "side": "buy",
            "ts": "1672515782136"
        }"#;

        let trade: OkxTrade = serde_json::from_str(json).unwrap();
        assert_eq!(trade.inst_id, "BTC-USDT");
        assert_eq!(trade.px, "42000.00");
    }

    #[test]
    fn test_order_response_deserialize() {
        let json = r#"{
            "clOrdId": "test123",
            "ordId": "12345",
            "sCode": "0",
            "sMsg": ""
        }"#;

        let order: OkxOrderResponse = serde_json::from_str(json).unwrap();
        assert_eq!(order.ord_id, "12345");
        assert_eq!(order.s_code, "0");
    }

    #[test]
    fn test_api_response_success() {
        let json = r#"{
            "code": "0",
            "msg": "",
            "data": [{"ordId": "123"}]
        }"#;

        let resp: OkxApiResponse<serde_json::Value> = serde_json::from_str(json).unwrap();
        assert!(resp.is_success());
    }
}
