//! State synchronization across distributed nodes.
//!
//! This module provides state synchronization for positions and orders
//! across cluster nodes to ensure consistency.

#![allow(unused_imports)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::collapsible_if)]

use std::collections::HashMap;
use std::time::Duration;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info};

use zephyr_core::data::{Order, OrderStatus};
use zephyr_core::types::{OrderId, Price, Quantity, Symbol, Timestamp};

/// Error type for state synchronization.
#[derive(Debug, Clone, Error)]
pub enum StateSyncError {
    /// Version conflict during sync.
    #[error("version conflict: local={local}, remote={remote}")]
    VersionConflict {
        /// Local version.
        local: u64,
        /// Remote version.
        remote: u64,
    },

    /// State not found.
    #[error("state not found for key: {0}")]
    NotFound(String),

    /// Network error during sync.
    #[error("network error: {0}")]
    NetworkError(String),

    /// Invalid state data.
    #[error("invalid state data: {0}")]
    InvalidData(String),

    /// Sync timeout.
    #[error("sync timeout after {0:?}")]
    Timeout(Duration),
}

/// Configuration for state synchronization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSyncConfig {
    /// Interval between sync operations.
    #[serde(with = "humantime_serde")]
    pub sync_interval: Duration,
    /// Maximum age of state before forced sync.
    #[serde(with = "humantime_serde")]
    pub max_state_age: Duration,
    /// Enable incremental sync.
    #[serde(default = "default_incremental")]
    pub incremental_sync: bool,
    /// Batch size for sync operations.
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    /// Conflict resolution strategy.
    #[serde(default)]
    pub conflict_resolution: ConflictResolution,
}

fn default_incremental() -> bool {
    true
}

fn default_batch_size() -> usize {
    100
}

impl Default for StateSyncConfig {
    fn default() -> Self {
        Self {
            sync_interval: Duration::from_secs(1),
            max_state_age: Duration::from_secs(30),
            incremental_sync: default_incremental(),
            batch_size: default_batch_size(),
            conflict_resolution: ConflictResolution::default(),
        }
    }
}

/// Conflict resolution strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    /// Use the latest timestamp.
    #[default]
    LatestWins,
    /// Use the highest version.
    HighestVersion,
    /// Prefer leader's state.
    LeaderWins,
    /// Merge states (for compatible types).
    Merge,
}

/// Position state for synchronization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionState {
    /// Symbol.
    pub symbol: Symbol,
    /// Current quantity.
    pub quantity: Quantity,
    /// Average entry price.
    pub entry_price: Price,
    /// Last update timestamp.
    pub updated_at: Timestamp,
    /// Version number.
    pub version: u64,
    /// Source node ID.
    pub source_node: String,
}

impl PositionState {
    /// Creates a new position state.
    #[must_use]
    pub fn new(
        symbol: Symbol,
        quantity: Quantity,
        entry_price: Price,
        source_node: impl Into<String>,
    ) -> Self {
        Self {
            symbol,
            quantity,
            entry_price,
            updated_at: Timestamp::now(),
            version: 1,
            source_node: source_node.into(),
        }
    }

    /// Updates the position.
    pub fn update(&mut self, quantity: Quantity, entry_price: Price) {
        self.quantity = quantity;
        self.entry_price = entry_price;
        self.updated_at = Timestamp::now();
        self.version += 1;
    }

    /// Returns true if this state is newer than another.
    #[must_use]
    pub fn is_newer_than(&self, other: &Self) -> bool {
        self.version > other.version
            || (self.version == other.version && self.updated_at > other.updated_at)
    }
}

/// Order state for synchronization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderState {
    /// Order ID.
    pub order_id: OrderId,
    /// Symbol.
    pub symbol: Symbol,
    /// Order status.
    pub status: OrderStatus,
    /// Filled quantity.
    pub filled_quantity: Quantity,
    /// Average fill price.
    pub avg_price: Option<Price>,
    /// Last update timestamp.
    pub updated_at: Timestamp,
    /// Version number.
    pub version: u64,
    /// Source node ID.
    pub source_node: String,
}

