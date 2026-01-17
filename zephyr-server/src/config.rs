//! Server configuration module.
//!
//! Provides configuration structures for the Zephyr server,
//! including component initialization settings.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

use zephyr_core::config::ZephyrConfig;

/// Server configuration.
///
/// Contains all settings needed to start and run the Zephyr server.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            zephyr: ZephyrConfig::default(),
            plugins: PluginConfig::default(),
            shutdown: ShutdownConfig::default(),
        }
    }
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

        // Validate plugin paths exist if specified
        if let Some(ref path) = self.plugins.strategy_dir {
            if !path.exists() {
                return Err(ConfigValidationError::InvalidPath {
                    field: "plugins.strategy_dir".to_string(),
                    path: path.display().to_string(),
                });
            }
        }

        if let Some(ref path) = self.plugins.adapter_dir {
            if !path.exists() {
                return Err(ConfigValidationError::InvalidPath {
                    field: "plugins.adapter_dir".to_string(),
                    path: path.display().to_string(),
                });
            }
        }

        Ok(())
    }
}

/// Plugin configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Directory containing strategy plugins.
    #[serde(default)]
    pub strategy_dir: Option<PathBuf>,

    /// Directory containing exchange adapter plugins.
    #[serde(default)]
    pub adapter_dir: Option<PathBuf>,

    /// Whether to enable hot-reloading of plugins.
    #[serde(default)]
    pub hot_reload: bool,

    /// Plugin scan interval in seconds (for hot-reload).
    #[serde(default = "default_scan_interval_secs")]
    pub scan_interval_secs: u64,

    /// List of strategies to load on startup.
    #[serde(default)]
    pub strategies: Vec<StrategyPluginConfig>,

    /// List of exchange adapters to load on startup.
    #[serde(default)]
    pub adapters: Vec<AdapterPluginConfig>,
}

fn default_scan_interval_secs() -> u64 {
    30
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            strategy_dir: None,
            adapter_dir: None,
            hot_reload: false,
            scan_interval_secs: default_scan_interval_secs(),
            strategies: Vec::new(),
            adapters: Vec::new(),
        }
    }
}

impl PluginConfig {
    /// Applies environment variable overrides.
    pub fn apply_env_overrides(&mut self) {
        if let Ok(val) = std::env::var("ZEPHYR_PLUGIN_STRATEGY_DIR") {
            self.strategy_dir = Some(PathBuf::from(val));
        }
        if let Ok(val) = std::env::var("ZEPHYR_PLUGIN_ADAPTER_DIR") {
            self.adapter_dir = Some(PathBuf::from(val));
        }
        if let Ok(val) = std::env::var("ZEPHYR_PLUGIN_HOT_RELOAD") {
            self.hot_reload = val.parse().unwrap_or(false);
        }
    }

    /// Returns the scan interval as a Duration.
    #[must_use]
    pub fn scan_interval(&self) -> Duration {
        Duration::from_secs(self.scan_interval_secs)
    }
}

/// Strategy plugin configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPluginConfig {
    /// Plugin name/identifier.
    pub name: String,

    /// Path to the plugin library (optional if in strategy_dir).
    #[serde(default)]
    pub path: Option<PathBuf>,

    /// Strategy type (cta, hft, uft, sel).
    #[serde(default = "default_strategy_type")]
    pub strategy_type: zephyr_core::types::StrategyType,

    /// Whether to auto-start this strategy.
    #[serde(default)]
    pub auto_start: bool,

    /// Strategy-specific configuration.
    #[serde(default)]
    pub config: serde_json::Value,
}

fn default_strategy_type() -> zephyr_core::types::StrategyType {
    zephyr_core::types::StrategyType::Cta
}

/// Exchange adapter plugin configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterPluginConfig {
    /// Plugin name/identifier (e.g., "binance", "okx").
    pub name: String,

    /// Path to the plugin library (optional if in adapter_dir).
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
        assert!(!config.plugins.hot_reload);
        assert!(config.shutdown.cancel_pending_orders);
    }

    #[test]
    fn test_plugin_config_scan_interval() {
        let config = PluginConfig::default();
        assert_eq!(config.scan_interval(), Duration::from_secs(30));
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
