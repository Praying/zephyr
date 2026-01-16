//! Unified strategy loading interface.
//!
//! This module provides a single entry point for loading strategies
//! regardless of their implementation language (Rust or Python).

use crate::loader::config::{StrategyConfig, StrategyType};
use crate::loader::rust_loader::load_rust_strategy;
use crate::r#trait::Strategy;
use anyhow::Result;

/// Type alias for strategy loading results
pub type StrategyLoadResult = (String, Result<Box<dyn Strategy>, LoadError>);

/// Type alias for successfully loaded strategies
pub type LoadedStrategies = Vec<(String, Box<dyn Strategy>)>;

#[cfg(feature = "python")]
use crate::loader::config::PythonConfig;
#[cfg(feature = "python")]
use crate::loader::python_loader::PythonLoader;

/// Errors that can occur during strategy loading.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// Strategy not found in registry
    #[error("strategy '{name}' of type '{strategy_type}' not found")]
    NotFound {
        /// Name of the strategy that was not found
        name: String,
        /// Type of the strategy
        strategy_type: StrategyType,
    },

    /// Failed to build strategy from configuration
    #[error("failed to build strategy '{name}': {source}")]
    BuildError {
        /// Name of the strategy that failed to build
        name: String,
        /// The underlying error
        #[source]
        source: anyhow::Error,
    },

    /// Python strategies are not yet supported
    #[error("Python strategy loading is not yet implemented (strategy: '{0}')")]
    PythonNotSupported(String),

    /// Missing required configuration
    #[error("missing required configuration for strategy '{name}': {field}")]
    MissingConfig {
        /// Name of the strategy
        name: String,
        /// Missing field
        field: String,
    },
}

/// Load a strategy from configuration.
///
/// This function routes to the appropriate loader based on the strategy type:
/// - `rust` strategies are loaded from the compile-time registry
/// - `python` strategies are loaded from Python scripts (not yet implemented)
///
/// # Arguments
///
/// * `config` - Strategy configuration specifying type, class, and parameters
///
/// # Returns
///
/// A boxed strategy instance ready for execution.
///
/// # Errors
///
/// Returns an error if:
/// - The strategy type is not supported
/// - The strategy is not found in the registry
/// - The strategy fails to build from configuration
///
/// # Example
///
/// ```ignore
/// use zephyr_strategy::loader::{load_strategy, StrategyConfig, StrategyType};
/// use serde_json::json;
///
/// let config = StrategyConfig {
///     name: "my_dual_thrust".to_string(),
///     strategy_type: StrategyType::Rust,
///     class: "DualThrust".to_string(),
///     path: None,
///     params: json!({"n": 20, "k1": 0.5, "k2": 0.5}),
/// };
///
/// let strategy = load_strategy(&config)?;
/// ```
pub fn load_strategy(config: &StrategyConfig) -> Result<Box<dyn Strategy>, LoadError> {
    match config.strategy_type {
        StrategyType::Rust => load_rust_strategy_from_config(config),
        StrategyType::Python => load_python_strategy_from_config(config),
    }
}

/// Load a Rust strategy from configuration.
fn load_rust_strategy_from_config(config: &StrategyConfig) -> Result<Box<dyn Strategy>, LoadError> {
    load_rust_strategy(&config.class, config.params.clone()).map_err(|e| {
        // Check if it's a "not found" error
        let err_str = e.to_string();
        if err_str.contains("not found") {
            LoadError::NotFound {
                name: config.class.clone(),
                strategy_type: StrategyType::Rust,
            }
        } else {
            LoadError::BuildError {
                name: config.name.clone(),
                source: e,
            }
        }
    })
}

/// Load a Python strategy from configuration.
///
/// When the `python` feature is enabled, this uses `PythonLoader` to load
/// Python strategies from script files.
#[cfg(feature = "python")]
fn load_python_strategy_from_config(
    config: &StrategyConfig,
) -> Result<Box<dyn Strategy>, LoadError> {
    // Validate that path is provided
    let path = config
        .path
        .as_ref()
        .ok_or_else(|| LoadError::MissingConfig {
            name: config.name.clone(),
            field: "path".to_string(),
        })?;

    // Create a Python loader with default config
    // In a real application, this would use the PythonConfig from EngineConfig
    let loader =
        PythonLoader::new(&PythonConfig::default()).map_err(|e| LoadError::BuildError {
            name: config.name.clone(),
            source: anyhow::anyhow!("{}", e),
        })?;

    // Load the strategy
    loader
        .load(
            path.to_string_lossy().as_ref(),
            &config.class,
            config.params.clone(),
        )
        .map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("not found") || err_str.contains("ClassNotFound") {
                LoadError::NotFound {
                    name: config.class.clone(),
                    strategy_type: StrategyType::Python,
                }
            } else {
                LoadError::BuildError {
                    name: config.name.clone(),
                    source: anyhow::anyhow!("{}", e),
                }
            }
        })
}