impl OrderState {
    /// Creates a new order state from an order.
    #[must_use]
    pub fn from_order(order: &Order, source_node: impl Into<String>) -> Self {
        Self {
            order_id: order.order_id.clone(),
            symbol: order.symbol.clone(),
            status: order.status,
            filled_quantity: order.filled_quantity,
            avg_price: order.avg_price,
            updated_at: Timestamp::now(),
            version: 1,
            source_node: source_node.into(),
        }
    }

    /// Updates from an order.
    pub fn update_from_order(&mut self, order: &Order) {
        self.status = order.status;
        self.filled_quantity = order.filled_quantity;
        self.avg_price = order.avg_price;
        self.updated_at = Timestamp::now();
        self.version += 1;
    }

    /// Returns true if this state is newer than another.
    #[must_use]
    pub fn is_newer_than(&self, other: &Self) -> bool {
        self.version > other.version
            || (self.version == other.version && self.updated_at > other.updated_at)
    }

    /// Returns true if the order is in a terminal state.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Filled
                | OrderStatus::Canceled
                | OrderStatus::Rejected
                | OrderStatus::Expired
        )
    }
}

/// Sync message for state transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncMessage {
    /// Full state snapshot.
    FullSnapshot {
        /// Position states.
        positions: Vec<PositionState>,
        /// Order states.
        orders: Vec<OrderState>,
        /// Snapshot timestamp.
        timestamp: Timestamp,
    },
    /// Incremental position update.
    PositionUpdate(PositionState),
    /// Incremental order update.
    OrderUpdate(OrderState),
    /// Request for full sync.
    SyncRequest {
        /// Version to sync from.
        from_version: u64,
    },
    /// Acknowledgment.
    Ack {
        /// Message ID being acknowledged.
        message_id: u64,
    },
}

/// State synchronization manager.
pub struct StateSync {
    config: StateSyncConfig,
    node_id: String,
    positions: RwLock<HashMap<Symbol, PositionState>>,
    orders: RwLock<HashMap<OrderId, OrderState>>,
    global_version: RwLock<u64>,
    last_sync: RwLock<Timestamp>,
}

impl StateSync {
    /// Creates a new state sync manager.
    #[must_use]
    pub fn new(config: StateSyncConfig, node_id: impl Into<String>) -> Self {
        Self {
            config,
            node_id: node_id.into(),
            positions: RwLock::new(HashMap::new()),
            orders: RwLock::new(HashMap::new()),
            global_version: RwLock::new(0),
            last_sync: RwLock::new(Timestamp::now()),
        }
    }

    /// Returns the node ID.
    #[must_use]
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Returns the current global version.
    #[must_use]
    pub fn global_version(&self) -> u64 {
        *self.global_version.read()
    }

    /// Updates a position locally.
    pub fn update_position(&self, symbol: Symbol, quantity: Quantity, entry_price: Price) {
        let mut positions = self.positions.write();
        let mut version = self.global_version.write();
        *version += 1;

        if let Some(state) = positions.get_mut(&symbol) {
            state.update(quantity, entry_price);
        } else {
            positions.insert(
                symbol.clone(),
                PositionState::new(symbol, quantity, entry_price, &self.node_id),
            );
        }
    }

    /// Updates an order locally.
    pub fn update_order(&self, order: &Order) {
        let mut orders = self.orders.write();
        let mut version = self.global_version.write();
        *version += 1;

        if let Some(state) = orders.get_mut(&order.order_id) {
            state.update_from_order(order);
        } else {
            orders.insert(
                order.order_id.clone(),
                OrderState::from_order(order, &self.node_id),
            );
        }
    }

    /// Gets a position state.
    #[must_use]
    pub fn get_position(&self, symbol: &Symbol) -> Option<PositionState> {
        self.positions.read().get(symbol).cloned()
    }

    /// Gets an order state.
    #[must_use]
    pub fn get_order(&self, order_id: &OrderId) -> Option<OrderState> {
        self.orders.read().get(order_id).cloned()
    }

    /// Gets all positions.
    #[must_use]
    pub fn all_positions(&self) -> Vec<PositionState> {
        self.positions.read().values().cloned().collect()
    }

    /// Gets all active orders.
    #[must_use]
    pub fn active_orders(&self) -> Vec<OrderState> {
        self.orders
            .read()
            .values()
            .filter(|o| !o.is_terminal())
            .cloned()
            .collect()
    }

