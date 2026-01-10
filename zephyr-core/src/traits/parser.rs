//! Market data parser trait definitions.
//!
//! This module provides trait definitions for market data parsers
//! that connect to exchange WebSocket/REST APIs and parse market data.
//!
//! # Architecture
//!
//! The parser system uses a callback-based architecture:
//! - [`MarketDataParser`] - Main trait for parser implementations
//! - [`ParserCallback`] - Callback trait for receiving parsed data
//! - [`ParserConfig`] - Configuration for parser initialization
//!
//! # Example
//!
//! ```ignore
//! use zephyr_core::traits::{MarketDataParser, ParserCallback, ParserConfig};
//!
//! struct MyParser { /* ... */ }
//!
//! #[async_trait::async_trait]
//! impl MarketDataParser for MyParser {
//!     async fn init(&mut self, config: &ParserConfig) -> Result<(), NetworkError> {
//!         // Initialize parser
//!         Ok(())
//!     }
//!     // ... other methods
//! }
//! ```

// Allow HashMap in this module - it's used for single-threaded parser configuration
#![allow(clippy::disallowed_types)]

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use crate::data::{OrderBook, TickData};
use crate::error::NetworkError;
use crate::types::Symbol;

/// Configuration for market data parser initialization.
///
/// Contains connection settings, authentication, and subscription options.
///
/// # Examples
///
/// ```
/// use zephyr_core::traits::ParserConfig;
/// use std::time::Duration;
///
/// let config = ParserConfig::builder()
///     .endpoint("wss://stream.binance.com:9443/ws")
///     .connect_timeout(Duration::from_secs(10))
///     .reconnect_enabled(true)
///     .build();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::disallowed_types)]
pub struct ParserConfig {
    /// WebSocket or REST endpoint URL.
    pub endpoint: String,

    /// Connection timeout in milliseconds.
    #[serde(default = "default_connect_timeout_ms")]
    pub connect_timeout_ms: u64,

    /// Read timeout in milliseconds.
    #[serde(default = "default_read_timeout_ms")]
    pub read_timeout_ms: u64,

    /// Whether to enable automatic reconnection.
    #[serde(default = "default_reconnect_enabled")]
    pub reconnect_enabled: bool,

    /// Maximum number of reconnection attempts.
    #[serde(default = "default_max_reconnect_attempts")]
    pub max_reconnect_attempts: u32,

    /// Initial reconnection delay in milliseconds.
    #[serde(default = "default_reconnect_delay_ms")]
    pub reconnect_delay_ms: u64,

    /// Maximum reconnection delay in milliseconds (for exponential backoff).
    #[serde(default = "default_max_reconnect_delay_ms")]
    pub max_reconnect_delay_ms: u64,

    /// Heartbeat/ping interval in milliseconds.
    #[serde(default = "default_heartbeat_interval_ms")]
    pub heartbeat_interval_ms: u64,

    /// Optional API key for authenticated streams.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Optional API secret for authenticated streams.
    #[serde(skip_serializing_if = "Option::is_none", skip_serializing)]
    pub api_secret: Option<String>,

    /// Exchange identifier (e.g., "binance", "okx").
    pub exchange: String,

    /// Additional exchange-specific options.
    #[serde(default)]
    pub options: HashMap<String, String>,
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
    10
}

fn default_reconnect_delay_ms() -> u64 {
    1_000
}

fn default_max_reconnect_delay_ms() -> u64 {
    60_000
}

fn default_heartbeat_interval_ms() -> u64 {
    30_000
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            connect_timeout_ms: default_connect_timeout_ms(),
            read_timeout_ms: default_read_timeout_ms(),
            reconnect_enabled: default_reconnect_enabled(),
            max_reconnect_attempts: default_max_reconnect_attempts(),
            reconnect_delay_ms: default_reconnect_delay_ms(),
            max_reconnect_delay_ms: default_max_reconnect_delay_ms(),
            heartbeat_interval_ms: default_heartbeat_interval_ms(),
            api_key: None,
            api_secret: None,
            exchange: String::new(),
            options: HashMap::new(),
        }
    }
}

