//! Zephyr system configuration structures.
//!
//! This module provides the main configuration structures for the Zephyr trading system,
//! demonstrating how to use the configuration framework with validation and environment
//! variable overrides.

// Allow HashMap in this module - it's used for single-threaded configuration storage
#![allow(clippy::disallowed_types)]

use super::traits::Validatable;
use super::validation::{EnvOverride, ValidationContext, Validator};
use crate::error::ConfigError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Main Zephyr system configuration.
///
/// This is the top-level configuration structure that contains all system settings.
///
/// # Example YAML
///
/// ```yaml
/// server:
///   host: "0.0.0.0"
///   port: 8080
///   
/// exchanges:
///   binance:
///     api_key: "your_api_key"
///     testnet: false
///     
/// risk:
///   max_position_value: 100000
///   daily_loss_limit: 5000
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]

pub struct ZephyrConfig {
    /// Server configuration.
    #[serde(default)]
    pub server: ServerConfig,

    /// Exchange configurations.
    #[serde(default)]
    pub exchanges: HashMap<String, ExchangeConfig>,

    /// Risk management configuration.
    #[serde(default)]
    pub risk: RiskConfig,

    /// Logging configuration.
    #[serde(default)]
    pub logging: LoggingConfig,

    /// Data storage configuration.
    #[serde(default)]
    pub storage: StorageConfig,
}

impl Validatable for ZephyrConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        let mut ctx = ValidationContext::new();

        // Validate server config
        ctx.enter("server");
        self.server.validate_with_context(&mut ctx);
        ctx.exit();

        // Validate each exchange config
        ctx.enter("exchanges");
        for (name, exchange) in &self.exchanges {
            ctx.enter(name);
            exchange.validate_with_context(&mut ctx);
            ctx.exit();
        }
        ctx.exit();

        // Validate risk config
        ctx.enter("risk");
        self.risk.validate_with_context(&mut ctx);
        ctx.exit();

        ctx.into_result()
    }
}

impl ZephyrConfig {
    /// Applies environment variable overrides to the configuration.
    ///
    /// Environment variables are prefixed with `ZEPHYR_` and use underscores
    /// to separate nested fields.
    ///
    /// # Examples
    ///
    /// - `ZEPHYR_SERVER_PORT=9090` overrides `server.port`
    /// - `ZEPHYR_RISK_DAILY_LOSS_LIMIT=10000` overrides `risk.daily_loss_limit`
    pub fn apply_env_overrides(&mut self) {
        self.server.apply_env_overrides("ZEPHYR_SERVER");
        self.risk.apply_env_overrides("ZEPHYR_RISK");
        self.logging.apply_env_overrides("ZEPHYR_LOGGING");
        self.storage.apply_env_overrides("ZEPHYR_STORAGE");

        // Apply exchange-specific overrides
        for (name, exchange) in &mut self.exchanges {
            let prefix = format!("ZEPHYR_EXCHANGE_{}", name.to_uppercase());
            exchange.apply_env_overrides(&prefix);
        }
    }
}

/// Server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Host address to bind to.
    #[serde(default = "default_host")]
    pub host: String,

    /// Port to listen on.
    #[serde(default = "default_port")]
    pub port: u16,

    /// Enable TLS.
    #[serde(default)]
    pub tls_enabled: bool,

    /// Path to TLS certificate file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls_cert_path: Option<String>,

    /// Path to TLS key file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls_key_path: Option<String>,

    /// Request timeout in milliseconds.
    #[serde(default = "default_request_timeout_ms")]
    pub request_timeout_ms: u64,

    /// Maximum concurrent connections.
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_request_timeout_ms() -> u64 {
    30_000
}

fn default_max_connections() -> u32 {
    1000
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
            request_timeout_ms: default_request_timeout_ms(),
            max_connections: default_max_connections(),
        }
    }
}

impl ServerConfig {
    fn validate_with_context(&self, ctx: &mut ValidationContext) {
        let mut validator = Validator::new(ctx);

        validator
            .require_non_empty("host", &self.host)
            .in_range("port", &self.port, &1, &65535)
            .in_range("max_connections", &self.max_connections, &1, &100_000);

        // If TLS is enabled, cert and key paths are required
        if self.tls_enabled {
            validator
                .require("tls_cert_path", &self.tls_cert_path)
                .require("tls_key_path", &self.tls_key_path);
        }
    }

