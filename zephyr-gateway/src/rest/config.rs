#![allow(clippy::disallowed_types)]

//! REST client configuration.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for REST client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestConfig {
    /// Base URL for API requests.
    pub base_url: String,

    /// API key for authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// API secret for signing requests.
    #[serde(skip_serializing, skip_deserializing)]
    pub api_secret: Option<String>,

    /// Optional passphrase (required by some exchanges).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passphrase: Option<String>,

    /// Request timeout in milliseconds.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    /// Maximum requests per second (0 = unlimited).
    #[serde(default = "default_rate_limit")]
    pub rate_limit_per_second: u32,

    /// Maximum requests per minute (0 = unlimited).
    #[serde(default)]
    pub rate_limit_per_minute: u32,

    /// Whether to retry on rate limit errors.
    #[serde(default = "default_retry_on_rate_limit")]
    pub retry_on_rate_limit: bool,

    /// Maximum retry attempts.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Initial retry delay in milliseconds.
    #[serde(default = "default_retry_delay_ms")]
    pub retry_delay_ms: u64,

    /// Maximum retry delay in milliseconds.
    #[serde(default = "default_max_retry_delay_ms")]
    pub max_retry_delay_ms: u64,

    /// Exchange identifier for logging.
    #[serde(default)]
    pub exchange: String,

    /// Whether this is a testnet/sandbox endpoint.
    #[serde(default)]
    pub testnet: bool,

    /// Additional headers to include in requests.
    #[serde(default)]
    #[allow(clippy::disallowed_types)]
    pub headers: std::collections::HashMap<String, String>,

    /// User agent string.
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
}

fn default_timeout_ms() -> u64 {
    30_000
}

fn default_rate_limit() -> u32 {
    10
}

fn default_retry_on_rate_limit() -> bool {
    true
}

fn default_max_retries() -> u32 {
    3
}

fn default_retry_delay_ms() -> u64 {
    1_000
}

fn default_max_retry_delay_ms() -> u64 {
    30_000
}

fn default_user_agent() -> String {
    format!("Zephyr/{}", env!("CARGO_PKG_VERSION"))
}

impl Default for RestConfig {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            api_key: None,
            api_secret: None,
            passphrase: None,
            timeout_ms: default_timeout_ms(),
            rate_limit_per_second: default_rate_limit(),
            rate_limit_per_minute: 0,
            retry_on_rate_limit: default_retry_on_rate_limit(),
            max_retries: default_max_retries(),
            retry_delay_ms: default_retry_delay_ms(),
            max_retry_delay_ms: default_max_retry_delay_ms(),
            exchange: String::new(),
            testnet: false,
            #[allow(clippy::disallowed_types)]
            headers: std::collections::HashMap::new(),
            user_agent: default_user_agent(),
        }
    }
}

impl RestConfig {
    /// Creates a new builder for `RestConfig`.
    #[must_use]
    pub fn builder() -> RestConfigBuilder {
        RestConfigBuilder::default()
    }

    /// Returns the request timeout as a Duration.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }

    /// Returns the retry delay as a Duration.
    #[must_use]
    pub fn retry_delay(&self) -> Duration {
        Duration::from_millis(self.retry_delay_ms)
    }

    /// Returns the max retry delay as a Duration.
    #[must_use]
    pub fn max_retry_delay(&self) -> Duration {
        Duration::from_millis(self.max_retry_delay_ms)
    }

    /// Calculates the retry delay for a given attempt using exponential backoff.
    #[must_use]
    pub fn calculate_retry_delay(&self, attempt: u32) -> Duration {
        let delay = self.retry_delay_ms as f64 * 2.0_f64.powi(attempt as i32);
        let capped_delay = delay.min(self.max_retry_delay_ms as f64) as u64;
        Duration::from_millis(capped_delay)
    }

    /// Returns whether a retry should be attempted.
    #[must_use]
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }

    /// Returns whether the client has authentication configured.
    #[must_use]
    pub fn has_auth(&self) -> bool {
        self.api_key.is_some() && self.api_secret.is_some()
    }
}

/// Builder for `RestConfig`.
#[derive(Debug, Default)]
pub struct RestConfigBuilder {
    base_url: Option<String>,
    api_key: Option<String>,
    api_secret: Option<String>,
    passphrase: Option<String>,
    timeout_ms: Option<u64>,
    rate_limit_per_second: Option<u32>,
    rate_limit_per_minute: Option<u32>,
    retry_on_rate_limit: Option<bool>,
    max_retries: Option<u32>,
    retry_delay_ms: Option<u64>,
    max_retry_delay_ms: Option<u64>,
    exchange: Option<String>,
    testnet: Option<bool>,
    #[allow(clippy::disallowed_types)]
    headers: std::collections::HashMap<String, String>,
    user_agent: Option<String>,
}

impl RestConfigBuilder {
    /// Sets the base URL.
    #[must_use]
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
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

    /// Sets the passphrase.
    #[must_use]
    pub fn passphrase(mut self, passphrase: impl Into<String>) -> Self {
        self.passphrase = Some(passphrase.into());
        self
    }

