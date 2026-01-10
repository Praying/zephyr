//! Hyperliquid-specific types and message formats.
//!
//! This module contains types for parsing Hyperliquid WebSocket and REST API responses.
//! Hyperliquid is a decentralized perpetual exchange built on its own L1 blockchain.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Hyperliquid market type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HyperliquidMarket {
    /// Perpetual futures (main market)
    #[default]
    Perpetual,
    /// Spot market
    Spot,
}

impl HyperliquidMarket {
    /// Returns the WebSocket base URL for this market.
    #[must_use]
    pub const fn ws_base_url(&self) -> &'static str {
        match self {
            Self::Perpetual | Self::Spot => "wss://api.hyperliquid.xyz/ws",
        }
    }

    /// Returns the REST API base URL for this market.
    #[must_use]
    pub const fn rest_base_url(&self) -> &'static str {
        match self {
            Self::Perpetual | Self::Spot => "https://api.hyperliquid.xyz",
        }
    }

    /// Returns the testnet WebSocket base URL.
    #[must_use]
    pub const fn testnet_ws_base_url(&self) -> &'static str {
        match self {
            Self::Perpetual | Self::Spot => "wss://api.hyperliquid-testnet.xyz/ws",
        }
    }

    /// Returns the testnet REST API base URL.
    #[must_use]
    pub const fn testnet_rest_base_url(&self) -> &'static str {
        match self {
            Self::Perpetual | Self::Spot => "https://api.hyperliquid-testnet.xyz",
        }
    }
}

// ============================================================================
// WebSocket Subscription Types
// ============================================================================

/// Hyperliquid WebSocket subscription request.
#[derive(Debug, Clone, Serialize)]
pub struct HyperliquidSubscription {
    /// Request method
    pub method: String,
    /// Subscription parameters
    pub subscription: HyperliquidSubParams,
}

/// Subscription parameters.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum HyperliquidSubParams {
    /// Subscribe to all mid prices
    #[serde(rename = "allMids")]
    AllMids,
    /// Subscribe to L2 order book
    #[serde(rename = "l2Book")]
    L2Book { coin: String },
    /// Subscribe to trades
    #[serde(rename = "trades")]
    Trades { coin: String },
    /// Subscribe to user events (requires authentication)
    #[serde(rename = "userEvents")]
    UserEvents { user: String },
    /// Subscribe to user fills
    #[serde(rename = "userFills")]
    UserFills { user: String },
    /// Subscribe to user funding
    #[serde(rename = "userFundings")]
    UserFundings { user: String },
    /// Subscribe to candles
    #[serde(rename = "candle")]
    Candle { coin: String, interval: String },
}

impl HyperliquidSubscription {
    /// Creates a subscription request.
    #[must_use]
    pub fn subscribe(params: HyperliquidSubParams) -> Self {
        Self {
            method: "subscribe".to_string(),
            subscription: params,
        }
    }

    /// Creates an unsubscription request.
    #[must_use]
    pub fn unsubscribe(params: HyperliquidSubParams) -> Self {
        Self {
            method: "unsubscribe".to_string(),
            subscription: params,
        }
    }
}

// ============================================================================
// WebSocket Message Types
// ============================================================================

/// Hyperliquid WebSocket message wrapper.
#[derive(Debug, Clone, Deserialize)]
pub struct HyperliquidWsResponse {
    /// Channel name
    pub channel: String,
    /// Message data
    pub data: serde_json::Value,
}

/// All mids (mid prices) update.
#[derive(Debug, Clone, Deserialize)]
pub struct HyperliquidAllMids {
    /// Map of coin to mid price
    pub mids: std::collections::HashMap<String, String>,
}

/// L2 order book snapshot.
#[derive(Debug, Clone, Deserialize)]
pub struct HyperliquidL2Book {
    /// Coin symbol
    pub coin: String,
    /// Timestamp
    pub time: i64,
    /// Order book levels
    pub levels: Vec<Vec<HyperliquidBookLevel>>,
}

/// Order book level.
#[derive(Debug, Clone, Deserialize)]
pub struct HyperliquidBookLevel {
    /// Price
    #[serde(rename = "px")]
    pub price: String,
    /// Size
    #[serde(rename = "sz")]
    pub size: String,
    /// Number of orders at this level
    #[serde(rename = "n")]
    pub num_orders: i32,
}

