//! Plugin registry for managing loaded plugins.

use dashmap::DashMap;
use tracing::info;

use crate::config::{AdapterPluginConfig, StrategyPluginConfig};

/// Plugin metadata.
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    /// Plugin name.
    pub name: String,
    /// Plugin version.
    pub version: String,
    /// Plugin description.
    pub description: String,
    /// Plugin author.
    pub author: Option<String>,
    /// API version this plugin targets.
    pub api_version: String,
}

impl Default for PluginMetadata {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: "0.1.0".to_string(),
            description: String::new(),
            author: None,
            api_version: "1.0".to_string(),
        }
    }
}

/// Strategy plugin entry.
#[derive(Debug, Clone)]
pub struct StrategyPluginEntry {
    /// Plugin configuration.
    pub config: StrategyPluginConfig,
    /// Plugin metadata.
    pub metadata: PluginMetadata,
    /// Whether the plugin is loaded.
    pub loaded: bool,
    /// Whether the plugin is running.
    pub running: bool,
}

/// Adapter plugin entry.
#[derive(Debug, Clone)]
pub struct AdapterPluginEntry {
    /// Plugin configuration.
    pub config: AdapterPluginConfig,
    /// Plugin metadata.
    pub metadata: PluginMetadata,
    /// Whether the plugin is loaded.
    pub loaded: bool,
    /// Whether the plugin is connected.
    pub connected: bool,
}

/// Plugin registry for managing loaded plugins.
///
/// Provides thread-safe access to registered plugins.
#[derive(Debug, Default)]
pub struct PluginRegistry {
    /// Registered strategy plugins.
    strategies: DashMap<String, StrategyPluginEntry>,
    /// Registered adapter plugins.
    adapters: DashMap<String, AdapterPluginEntry>,
}