    /// Creates a full snapshot for sync.
    #[must_use]
    pub fn create_snapshot(&self) -> SyncMessage {
        SyncMessage::FullSnapshot {
            positions: self.all_positions(),
            orders: self.orders.read().values().cloned().collect(),
            timestamp: Timestamp::now(),
        }
    }

    /// Applies a sync message from another node.
    ///
    /// # Errors
    ///
    /// Returns an error if there's a conflict that can't be resolved.
    pub fn apply_sync(&self, message: SyncMessage) -> Result<(), StateSyncError> {
        match message {
            SyncMessage::FullSnapshot {
                positions,
                orders,
                timestamp,
            } => self.apply_full_snapshot(positions, orders, timestamp),
            SyncMessage::PositionUpdate(state) => self.apply_position_update(state),
            SyncMessage::OrderUpdate(state) => self.apply_order_update(state),
            SyncMessage::SyncRequest { .. } | SyncMessage::Ack { .. } => Ok(()),
        }
    }

    fn apply_full_snapshot(
        &self,
        positions: Vec<PositionState>,
        orders: Vec<OrderState>,
        _timestamp: Timestamp,
    ) -> Result<(), StateSyncError> {
        let mut local_positions = self.positions.write();
        let mut local_orders = self.orders.write();

        for remote_pos in positions {
            self.merge_position(&mut local_positions, remote_pos);
        }

        for remote_order in orders {
            self.merge_order(&mut local_orders, remote_order);
        }

        *self.last_sync.write() = Timestamp::now();

        info!(
            node_id = %self.node_id,
            positions = local_positions.len(),
            orders = local_orders.len(),
            "Applied full snapshot"
        );

        Ok(())
    }

    fn apply_position_update(&self, state: PositionState) -> Result<(), StateSyncError> {
        let mut positions = self.positions.write();
        self.merge_position(&mut positions, state);
        Ok(())
    }

    fn apply_order_update(&self, state: OrderState) -> Result<(), StateSyncError> {
        let mut orders = self.orders.write();
        self.merge_order(&mut orders, state);
        Ok(())
    }

    fn merge_position(
        &self,
        positions: &mut HashMap<Symbol, PositionState>,
        remote: PositionState,
    ) {
        let should_update = match positions.get(&remote.symbol) {
            None => true,
            Some(local) => self.should_use_remote_position(local, &remote),
        };

        if should_update {
            debug!(
                symbol = %remote.symbol,
                version = remote.version,
                "Merged remote position"
            );
            positions.insert(remote.symbol.clone(), remote);
        }
    }

    fn merge_order(&self, orders: &mut HashMap<OrderId, OrderState>, remote: OrderState) {
        let should_update = match orders.get(&remote.order_id) {
            None => true,
            Some(local) => self.should_use_remote_order(local, &remote),
        };

        if should_update {
            debug!(
                order_id = %remote.order_id,
                version = remote.version,
                "Merged remote order"
            );
            orders.insert(remote.order_id.clone(), remote);
        }
    }

    fn should_use_remote_position(&self, local: &PositionState, remote: &PositionState) -> bool {
        match self.config.conflict_resolution {
            ConflictResolution::LatestWins => remote.updated_at > local.updated_at,
            ConflictResolution::HighestVersion => remote.version > local.version,
            ConflictResolution::LeaderWins => {
                // In leader wins mode, we'd need to know who the leader is
                // For now, fall back to latest wins
                remote.updated_at > local.updated_at
            }
            ConflictResolution::Merge => {
                // For positions, we can't really merge, so use latest
                remote.updated_at > local.updated_at
            }
        }
    }

    fn should_use_remote_order(&self, local: &OrderState, remote: &OrderState) -> bool {
        // For orders, terminal states always win
        if remote.is_terminal() && !local.is_terminal() {
            return true;
        }
        if local.is_terminal() && !remote.is_terminal() {
            return false;
        }

        match self.config.conflict_resolution {
            ConflictResolution::LatestWins => remote.updated_at > local.updated_at,
            ConflictResolution::HighestVersion => remote.version > local.version,
            ConflictResolution::LeaderWins => remote.updated_at > local.updated_at,
            ConflictResolution::Merge => remote.updated_at > local.updated_at,
        }
    }