impl ParserConfig {
    /// Creates a new builder for `ParserConfig`.
    #[must_use]
    pub fn builder() -> ParserConfigBuilder {
        ParserConfigBuilder::default()
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
}

/// Builder for `ParserConfig`.
#[derive(Debug, Default)]
#[allow(clippy::disallowed_types)]
pub struct ParserConfigBuilder {
    endpoint: Option<String>,
    connect_timeout_ms: Option<u64>,
    read_timeout_ms: Option<u64>,
    reconnect_enabled: Option<bool>,
    max_reconnect_attempts: Option<u32>,
    reconnect_delay_ms: Option<u64>,
    max_reconnect_delay_ms: Option<u64>,
    heartbeat_interval_ms: Option<u64>,
    api_key: Option<String>,
    api_secret: Option<String>,
    exchange: Option<String>,
    options: std::collections::HashMap<String, String>,
}

impl ParserConfigBuilder {
    /// Sets the endpoint URL.
    #[must_use]
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Sets the connection timeout.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout_ms = Some(timeout.as_millis() as u64);
        self
    }

    /// Sets the read timeout.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
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
    #[allow(clippy::cast_possible_truncation)]
    pub fn reconnect_delay(mut self, delay: Duration) -> Self {
        self.reconnect_delay_ms = Some(delay.as_millis() as u64);
        self
    }

    /// Sets the maximum reconnection delay.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn max_reconnect_delay(mut self, delay: Duration) -> Self {
        self.max_reconnect_delay_ms = Some(delay.as_millis() as u64);
        self
    }

    /// Sets the heartbeat interval.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn heartbeat_interval(mut self, interval: Duration) -> Self {
        self.heartbeat_interval_ms = Some(interval.as_millis() as u64);
        self
    }

    /// Sets the API key.
    #[must_use]
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Sets the API secret.
    #[must_use]
    pub fn api_secret(mut self, secret: impl Into<String>) -> Self {
        self.api_secret = Some(secret.into());
        self
    }

    /// Sets the exchange identifier.
    #[must_use]
    pub fn exchange(mut self, exchange: impl Into<String>) -> Self {
        self.exchange = Some(exchange.into());
        self
    }

    /// Adds an option.
    #[must_use]
    pub fn option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }

    /// Builds the `ParserConfig`.
    #[must_use]
    pub fn build(self) -> ParserConfig {
        ParserConfig {
            endpoint: self.endpoint.unwrap_or_default(),
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
            heartbeat_interval_ms: self
                .heartbeat_interval_ms
                .unwrap_or_else(default_heartbeat_interval_ms),
            api_key: self.api_key,
            api_secret: self.api_secret,
            exchange: self.exchange.unwrap_or_default(),
            options: self.options,
        }
    }
}

/// Callback trait for receiving parsed market data.
///
/// Implementations receive callbacks when market data events occur.
/// All methods are async to support non-blocking processing.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow callbacks from
/// multiple parser threads.
#[async_trait]
pub trait ParserCallback: Send + Sync {
    /// Called when tick data is received.
    ///
    /// # Arguments
    ///
    /// * `tick` - The parsed tick data
    async fn on_tick(&self, tick: TickData);

    /// Called when order book data is received.
    ///
    /// # Arguments
    ///
    /// * `orderbook` - The parsed order book data
    async fn on_orderbook(&self, orderbook: OrderBook);

    /// Called when the parser successfully connects.
    async fn on_connected(&self);

    /// Called when the parser disconnects.
    ///
    /// # Arguments
    ///
    /// * `reason` - Optional reason for disconnection
    async fn on_disconnected(&self, reason: Option<String>);

    /// Called when an error occurs during parsing.
    ///
    /// # Arguments
    ///
    /// * `error` - The network error that occurred
    async fn on_error(&self, error: NetworkError);

    /// Called when a reconnection attempt is made.
    ///
    /// # Arguments
    ///
    /// * `attempt` - The current reconnection attempt number
    /// * `max_attempts` - The maximum number of attempts configured
    async fn on_reconnecting(&self, attempt: u32, max_attempts: u32) {
        // Default implementation does nothing
        let _ = (attempt, max_attempts);
    }
}