/// Load a Python strategy from configuration.
///
/// Note: Python strategy loading requires the `python` feature.
/// This function will return an error when the feature is not enabled.
#[cfg(not(feature = "python"))]
fn load_python_strategy_from_config(
    config: &StrategyConfig,
) -> Result<Box<dyn Strategy>, LoadError> {
    // Validate that path is provided
    if config.path.is_none() {
        return Err(LoadError::MissingConfig {
            name: config.name.clone(),
            field: "path".to_string(),
        });
    }

    // Python loading is not available without the feature
    Err(LoadError::PythonNotSupported(config.name.clone()))
}

/// Load a Python strategy with custom Python configuration.
///
/// This function allows specifying custom Python paths and virtual environment
/// settings when loading a Python strategy.
///
/// # Arguments
///
/// * `config` - Strategy configuration specifying class, path, and parameters
/// * `python_config` - Python environment configuration
///
/// # Returns
///
/// A boxed strategy instance ready for execution.
///
/// # Errors
///
/// Returns an error if:
/// - The `python` feature is not enabled
/// - The strategy path is not provided
/// - The Python script cannot be loaded
/// - The strategy class is not found
///
/// # Example
///
/// ```ignore
/// use zephyr_strategy::loader::{load_python_strategy_with_config, StrategyConfig, PythonConfig};
/// use serde_json::json;
///
/// let python_config = PythonConfig {
///     venv_dir: Some("/path/to/venv".into()),
///     python_paths: vec!["./strategies".into()],
/// };
///
/// let strategy_config = StrategyConfig {
///     name: "grid_eth".to_string(),
///     strategy_type: StrategyType::Python,
///     class: "GridStrategy".to_string(),
///     path: Some("./strategies/grid.py".into()),
///     params: json!({"grid_size": 10}),
/// };
///
/// let strategy = load_python_strategy_with_config(&strategy_config, &python_config)?;
/// ```
#[cfg(feature = "python")]
pub fn load_python_strategy_with_config(
    config: &StrategyConfig,
    python_config: &PythonConfig,
) -> Result<Box<dyn Strategy>, LoadError> {
    // Validate that path is provided
    let path = config
        .path
        .as_ref()
        .ok_or_else(|| LoadError::MissingConfig {
            name: config.name.clone(),
            field: "path".to_string(),
        })?;

    // Create a Python loader with the provided config
    let loader = PythonLoader::new(python_config).map_err(|e| LoadError::BuildError {
        name: config.name.clone(),
        source: anyhow::anyhow!("{}", e),
    })?;

    // Load the strategy
    loader
        .load(
            path.to_string_lossy().as_ref(),
            &config.class,
            config.params.clone(),
        )
        .map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("not found") || err_str.contains("ClassNotFound") {
                LoadError::NotFound {
                    name: config.class.clone(),
                    strategy_type: StrategyType::Python,
                }
            } else {
                LoadError::BuildError {
                    name: config.name.clone(),
                    source: anyhow::anyhow!("{}", e),
                }
            }
        })
}

/// Load a Python strategy with custom Python configuration.
///
/// Note: Python strategy loading requires the `python` feature.
#[cfg(not(feature = "python"))]
pub fn load_python_strategy_with_config(
    config: &StrategyConfig,
    _python_config: &crate::loader::config::PythonConfig,
) -> Result<Box<dyn Strategy>, LoadError> {
    Err(LoadError::PythonNotSupported(config.name.clone()))
}

/// Load multiple strategies from an engine configuration.
///
/// This function loads all strategies defined in the configuration,
/// returning a vector of (name, strategy) pairs.
///
/// # Arguments
///
/// * `strategies` - Slice of strategy configurations to load
///
/// # Returns
///
/// A vector of tuples containing strategy names and their instances.
/// Strategies that fail to load are skipped with a warning logged.
///
/// # Example
///
/// ```ignore
/// use zephyr_strategy::loader::{load_strategies, EngineConfig};
///
/// let config = EngineConfig::from_file("strategies.toml")?;
/// let strategies = load_strategies(&config.strategies)?;
///
/// for (name, strategy) in strategies {
///     println!("Loaded strategy: {}", name);
/// }
/// ```
#[must_use]
pub fn load_strategies(strategies: &[StrategyConfig]) -> Vec<StrategyLoadResult> {
    strategies
        .iter()
        .map(|config| {
            let result = load_strategy(config);
            (config.name.clone(), result)
        })
        .collect()
}