    fn apply_env_overrides(&mut self, prefix: &str) {
        EnvOverride::apply_string(&format!("{prefix}_HOST"), &mut self.host);
        EnvOverride::apply_number(&format!("{prefix}_PORT"), &mut self.port);
        EnvOverride::apply_bool(&format!("{prefix}_TLS_ENABLED"), &mut self.tls_enabled);
        EnvOverride::apply_optional_string(
            &format!("{prefix}_TLS_CERT_PATH"),
            &mut self.tls_cert_path,
        );
        EnvOverride::apply_optional_string(
            &format!("{prefix}_TLS_KEY_PATH"),
            &mut self.tls_key_path,
        );
        EnvOverride::apply_duration_ms(
            &format!("{prefix}_REQUEST_TIMEOUT_MS"),
            &mut self.request_timeout_ms,
        );
        EnvOverride::apply_number(
            &format!("{prefix}_MAX_CONNECTIONS"),
            &mut self.max_connections,
        );
    }

    /// Returns the request timeout as a Duration.
    #[must_use]
    pub fn request_timeout(&self) -> Duration {
        Duration::from_millis(self.request_timeout_ms)
    }
}

/// Exchange configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExchangeConfig {
    /// API key for authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// API secret for signing requests (not serialized for security).
    #[serde(skip_serializing, skip_deserializing)]
    pub api_secret: Option<String>,

    /// Optional passphrase (required by some exchanges like OKX).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passphrase: Option<String>,

    /// Whether to use testnet/sandbox mode.
    #[serde(default)]
    pub testnet: bool,

    /// WebSocket URL override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ws_url: Option<String>,

    /// REST API URL override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rest_url: Option<String>,

    /// Rate limit per second.
    #[serde(default = "default_rate_limit")]
    pub rate_limit_per_second: u32,

    /// Whether this exchange is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_rate_limit() -> u32 {
    10
}

fn default_enabled() -> bool {
    true
}

impl ExchangeConfig {
    fn validate_with_context(&self, ctx: &mut ValidationContext) {
        let mut validator = Validator::new(ctx);

        if self.enabled {
            // API key is required for enabled exchanges
            validator.require("api_key", &self.api_key);
        }

        validator.in_range(
            "rate_limit_per_second",
            &self.rate_limit_per_second,
            &1,
            &1000,
        );

        // Validate URLs if provided
        if let Some(ref url) = self.ws_url {
            validator.valid_url("ws_url", url);
        }
        if let Some(ref url) = self.rest_url {
            validator.valid_url("rest_url", url);
        }
    }

    fn apply_env_overrides(&mut self, prefix: &str) {
        EnvOverride::apply_optional_string(&format!("{prefix}_API_KEY"), &mut self.api_key);
        EnvOverride::apply_optional_string(&format!("{prefix}_API_SECRET"), &mut self.api_secret);
        EnvOverride::apply_optional_string(&format!("{prefix}_PASSPHRASE"), &mut self.passphrase);
        EnvOverride::apply_bool(&format!("{prefix}_TESTNET"), &mut self.testnet);
        EnvOverride::apply_optional_string(&format!("{prefix}_WS_URL"), &mut self.ws_url);
        EnvOverride::apply_optional_string(&format!("{prefix}_REST_URL"), &mut self.rest_url);
        EnvOverride::apply_number(
            &format!("{prefix}_RATE_LIMIT"),
            &mut self.rate_limit_per_second,
        );
        EnvOverride::apply_bool(&format!("{prefix}_ENABLED"), &mut self.enabled);
    }

    /// Returns whether the exchange has authentication configured.
    #[must_use]
    pub fn has_auth(&self) -> bool {
        self.api_key.is_some() && self.api_secret.is_some()
    }
}

/// Risk management configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    /// Maximum position value per symbol (in quote currency).
    #[serde(default = "default_max_position_value")]
    pub max_position_value: f64,

    /// Maximum total position value across all symbols.
    #[serde(default = "default_max_total_position")]
    pub max_total_position: f64,

    /// Daily loss limit (in quote currency).
    #[serde(default = "default_daily_loss_limit")]
    pub daily_loss_limit: f64,

    /// Maximum orders per second.
    #[serde(default = "default_max_orders_per_second")]
    pub max_orders_per_second: u32,

    /// Maximum orders per minute.
    #[serde(default = "default_max_orders_per_minute")]
    pub max_orders_per_minute: u32,

    /// Whether to enable automatic position reduction on risk breach.
    #[serde(default)]
    pub auto_reduce_enabled: bool,

    /// Percentage to reduce position by on risk breach.
    #[serde(default = "default_auto_reduce_percent")]
    pub auto_reduce_percent: f64,
}