/// Market data parser trait.
///
/// Defines the interface for market data parsers that connect to
/// exchange APIs and parse market data into standardized structures.
///
/// # Lifecycle
///
/// 1. Create parser instance
/// 2. Call `init()` with configuration
/// 3. Set callback with `set_callback()`
/// 4. Call `connect()` to establish connection
/// 5. Call `subscribe()` to subscribe to symbols
/// 6. Receive data via callbacks
/// 7. Call `disconnect()` when done
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow use across threads.
#[async_trait]
pub trait MarketDataParser: Send + Sync {
    /// Initializes the parser with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Parser configuration
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if initialization fails.
    async fn init(&mut self, config: &ParserConfig) -> Result<(), NetworkError>;

    /// Connects to the market data source.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if connection fails.
    async fn connect(&mut self) -> Result<(), NetworkError>;

    /// Disconnects from the market data source.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if disconnection fails.
    async fn disconnect(&mut self) -> Result<(), NetworkError>;

    /// Returns whether the parser is currently connected.
    fn is_connected(&self) -> bool;

    /// Subscribes to market data for the given symbols.
    ///
    /// # Arguments
    ///
    /// * `symbols` - Symbols to subscribe to
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if subscription fails.
    async fn subscribe(&mut self, symbols: &[Symbol]) -> Result<(), NetworkError>;

    /// Unsubscribes from market data for the given symbols.
    ///
    /// # Arguments
    ///
    /// * `symbols` - Symbols to unsubscribe from
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if unsubscription fails.
    async fn unsubscribe(&mut self, symbols: &[Symbol]) -> Result<(), NetworkError>;

    /// Sets the callback for receiving parsed data.
    ///
    /// # Arguments
    ///
    /// * `callback` - The callback implementation
    fn set_callback(&mut self, callback: Box<dyn ParserCallback>);

    /// Returns the list of currently subscribed symbols.
    fn subscribed_symbols(&self) -> Vec<Symbol>;

    /// Returns the exchange identifier.
    fn exchange(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_config_builder() {
        let config = ParserConfig::builder()
            .endpoint("wss://stream.binance.com:9443/ws")
            .exchange("binance")
            .connect_timeout(Duration::from_secs(15))
            .reconnect_enabled(true)
            .max_reconnect_attempts(5)
            .build();

        assert_eq!(config.endpoint, "wss://stream.binance.com:9443/ws");
        assert_eq!(config.exchange, "binance");
        assert_eq!(config.connect_timeout(), Duration::from_secs(15));
        assert!(config.reconnect_enabled);
        assert_eq!(config.max_reconnect_attempts, 5);
    }

    #[test]
    fn test_parser_config_defaults() {
        let config = ParserConfig::default();

        assert!(config.endpoint.is_empty());
        assert_eq!(config.connect_timeout_ms, 10_000);
        assert_eq!(config.read_timeout_ms, 30_000);
        assert!(config.reconnect_enabled);
        assert_eq!(config.max_reconnect_attempts, 10);
    }

    #[test]
    fn test_parser_config_with_options() {
        let config = ParserConfig::builder()
            .endpoint("wss://example.com")
            .exchange("test")
            .option("stream_type", "combined")
            .option("compression", "gzip")
            .build();

        assert_eq!(
            config.options.get("stream_type"),
            Some(&"combined".to_string())
        );
        assert_eq!(config.options.get("compression"), Some(&"gzip".to_string()));
    }

    #[test]
    fn test_parser_config_with_auth() {
        let config = ParserConfig::builder()
            .endpoint("wss://example.com")
            .exchange("test")
            .api_key("my_api_key")
            .api_secret("my_api_secret")
            .build();

        assert_eq!(config.api_key, Some("my_api_key".to_string()));
        assert_eq!(config.api_secret, Some("my_api_secret".to_string()));
    }

    #[test]
    fn test_parser_config_serde_roundtrip() {
        let config = ParserConfig::builder()
            .endpoint("wss://example.com")
            .exchange("binance")
            .connect_timeout(Duration::from_secs(20))
            .build();

        let json = serde_json::to_string(&config).unwrap();
        let parsed: ParserConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.endpoint, parsed.endpoint);
        assert_eq!(config.exchange, parsed.exchange);
        assert_eq!(config.connect_timeout_ms, parsed.connect_timeout_ms);
    }
}
