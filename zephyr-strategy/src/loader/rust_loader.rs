//! Rust strategy loader using compile-time registry.
//!
//! This module provides the `StrategyBuilder` trait and registry for
//! automatically discovering and loading Rust strategies.

use crate::r#trait::Strategy;
use anyhow::{Result, anyhow};

/// Factory trait for creating Rust strategies.
///
/// Implement this trait to make a strategy discoverable by the loader.
/// Use the `register_strategy!` macro to register the builder.
///
/// # Example
///
/// ```ignore
/// use zephyr_strategy::loader::StrategyBuilder;
/// use zephyr_strategy::r#trait::Strategy;
///
/// #[derive(Default)]
/// struct DualThrustBuilder;
///
/// impl StrategyBuilder for DualThrustBuilder {
///     fn name(&self) -> &'static str {
///         "DualThrust"
///     }
///
///     fn build(&self, config: serde_json::Value) -> anyhow::Result<Box<dyn Strategy>> {
///         let params: DualThrustParams = serde_json::from_value(config)?;
///         Ok(Box::new(DualThrust::new(params)))
///     }
/// }
/// ```
pub trait StrategyBuilder: Send + Sync + 'static {
    /// Returns the unique name of this strategy type.
    ///
    /// This name is used to look up the builder in the registry.
    fn name(&self) -> &'static str;

    /// Creates a new strategy instance from configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - JSON configuration for the strategy
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid or strategy
    /// creation fails.
    fn build(&self, config: serde_json::Value) -> Result<Box<dyn Strategy>>;
}

// Collect all registered strategy builders using static references
inventory::collect!(&'static dyn StrategyBuilder);

/// Loads a Rust strategy by name from the registry.
///
/// Searches the compile-time registry for a builder with the given name
/// and uses it to create a strategy instance.
///
/// # Arguments
///
/// * `name` - The strategy name to look up
/// * `config` - JSON configuration to pass to the builder
///
/// # Errors
///
/// Returns an error if:
/// - No strategy with the given name is found in the registry
/// - The builder fails to create the strategy
///
/// # Example
///
/// ```ignore
/// use zephyr_strategy::loader::load_rust_strategy;
/// use serde_json::json;
///
/// let strategy = load_rust_strategy("DualThrust", json!({
///     "n": 20,
///     "k1": 0.5,
///     "k2": 0.5,
/// }))?;
/// ```
pub fn load_rust_strategy(name: &str, config: serde_json::Value) -> Result<Box<dyn Strategy>> {
    for builder in inventory::iter::<&'static dyn StrategyBuilder> {
        if builder.name() == name {
            return builder.build(config);
        }
    }

    // Collect available strategy names for error message
    let available: Vec<&str> = inventory::iter::<&'static dyn StrategyBuilder>()
        .map(|b| b.name())
        .collect();

    if available.is_empty() {
        Err(anyhow!(
            "Rust strategy '{name}' not found in registry. No strategies are registered."
        ))
    } else {
        Err(anyhow!(
            "Rust strategy '{}' not found in registry. Available strategies: {}",
            name,
            available.join(", ")
        ))
    }
}

/// Returns a list of all registered Rust strategy names.
#[must_use]
pub fn list_rust_strategies() -> Vec<&'static str> {
    inventory::iter::<&'static dyn StrategyBuilder>()
        .map(|b| b.name())
        .collect()
}

/// Returns true if a Rust strategy with the given name is registered.
#[must_use]
pub fn is_rust_strategy_registered(name: &str) -> bool {
    inventory::iter::<&'static dyn StrategyBuilder>().any(|b| b.name() == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_nonexistent_strategy() {
        let result = load_rust_strategy("NonExistent", serde_json::json!({}));
        assert!(result.is_err());
        let err = format!("{}", result.err().unwrap());
        assert!(err.contains("NonExistent"));
        assert!(err.contains("not found"));
    }

    #[test]
    fn test_list_strategies() {
        // This will return an empty list in tests unless strategies are registered
        let strategies = list_rust_strategies();
        // Just verify it doesn't panic and returns a valid Vec
        let _ = strategies;
    }

    #[test]
    fn test_is_registered() {
        assert!(!is_rust_strategy_registered("NonExistent"));
    }
}