fn default_max_position_value() -> f64 {
    100_000.0
}

fn default_max_total_position() -> f64 {
    500_000.0
}

fn default_daily_loss_limit() -> f64 {
    10_000.0
}

fn default_max_orders_per_second() -> u32 {
    10
}

fn default_max_orders_per_minute() -> u32 {
    300
}

fn default_auto_reduce_percent() -> f64 {
    50.0
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_position_value: default_max_position_value(),
            max_total_position: default_max_total_position(),
            daily_loss_limit: default_daily_loss_limit(),
            max_orders_per_second: default_max_orders_per_second(),
            max_orders_per_minute: default_max_orders_per_minute(),
            auto_reduce_enabled: false,
            auto_reduce_percent: default_auto_reduce_percent(),
        }
    }
}

impl RiskConfig {
    fn validate_with_context(&self, ctx: &mut ValidationContext) {
        let mut validator = Validator::new(ctx);

        validator
            .positive("max_position_value", &self.max_position_value)
            .positive("max_total_position", &self.max_total_position)
            .positive("daily_loss_limit", &self.daily_loss_limit)
            .in_range(
                "max_orders_per_second",
                &self.max_orders_per_second,
                &1,
                &1000,
            )
            .in_range(
                "max_orders_per_minute",
                &self.max_orders_per_minute,
                &1,
                &10000,
            );

        if self.auto_reduce_enabled {
            validator.in_range(
                "auto_reduce_percent",
                &self.auto_reduce_percent,
                &1.0,
                &100.0,
            );
        }
    }

    fn apply_env_overrides(&mut self, prefix: &str) {
        EnvOverride::apply_number(
            &format!("{prefix}_MAX_POSITION_VALUE"),
            &mut self.max_position_value,
        );
        EnvOverride::apply_number(
            &format!("{prefix}_MAX_TOTAL_POSITION"),
            &mut self.max_total_position,
        );
        EnvOverride::apply_number(
            &format!("{prefix}_DAILY_LOSS_LIMIT"),
            &mut self.daily_loss_limit,
        );
        EnvOverride::apply_number(
            &format!("{prefix}_MAX_ORDERS_PER_SECOND"),
            &mut self.max_orders_per_second,
        );
        EnvOverride::apply_number(
            &format!("{prefix}_MAX_ORDERS_PER_MINUTE"),
            &mut self.max_orders_per_minute,
        );
        EnvOverride::apply_bool(
            &format!("{prefix}_AUTO_REDUCE_ENABLED"),
            &mut self.auto_reduce_enabled,
        );
        EnvOverride::apply_number(
            &format!("{prefix}_AUTO_REDUCE_PERCENT"),
            &mut self.auto_reduce_percent,
        );
    }
}

/// Logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error).
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Log format (json, pretty).
    #[serde(default = "default_log_format")]
    pub format: String,

    /// Log output directory.
    #[serde(default = "default_log_dir")]
    pub directory: String,

    /// Maximum log file size in bytes before rotation.
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,

    /// Maximum number of rotated log files to keep.
    #[serde(default = "default_max_files")]
    pub max_files: u32,

    /// Whether to also log to stdout.
    #[serde(default = "default_stdout_enabled")]
    pub stdout_enabled: bool,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "json".to_string()
}

fn default_log_dir() -> String {
    "./logs".to_string()
}

fn default_max_file_size() -> u64 {
    100 * 1024 * 1024 // 100 MB
}

fn default_max_files() -> u32 {
    10
}

fn default_stdout_enabled() -> bool {
    true
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
            directory: default_log_dir(),
            max_file_size: default_max_file_size(),
            max_files: default_max_files(),
            stdout_enabled: default_stdout_enabled(),
        }
    }
}

impl LoggingConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        EnvOverride::apply_string(&format!("{prefix}_LEVEL"), &mut self.level);
        EnvOverride::apply_string(&format!("{prefix}_FORMAT"), &mut self.format);
        EnvOverride::apply_string(&format!("{prefix}_DIRECTORY"), &mut self.directory);
        EnvOverride::apply_number(&format!("{prefix}_MAX_FILE_SIZE"), &mut self.max_file_size);
        EnvOverride::apply_number(&format!("{prefix}_MAX_FILES"), &mut self.max_files);
        EnvOverride::apply_bool(
            &format!("{prefix}_STDOUT_ENABLED"),
            &mut self.stdout_enabled,
        );
    }
}

