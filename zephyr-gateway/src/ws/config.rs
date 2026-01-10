//! WebSocket client configuration.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for WebSocket client.
///
/// Contains connection settings, reconnection parameters, and heartbeat configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    /// WebSocket endpoint URL.
    pub url: String,

    /// Connection timeout in milliseconds.
    #[serde(default = "default_connect_timeout_ms")]
    pub connect_timeout_ms: u64,

    /// Read timeout in milliseconds (for ping/pong).
    #[serde(default = "default_read_timeout_ms")]
    pub read_timeout_ms: u64,

    /// Whether automatic reconnection is enabled.
    #[serde(default = "default_reconnect_enabled")]
    pub reconnect_enabled: bool,

    /// Maximum number of reconnection attempts (0 = unlimited).
    #[serde(default = "default_max_reconnect_attempts")]
    pub max_reconnect_attempts: u32,

    /// Initial reconnection delay in milliseconds.
    #[serde(default = "default_reconnect_delay_ms")]
    pub reconnect_delay_ms: u64,

    /// Maximum reconnection delay in milliseconds (for exponential backoff).
    #[serde(default = "default_max_reconnect_delay_ms")]
    pub max_reconnect_delay_ms: u64,

    /// Backoff multiplier for exponential backoff.
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,

    /// Heartbeat/ping interval in milliseconds.
    #[serde(default = "default_heartbeat_interval_ms")]
    pub heartbeat_interval_ms: u64,

    /// Pong timeout in milliseconds (how long to wait for pong after ping).
    #[serde(default = "default_pong_timeout_ms")]
    pub pong_timeout_ms: u64,

    /// Whether to send ping frames automatically.
    #[serde(default = "default_auto_ping")]
    pub auto_ping: bool,

    /// Custom ping message (if exchange requires specific format).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_ping_message: Option<String>,

    /// Exchange identifier for logging.
    #[serde(default)]
    pub exchange: String,

    /// Additional headers for the WebSocket connection.
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

fn default_connect_timeout_ms() -> u64 {
    10_000
}

fn default_read_timeout_ms() -> u64 {
    30_000
}

fn default_reconnect_enabled() -> bool {
    true
}

fn default_max_reconnect_attempts() -> u32 {
    0 // unlimited
}

fn default_reconnect_delay_ms() -> u64 {
    1_000
}

fn default_max_reconnect_delay_ms() -> u64 {
    60_000
}

fn default_backoff_multiplier() -> f64 {
    2.0
}

fn default_heartbeat_interval_ms() -> u64 {
    30_000
}

fn default_pong_timeout_ms() -> u64 {
    10_000
}

fn default_auto_ping() -> bool {
    true
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            connect_timeout_ms: default_connect_timeout_ms(),
            read_timeout_ms: default_read_timeout_ms(),
            reconnect_enabled: default_reconnect_enabled(),
            max_reconnect_attempts: default_max_reconnect_attempts(),
            reconnect_delay_ms: default_reconnect_delay_ms(),
            max_reconnect_delay_ms: default_max_reconnect_delay_ms(),
            backoff_multiplier: default_backoff_multiplier(),
            heartbeat_interval_ms: default_heartbeat_interval_ms(),
            pong_timeout_ms: default_pong_timeout_ms(),
            auto_ping: default_auto_ping(),
            custom_ping_message: None,
            exchange: String::new(),
            headers: std::collections::HashMap::new(),
        }
    }
}

impl WebSocketConfig {
    /// Creates a new builder for `WebSocketConfig`.
    #[must_use]
    pub fn builder() -> WebSocketConfigBuilder {
        WebSocketConfigBuilder::default()
    }

    /// Returns the connection timeout as a Duration.
    #[must_use]
    pub fn connect_timeout(&self) -> Duration {
        Duration::from_millis(self.connect_timeout_ms)
    }

    /// Returns the read timeout as a Duration.
    #[must_use]
    pub fn read_timeout(&self) -> Duration {
        Duration::from_millis(self.read_timeout_ms)
    }

    /// Returns the reconnect delay as a Duration.
    #[must_use]
    pub fn reconnect_delay(&self) -> Duration {
        Duration::from_millis(self.reconnect_delay_ms)
    }

