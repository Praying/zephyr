//! WebSocket message types.
//!
//! This module defines the message types for WebSocket communication including:
//! - Client messages (subscribe, unsubscribe, ping)
//! - Server messages (data updates, pong, errors)
//! - Channel definitions

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// WebSocket channel types for subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WsChannel {
    /// Position updates
    Positions,
    /// Order updates
    Orders,
    /// PnL updates
    Pnl,
    /// Market data (tick data)
    MarketData,
    /// Strategy status updates
    Strategies,
    /// Risk alerts
    RiskAlerts,
    /// Log streaming
    Logs,
}

impl WsChannel {
    /// Returns all available channels.
    #[must_use]
    pub fn all() -> HashSet<Self> {
        let mut set = HashSet::new();
        set.insert(Self::Positions);
        set.insert(Self::Orders);
        set.insert(Self::Pnl);
        set.insert(Self::MarketData);
        set.insert(Self::Strategies);
        set.insert(Self::RiskAlerts);
        set.insert(Self::Logs);
        set
    }
}

/// Client-to-server message types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Subscribe to channels
    Subscribe {
        /// Channels to subscribe to
        channels: Vec<WsChannel>,
        /// Optional symbols to filter (for market data)
        #[serde(default)]
        symbols: Vec<String>,
    },
    /// Unsubscribe from channels
    Unsubscribe {
        /// Channels to unsubscribe from
        channels: Vec<WsChannel>,
        /// Optional symbols to unfilter
        #[serde(default)]
        symbols: Vec<String>,
    },
    /// Ping message (client heartbeat)
    Ping {
        /// Optional timestamp for latency measurement
        #[serde(skip_serializing_if = "Option::is_none")]
        timestamp: Option<i64>,
    },
    /// Authentication message
    Auth {
        /// JWT token
        token: String,
    },
}

/// Server-to-client message types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Authentication result
    AuthResult {
        /// Whether authentication succeeded
        success: bool,
        /// Error message if failed
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        /// User ID if successful
        #[serde(skip_serializing_if = "Option::is_none")]
        user_id: Option<String>,
    },
    /// Subscription confirmation
    Subscribed {
        /// Channels successfully subscribed
        channels: Vec<WsChannel>,
    },
    /// Unsubscription confirmation
    Unsubscribed {
        /// Channels successfully unsubscribed
        channels: Vec<WsChannel>,
    },
    /// Pong response (server heartbeat)
    Pong {
        /// Echo back client timestamp
        #[serde(skip_serializing_if = "Option::is_none")]
        timestamp: Option<i64>,
        /// Server timestamp
        server_time: i64,
    },
    /// Position update
    PositionUpdate {
        /// Position data
        data: PositionUpdateData,
    },
    /// Order update
    OrderUpdate {
        /// Order data
        data: OrderUpdateData,
    },
    /// PnL update
    PnlUpdate {
        /// PnL data
        data: PnlUpdateData,
    },
    /// Market data update
    MarketDataUpdate {
        /// Market data
        data: MarketDataUpdateData,
    },
    /// Strategy status update
    StrategyUpdate {
        /// Strategy data
        data: StrategyUpdateData,
    },
    /// Risk alert
    RiskAlert {
        /// Alert data
        data: RiskAlertData,
    },
    /// Log entry
    LogEntry {
        /// Log data
        data: LogEntryData,
    },
    /// Error message
    Error {
        /// Error code
        code: String,
        /// Error message
        message: String,
    },
    /// Server heartbeat (sent periodically)
    Heartbeat {
        /// Server timestamp
        server_time: i64,
    },
}

/// Position update data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionUpdateData {
    /// Symbol
    pub symbol: String,
    /// Position side
    pub side: String,
    /// Quantity
    pub quantity: String,
    /// Entry price
    pub entry_price: String,
    /// Mark price
    pub mark_price: String,
    /// Unrealized PnL
    pub unrealized_pnl: String,
    /// Leverage
    pub leverage: u8,
    /// Update timestamp
    pub update_time: i64,
}