    /// Checks if sync is needed based on time.
    #[must_use]
    pub fn needs_sync(&self) -> bool {
        let last = *self.last_sync.read();
        let elapsed = Timestamp::now().as_millis() - last.as_millis();
        elapsed > self.config.sync_interval.as_millis() as i64
    }

    /// Cleans up old terminal orders.
    pub fn cleanup_old_orders(&self, max_age: Duration) {
        let mut orders = self.orders.write();
        let cutoff = Timestamp::now().as_millis() - max_age.as_millis() as i64;

        orders.retain(|_, o| !o.is_terminal() || o.updated_at.as_millis() > cutoff);
    }

    /// Gets changes since a version.
    #[must_use]
    pub fn changes_since(&self, version: u64) -> Vec<SyncMessage> {
        let mut messages = Vec::new();

        for pos in self.positions.read().values() {
            if pos.version > version {
                messages.push(SyncMessage::PositionUpdate(pos.clone()));
            }
        }

        for order in self.orders.read().values() {
            if order.version > version {
                messages.push(SyncMessage::OrderUpdate(order.clone()));
            }
        }

        messages
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use zephyr_core::data::{OrderSide, TimeInForce};

    fn create_test_order() -> Order {
        Order {
            order_id: OrderId::new("order-1").unwrap(),
            client_order_id: None,
            symbol: Symbol::new("BTC-USDT").unwrap(),
            side: OrderSide::Buy,
            order_type: zephyr_core::data::OrderType::Limit,
            status: OrderStatus::New,
            price: Some(Price::new(dec!(50000)).unwrap()),
            stop_price: None,
            quantity: Quantity::new(dec!(1.0)).unwrap(),
            filled_quantity: Quantity::ZERO,
            avg_price: None,
            time_in_force: TimeInForce::Gtc,
            reduce_only: false,
            post_only: false,
            create_time: Timestamp::now(),
            update_time: Timestamp::now(),
        }
    }

    #[test]
    fn test_position_state_new() {
        let state = PositionState::new(
            Symbol::new("BTC-USDT").unwrap(),
            Quantity::new(dec!(1.0)).unwrap(),
            Price::new(dec!(50000)).unwrap(),
            "node-1",
        );

        assert_eq!(state.version, 1);
        assert_eq!(state.source_node, "node-1");
    }

    #[test]
    fn test_position_state_update() {
        let mut state = PositionState::new(
            Symbol::new("BTC-USDT").unwrap(),
            Quantity::new(dec!(1.0)).unwrap(),
            Price::new(dec!(50000)).unwrap(),
            "node-1",
        );

        state.update(
            Quantity::new(dec!(2.0)).unwrap(),
            Price::new(dec!(51000)).unwrap(),
        );

        assert_eq!(state.version, 2);
        assert_eq!(state.quantity, Quantity::new(dec!(2.0)).unwrap());
    }

    #[test]
    fn test_order_state_from_order() {
        let order = create_test_order();
        let state = OrderState::from_order(&order, "node-1");

        assert_eq!(state.order_id, order.order_id);
        assert_eq!(state.status, OrderStatus::New);
        assert!(!state.is_terminal());
    }

    #[test]
    fn test_order_state_terminal() {
        let mut order = create_test_order();
        order.status = OrderStatus::Filled;
        let state = OrderState::from_order(&order, "node-1");

        assert!(state.is_terminal());
    }

    #[test]
    fn test_state_sync_update_position() {
        let sync = StateSync::new(StateSyncConfig::default(), "node-1");

        sync.update_position(
            Symbol::new("BTC-USDT").unwrap(),
            Quantity::new(dec!(1.0)).unwrap(),
            Price::new(dec!(50000)).unwrap(),
        );

        let pos = sync.get_position(&Symbol::new("BTC-USDT").unwrap());
        assert!(pos.is_some());
        assert_eq!(pos.unwrap().quantity, Quantity::new(dec!(1.0)).unwrap());
    }

    #[test]
    fn test_state_sync_update_order() {
        let sync = StateSync::new(StateSyncConfig::default(), "node-1");
        let order = create_test_order();

        sync.update_order(&order);

        let state = sync.get_order(&order.order_id);
        assert!(state.is_some());
        assert_eq!(state.unwrap().status, OrderStatus::New);
    }

    #[test]
    fn test_state_sync_create_snapshot() {
        let sync = StateSync::new(StateSyncConfig::default(), "node-1");

        sync.update_position(
            Symbol::new("BTC-USDT").unwrap(),
            Quantity::new(dec!(1.0)).unwrap(),
            Price::new(dec!(50000)).unwrap(),
        );

        let snapshot = sync.create_snapshot();
        match snapshot {
            SyncMessage::FullSnapshot { positions, .. } => {
                assert_eq!(positions.len(), 1);
            }
            _ => panic!("Expected FullSnapshot"),
        }
    }

    #[test]
    fn test_state_sync_apply_position_update() {
        let sync = StateSync::new(StateSyncConfig::default(), "node-1");

        let remote_state = PositionState::new(
            Symbol::new("ETH-USDT").unwrap(),
            Quantity::new(dec!(10.0)).unwrap(),
            Price::new(dec!(3000)).unwrap(),
            "node-2",
        );

        sync.apply_sync(SyncMessage::PositionUpdate(remote_state))
            .unwrap();

        let pos = sync.get_position(&Symbol::new("ETH-USDT").unwrap());
        assert!(pos.is_some());
        assert_eq!(pos.unwrap().source_node, "node-2");
    }

    #[test]
    fn test_state_sync_conflict_resolution() {
        let sync = StateSync::new(StateSyncConfig::default(), "node-1");

        // Add local position
        sync.update_position(
            Symbol::new("BTC-USDT").unwrap(),
            Quantity::new(dec!(1.0)).unwrap(),
            Price::new(dec!(50000)).unwrap(),
        );

        // Create older remote state
        let mut remote_state = PositionState::new(
            Symbol::new("BTC-USDT").unwrap(),
            Quantity::new(dec!(2.0)).unwrap(),
            Price::new(dec!(51000)).unwrap(),
            "node-2",
        );
        remote_state.updated_at = Timestamp::new_unchecked(0); // Very old

        sync.apply_sync(SyncMessage::PositionUpdate(remote_state))
            .unwrap();

        // Local should win (newer)
        let pos = sync
            .get_position(&Symbol::new("BTC-USDT").unwrap())
            .unwrap();
        assert_eq!(pos.quantity, Quantity::new(dec!(1.0)).unwrap());
    }

    #[test]
    fn test_state_sync_all_positions() {
        let sync = StateSync::new(StateSyncConfig::default(), "node-1");

        sync.update_position(
            Symbol::new("BTC-USDT").unwrap(),
            Quantity::new(dec!(1.0)).unwrap(),
            Price::new(dec!(50000)).unwrap(),
        );
        sync.update_position(
            Symbol::new("ETH-USDT").unwrap(),
            Quantity::new(dec!(10.0)).unwrap(),
            Price::new(dec!(3000)).unwrap(),
        );

        let positions = sync.all_positions();
        assert_eq!(positions.len(), 2);
    }

    #[test]
    fn test_state_sync_active_orders() {
        let sync = StateSync::new(StateSyncConfig::default(), "node-1");

        let mut order1 = create_test_order();
        order1.order_id = OrderId::new("order-1").unwrap();
        sync.update_order(&order1);

        let mut order2 = create_test_order();
        order2.order_id = OrderId::new("order-2").unwrap();
        order2.status = OrderStatus::Filled;
        sync.update_order(&order2);

        let active = sync.active_orders();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].order_id.as_str(), "order-1");
    }

    #[test]
    fn test_state_sync_changes_since() {
        let sync = StateSync::new(StateSyncConfig::default(), "node-1");

        sync.update_position(
            Symbol::new("BTC-USDT").unwrap(),
            Quantity::new(dec!(1.0)).unwrap(),
            Price::new(dec!(50000)).unwrap(),
        );

        let changes = sync.changes_since(0);
        assert_eq!(changes.len(), 1);

        let changes = sync.changes_since(100);
        assert_eq!(changes.len(), 0);
    }

    #[test]
    fn test_conflict_resolution_default() {
        assert_eq!(
            ConflictResolution::default(),
            ConflictResolution::LatestWins
        );
    }
}