    /// Returns the max reconnect delay as a Duration.
    #[must_use]
    pub fn max_reconnect_delay(&self) -> Duration {
        Duration::from_millis(self.max_reconnect_delay_ms)
    }

    /// Returns the heartbeat interval as a Duration.
    #[must_use]
    pub fn heartbeat_interval(&self) -> Duration {
        Duration::from_millis(self.heartbeat_interval_ms)
    }

    /// Returns the pong timeout as a Duration.
    #[must_use]
    pub fn pong_timeout(&self) -> Duration {
        Duration::from_millis(self.pong_timeout_ms)
    }

    /// Calculates the reconnect delay for a given attempt using exponential backoff.
    #[must_use]
    pub fn calculate_reconnect_delay(&self, attempt: u32) -> Duration {
        let delay = self.reconnect_delay_ms as f64 * self.backoff_multiplier.powi(attempt as i32);
        let capped_delay = delay.min(self.max_reconnect_delay_ms as f64) as u64;
        Duration::from_millis(capped_delay)
    }

    /// Returns whether reconnection should be attempted.
    #[must_use]
    pub fn should_reconnect(&self, attempt: u32) -> bool {
        self.reconnect_enabled
            && (self.max_reconnect_attempts == 0 || attempt < self.max_reconnect_attempts)
    }
}

/// Builder for `WebSocketConfig`.
#[derive(Debug, Default)]
pub struct WebSocketConfigBuilder {
    url: Option<String>,
    connect_timeout_ms: Option<u64>,
    read_timeout_ms: Option<u64>,
    reconnect_enabled: Option<bool>,
    max_reconnect_attempts: Option<u32>,
    reconnect_delay_ms: Option<u64>,
    max_reconnect_delay_ms: Option<u64>,
    backoff_multiplier: Option<f64>,
    heartbeat_interval_ms: Option<u64>,
    pong_timeout_ms: Option<u64>,
    auto_ping: Option<bool>,
    custom_ping_message: Option<String>,
    exchange: Option<String>,
    headers: std::collections::HashMap<String, String>,
}

