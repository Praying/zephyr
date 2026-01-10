//! WebSocket connection management.
//!
//! This module provides connection state tracking and management including:
//! - Connection state (authenticated, subscriptions)
//! - Connection registry for broadcasting
//! - Heartbeat tracking

use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tokio::sync::mpsc;

use super::message::{ServerMessage, WsChannel};
use crate::auth::Claims;

/// Unique connection identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionId(u64);

impl ConnectionId {
    /// Generates a new unique connection ID.
    pub fn generate() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Returns the inner ID value.
    #[must_use]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "conn-{}", self.0)
    }
}

/// State of a single WebSocket connection.
#[derive(Debug)]
pub struct ConnectionState {
    /// Connection ID
    pub id: ConnectionId,
    /// User claims (if authenticated)
    pub claims: Option<Claims>,
    /// Subscribed channels
    pub subscriptions: HashSet<WsChannel>,
    /// Subscribed symbols (for market data filtering)
    pub symbols: HashSet<String>,
    /// Last heartbeat received
    pub last_heartbeat: Instant,
    /// Message sender channel
    pub sender: mpsc::Sender<ServerMessage>,
}

impl ConnectionState {
    /// Creates a new connection state.
    pub fn new(id: ConnectionId, sender: mpsc::Sender<ServerMessage>) -> Self {
        Self {
            id,
            claims: None,
            subscriptions: HashSet::new(),
            symbols: HashSet::new(),
            last_heartbeat: Instant::now(),
            sender,
        }
    }

    /// Returns true if the connection is authenticated.
    #[must_use]
    pub fn is_authenticated(&self) -> bool {
        self.claims.is_some()
    }

    /// Returns the user ID if authenticated.
    #[must_use]
    pub fn user_id(&self) -> Option<&str> {
        self.claims.as_ref().map(|c| c.sub.as_str())
    }

    /// Returns the tenant ID if available.
    #[must_use]
    pub fn tenant_id(&self) -> Option<&str> {
        self.claims.as_ref().and_then(|c| c.tenant_id.as_deref())
    }

    /// Sets the authentication claims.
    pub fn set_claims(&mut self, claims: Claims) {
        self.claims = Some(claims);
    }

    /// Subscribes to channels.
    pub fn subscribe(&mut self, channels: &[WsChannel], symbols: &[String]) {
        for channel in channels {
            self.subscriptions.insert(*channel);
        }
        for symbol in symbols {
            self.symbols.insert(symbol.clone());
        }
    }

    /// Unsubscribes from channels.
    pub fn unsubscribe(&mut self, channels: &[WsChannel], symbols: &[String]) {
        for channel in channels {
            self.subscriptions.remove(channel);
        }
        for symbol in symbols {
            self.symbols.remove(symbol);
        }
    }

    /// Returns true if subscribed to the given channel.
    #[must_use]
    pub fn is_subscribed(&self, channel: WsChannel) -> bool {
        self.subscriptions.contains(&channel)
    }

    /// Returns true if subscribed to the given symbol (or no symbol filter).
    #[must_use]
    pub fn is_symbol_subscribed(&self, symbol: &str) -> bool {
        self.symbols.is_empty() || self.symbols.contains(symbol)
    }

    /// Updates the last heartbeat time.
    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = Instant::now();
    }

    /// Returns the time since last heartbeat.
    #[must_use]
    pub fn time_since_heartbeat(&self) -> std::time::Duration {
        self.last_heartbeat.elapsed()
    }
}

/// Registry of all active WebSocket connections.
#[derive(Debug, Default)]
pub struct ConnectionRegistry {
    /// Active connections by ID
    connections: DashMap<ConnectionId, Arc<parking_lot::RwLock<ConnectionState>>>,
}