/// Data storage configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Data directory path.
    #[serde(default = "default_data_dir")]
    pub data_directory: String,

    /// Whether to enable data compression.
    #[serde(default = "default_compression_enabled")]
    pub compression_enabled: bool,

    /// Cache size in megabytes.
    #[serde(default = "default_cache_size_mb")]
    pub cache_size_mb: u32,

    /// Whether to use memory-mapped files.
    #[serde(default = "default_mmap_enabled")]
    pub mmap_enabled: bool,
}

fn default_data_dir() -> String {
    "./data".to_string()
}

fn default_compression_enabled() -> bool {
    true
}

fn default_cache_size_mb() -> u32 {
    512
}

fn default_mmap_enabled() -> bool {
    true
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_directory: default_data_dir(),
            compression_enabled: default_compression_enabled(),
            cache_size_mb: default_cache_size_mb(),
            mmap_enabled: default_mmap_enabled(),
        }
    }
}

impl StorageConfig {
    fn apply_env_overrides(&mut self, prefix: &str) {
        EnvOverride::apply_string(
            &format!("{prefix}_DATA_DIRECTORY"),
            &mut self.data_directory,
        );
        EnvOverride::apply_bool(
            &format!("{prefix}_COMPRESSION_ENABLED"),
            &mut self.compression_enabled,
        );
        EnvOverride::apply_number(&format!("{prefix}_CACHE_SIZE_MB"), &mut self.cache_size_mb);
        EnvOverride::apply_bool(&format!("{prefix}_MMAP_ENABLED"), &mut self.mmap_enabled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigFormat;
    use crate::config::ConfigLoader;

    #[test]
    fn test_default_config() {
        let config = ZephyrConfig::default();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
        assert!(config.exchanges.is_empty());
    }

    #[test]
    fn test_config_validation_success() {
        let config = ZephyrConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_invalid_port() {
        let mut config = ZephyrConfig::default();
        config.server.port = 0;
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_config_yaml_roundtrip() {
        let mut config = ZephyrConfig::default();
        config.exchanges.insert(
            "binance".to_string(),
            ExchangeConfig {
                api_key: Some("test_key".to_string()),
                testnet: true,
                ..Default::default()
            },
        );

        let yaml = ConfigLoader::serialize(&config, ConfigFormat::Yaml).unwrap();
        let loader = ConfigLoader::new();
        let parsed: ZephyrConfig = loader.load_str(&yaml, ConfigFormat::Yaml).unwrap();

        assert_eq!(config.server.host, parsed.server.host);
        assert_eq!(config.server.port, parsed.server.port);
        assert!(parsed.exchanges.contains_key("binance"));
    }

    #[test]
    fn test_config_toml_roundtrip() {
        let config = ZephyrConfig::default();

        let toml = ConfigLoader::serialize(&config, ConfigFormat::Toml).unwrap();
        let loader = ConfigLoader::new();
        let parsed: ZephyrConfig = loader.load_str(&toml, ConfigFormat::Toml).unwrap();

        assert_eq!(config.server.host, parsed.server.host);
        assert_eq!(config.server.port, parsed.server.port);
    }

    #[test]
    fn test_exchange_config_validation() {
        let mut ctx = ValidationContext::new();

        // Enabled exchange without API key should fail
        let exchange = ExchangeConfig {
            enabled: true,
            api_key: None,
            ..Default::default()
        };
        exchange.validate_with_context(&mut ctx);
        assert!(!ctx.is_valid());
    }

    #[test]
    fn test_risk_config_validation() {
        let mut ctx = ValidationContext::new();

        let risk = RiskConfig {
            max_position_value: -100.0, // Invalid: negative
            ..Default::default()
        };
        risk.validate_with_context(&mut ctx);
        assert!(!ctx.is_valid());
    }

    #[test]
    fn test_server_tls_validation() {
        let mut ctx = ValidationContext::new();

        // TLS enabled without cert path should fail
        let server = ServerConfig {
            tls_enabled: true,
            tls_cert_path: None,
            tls_key_path: None,
            ..Default::default()
        };
        server.validate_with_context(&mut ctx);
        assert!(!ctx.is_valid());
    }
}
