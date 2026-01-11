//! Execution unit factory for creating execution algorithm instances.

#![allow(clippy::disallowed_types)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::unnecessary_literal_bound)]

use std::collections::HashMap;
use std::sync::Arc;

use super::traits::ExecuteUnit;
use super::{
    MinImpactConfig, MinImpactExecutor, TwapConfig, TwapExecutor, VwapConfig, VwapExecutor,
};

/// Factory trait for creating execution units.
///
/// Implementations provide a way to create execution unit instances
/// by name, enabling dynamic loading and configuration of execution
/// algorithms.
///
/// # Example
///
/// ```ignore
/// let factory = DefaultExecuteUnitFactory::new();
/// let twap = factory.create("twap").unwrap();
/// let vwap = factory.create("vwap").unwrap();
/// ```
pub trait ExecuteUnitFactory: Send + Sync {
    /// Returns the factory name.
    fn name(&self) -> &str;

    /// Creates an execution unit by type name.
    ///
    /// # Arguments
    ///
    /// * `unit_type` - The type of execution unit to create
    ///
    /// # Returns
    ///
    /// Returns `Some(unit)` if the type is supported, `None` otherwise.
    fn create(&self, unit_type: &str) -> Option<Box<dyn ExecuteUnit>>;

    /// Lists all supported execution unit types.
    fn list_units(&self) -> Vec<&str>;

    /// Returns whether a unit type is supported.
    fn supports(&self, unit_type: &str) -> bool {
        self.list_units().contains(&unit_type)
    }
}

/// Default execution unit factory.
///
/// Provides built-in execution algorithms:
/// - `twap` - Time-Weighted Average Price
/// - `vwap` - Volume-Weighted Average Price
/// - `min_impact` - Minimum market impact
#[derive(Debug)]
pub struct DefaultExecuteUnitFactory {
    /// TWAP configuration.
    twap_config: TwapConfig,
    /// VWAP configuration.
    vwap_config: VwapConfig,
    /// MinImpact configuration.
    min_impact_config: MinImpactConfig,
    /// Custom configurations by unit type.
    custom_configs: HashMap<String, ExecuteUnitConfig>,
}

/// Configuration for execution units.
#[derive(Debug, Clone)]
pub enum ExecuteUnitConfig {
    /// TWAP configuration.
    Twap(TwapConfig),
    /// VWAP configuration.
    Vwap(VwapConfig),
    /// MinImpact configuration.
    MinImpact(MinImpactConfig),
}

impl Default for DefaultExecuteUnitFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultExecuteUnitFactory {
    /// Creates a new default factory with default configurations.
    #[must_use]
    pub fn new() -> Self {
        Self {
            twap_config: TwapConfig::default(),
            vwap_config: VwapConfig::default(),
            min_impact_config: MinImpactConfig::default(),
            custom_configs: HashMap::new(),
        }
    }

    /// Creates a factory with custom TWAP configuration.
    #[must_use]
    pub fn with_twap_config(mut self, config: TwapConfig) -> Self {
        self.twap_config = config;
        self
    }

    /// Creates a factory with custom VWAP configuration.
    #[must_use]
    pub fn with_vwap_config(mut self, config: VwapConfig) -> Self {
        self.vwap_config = config;
        self
    }

    /// Creates a factory with custom MinImpact configuration.
    #[must_use]
    pub fn with_min_impact_config(mut self, config: MinImpactConfig) -> Self {
        self.min_impact_config = config;
        self
    }

    /// Registers a custom configuration for a unit type.
    pub fn register_config(&mut self, unit_type: impl Into<String>, config: ExecuteUnitConfig) {
        self.custom_configs.insert(unit_type.into(), config);
    }

    /// Gets the TWAP configuration.
    #[must_use]
    pub fn twap_config(&self) -> &TwapConfig {
        &self.twap_config
    }

    /// Gets the VWAP configuration.
    #[must_use]
    pub fn vwap_config(&self) -> &VwapConfig {
        &self.vwap_config
    }

    /// Gets the MinImpact configuration.
    #[must_use]
    pub fn min_impact_config(&self) -> &MinImpactConfig {
        &self.min_impact_config
    }
}

impl ExecuteUnitFactory for DefaultExecuteUnitFactory {
    fn name(&self) -> &str {
        "default"
    }