/// Load all strategies from configuration, failing on first error.
///
/// Unlike `load_strategies`, this function returns an error immediately
/// if any strategy fails to load.
///
/// # Arguments
///
/// * `strategies` - Slice of strategy configurations to load
///
/// # Returns
///
/// A vector of tuples containing strategy names and their instances.
///
/// # Errors
///
/// Returns the first error encountered during loading.
pub fn load_all_strategies(strategies: &[StrategyConfig]) -> Result<LoadedStrategies, LoadError> {
    strategies
        .iter()
        .map(|config| {
            let strategy = load_strategy(config)?;
            Ok((config.name.clone(), strategy))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::config::StrategyType;
    use std::path::PathBuf;

    fn make_rust_config(name: &str, class: &str, params: serde_json::Value) -> StrategyConfig {
        StrategyConfig {
            name: name.to_string(),
            strategy_type: StrategyType::Rust,
            class: class.to_string(),
            path: None,
            params,
        }
    }

    fn make_python_config(name: &str, class: &str, path: &str) -> StrategyConfig {
        StrategyConfig {
            name: name.to_string(),
            strategy_type: StrategyType::Python,
            class: class.to_string(),
            path: Some(PathBuf::from(path)),
            params: serde_json::json!({}),
        }
    }

    #[test]
    fn test_load_nonexistent_rust_strategy() {
        let config = make_rust_config("test", "NonExistent", serde_json::json!({}));
        let result = load_strategy(&config);

        assert!(result.is_err());
        match result {
            Err(LoadError::NotFound { .. }) => {}
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    #[cfg(not(feature = "python"))]
    fn test_load_python_strategy_not_supported() {
        let config = make_python_config("test", "GridStrategy", "./strategies/grid.py");
        let result = load_strategy(&config);

        assert!(result.is_err());
        match result {
            Err(LoadError::PythonNotSupported(_)) => {}
            _ => panic!("Expected PythonNotSupported error"),
        }
    }

    #[test]
    #[cfg(feature = "python")]
    fn test_load_python_strategy_file_not_found() {
        let config = make_python_config("test", "GridStrategy", "./nonexistent/grid.py");
        let result = load_strategy(&config);

        // Should fail because the file doesn't exist
        assert!(result.is_err());
    }

    #[test]
    fn test_load_python_strategy_missing_path() {
        let config = StrategyConfig {
            name: "test".to_string(),
            strategy_type: StrategyType::Python,
            class: "GridStrategy".to_string(),
            path: None,
            params: serde_json::json!({}),
        };
        let result = load_strategy(&config);

        assert!(result.is_err());
        match result {
            Err(LoadError::MissingConfig { field, .. }) if field == "path" => {}
            _ => panic!("Expected MissingConfig error with field 'path'"),
        }
    }

    #[test]
    fn test_load_strategies_returns_results_for_each() {
        let configs = vec![
            make_rust_config("strat1", "NonExistent1", serde_json::json!({})),
            make_rust_config("strat2", "NonExistent2", serde_json::json!({})),
        ];

        let results = load_strategies(&configs);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "strat1");
        assert_eq!(results[1].0, "strat2");
        assert!(results[0].1.is_err());
        assert!(results[1].1.is_err());
    }

    #[test]
    fn test_load_all_strategies_fails_on_first_error() {
        let configs = vec![
            make_rust_config("strat1", "NonExistent1", serde_json::json!({})),
            make_rust_config("strat2", "NonExistent2", serde_json::json!({})),
        ];

        let result = load_all_strategies(&configs);

        assert!(result.is_err());
    }

    #[test]
    fn test_error_display() {
        let err = LoadError::NotFound {
            name: "TestStrategy".to_string(),
            strategy_type: StrategyType::Rust,
        };
        let msg = format!("{err}");
        assert!(msg.contains("TestStrategy"));
        assert!(msg.contains("rust"));
        assert!(msg.contains("not found"));

        let err = LoadError::PythonNotSupported("MyPyStrategy".to_string());
        let msg = format!("{err}");
        assert!(msg.contains("MyPyStrategy"));
        assert!(msg.contains("not yet implemented"));
    }
}
