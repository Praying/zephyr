//! Plugin loader for dynamic plugin loading.
//!
//! Provides functionality to load strategy and adapter plugins
//! from shared libraries or built-in implementations.

#![allow(
    clippy::unused_self,
    clippy::unnecessary_wraps,
    clippy::map_unwrap_or,
    clippy::doc_markdown,
    clippy::unnecessary_map_or
)]

use tracing::{error, info};

use crate::config::{AdapterPluginConfig, PluginConfig, StrategyLanguage, StrategyPluginConfig};
use crate::plugin::registry::{AdapterPluginEntry, PluginRegistry, StrategyPluginEntry};
use zephyr_strategy::loader::{StrategyConfig, StrategyType, load_strategy};

/// Plugin API version for compatibility checking.
pub const PLUGIN_API_VERSION: &str = "1.0";

/// Plugin loader for loading strategy and adapter plugins.
///
/// Supports loading plugins from:
/// - Built-in implementations
/// - Shared libraries (future)
/// - Python scripts via PyO3 (future)
#[derive(Debug)]
pub struct PluginLoader {
    /// Plugin configuration.
    config: PluginConfig,
}

impl PluginLoader {
    /// Creates a new plugin loader with the given configuration.
    #[must_use]
    pub fn new(config: PluginConfig) -> Self {
        Self { config }
    }

    /// Loads all configured strategy plugins into the registry.
    ///
    /// # Errors
    ///
    /// Returns an error if plugin loading fails.
    pub fn load_strategies(&self, registry: &mut PluginRegistry) -> Result<(), PluginError> {
        for strategy_config in &self.config.strategies {
            match self.load_strategy(strategy_config) {
                Ok(entry) => {
                    registry.register_strategy(&entry.config.name, entry.config.clone());
                    registry.mark_strategy_loaded(&entry.config.name, true);
                    info!("Loaded strategy plugin: {}", entry.config.name);
                }
                Err(e) => {
                    error!(
                        "Failed to load strategy plugin {}: {}",
                        strategy_config.name, e
                    );
                    // Continue loading other plugins
                }
            }
        }

        Ok(())
    }

    /// Loads a single strategy plugin.
    fn load_strategy(
        &self,
        config: &StrategyPluginConfig,
    ) -> Result<StrategyPluginEntry, PluginError> {
        let strategy_config = StrategyConfig {
            name: config.name.clone(),
            strategy_type: match config.strategy_type {
                StrategyLanguage::Rust => StrategyType::Rust,
                StrategyLanguage::Python => StrategyType::Python,
            },
            class: config.class.clone(),
            path: config.path.clone(),
            params: config.params.clone(),
        };

        load_strategy(&strategy_config).map_err(|e| PluginError::LoadError {
            name: config.name.clone(),
            reason: e.to_string(),
        })?;

        let entry = StrategyPluginEntry {
            config: config.clone(),
            metadata: crate::plugin::registry::PluginMetadata {
                name: config.name.clone(),
                version: "0.1.0".to_string(),
                description: format!("{:?} strategy", config.strategy_type),
                author: None,
                api_version: PLUGIN_API_VERSION.to_string(),
            },
            loaded: true,
            running: false,
        };

        Ok(entry)
    }

    /// Loads all configured adapter plugins into the registry.
    ///
    /// # Errors
    ///
    /// Returns an error if plugin loading fails.
    pub fn load_adapters(&self, registry: &mut PluginRegistry) -> Result<(), PluginError> {
        for adapter_config in &self.config.adapters {
            if !adapter_config.enabled {
                continue;
            }

            match self.load_adapter(adapter_config) {
                Ok(entry) => {
                    registry.register_adapter(&entry.config.name, entry.config.clone());
                    registry.mark_adapter_loaded(&entry.config.name, true);
                    info!("Loaded adapter plugin: {}", entry.config.name);
                }
                Err(e) => {
                    error!(
                        "Failed to load adapter plugin {}: {}",
                        adapter_config.name, e
                    );
                    // Continue loading other plugins
                }
            }
        }

        Ok(())
    }