/// Trade message.
#[derive(Debug, Clone, Deserialize)]
pub struct HyperliquidTrade {
    /// Coin symbol
    pub coin: String,
    /// Trade side ("B" for buy, "S" for sell)
    pub side: String,
    /// Price
    #[serde(rename = "px")]
    pub price: String,
    /// Size
    #[serde(rename = "sz")]
    pub size: String,
    /// Trade hash
    pub hash: String,
    /// Timestamp
    pub time: i64,
    /// Trade ID
    pub tid: i64,
}

/// Candle/Kline data.
#[derive(Debug, Clone, Deserialize)]
pub struct HyperliquidCandle {
    /// Timestamp (start of candle)
    #[serde(rename = "t")]
    pub timestamp: i64,
    /// Open price
    #[serde(rename = "o")]
    pub open: String,
    /// High price
    #[serde(rename = "h")]
    pub high: String,
    /// Low price
    #[serde(rename = "l")]
    pub low: String,
    /// Close price
    #[serde(rename = "c")]
    pub close: String,
    /// Volume
    #[serde(rename = "v")]
    pub volume: String,
    /// Number of trades
    #[serde(rename = "n")]
    pub num_trades: i64,
}

/// User fill event.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HyperliquidUserFill {
    /// Coin symbol
    pub coin: String,
    /// Price
    pub px: String,
    /// Size
    pub sz: String,
    /// Side ("B" or "S")
    pub side: String,
    /// Timestamp
    pub time: i64,
    /// Start position
    pub start_position: String,
    /// Direction
    pub dir: String,
    /// Closed PnL
    pub closed_pnl: String,
    /// Trade hash
    pub hash: String,
    /// Order ID
    pub oid: i64,
    /// Whether crossed (taker)
    pub crossed: bool,
    /// Fee
    pub fee: String,
    /// Trade ID
    pub tid: i64,
}

/// User funding event.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HyperliquidUserFunding {
    /// Timestamp
    pub time: i64,
    /// Coin symbol
    pub coin: String,
    /// USD value
    pub usdc: String,
    /// Size
    pub szi: String,
    /// Funding rate
    pub funding_rate: String,
}

// ============================================================================
// REST API Types
// ============================================================================

/// Hyperliquid order side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HyperliquidOrderSide {
    /// Buy order
    #[serde(rename = "B")]
    Buy,
    /// Sell order
    #[serde(rename = "A")]
    Sell,
}

/// Hyperliquid order type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "limit")]
pub enum HyperliquidOrderType {
    /// Good till cancel
    #[serde(rename = "Gtc")]
    Gtc,
    /// Immediate or cancel
    #[serde(rename = "Ioc")]
    Ioc,
    /// Fill or kill
    #[serde(rename = "Alo")]
    Alo,
}

/// Hyperliquid time in force.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HyperliquidTif {
    /// Good till cancel
    Gtc,
    /// Immediate or cancel
    Ioc,
    /// Add liquidity only (post only)
    Alo,
}

/// Order request for Hyperliquid.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HyperliquidOrderRequest {
    /// Asset index
    #[serde(rename = "a")]
    pub asset: u32,
    /// Is buy
    #[serde(rename = "b")]
    pub is_buy: bool,
    /// Price
    #[serde(rename = "p")]
    pub price: String,
    /// Size
    #[serde(rename = "s")]
    pub size: String,
    /// Reduce only
    #[serde(rename = "r")]
    pub reduce_only: bool,
    /// Order type
    #[serde(rename = "t")]
    pub order_type: HyperliquidOrderTypeSpec,
}

/// Order type specification.
#[derive(Debug, Clone, Serialize)]
pub struct HyperliquidOrderTypeSpec {
    /// Limit order parameters
    pub limit: HyperliquidLimitParams,
}

/// Limit order parameters.
#[derive(Debug, Clone, Serialize)]
pub struct HyperliquidLimitParams {
    /// Time in force
    pub tif: String,
}

/// Cancel order request.
#[derive(Debug, Clone, Serialize)]
pub struct HyperliquidCancelRequest {
    /// Asset index
    #[serde(rename = "a")]
    pub asset: u32,
    /// Order ID
    #[serde(rename = "o")]
    pub oid: i64,
}

