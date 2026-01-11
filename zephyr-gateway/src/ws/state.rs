//! WebSocket connection state management.

#![allow(clippy::redundant_pub_crate)]

use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Connection state for WebSocket client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// Not connected.
    Disconnected,
    /// Attempting to connect.
    Connecting,
    /// Successfully connected.
    Connected,
    /// Attempting to reconnect.
    Reconnecting,
    /// Connection closed intentionally.
    Closed,
}

impl ConnectionState {
    /// Returns true if the connection is active.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }

    /// Returns true if the connection is in a transitional state.
    #[must_use]
    pub fn is_transitioning(&self) -> bool {
        matches!(self, Self::Connecting | Self::Reconnecting)
    }

    /// Returns true if the connection is closed or disconnected.
    #[must_use]
    pub fn is_inactive(&self) -> bool {
        matches!(self, Self::Disconnected | Self::Closed)
    }
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected => write!(f, "Disconnected"),
            Self::Connecting => write!(f, "Connecting"),
            Self::Connected => write!(f, "Connected"),
            Self::Reconnecting => write!(f, "Reconnecting"),
            Self::Closed => write!(f, "Closed"),
        }
    }
}

/// Internal state tracking for the WebSocket client.
#[derive(Debug)]
pub(crate) struct InternalState {
    /// Current connection state.
    pub state: ConnectionState,
    /// Number of reconnection attempts.
    pub reconnect_attempts: u32,
    /// Last successful connection time.
    pub last_connected: Option<Instant>,
    /// Last message received time.
    pub last_message: Option<Instant>,
    /// Last ping sent time.
    pub last_ping: Option<Instant>,
    /// Last pong received time.
    pub last_pong: Option<Instant>,
    /// Whether we're waiting for a pong response.
    pub awaiting_pong: bool,
}

impl Default for InternalState {
    fn default() -> Self {
        Self {
            state: ConnectionState::Disconnected,
            reconnect_attempts: 0,
            last_connected: None,
            last_message: None,
            last_ping: None,
            last_pong: None,
            awaiting_pong: false,
        }
    }
}

impl InternalState {
    /// Creates a new internal state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks the connection as connected.
    pub fn mark_connected(&mut self) {
        self.state = ConnectionState::Connected;
        self.reconnect_attempts = 0;
        self.last_connected = Some(Instant::now());
        self.awaiting_pong = false;
    }

    /// Marks the connection as disconnected.
    pub fn mark_disconnected(&mut self) {
        self.state = ConnectionState::Disconnected;
        self.awaiting_pong = false;
    }

    /// Marks the connection as reconnecting.
    #[allow(dead_code)]
    pub fn mark_reconnecting(&mut self) {
        self.state = ConnectionState::Reconnecting;
        self.reconnect_attempts += 1;
    }

    /// Marks the connection as closed.
    pub fn mark_closed(&mut self) {
        self.state = ConnectionState::Closed;
        self.awaiting_pong = false;
    }

    /// Records that a message was received.
    pub fn record_message(&mut self) {
        self.last_message = Some(Instant::now());
    }

    /// Records that a ping was sent.
    pub fn record_ping(&mut self) {
        self.last_ping = Some(Instant::now());
        self.awaiting_pong = true;
    }

    /// Records that a pong was received.
    pub fn record_pong(&mut self) {
        self.last_pong = Some(Instant::now());
        self.awaiting_pong = false;
    }

    /// Resets reconnection attempts counter.
    #[allow(dead_code)]
    pub fn reset_reconnect_attempts(&mut self) {
        self.reconnect_attempts = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_state_display() {
        assert_eq!(ConnectionState::Disconnected.to_string(), "Disconnected");
        assert_eq!(ConnectionState::Connected.to_string(), "Connected");
        assert_eq!(ConnectionState::Reconnecting.to_string(), "Reconnecting");
    }

    #[test]
    fn test_connection_state_checks() {
        assert!(ConnectionState::Connected.is_connected());
        assert!(!ConnectionState::Disconnected.is_connected());

        assert!(ConnectionState::Connecting.is_transitioning());
        assert!(ConnectionState::Reconnecting.is_transitioning());
        assert!(!ConnectionState::Connected.is_transitioning());

        assert!(ConnectionState::Disconnected.is_inactive());
        assert!(ConnectionState::Closed.is_inactive());
        assert!(!ConnectionState::Connected.is_inactive());
    }

    #[test]
    fn test_internal_state_transitions() {
        let mut state = InternalState::new();
        assert_eq!(state.state, ConnectionState::Disconnected);
        assert_eq!(state.reconnect_attempts, 0);

        state.mark_connected();
        assert_eq!(state.state, ConnectionState::Connected);
        assert!(state.last_connected.is_some());

        state.mark_disconnected();
        assert_eq!(state.state, ConnectionState::Disconnected);

        state.mark_reconnecting();
        assert_eq!(state.state, ConnectionState::Reconnecting);
        assert_eq!(state.reconnect_attempts, 1);

        state.mark_reconnecting();
        assert_eq!(state.reconnect_attempts, 2);

        state.mark_connected();
        assert_eq!(state.reconnect_attempts, 0);
    }

    #[test]
    fn test_ping_pong_tracking() {
        let mut state = InternalState::new();
        assert!(!state.awaiting_pong);

        state.record_ping();
        assert!(state.awaiting_pong);
        assert!(state.last_ping.is_some());

        state.record_pong();
        assert!(!state.awaiting_pong);
        assert!(state.last_pong.is_some());
    }
}