impl ConnectionRegistry {
    /// Creates a new connection registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            connections: DashMap::new(),
        }
    }

    /// Registers a new connection.
    pub fn register(&self, state: ConnectionState) -> Arc<parking_lot::RwLock<ConnectionState>> {
        let id = state.id;
        let state = Arc::new(parking_lot::RwLock::new(state));
        self.connections.insert(id, state.clone());
        state
    }

    /// Unregisters a connection.
    pub fn unregister(&self, id: ConnectionId) {
        self.connections.remove(&id);
    }

    /// Returns the number of active connections.
    #[must_use]
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Broadcasts a message to all connections subscribed to a channel.
    pub async fn broadcast(&self, channel: WsChannel, message: ServerMessage) {
        for entry in self.connections.iter() {
            let sender = {
                let state = entry.value().read();
                if state.is_authenticated() && state.is_subscribed(channel) {
                    Some(state.sender.clone())
                } else {
                    None
                }
            };
            if let Some(sender) = sender {
                let _ = sender.send(message.clone()).await;
            }
        }
    }

    /// Broadcasts a message to connections subscribed to a specific symbol.
    pub async fn broadcast_symbol(&self, channel: WsChannel, symbol: &str, message: ServerMessage) {
        for entry in self.connections.iter() {
            let sender = {
                let state = entry.value().read();
                if state.is_authenticated()
                    && state.is_subscribed(channel)
                    && state.is_symbol_subscribed(symbol)
                {
                    Some(state.sender.clone())
                } else {
                    None
                }
            };
            if let Some(sender) = sender {
                let _ = sender.send(message.clone()).await;
            }
        }
    }

    /// Broadcasts a message to a specific tenant's connections.
    pub async fn broadcast_tenant(
        &self,
        tenant_id: &str,
        channel: WsChannel,
        message: ServerMessage,
    ) {
        for entry in self.connections.iter() {
            let sender = {
                let state = entry.value().read();
                if state.is_authenticated()
                    && state.tenant_id() == Some(tenant_id)
                    && state.is_subscribed(channel)
                {
                    Some(state.sender.clone())
                } else {
                    None
                }
            };
            if let Some(sender) = sender {
                let _ = sender.send(message.clone()).await;
            }
        }
    }

    /// Returns connections that have timed out.
    pub fn get_timed_out_connections(&self, timeout: std::time::Duration) -> Vec<ConnectionId> {
        self.connections
            .iter()
            .filter_map(|entry| {
                let state = entry.value().read();
                if state.time_since_heartbeat() > timeout {
                    Some(state.id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Sends a heartbeat to all connections.
    pub async fn send_heartbeat(&self) {
        let server_time = chrono::Utc::now().timestamp_millis();
        let message = ServerMessage::Heartbeat { server_time };

        for entry in self.connections.iter() {
            let sender = entry.value().read().sender.clone();
            let _ = sender.send(message.clone()).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_id_generate() {
        let id1 = ConnectionId::generate();
        let id2 = ConnectionId::generate();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_connection_id_display() {
        let id = ConnectionId(42);
        assert_eq!(format!("{}", id), "conn-42");
    }

    #[tokio::test]
    async fn test_connection_state_new() {
        let (tx, _rx) = mpsc::channel(10);
        let id = ConnectionId::generate();
        let state = ConnectionState::new(id, tx);

        assert!(!state.is_authenticated());
        assert!(state.subscriptions.is_empty());
        assert!(state.symbols.is_empty());
    }

    #[tokio::test]
    async fn test_connection_state_subscribe() {
        let (tx, _rx) = mpsc::channel(10);
        let id = ConnectionId::generate();
        let mut state = ConnectionState::new(id, tx);

        state.subscribe(
            &[WsChannel::Positions, WsChannel::Orders],
            &["BTC-USDT".to_string()],
        );

        assert!(state.is_subscribed(WsChannel::Positions));
        assert!(state.is_subscribed(WsChannel::Orders));
        assert!(!state.is_subscribed(WsChannel::Pnl));
        assert!(state.is_symbol_subscribed("BTC-USDT"));
        assert!(!state.is_symbol_subscribed("ETH-USDT"));
    }

    #[tokio::test]
    async fn test_connection_state_unsubscribe() {
        let (tx, _rx) = mpsc::channel(10);
        let id = ConnectionId::generate();
        let mut state = ConnectionState::new(id, tx);

        state.subscribe(&[WsChannel::Positions, WsChannel::Orders], &[]);
        state.unsubscribe(&[WsChannel::Positions], &[]);

        assert!(!state.is_subscribed(WsChannel::Positions));
        assert!(state.is_subscribed(WsChannel::Orders));
    }

    #[tokio::test]
    async fn test_connection_registry_register() {
        let registry = ConnectionRegistry::new();
        let (tx, _rx) = mpsc::channel(10);
        let id = ConnectionId::generate();
        let state = ConnectionState::new(id, tx);

        registry.register(state);
        assert_eq!(registry.connection_count(), 1);
    }

    #[tokio::test]
    async fn test_connection_registry_unregister() {
        let registry = ConnectionRegistry::new();
        let (tx, _rx) = mpsc::channel(10);
        let id = ConnectionId::generate();
        let state = ConnectionState::new(id, tx);

        registry.register(state);
        assert_eq!(registry.connection_count(), 1);

        registry.unregister(id);
        assert_eq!(registry.connection_count(), 0);
    }
}