/// Hyperliquid action types for signing.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum HyperliquidAction {
    /// Place order
    #[serde(rename = "order")]
    Order {
        /// Orders to place
        orders: Vec<HyperliquidOrderRequest>,
        /// Grouping type
        grouping: String,
    },
    /// Cancel order
    #[serde(rename = "cancel")]
    Cancel {
        /// Cancels
        cancels: Vec<HyperliquidCancelRequest>,
    },
    /// Cancel by client ID
    #[serde(rename = "cancelByCloid")]
    CancelByCloid {
        /// Cancels
        cancels: Vec<HyperliquidCancelByCloid>,
    },
}

/// Cancel by client order ID.
#[derive(Debug, Clone, Serialize)]
pub struct HyperliquidCancelByCloid {
    /// Asset index
    pub asset: u32,
    /// Client order ID
    pub cloid: String,
}

/// Exchange request wrapper.
#[derive(Debug, Clone, Serialize)]
pub struct HyperliquidExchangeRequest {
    /// Action to perform
    pub action: HyperliquidAction,
    /// Nonce
    pub nonce: u64,
    /// Signature
    pub signature: HyperliquidSignature,
    /// Vault address (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vault_address: Option<String>,
}

/// Signature for Hyperliquid requests.
#[derive(Debug, Clone, Serialize)]
pub struct HyperliquidSignature {
    /// R component
    pub r: String,
    /// S component
    pub s: String,
    /// V component
    pub v: u8,
}

// ============================================================================
// REST API Response Types
// ============================================================================

/// Order response from Hyperliquid.
#[derive(Debug, Clone, Deserialize)]
pub struct HyperliquidOrderResponse {
    /// Status
    pub status: String,
    /// Response data
    pub response: Option<HyperliquidOrderResponseData>,
}

/// Order response data.
#[derive(Debug, Clone, Deserialize)]
pub struct HyperliquidOrderResponseData {
    /// Type of response
    #[serde(rename = "type")]
    pub response_type: String,
    /// Order statuses
    pub data: Option<HyperliquidOrderStatuses>,
}

/// Order statuses.
#[derive(Debug, Clone, Deserialize)]
pub struct HyperliquidOrderStatuses {
    /// Statuses for each order
    pub statuses: Vec<HyperliquidOrderStatus>,
}

/// Individual order status.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum HyperliquidOrderStatus {
    /// Successful order
    Success {
        /// Resting order info
        resting: Option<HyperliquidRestingOrder>,
        /// Filled order info
        filled: Option<HyperliquidFilledOrder>,
    },
    /// Error
    Error {
        /// Error message
        error: String,
    },
}

/// Resting order info.
#[derive(Debug, Clone, Deserialize)]
pub struct HyperliquidRestingOrder {
    /// Order ID
    pub oid: i64,
}

/// Filled order info.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HyperliquidFilledOrder {
    /// Total size filled
    pub total_sz: String,
    /// Average price
    pub avg_px: String,
    /// Order ID
    pub oid: i64,
}

/// User state (positions and account info).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HyperliquidUserState {
    /// Asset positions
    pub asset_positions: Vec<HyperliquidAssetPosition>,
    /// Cross margin summary
    pub cross_margin_summary: HyperliquidMarginSummary,
    /// Margin summary (deprecated, use cross_margin_summary)
    pub margin_summary: Option<HyperliquidMarginSummary>,
    /// Withdrawable amount
    pub withdrawable: String,
}

/// Asset position.
#[derive(Debug, Clone, Deserialize)]
pub struct HyperliquidAssetPosition {
    /// Position info
    pub position: HyperliquidPosition,
    /// Position type
    #[serde(rename = "type")]
    pub position_type: String,
}

/// Position details.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HyperliquidPosition {
    /// Coin symbol
    pub coin: String,
    /// Entry price
    pub entry_px: Option<String>,
    /// Leverage info
    pub leverage: HyperliquidLeverage,
    /// Liquidation price
    pub liquidation_px: Option<String>,
    /// Margin used
    pub margin_used: String,
    /// Max trade sizes
    pub max_trade_szs: Vec<String>,
    /// Position value
    pub position_value: String,
    /// Return on equity
    pub return_on_equity: String,
    /// Size (positive for long, negative for short)
    pub szi: String,
    /// Unrealized PnL
    pub unrealized_pnl: String,
}

/// Leverage info.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HyperliquidLeverage {
    /// Leverage type
    #[serde(rename = "type")]
    pub leverage_type: String,
    /// Leverage value
    pub value: i32,
    /// Raw USD value (for isolated margin)
    pub raw_usd: Option<String>,
}