    /// Loads a single adapter plugin.
    fn load_adapter(
        &self,
        config: &AdapterPluginConfig,
    ) -> Result<AdapterPluginEntry, PluginError> {
        // For now, we support built-in adapters (binance, okx, bitget, hyperliquid)
        // Future: Load from shared library

        let supported_adapters = ["binance", "okx", "bitget", "hyperliquid"];

        if !supported_adapters.contains(&config.name.to_lowercase().as_str()) {
            return Err(PluginError::UnsupportedPlugin {
                name: config.name.clone(),
                reason: format!(
                    "Unknown adapter. Supported: {}",
                    supported_adapters.join(", ")
                ),
            });
        }

        let entry = AdapterPluginEntry {
            config: config.clone(),
            metadata: crate::plugin::registry::PluginMetadata {
                name: config.name.clone(),
                version: "0.1.0".to_string(),
                description: format!("{} exchange adapter", config.name),
                author: None,
                api_version: PLUGIN_API_VERSION.to_string(),
            },
            loaded: true,
            connected: false,
        };

        Ok(entry)
    }

    #[must_use]
    pub fn is_compatible(plugin_api_version: &str) -> bool {
        plugin_api_version == PLUGIN_API_VERSION
    }
}

/// Plugin loading error.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// I/O error.
    #[error("I/O error at {path}: {reason}")]
    IoError {
        /// Path where error occurred.
        path: String,
        /// Error reason.
        reason: String,
    },

    /// Plugin not found.
    #[error("Plugin not found: {name}")]
    NotFound {
        /// Plugin name.
        name: String,
    },

    /// Unsupported plugin.
    #[error("Unsupported plugin {name}: {reason}")]
    UnsupportedPlugin {
        /// Plugin name.
        name: String,
        /// Reason.
        reason: String,
    },

    /// Plugin load error.
    #[error("Failed to load plugin {name}: {reason}")]
    LoadError {
        /// Plugin name.
        name: String,
        /// Error reason.
        reason: String,
    },

    /// Version incompatibility.
    #[error("Plugin {name} has incompatible API version {version} (expected {expected})")]
    IncompatibleVersion {
        /// Plugin name.
        name: String,
        /// Plugin version.
        version: String,
        /// Expected version.
        expected: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_compatible() {
        assert!(PluginLoader::is_compatible(PLUGIN_API_VERSION));
        assert!(!PluginLoader::is_compatible("0.9"));
        assert!(!PluginLoader::is_compatible("2.0"));
    }

    #[test]
    fn test_load_builtin_adapter() {
        let config = PluginConfig::default();
        let loader = PluginLoader::new(config);

        let adapter_config = AdapterPluginConfig {
            name: "binance".to_string(),
            path: None,
            enabled: true,
            config: serde_json::Value::Null,
        };

        let result = loader.load_adapter(&adapter_config);
        assert!(result.is_ok());

        let entry = result.unwrap();
        assert_eq!(entry.config.name, "binance");
        assert!(entry.loaded);
    }

    #[test]
    fn test_load_unsupported_adapter() {
        let config = PluginConfig::default();
        let loader = PluginLoader::new(config);

        let adapter_config = AdapterPluginConfig {
            name: "unknown_exchange".to_string(),
            path: None,
            enabled: true,
            config: serde_json::Value::Null,
        };

        let result = loader.load_adapter(&adapter_config);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PluginError::UnsupportedPlugin { .. }
        ));
    }

    #[test]
    fn test_load_strategy() {
        let config = PluginConfig::default();
        let loader = PluginLoader::new(config);

        let strategy_config = StrategyPluginConfig {
            name: "test_strategy".to_string(),
            strategy_type: crate::config::StrategyLanguage::Rust,
            class: "DualThrust".to_string(),
            path: None,
            auto_start: false,
            params: serde_json::json!({
                "name": "test_strategy",
                "symbol": "BTC-USDT"
            }),
        };

        let result = loader.load_strategy(&strategy_config);
        assert!(result.is_ok());

        let entry = result.unwrap();
        assert_eq!(entry.config.name, "test_strategy");
        assert!(entry.loaded);
    }

    #[test]
    fn test_plugin_error_display() {
        let err = PluginError::NotFound {
            name: "test".to_string(),
        };
        assert_eq!(err.to_string(), "Plugin not found: test");

        let err = PluginError::IncompatibleVersion {
            name: "test".to_string(),
            version: "0.9".to_string(),
            expected: "1.0".to_string(),
        };
        assert!(err.to_string().contains("incompatible API version"));
    }
}