impl WebSocketConfigBuilder {
    /// Sets the WebSocket URL.
    #[must_use]
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Sets the connection timeout.
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout_ms = Some(timeout.as_millis() as u64);
        self
    }

    /// Sets the read timeout.
    #[must_use]
    pub fn read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout_ms = Some(timeout.as_millis() as u64);
        self
    }

    /// Sets whether reconnection is enabled.
    #[must_use]
    pub fn reconnect_enabled(mut self, enabled: bool) -> Self {
        self.reconnect_enabled = Some(enabled);
        self
    }

    /// Sets the maximum reconnection attempts.
    #[must_use]
    pub fn max_reconnect_attempts(mut self, attempts: u32) -> Self {
        self.max_reconnect_attempts = Some(attempts);
        self
    }

    /// Sets the initial reconnection delay.
    #[must_use]
    pub fn reconnect_delay(mut self, delay: Duration) -> Self {
        self.reconnect_delay_ms = Some(delay.as_millis() as u64);
        self
    }

    /// Sets the maximum reconnection delay.
    #[must_use]
    pub fn max_reconnect_delay(mut self, delay: Duration) -> Self {
        self.max_reconnect_delay_ms = Some(delay.as_millis() as u64);
        self
    }

    /// Sets the backoff multiplier.
    #[must_use]
    pub fn backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = Some(multiplier);
        self
    }

    /// Sets the heartbeat interval.
    #[must_use]
    pub fn heartbeat_interval(mut self, interval: Duration) -> Self {
        self.heartbeat_interval_ms = Some(interval.as_millis() as u64);
        self
    }

    /// Sets the pong timeout.
    #[must_use]
    pub fn pong_timeout(mut self, timeout: Duration) -> Self {
        self.pong_timeout_ms = Some(timeout.as_millis() as u64);
        self
    }

    /// Sets whether auto ping is enabled.
    #[must_use]
    pub fn auto_ping(mut self, enabled: bool) -> Self {
        self.auto_ping = Some(enabled);
        self
    }

    /// Sets a custom ping message.
    #[must_use]
    pub fn custom_ping_message(mut self, message: impl Into<String>) -> Self {
        self.custom_ping_message = Some(message.into());
        self
    }

    /// Sets the exchange identifier.
    #[must_use]
    pub fn exchange(mut self, exchange: impl Into<String>) -> Self {
        self.exchange = Some(exchange.into());
        self
    }

    /// Adds a header.
    #[must_use]
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Builds the `WebSocketConfig`.
    #[must_use]
    pub fn build(self) -> WebSocketConfig {
        WebSocketConfig {
            url: self.url.unwrap_or_default(),
            connect_timeout_ms: self
                .connect_timeout_ms
                .unwrap_or_else(default_connect_timeout_ms),
            read_timeout_ms: self.read_timeout_ms.unwrap_or_else(default_read_timeout_ms),
            reconnect_enabled: self
                .reconnect_enabled
                .unwrap_or_else(default_reconnect_enabled),
            max_reconnect_attempts: self
                .max_reconnect_attempts
                .unwrap_or_else(default_max_reconnect_attempts),
            reconnect_delay_ms: self
                .reconnect_delay_ms
                .unwrap_or_else(default_reconnect_delay_ms),
            max_reconnect_delay_ms: self
                .max_reconnect_delay_ms
                .unwrap_or_else(default_max_reconnect_delay_ms),
            backoff_multiplier: self
                .backoff_multiplier
                .unwrap_or_else(default_backoff_multiplier),
            heartbeat_interval_ms: self
                .heartbeat_interval_ms
                .unwrap_or_else(default_heartbeat_interval_ms),
            pong_timeout_ms: self.pong_timeout_ms.unwrap_or_else(default_pong_timeout_ms),
            auto_ping: self.auto_ping.unwrap_or_else(default_auto_ping),
            custom_ping_message: self.custom_ping_message,
            exchange: self.exchange.unwrap_or_default(),
            headers: self.headers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = WebSocketConfig::builder()
            .url("wss://example.com/ws")
            .exchange("binance")
            .connect_timeout(Duration::from_secs(15))
            .reconnect_enabled(true)
            .max_reconnect_attempts(5)
            .build();

        assert_eq!(config.url, "wss://example.com/ws");
        assert_eq!(config.exchange, "binance");
        assert_eq!(config.connect_timeout(), Duration::from_secs(15));
        assert!(config.reconnect_enabled);
        assert_eq!(config.max_reconnect_attempts, 5);
    }

    #[test]
    fn test_config_defaults() {
        let config = WebSocketConfig::default();

        assert!(config.url.is_empty());
        assert_eq!(config.connect_timeout_ms, 10_000);
        assert!(config.reconnect_enabled);
        assert_eq!(config.max_reconnect_attempts, 0);
        assert!(config.auto_ping);
    }

    #[test]
    fn test_exponential_backoff() {
        let config = WebSocketConfig::builder()
            .reconnect_delay(Duration::from_secs(1))
            .max_reconnect_delay(Duration::from_secs(60))
            .backoff_multiplier(2.0)
            .build();

        assert_eq!(config.calculate_reconnect_delay(0), Duration::from_secs(1));
        assert_eq!(config.calculate_reconnect_delay(1), Duration::from_secs(2));
        assert_eq!(config.calculate_reconnect_delay(2), Duration::from_secs(4));
        assert_eq!(config.calculate_reconnect_delay(3), Duration::from_secs(8));
        // Should cap at max
        assert_eq!(
            config.calculate_reconnect_delay(10),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn test_should_reconnect() {
        let config = WebSocketConfig::builder()
            .reconnect_enabled(true)
            .max_reconnect_attempts(3)
            .build();

        assert!(config.should_reconnect(0));
        assert!(config.should_reconnect(1));
        assert!(config.should_reconnect(2));
        assert!(!config.should_reconnect(3));

        // Unlimited attempts
        let unlimited = WebSocketConfig::builder()
            .reconnect_enabled(true)
            .max_reconnect_attempts(0)
            .build();

        assert!(unlimited.should_reconnect(100));
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let config = WebSocketConfig::builder()
            .url("wss://example.com")
            .exchange("test")
            .connect_timeout(Duration::from_secs(20))
            .build();

        let json = serde_json::to_string(&config).unwrap();
        let parsed: WebSocketConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.url, parsed.url);
        assert_eq!(config.exchange, parsed.exchange);
        assert_eq!(config.connect_timeout_ms, parsed.connect_timeout_ms);
    }
}