/// Order update data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderUpdateData {
    /// Order ID
    pub order_id: String,
    /// Client order ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
    /// Symbol
    pub symbol: String,
    /// Order side
    pub side: String,
    /// Order type
    pub order_type: String,
    /// Order status
    pub status: String,
    /// Price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<String>,
    /// Quantity
    pub quantity: String,
    /// Filled quantity
    pub filled_quantity: String,
    /// Average fill price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_price: Option<String>,
    /// Update timestamp
    pub update_time: i64,
}

/// PnL update data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnlUpdateData {
    /// Total unrealized PnL
    pub unrealized_pnl: String,
    /// Total realized PnL
    pub realized_pnl: String,
    /// Total equity
    pub total_equity: String,
    /// Available balance
    pub available_balance: String,
    /// Margin used
    pub margin_used: String,
    /// Update timestamp
    pub update_time: i64,
}

/// Market data update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataUpdateData {
    /// Symbol
    pub symbol: String,
    /// Last price
    pub price: String,
    /// Volume
    pub volume: String,
    /// Best bid price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid_price: Option<String>,
    /// Best ask price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask_price: Option<String>,
    /// Mark price (for perpetuals)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mark_price: Option<String>,
    /// Funding rate (for perpetuals)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funding_rate: Option<String>,
    /// Update timestamp
    pub timestamp: i64,
}

/// Strategy status update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyUpdateData {
    /// Strategy ID
    pub strategy_id: String,
    /// Strategy name
    pub name: String,
    /// Strategy status
    pub status: String,
    /// Associated symbols
    pub symbols: Vec<String>,
    /// Update timestamp
    pub update_time: i64,
}

/// Risk alert data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAlertData {
    /// Alert ID
    pub alert_id: String,
    /// Alert level (info, warning, critical)
    pub level: String,
    /// Alert type
    pub alert_type: String,
    /// Alert message
    pub message: String,
    /// Related symbol (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// Alert timestamp
    pub timestamp: i64,
}

/// Log entry data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntryData {
    /// Log level
    pub level: String,
    /// Log target/component
    pub target: String,
    /// Log message
    pub message: String,
    /// Span ID (for tracing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,
    /// Log timestamp
    pub timestamp: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_message_subscribe_serde() {
        let msg = ClientMessage::Subscribe {
            channels: vec![WsChannel::Positions, WsChannel::Orders],
            symbols: vec!["BTC-USDT".to_string()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();

        if let ClientMessage::Subscribe { channels, symbols } = parsed {
            assert_eq!(channels.len(), 2);
            assert_eq!(symbols.len(), 1);
        } else {
            panic!("Wrong message type");
        }
    }

    #[test]
    fn test_client_message_ping_serde() {
        let msg = ClientMessage::Ping {
            timestamp: Some(1_234_567_890),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("ping"));

        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        if let ClientMessage::Ping { timestamp } = parsed {
            assert_eq!(timestamp, Some(1_234_567_890));
        } else {
            panic!("Wrong message type");
        }
    }

    #[test]
    fn test_server_message_pong_serde() {
        let msg = ServerMessage::Pong {
            timestamp: Some(1_234_567_890),
            server_time: 1_234_567_891,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ServerMessage = serde_json::from_str(&json).unwrap();

        if let ServerMessage::Pong {
            timestamp,
            server_time,
        } = parsed
        {
            assert_eq!(timestamp, Some(1_234_567_890));
            assert_eq!(server_time, 1_234_567_891);
        } else {
            panic!("Wrong message type");
        }
    }

    #[test]
    fn test_ws_channel_all() {
        let all = WsChannel::all();
        assert_eq!(all.len(), 7);
        assert!(all.contains(&WsChannel::Positions));
        assert!(all.contains(&WsChannel::Orders));
        assert!(all.contains(&WsChannel::Pnl));
    }

    #[test]
    fn test_position_update_serde() {
        let data = PositionUpdateData {
            symbol: "BTC-USDT".to_string(),
            side: "LONG".to_string(),
            quantity: "0.1".to_string(),
            entry_price: "50000".to_string(),
            mark_price: "51000".to_string(),
            unrealized_pnl: "100".to_string(),
            leverage: 10,
            update_time: 1_234_567_890,
        };
        let msg = ServerMessage::PositionUpdate { data };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("position_update"));
        assert!(json.contains("BTC-USDT"));
    }
}