impl PluginRegistry {
    /// Creates a new plugin registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            strategies: DashMap::new(),
            adapters: DashMap::new(),
        }
    }

    /// Registers a strategy plugin.
    pub fn register_strategy(&mut self, name: &str, config: StrategyPluginConfig) {
        let entry = StrategyPluginEntry {
            config,
            metadata: PluginMetadata {
                name: name.to_string(),
                ..Default::default()
            },
            loaded: false,
            running: false,
        };
        self.strategies.insert(name.to_string(), entry);
        info!("Registered strategy plugin: {}", name);
    }

    /// Registers an adapter plugin.
    pub fn register_adapter(&mut self, name: &str, config: AdapterPluginConfig) {
        let entry = AdapterPluginEntry {
            config,
            metadata: PluginMetadata {
                name: name.to_string(),
                ..Default::default()
            },
            loaded: false,
            connected: false,
        };
        self.adapters.insert(name.to_string(), entry);
        info!("Registered adapter plugin: {}", name);
    }

    /// Unregisters a strategy plugin.
    pub fn unregister_strategy(&mut self, name: &str) -> Option<StrategyPluginEntry> {
        let entry = self.strategies.remove(name).map(|(_, v)| v);
        if entry.is_some() {
            info!("Unregistered strategy plugin: {}", name);
        }
        entry
    }

    /// Unregisters an adapter plugin.
    pub fn unregister_adapter(&mut self, name: &str) -> Option<AdapterPluginEntry> {
        let entry = self.adapters.remove(name).map(|(_, v)| v);
        if entry.is_some() {
            info!("Unregistered adapter plugin: {}", name);
        }
        entry
    }

    /// Gets a strategy plugin by name.
    #[must_use]
    pub fn get_strategy(&self, name: &str) -> Option<StrategyPluginEntry> {
        self.strategies.get(name).map(|e| e.clone())
    }

    /// Gets an adapter plugin by name.
    #[must_use]
    pub fn get_adapter(&self, name: &str) -> Option<AdapterPluginEntry> {
        self.adapters.get(name).map(|e| e.clone())
    }

    /// Returns the number of registered strategy plugins.
    #[must_use]
    pub fn strategy_count(&self) -> usize {
        self.strategies.len()
    }

    /// Returns the number of registered adapter plugins.
    #[must_use]
    pub fn adapter_count(&self) -> usize {
        self.adapters.len()
    }

    /// Returns all strategy plugin names.
    #[must_use]
    pub fn strategy_names(&self) -> Vec<String> {
        self.strategies.iter().map(|e| e.key().clone()).collect()
    }

    /// Returns all adapter plugin names.
    #[must_use]
    pub fn adapter_names(&self) -> Vec<String> {
        self.adapters.iter().map(|e| e.key().clone()).collect()
    }

    /// Marks a strategy as loaded.
    pub fn mark_strategy_loaded(&self, name: &str, loaded: bool) {
        if let Some(mut entry) = self.strategies.get_mut(name) {
            entry.loaded = loaded;
        }
    }

    /// Marks a strategy as running.
    pub fn mark_strategy_running(&self, name: &str, running: bool) {
        if let Some(mut entry) = self.strategies.get_mut(name) {
            entry.running = running;
        }
    }

    /// Marks an adapter as loaded.
    pub fn mark_adapter_loaded(&self, name: &str, loaded: bool) {
        if let Some(mut entry) = self.adapters.get_mut(name) {
            entry.loaded = loaded;
        }
    }

    /// Marks an adapter as connected.
    pub fn mark_adapter_connected(&self, name: &str, connected: bool) {
        if let Some(mut entry) = self.adapters.get_mut(name) {
            entry.connected = connected;
        }
    }

    /// Clears all registered plugins.
    pub fn clear(&mut self) {
        let strategy_count = self.strategies.len();
        let adapter_count = self.adapters.len();

        self.strategies.clear();
        self.adapters.clear();

        info!(
            "Cleared {} strategies and {} adapters from registry",
            strategy_count, adapter_count
        );
    }

    /// Returns strategies that should auto-start.
    #[must_use]
    pub fn auto_start_strategies(&self) -> Vec<String> {
        self.strategies
            .iter()
            .filter(|e| e.config.auto_start)
            .map(|e| e.key().clone())
            .collect()
    }

    /// Returns enabled adapters.
    #[must_use]
    pub fn enabled_adapters(&self) -> Vec<String> {
        self.adapters
            .iter()
            .filter(|e| e.config.enabled)
            .map(|e| e.key().clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_strategy_config(name: &str) -> StrategyPluginConfig {
        StrategyPluginConfig {
            name: name.to_string(),
            strategy_type: crate::config::StrategyLanguage::Rust,
            class: "DualThrust".to_string(),
            path: None,
            auto_start: false,
            params: serde_json::Value::Null,
        }
    }

    fn create_test_adapter_config(name: &str) -> AdapterPluginConfig {
        AdapterPluginConfig {
            name: name.to_string(),
            path: None,
            enabled: true,
            config: serde_json::Value::Null,
        }
    }

    #[test]
    fn test_registry_new() {
        let registry = PluginRegistry::new();
        assert_eq!(registry.strategy_count(), 0);
        assert_eq!(registry.adapter_count(), 0);
    }

    #[test]
    fn test_register_strategy() {
        let mut registry = PluginRegistry::new();
        let config = create_test_strategy_config("test_strategy");

        registry.register_strategy("test_strategy", config);

        assert_eq!(registry.strategy_count(), 1);
        assert!(registry.get_strategy("test_strategy").is_some());
    }

    #[test]
    fn test_register_adapter() {
        let mut registry = PluginRegistry::new();
        let config = create_test_adapter_config("binance");

        registry.register_adapter("binance", config);

        assert_eq!(registry.adapter_count(), 1);
        assert!(registry.get_adapter("binance").is_some());
    }

    #[test]
    fn test_unregister_strategy() {
        let mut registry = PluginRegistry::new();
        let config = create_test_strategy_config("test_strategy");

        registry.register_strategy("test_strategy", config);
        let entry = registry.unregister_strategy("test_strategy");

        assert!(entry.is_some());
        assert_eq!(registry.strategy_count(), 0);
    }

    #[test]
    fn test_strategy_names() {
        let mut registry = PluginRegistry::new();
        registry.register_strategy("strategy1", create_test_strategy_config("strategy1"));
        registry.register_strategy("strategy2", create_test_strategy_config("strategy2"));

        let names = registry.strategy_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"strategy1".to_string()));
        assert!(names.contains(&"strategy2".to_string()));
    }

    #[test]
    fn test_mark_strategy_loaded() {
        let mut registry = PluginRegistry::new();
        registry.register_strategy("test", create_test_strategy_config("test"));

        registry.mark_strategy_loaded("test", true);

        let entry = registry.get_strategy("test").unwrap();
        assert!(entry.loaded);
    }

    #[test]
    fn test_auto_start_strategies() {
        let mut registry = PluginRegistry::new();

        let mut config1 = create_test_strategy_config("auto_start");
        config1.auto_start = true;
        registry.register_strategy("auto_start", config1);

        let config2 = create_test_strategy_config("manual_start");
        registry.register_strategy("manual_start", config2);

        let auto_start = registry.auto_start_strategies();
        assert_eq!(auto_start.len(), 1);
        assert!(auto_start.contains(&"auto_start".to_string()));
    }

    #[test]
    fn test_enabled_adapters() {
        let mut registry = PluginRegistry::new();

        let config1 = create_test_adapter_config("enabled");
        registry.register_adapter("enabled", config1);

        let mut config2 = create_test_adapter_config("disabled");
        config2.enabled = false;
        registry.register_adapter("disabled", config2);

        let enabled = registry.enabled_adapters();
        assert_eq!(enabled.len(), 1);
        assert!(enabled.contains(&"enabled".to_string()));
    }

    #[test]
    fn test_clear() {
        let mut registry = PluginRegistry::new();
        registry.register_strategy("strategy", create_test_strategy_config("strategy"));
        registry.register_adapter("adapter", create_test_adapter_config("adapter"));

        registry.clear();

        assert_eq!(registry.strategy_count(), 0);
        assert_eq!(registry.adapter_count(), 0);
    }
}
