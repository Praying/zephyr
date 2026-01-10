//! WebSocket broadcaster for pushing updates to clients.
//!
//! This module provides functionality to broadcast various types of updates
//! to connected WebSocket clients based on their subscriptions.

use std::sync::Arc;
use tracing::{debug, instrument};

use super::connection::ConnectionRegistry;
use super::message::{
    LogEntryData, MarketDataUpdateData, OrderUpdateData, PnlUpdateData, PositionUpdateData,
    RiskAlertData, ServerMessage, StrategyUpdateData, WsChannel,
};

/// WebSocket broadcaster for pushing updates to clients.
#[derive(Debug, Clone)]
pub struct WsBroadcaster {
    registry: Arc<ConnectionRegistry>,
}

impl WsBroadcaster {
    /// Creates a new broadcaster.
    #[must_use]
    pub fn new(registry: Arc<ConnectionRegistry>) -> Self {
        Self { registry }
    }

    /// Broadcasts a position update to all subscribed clients.
    #[instrument(skip(self, data), fields(symbol = %data.symbol))]
    pub async fn broadcast_position(&self, data: PositionUpdateData) {
        debug!("Broadcasting position update");
        let message = ServerMessage::PositionUpdate { data };
        self.registry.broadcast(WsChannel::Positions, message).await;
    }

    /// Broadcasts a position update to a specific tenant.
    #[instrument(skip(self, data), fields(tenant_id = %tenant_id, symbol = %data.symbol))]
    pub async fn broadcast_position_to_tenant(&self, tenant_id: &str, data: PositionUpdateData) {
        debug!("Broadcasting position update to tenant");
        let message = ServerMessage::PositionUpdate { data };
        self.registry
            .broadcast_tenant(tenant_id, WsChannel::Positions, message)
            .await;
    }

    /// Broadcasts an order update to all subscribed clients.
    #[instrument(skip(self, data), fields(order_id = %data.order_id, symbol = %data.symbol))]
    pub async fn broadcast_order(&self, data: OrderUpdateData) {
        debug!("Broadcasting order update");
        let message = ServerMessage::OrderUpdate { data };
        self.registry.broadcast(WsChannel::Orders, message).await;
    }

    /// Broadcasts an order update to a specific tenant.
    #[instrument(skip(self, data), fields(tenant_id = %tenant_id, order_id = %data.order_id))]
    pub async fn broadcast_order_to_tenant(&self, tenant_id: &str, data: OrderUpdateData) {
        debug!("Broadcasting order update to tenant");
        let message = ServerMessage::OrderUpdate { data };
        self.registry
            .broadcast_tenant(tenant_id, WsChannel::Orders, message)
            .await;
    }

    /// Broadcasts a PnL update to all subscribed clients.
    #[instrument(skip(self, data))]
    pub async fn broadcast_pnl(&self, data: PnlUpdateData) {
        debug!("Broadcasting PnL update");
        let message = ServerMessage::PnlUpdate { data };
        self.registry.broadcast(WsChannel::Pnl, message).await;
    }

    /// Broadcasts a PnL update to a specific tenant.
    #[instrument(skip(self, data), fields(tenant_id = %tenant_id))]
    pub async fn broadcast_pnl_to_tenant(&self, tenant_id: &str, data: PnlUpdateData) {
        debug!("Broadcasting PnL update to tenant");
        let message = ServerMessage::PnlUpdate { data };
        self.registry
            .broadcast_tenant(tenant_id, WsChannel::Pnl, message)
            .await;
    }

    /// Broadcasts market data to clients subscribed to the symbol.
    #[instrument(skip(self, data), fields(symbol = %data.symbol))]
    pub async fn broadcast_market_data(&self, data: MarketDataUpdateData) {
        debug!("Broadcasting market data");
        let symbol = data.symbol.clone();
        let message = ServerMessage::MarketDataUpdate { data };
        self.registry
            .broadcast_symbol(WsChannel::MarketData, &symbol, message)
            .await;
    }

    /// Broadcasts a strategy status update to all subscribed clients.
    #[instrument(skip(self, data), fields(strategy_id = %data.strategy_id))]
    pub async fn broadcast_strategy(&self, data: StrategyUpdateData) {
        debug!("Broadcasting strategy update");
        let message = ServerMessage::StrategyUpdate { data };
        self.registry
            .broadcast(WsChannel::Strategies, message)
            .await;
    }

    /// Broadcasts a strategy update to a specific tenant.
    #[instrument(skip(self, data), fields(tenant_id = %tenant_id, strategy_id = %data.strategy_id))]
    pub async fn broadcast_strategy_to_tenant(&self, tenant_id: &str, data: StrategyUpdateData) {
        debug!("Broadcasting strategy update to tenant");
        let message = ServerMessage::StrategyUpdate { data };
        self.registry
            .broadcast_tenant(tenant_id, WsChannel::Strategies, message)
            .await;
    }

