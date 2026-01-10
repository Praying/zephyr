//! WebSocket server configuration.
//!
//! This module provides configuration for the WebSocket server including:
//! - Heartbeat intervals
//! - Connection timeouts
//! - Message compression settings
//! - Buffer sizes

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// WebSocket server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsConfig {
    /// Heartbeat interval in seconds
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,

    /// Connection timeout in seconds (no heartbeat response)
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout_secs: u64,

    /// Enable message compression
    #[serde(default = "default_true")]
    pub enable_compression: bool,

    /// Compression threshold in bytes (messages smaller than this won't be compressed)
    #[serde(default = "default_compression_threshold")]
    pub compression_threshold: usize,

    /// Maximum message size in bytes
    #[serde(default = "default_max_message_size")]
    pub max_message_size: usize,

    /// Maximum number of queued messages per connection
    #[serde(default = "default_max_queue_size")]
    pub max_queue_size: usize,

    /// PnL update interval in milliseconds
    #[serde(default = "default_pnl_update_interval")]
    pub pnl_update_interval_ms: u64,
}

impl Default for WsConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_secs: default_heartbeat_interval(),
            connection_timeout_secs: default_connection_timeout(),
            enable_compression: true,
            compression_threshold: default_compression_threshold(),
            max_message_size: default_max_message_size(),
            max_queue_size: default_max_queue_size(),
            pnl_update_interval_ms: default_pnl_update_interval(),
        }
    }
}

impl WsConfig {
    /// Returns the heartbeat interval as a Duration.
    #[must_use]
    pub fn heartbeat_interval(&self) -> Duration {
        Duration::from_secs(self.heartbeat_interval_secs)
    }

    /// Returns the connection timeout as a Duration.
    #[must_use]
    pub fn connection_timeout(&self) -> Duration {
        Duration::from_secs(self.connection_timeout_secs)
    }

    /// Returns the PnL update interval as a Duration.
    #[must_use]
    pub fn pnl_update_interval(&self) -> Duration {
        Duration::from_millis(self.pnl_update_interval_ms)
    }
}

fn default_heartbeat_interval() -> u64 {
    30
}

fn default_connection_timeout() -> u64 {
    90
}

fn default_true() -> bool {
    true
}

fn default_compression_threshold() -> usize {
    1024 // 1KB
}

fn default_max_message_size() -> usize {
    1024 * 1024 // 1MB
}

fn default_max_queue_size() -> usize {
    1000
}

fn default_pnl_update_interval() -> u64 {
    1000 // 1 second
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_config_default() {
        let config = WsConfig::default();
        assert_eq!(config.heartbeat_interval_secs, 30);
        assert_eq!(config.connection_timeout_secs, 90);
        assert!(config.enable_compression);
    }

    #[test]
    fn test_ws_config_durations() {
        let config = WsConfig::default();
        assert_eq!(config.heartbeat_interval(), Duration::from_secs(30));
        assert_eq!(config.connection_timeout(), Duration::from_secs(90));
        assert_eq!(config.pnl_update_interval(), Duration::from_millis(1000));
    }
}