    /// Sets the request timeout.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout_ms = Some(timeout.as_millis() as u64);
        self
    }

    /// Sets the rate limit per second.
    #[must_use]
    pub fn rate_limit_per_second(mut self, limit: u32) -> Self {
        self.rate_limit_per_second = Some(limit);
        self
    }

    /// Sets the rate limit per minute.
    #[must_use]
    pub fn rate_limit_per_minute(mut self, limit: u32) -> Self {
        self.rate_limit_per_minute = Some(limit);
        self
    }

    /// Sets whether to retry on rate limit errors.
    #[must_use]
    pub fn retry_on_rate_limit(mut self, retry: bool) -> Self {
        self.retry_on_rate_limit = Some(retry);
        self
    }

    /// Sets the maximum retry attempts.
    #[must_use]
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = Some(retries);
        self
    }

    /// Sets the initial retry delay.
    #[must_use]
    pub fn retry_delay(mut self, delay: Duration) -> Self {
        self.retry_delay_ms = Some(delay.as_millis() as u64);
        self
    }

    /// Sets the maximum retry delay.
    #[must_use]
    pub fn max_retry_delay(mut self, delay: Duration) -> Self {
        self.max_retry_delay_ms = Some(delay.as_millis() as u64);
        self
    }

    /// Sets the exchange identifier.
    #[must_use]
    pub fn exchange(mut self, exchange: impl Into<String>) -> Self {
        self.exchange = Some(exchange.into());
        self
    }

    /// Sets whether this is a testnet endpoint.
    #[must_use]
    pub fn testnet(mut self, testnet: bool) -> Self {
        self.testnet = Some(testnet);
        self
    }

    /// Adds a header.
    #[must_use]
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Sets the user agent.
    #[must_use]
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Builds the `RestConfig`.
    #[must_use]
    pub fn build(self) -> RestConfig {
        RestConfig {
            base_url: self.base_url.unwrap_or_default(),
            api_key: self.api_key,
            api_secret: self.api_secret,
            passphrase: self.passphrase,
            timeout_ms: self.timeout_ms.unwrap_or_else(default_timeout_ms),
            rate_limit_per_second: self
                .rate_limit_per_second
                .unwrap_or_else(default_rate_limit),
            rate_limit_per_minute: self.rate_limit_per_minute.unwrap_or(0),
            retry_on_rate_limit: self
                .retry_on_rate_limit
                .unwrap_or_else(default_retry_on_rate_limit),
            max_retries: self.max_retries.unwrap_or_else(default_max_retries),
            retry_delay_ms: self.retry_delay_ms.unwrap_or_else(default_retry_delay_ms),
            max_retry_delay_ms: self
                .max_retry_delay_ms
                .unwrap_or_else(default_max_retry_delay_ms),
            exchange: self.exchange.unwrap_or_default(),
            testnet: self.testnet.unwrap_or(false),
            headers: self.headers,
            user_agent: self.user_agent.unwrap_or_else(default_user_agent),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = RestConfig::builder()
            .base_url("https://api.binance.com")
            .api_key("my_key")
            .api_secret("my_secret")
            .exchange("binance")
            .timeout(Duration::from_secs(15))
            .rate_limit_per_second(20)
            .build();

        assert_eq!(config.base_url, "https://api.binance.com");
        assert_eq!(config.api_key, Some("my_key".to_string()));
        assert_eq!(config.api_secret, Some("my_secret".to_string()));
        assert_eq!(config.exchange, "binance");
        assert_eq!(config.timeout(), Duration::from_secs(15));
        assert_eq!(config.rate_limit_per_second, 20);
        assert!(config.has_auth());
    }

    #[test]
    fn test_config_defaults() {
        let config = RestConfig::default();

        assert!(config.base_url.is_empty());
        assert!(config.api_key.is_none());
        assert_eq!(config.timeout_ms, 30_000);
        assert_eq!(config.rate_limit_per_second, 10);
        assert!(config.retry_on_rate_limit);
        assert!(!config.has_auth());
    }

    #[test]
    fn test_exponential_backoff() {
        let config = RestConfig::builder()
            .retry_delay(Duration::from_secs(1))
            .max_retry_delay(Duration::from_secs(30))
            .build();

        assert_eq!(config.calculate_retry_delay(0), Duration::from_secs(1));
        assert_eq!(config.calculate_retry_delay(1), Duration::from_secs(2));
        assert_eq!(config.calculate_retry_delay(2), Duration::from_secs(4));
        assert_eq!(config.calculate_retry_delay(3), Duration::from_secs(8));
        // Should cap at max
        assert_eq!(config.calculate_retry_delay(10), Duration::from_secs(30));
    }

    #[test]
    fn test_should_retry() {
        let config = RestConfig::builder().max_retries(3).build();

        assert!(config.should_retry(0));
        assert!(config.should_retry(1));
        assert!(config.should_retry(2));
        assert!(!config.should_retry(3));
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let config = RestConfig::builder()
            .base_url("https://api.example.com")
            .exchange("test")
            .timeout(Duration::from_secs(20))
            .build();

        let json = serde_json::to_string(&config).unwrap();
        let parsed: RestConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.base_url, parsed.base_url);
        assert_eq!(config.exchange, parsed.exchange);
        assert_eq!(config.timeout_ms, parsed.timeout_ms);
    }
}