    fn create(&self, unit_type: &str) -> Option<Box<dyn ExecuteUnit>> {
        // Check for custom configuration first
        if let Some(config) = self.custom_configs.get(unit_type) {
            return match config {
                ExecuteUnitConfig::Twap(cfg) => Some(Box::new(TwapExecutor::new(cfg.clone()))),
                ExecuteUnitConfig::Vwap(cfg) => Some(Box::new(VwapExecutor::new(cfg.clone()))),
                ExecuteUnitConfig::MinImpact(cfg) => {
                    Some(Box::new(MinImpactExecutor::new(cfg.clone())))
                }
            };
        }

        // Use default configurations
        match unit_type.to_lowercase().as_str() {
            "twap" => Some(Box::new(TwapExecutor::new(self.twap_config.clone()))),
            "vwap" => Some(Box::new(VwapExecutor::new(self.vwap_config.clone()))),
            "min_impact" | "minimpact" => Some(Box::new(MinImpactExecutor::new(
                self.min_impact_config.clone(),
            ))),
            _ => None,
        }
    }

    fn list_units(&self) -> Vec<&str> {
        vec!["twap", "vwap", "min_impact"]
    }
}

/// Registry for execution unit factories.
///
/// Allows registering multiple factories and creating units from any of them.
#[derive(Default)]
#[allow(dead_code)]
pub struct ExecuteUnitRegistry {
    factories: HashMap<String, Arc<dyn ExecuteUnitFactory>>,
}

#[allow(dead_code)]
impl ExecuteUnitRegistry {
    /// Creates a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }

    /// Creates a registry with the default factory.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(DefaultExecuteUnitFactory::new()));
        registry
    }

    /// Registers a factory.
    pub fn register(&mut self, factory: Arc<dyn ExecuteUnitFactory>) {
        self.factories.insert(factory.name().to_string(), factory);
    }

    /// Creates an execution unit.
    ///
    /// # Arguments
    ///
    /// * `factory_name` - Name of the factory to use (or None for any)
    /// * `unit_type` - Type of execution unit to create
    ///
    /// # Returns
    ///
    /// Returns `Some(unit)` if creation succeeds, `None` otherwise.
    pub fn create(
        &self,
        factory_name: Option<&str>,
        unit_type: &str,
    ) -> Option<Box<dyn ExecuteUnit>> {
        if let Some(name) = factory_name {
            // Use specific factory
            self.factories.get(name).and_then(|f| f.create(unit_type))
        } else {
            // Try all factories
            for factory in self.factories.values() {
                if let Some(unit) = factory.create(unit_type) {
                    return Some(unit);
                }
            }
            None
        }
    }

    /// Lists all available unit types across all factories.
    pub fn list_all_units(&self) -> Vec<(&str, &str)> {
        let mut units = Vec::new();
        for (factory_name, factory) in &self.factories {
            for unit_type in factory.list_units() {
                units.push((factory_name.as_str(), unit_type));
            }
        }
        units
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_factory_create_twap() {
        let factory = DefaultExecuteUnitFactory::new();
        let unit = factory.create("twap");
        assert!(unit.is_some());
        assert_eq!(unit.unwrap().name(), "twap");
    }

    #[test]
    fn test_default_factory_create_vwap() {
        let factory = DefaultExecuteUnitFactory::new();
        let unit = factory.create("vwap");
        assert!(unit.is_some());
        assert_eq!(unit.unwrap().name(), "vwap");
    }

    #[test]
    fn test_default_factory_create_min_impact() {
        let factory = DefaultExecuteUnitFactory::new();
        let unit = factory.create("min_impact");
        assert!(unit.is_some());
        assert_eq!(unit.unwrap().name(), "min_impact");
    }

    #[test]
    fn test_default_factory_create_unknown() {
        let factory = DefaultExecuteUnitFactory::new();
        let unit = factory.create("unknown");
        assert!(unit.is_none());
    }

    #[test]
    fn test_default_factory_list_units() {
        let factory = DefaultExecuteUnitFactory::new();
        let units = factory.list_units();
        assert!(units.contains(&"twap"));
        assert!(units.contains(&"vwap"));
        assert!(units.contains(&"min_impact"));
    }

    #[test]
    fn test_default_factory_supports() {
        let factory = DefaultExecuteUnitFactory::new();
        assert!(factory.supports("twap"));
        assert!(factory.supports("vwap"));
        assert!(!factory.supports("unknown"));
    }

    #[test]
    fn test_registry_with_defaults() {
        let registry = ExecuteUnitRegistry::with_defaults();
        let unit = registry.create(None, "twap");
        assert!(unit.is_some());
    }

    #[test]
    fn test_registry_create_with_factory_name() {
        let registry = ExecuteUnitRegistry::with_defaults();
        let unit = registry.create(Some("default"), "vwap");
        assert!(unit.is_some());
    }

    #[test]
    fn test_registry_list_all_units() {
        let registry = ExecuteUnitRegistry::with_defaults();
        let units = registry.list_all_units();
        assert!(!units.is_empty());
    }
}
