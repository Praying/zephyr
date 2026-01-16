//! Server configuration module.
//!
//! Provides configuration structures for the Zephyr server,
//! including component initialization settings.

#![allow(clippy::collapsible_if, clippy::doc_markdown)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use zephyr_core::config::ZephyrConfig;

/// Server configuration.
///
/// Contains all settings needed to start and run the Zephyr server.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerConfig {
    /// Base Zephyr configuration.
    #[serde(flatten)]
    pub zephyr: ZephyrConfig,

    /// Plugin configuration.
    #[serde(default)]
    pub plugins: PluginConfig,

    /// Shutdown configuration.
    #[serde(default)]
    pub shutdown: ShutdownConfig,
}

impl ServerConfig {
    /// Creates a new server configuration with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies environment variable overrides.
    pub fn apply_env_overrides(&mut self) {
        self.zephyr.apply_env_overrides();
        self.plugins.apply_env_overrides();
        self.shutdown.apply_env_overrides();
    }

    /// Validates the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails.
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        use zephyr_core::config::Validatable;

        // Validate base config
        self.zephyr
            .validate()
            .map_err(|e| ConfigValidationError::InvalidConfig(e.to_string()))?;

        for strategy in &self.plugins.strategies {
            if strategy.class.is_empty() {
                return Err(ConfigValidationError::InvalidConfig(
                    "plugins.strategies.class cannot be empty".to_string(),
                ));
            }

            if strategy.strategy_type == StrategyLanguage::Python && strategy.path.is_none() {
                return Err(ConfigValidationError::InvalidConfig(format!(
                    "plugins.strategies.path missing for python strategy {}",
                    strategy.name
                )));
            }
        }

        Ok(())
    }
}

/// Plugin configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// List of strategies to load on startup.
    #[serde(default)]
    pub strategies: Vec<StrategyPluginConfig>,

    /// List of exchange adapters to load on startup.
    #[serde(default)]
    pub adapters: Vec<AdapterPluginConfig>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            strategies: Vec::new(),
            adapters: Vec::new(),
        }
    }
}

impl PluginConfig {
    pub fn apply_env_overrides(&mut self) {}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPluginConfig {
    pub name: String,

    #[serde(rename = "type")]
    pub strategy_type: StrategyLanguage,

    pub class: String,

    #[serde(default)]
    pub path: Option<PathBuf>,

    #[serde(default)]
    pub auto_start: bool,

    #[serde(default, rename = "params")]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StrategyLanguage {
    Rust,
    Python,
}

/// Exchange adapter plugin configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterPluginConfig {
    /// Plugin name/identifier (e.g., "binance", "okx").
    pub name: String,

    #[serde(default)]
    pub path: Option<PathBuf>,

    /// Whether to enable this adapter.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Adapter-specific configuration.
    #[serde(default)]
    pub config: serde_json::Value,
}

fn default_enabled() -> bool {
    true
}

/// Shutdown configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownConfig {
    /// Timeout for graceful shutdown in seconds.
    #[serde(default = "default_shutdown_timeout_secs")]
    pub timeout_secs: u64,

    /// Whether to cancel pending orders on shutdown.
    #[serde(default = "default_cancel_orders")]
    pub cancel_pending_orders: bool,

    /// Whether to save state on shutdown.
    #[serde(default = "default_save_state")]
    pub save_state: bool,

    /// Timeout for order cancellation in seconds.
    #[serde(default = "default_cancel_timeout_secs")]
    pub cancel_timeout_secs: u64,
}

fn default_shutdown_timeout_secs() -> u64 {
    30
}

fn default_cancel_orders() -> bool {
    true
}

fn default_save_state() -> bool {
    true
}

fn default_cancel_timeout_secs() -> u64 {
    10
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            timeout_secs: default_shutdown_timeout_secs(),
            cancel_pending_orders: default_cancel_orders(),
            save_state: default_save_state(),
            cancel_timeout_secs: default_cancel_timeout_secs(),
        }
    }
}

impl ShutdownConfig {
    /// Applies environment variable overrides.
    pub fn apply_env_overrides(&mut self) {
        if let Ok(val) = std::env::var("ZEPHYR_SHUTDOWN_TIMEOUT") {
            if let Ok(secs) = val.parse() {
                self.timeout_secs = secs;
            }
        }
        if let Ok(val) = std::env::var("ZEPHYR_SHUTDOWN_CANCEL_ORDERS") {
            self.cancel_pending_orders = val.parse().unwrap_or(true);
        }
    }

    /// Returns the shutdown timeout as a Duration.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }

    /// Returns the cancel timeout as a Duration.
    #[must_use]
    pub fn cancel_timeout(&self) -> Duration {
        Duration::from_secs(self.cancel_timeout_secs)
    }
}

/// Configuration validation error.
#[derive(Debug, thiserror::Error)]
pub enum ConfigValidationError {
    /// Invalid configuration value.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Invalid path.
    #[error("Invalid path for {field}: {path}")]
    InvalidPath {
        /// Field name.
        field: String,
        /// Path value.
        path: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();
        assert!(config.shutdown.cancel_pending_orders);
    }

    #[test]
    fn test_shutdown_config_timeout() {
        let config = ShutdownConfig::default();
        assert_eq!(config.timeout(), Duration::from_secs(30));
        assert_eq!(config.cancel_timeout(), Duration::from_secs(10));
    }

    #[test]
    fn test_config_serialization() {
        let config = ServerConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ServerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.shutdown.timeout_secs, parsed.shutdown.timeout_secs);
    }
}