    /// Broadcasts a risk alert to all subscribed clients.
    #[instrument(skip(self, data), fields(alert_id = %data.alert_id, level = %data.level))]
    pub async fn broadcast_risk_alert(&self, data: RiskAlertData) {
        debug!("Broadcasting risk alert");
        let message = ServerMessage::RiskAlert { data };
        self.registry
            .broadcast(WsChannel::RiskAlerts, message)
            .await;
    }

    /// Broadcasts a risk alert to a specific tenant.
    #[instrument(skip(self, data), fields(tenant_id = %tenant_id, alert_id = %data.alert_id))]
    pub async fn broadcast_risk_alert_to_tenant(&self, tenant_id: &str, data: RiskAlertData) {
        debug!("Broadcasting risk alert to tenant");
        let message = ServerMessage::RiskAlert { data };
        self.registry
            .broadcast_tenant(tenant_id, WsChannel::RiskAlerts, message)
            .await;
    }

    /// Broadcasts a log entry to all subscribed clients.
    #[instrument(skip(self, data), fields(level = %data.level, target = %data.target))]
    pub async fn broadcast_log(&self, data: LogEntryData) {
        debug!("Broadcasting log entry");
        let message = ServerMessage::LogEntry { data };
        self.registry.broadcast(WsChannel::Logs, message).await;
    }

    /// Broadcasts a log entry to a specific tenant.
    pub async fn broadcast_log_to_tenant(&self, tenant_id: &str, data: LogEntryData) {
        let message = ServerMessage::LogEntry { data };
        self.registry
            .broadcast_tenant(tenant_id, WsChannel::Logs, message)
            .await;
    }

    /// Sends a heartbeat to all connected clients.
    pub async fn send_heartbeat(&self) {
        self.registry.send_heartbeat().await;
    }

    /// Returns the number of active connections.
    #[must_use]
    pub fn connection_count(&self) -> usize {
        self.registry.connection_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_broadcaster_new() {
        let registry = Arc::new(ConnectionRegistry::new());
        let broadcaster = WsBroadcaster::new(registry);
        assert_eq!(broadcaster.connection_count(), 0);
    }

    #[tokio::test]
    async fn test_broadcast_position_no_connections() {
        let registry = Arc::new(ConnectionRegistry::new());
        let broadcaster = WsBroadcaster::new(registry);

        let data = PositionUpdateData {
            symbol: "BTC-USDT".to_string(),
            side: "LONG".to_string(),
            quantity: "0.1".to_string(),
            entry_price: "50000".to_string(),
            mark_price: "51000".to_string(),
            unrealized_pnl: "100".to_string(),
            leverage: 10,
            update_time: chrono::Utc::now().timestamp_millis(),
        };

        // Should not panic even with no connections
        broadcaster.broadcast_position(data).await;
    }

    #[tokio::test]
    async fn test_broadcast_order_no_connections() {
        let registry = Arc::new(ConnectionRegistry::new());
        let broadcaster = WsBroadcaster::new(registry);

        let data = OrderUpdateData {
            order_id: "order-123".to_string(),
            client_order_id: None,
            symbol: "BTC-USDT".to_string(),
            side: "BUY".to_string(),
            order_type: "LIMIT".to_string(),
            status: "NEW".to_string(),
            price: Some("50000".to_string()),
            quantity: "0.1".to_string(),
            filled_quantity: "0".to_string(),
            avg_price: None,
            update_time: chrono::Utc::now().timestamp_millis(),
        };

        broadcaster.broadcast_order(data).await;
    }

    #[tokio::test]
    async fn test_broadcast_pnl_no_connections() {
        let registry = Arc::new(ConnectionRegistry::new());
        let broadcaster = WsBroadcaster::new(registry);

        let data = PnlUpdateData {
            unrealized_pnl: "100".to_string(),
            realized_pnl: "50".to_string(),
            total_equity: "10000".to_string(),
            available_balance: "8000".to_string(),
            margin_used: "2000".to_string(),
            update_time: chrono::Utc::now().timestamp_millis(),
        };

        broadcaster.broadcast_pnl(data).await;
    }

    #[tokio::test]
    async fn test_broadcast_market_data_no_connections() {
        let registry = Arc::new(ConnectionRegistry::new());
        let broadcaster = WsBroadcaster::new(registry);

        let data = MarketDataUpdateData {
            symbol: "BTC-USDT".to_string(),
            price: "50000".to_string(),
            volume: "100".to_string(),
            bid_price: Some("49999".to_string()),
            ask_price: Some("50001".to_string()),
            mark_price: None,
            funding_rate: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        broadcaster.broadcast_market_data(data).await;
    }

    #[tokio::test]
    async fn test_send_heartbeat_no_connections() {
        let registry = Arc::new(ConnectionRegistry::new());
        let broadcaster = WsBroadcaster::new(registry);

        broadcaster.send_heartbeat().await;
    }
}