/// Margin summary.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HyperliquidMarginSummary {
    /// Account value
    pub account_value: String,
    /// Total margin used
    pub total_margin_used: String,
    /// Total notional position
    pub total_ntl_pos: String,
    /// Total raw USD
    pub total_raw_usd: String,
}

/// Open orders response.
#[derive(Debug, Clone, Deserialize)]
pub struct HyperliquidOpenOrder {
    /// Coin symbol
    pub coin: String,
    /// Limit price
    #[serde(rename = "limitPx")]
    pub limit_px: String,
    /// Order ID
    pub oid: i64,
    /// Side ("B" or "A")
    pub side: String,
    /// Size
    pub sz: String,
    /// Timestamp
    pub timestamp: i64,
}

/// Meta info for assets.
#[derive(Debug, Clone, Deserialize)]
pub struct HyperliquidMeta {
    /// Universe of assets
    pub universe: Vec<HyperliquidAssetInfo>,
}

/// Asset info.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HyperliquidAssetInfo {
    /// Asset name/symbol
    pub name: String,
    /// Size decimals
    pub sz_decimals: i32,
    /// Max leverage
    pub max_leverage: i32,
    /// Only isolated margin allowed
    pub only_isolated: Option<bool>,
}

/// API error response.
#[derive(Debug, Clone, Deserialize)]
pub struct HyperliquidApiError {
    /// Error message
    pub error: String,
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
            HyperliquidMarket::Perpetual.ws_base_url(),
            "wss://api.hyperliquid.xyz/ws"
        );
        assert_eq!(
            HyperliquidMarket::Perpetual.rest_base_url(),
            "https://api.hyperliquid.xyz"
        );
    }

    #[test]
    fn test_subscription_serialize() {
        let sub = HyperliquidSubscription::subscribe(HyperliquidSubParams::AllMids);
        let json = serde_json::to_string(&sub).unwrap();
        assert!(json.contains("subscribe"));
        assert!(json.contains("allMids"));
    }

    #[test]
    fn test_l2_book_subscribe() {
        let sub = HyperliquidSubscription::subscribe(HyperliquidSubParams::L2Book {
            coin: "BTC".to_string(),
        });
        let json = serde_json::to_string(&sub).unwrap();
        assert!(json.contains("l2Book"));
        assert!(json.contains("BTC"));
    }

    #[test]
    fn test_trade_deserialize() {
        let json = r#"{
            "coin": "BTC",
            "side": "B",
            "px": "42000.0",
            "sz": "0.1",
            "hash": "0x123",
            "time": 1672515782136,
            "tid": 12345
        }"#;

        let trade: HyperliquidTrade = serde_json::from_str(json).unwrap();
        assert_eq!(trade.coin, "BTC");
        assert_eq!(trade.side, "B");
        assert_eq!(trade.price, "42000.0");
    }

    #[test]
    fn test_book_level_deserialize() {
        let json = r#"{"px": "42000.0", "sz": "1.5", "n": 3}"#;
        let level: HyperliquidBookLevel = serde_json::from_str(json).unwrap();
        assert_eq!(level.price, "42000.0");
        assert_eq!(level.size, "1.5");
        assert_eq!(level.num_orders, 3);
    }

    #[test]
    fn test_candle_deserialize() {
        let json = r#"{
            "t": 1672515780000,
            "o": "42000.0",
            "h": "42100.0",
            "l": "41900.0",
            "c": "42050.0",
            "v": "100.5",
            "n": 150
        }"#;

        let candle: HyperliquidCandle = serde_json::from_str(json).unwrap();
        assert_eq!(candle.timestamp, 1672515780000);
        assert_eq!(candle.open, "42000.0");
        assert_eq!(candle.close, "42050.0");
    }

    #[test]
    fn test_order_request_serialize() {
        let order = HyperliquidOrderRequest {
            asset: 0,
            is_buy: true,
            price: "42000.0".to_string(),
            size: "0.1".to_string(),
            reduce_only: false,
            order_type: HyperliquidOrderTypeSpec {
                limit: HyperliquidLimitParams {
                    tif: "Gtc".to_string(),
                },
            },
        };
        let json = serde_json::to_string(&order).unwrap();
        assert!(json.contains("\"a\":0"));
        assert!(json.contains("\"b\":true"));
    }
}
