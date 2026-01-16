//! Configuration types for strategy loading.
//!
//! This module defines the configuration structures for loading strategies
//! from TOML configuration files. It supports both Rust and Python strategies
//! with a unified configuration format.
//!
//! # Configuration Format
//!
//! ```toml
//! [python]
//! venv_dir = "/path/to/venv"
//! python_paths = ["./strategies", "./lib"]
//!
//! [[strategies]]
//! name = "dual_thrust_btc"
//! type = "rust"
//! class = "DualThrust"
//! [strategies.params]
//! n = 20
//! k1 = 0.5
//! k2 = 0.5
//!
//! [[strategies]]
//! name = "grid_eth"
//! type = "python"
//! path = "./strategies/grid.py"
//! class = "GridStrategy"
//! [strategies.params]
//! grid_size = 10
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use thiserror::Error;

/// Strategy type discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StrategyType {
    /// Rust strategy loaded from compile-time registry
    Rust,
    /// Python strategy loaded from file
    Python,
}

impl std::fmt::Display for StrategyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rust => write!(f, "rust"),
            Self::Python => write!(f, "python"),
        }
    }
}

/// Configuration for a single strategy instance.
///
/// This struct represents one strategy entry in the configuration file.
/// It contains all information needed to load and instantiate a strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    /// Unique name for this strategy instance
    pub name: String,

    /// Strategy type (rust or python)
    #[serde(rename = "type")]
    pub strategy_type: StrategyType,

    /// Class/builder name to instantiate
    pub class: String,

    /// Path to Python script (required for Python strategies)
    #[serde(default)]
    pub path: Option<PathBuf>,

    /// Strategy-specific parameters
    #[serde(default)]
    pub params: serde_json::Value,
}

/// Python environment configuration.
///
/// Specifies how to configure the Python interpreter for loading
/// Python strategies.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PythonConfig {
    /// Path to Python virtual environment directory
    #[serde(default)]
    pub venv_dir: Option<PathBuf>,

    /// Additional paths to add to Python's sys.path
    #[serde(default)]
    pub python_paths: Vec<PathBuf>,
}

/// Top-level engine configuration.
///
/// This is the root configuration structure that contains all strategy
/// definitions and Python environment settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EngineConfig {
    /// Python environment configuration
    #[serde(default)]
    pub python: PythonConfig,

    /// List of strategy configurations
    #[serde(default)]
    pub strategies: Vec<StrategyConfig>,
}

/// Errors that can occur during configuration loading and validation.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Failed to read configuration file
    #[error("failed to read config file '{path}': {source}")]
    ReadError {
        /// Path to the file that could not be read
        path: PathBuf,
        /// The underlying IO error
        #[source]
        source: std::io::Error,
    },

    /// Failed to parse TOML configuration
    #[error("failed to parse TOML config: {0}")]
    ParseError(#[from] toml::de::Error),

    /// Validation error in configuration
    #[error("config validation error: {0}")]
    ValidationError(String),

    /// Missing required field
    #[error("missing required field '{field}' for strategy '{strategy}'")]
    MissingField {
        /// Name of the strategy with the missing field
        strategy: String,
        /// Name of the missing field
        field: String,
    },

    /// Duplicate strategy name
    #[error("duplicate strategy name: '{0}'")]
    DuplicateName(String),
}

impl EngineConfig {
    /// Load configuration from a TOML file.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the TOML configuration file
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be read
    /// - The TOML is malformed
    /// - Required fields are missing
    /// - Validation fails
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| ConfigError::ReadError {
            path: path.to_path_buf(),
            source: e,
        })?;

        Self::from_toml(&content)
    }

    /// Parse configuration from a TOML string.
    ///
    /// # Arguments
    ///
    /// * `content` - TOML configuration string
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The TOML is malformed
    /// - Required fields are missing
    /// - Validation fails
    pub fn from_toml(content: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(content)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration.
    ///
    /// Checks for:
    /// - Duplicate strategy names
    /// - Required fields for each strategy type
    /// - Valid paths for Python strategies
    fn validate(&self) -> Result<(), ConfigError> {
        let mut seen_names = HashSet::new();

        for strategy in &self.strategies {
            // Check for duplicate names
            if !seen_names.insert(&strategy.name) {
                return Err(ConfigError::DuplicateName(strategy.name.clone()));
            }

            // Validate strategy-specific requirements
            Self::validate_strategy(strategy)?;
        }

        Ok(())
    }

    /// Validate a single strategy configuration.
    fn validate_strategy(strategy: &StrategyConfig) -> Result<(), ConfigError> {
        // Name is required
        if strategy.name.is_empty() {
            return Err(ConfigError::ValidationError(
                "strategy name cannot be empty".to_string(),
            ));
        }

        // Class is required
        if strategy.class.is_empty() {
            return Err(ConfigError::MissingField {
                strategy: strategy.name.clone(),
                field: "class".to_string(),
            });
        }

        // Python strategies require a path
        if strategy.strategy_type == StrategyType::Python && strategy.path.is_none() {
            return Err(ConfigError::MissingField {
                strategy: strategy.name.clone(),
                field: "path".to_string(),
            });
        }

        Ok(())
    }

    /// Get a strategy configuration by name.
    #[must_use]
    pub fn get_strategy(&self, name: &str) -> Option<&StrategyConfig> {
        self.strategies.iter().find(|s| s.name == name)
    }

    /// Get all Rust strategy configurations.
    #[must_use]
    pub fn rust_strategies(&self) -> Vec<&StrategyConfig> {
        self.strategies
            .iter()
            .filter(|s| s.strategy_type == StrategyType::Rust)
            .collect()
    }

    /// Get all Python strategy configurations.
    #[must_use]
    pub fn python_strategies(&self) -> Vec<&StrategyConfig> {
        self.strategies
            .iter()
            .filter(|s| s.strategy_type == StrategyType::Python)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_config() {
        let config = EngineConfig::from_toml("").unwrap();
        assert!(config.strategies.is_empty());
        assert!(config.python.venv_dir.is_none());
        assert!(config.python.python_paths.is_empty());
    }

    #[test]
    fn test_parse_rust_strategy() {
        let toml = r#"
[[strategies]]
name = "dual_thrust_btc"
type = "rust"
class = "DualThrust"
[strategies.params]
n = 20
k1 = 0.5
"#;
        let config = EngineConfig::from_toml(toml).unwrap();
        assert_eq!(config.strategies.len(), 1);

        let strategy = &config.strategies[0];
        assert_eq!(strategy.name, "dual_thrust_btc");
        assert_eq!(strategy.strategy_type, StrategyType::Rust);
        assert_eq!(strategy.class, "DualThrust");
        assert!(strategy.path.is_none());
        assert_eq!(strategy.params["n"], 20);
    }

    #[test]
    fn test_parse_python_strategy() {
        let toml = r#"
[[strategies]]
name = "grid_eth"
type = "python"
path = "./strategies/grid.py"
class = "GridStrategy"
[strategies.params]
grid_size = 10
"#;
        let config = EngineConfig::from_toml(toml).unwrap();
        assert_eq!(config.strategies.len(), 1);

        let strategy = &config.strategies[0];
        assert_eq!(strategy.name, "grid_eth");
        assert_eq!(strategy.strategy_type, StrategyType::Python);
        assert_eq!(strategy.class, "GridStrategy");
        assert_eq!(strategy.path, Some(PathBuf::from("./strategies/grid.py")));
    }

    #[test]
    fn test_parse_python_config() {
        let toml = r#"
[python]
venv_dir = "/path/to/venv"
python_paths = ["./strategies", "./lib"]
"#;
        let config = EngineConfig::from_toml(toml).unwrap();
        assert_eq!(config.python.venv_dir, Some(PathBuf::from("/path/to/venv")));
        assert_eq!(config.python.python_paths.len(), 2);
        assert_eq!(config.python.python_paths[0], PathBuf::from("./strategies"));
        assert_eq!(config.python.python_paths[1], PathBuf::from("./lib"));
    }

    #[test]
    fn test_parse_full_config() {
        let toml = r#"
[python]
venv_dir = "/path/to/venv"
python_paths = ["./strategies"]

[[strategies]]
name = "dual_thrust_btc"
type = "rust"
class = "DualThrust"
[strategies.params]
n = 20

[[strategies]]
name = "grid_eth"
type = "python"
path = "./strategies/grid.py"
class = "GridStrategy"
[strategies.params]
grid_size = 10
"#;
        let config = EngineConfig::from_toml(toml).unwrap();
        assert_eq!(config.strategies.len(), 2);
        assert_eq!(config.rust_strategies().len(), 1);
        assert_eq!(config.python_strategies().len(), 1);
    }

    #[test]
    fn test_validation_duplicate_name() {
        let toml = r#"
[[strategies]]
name = "my_strategy"
type = "rust"
class = "DualThrust"

[[strategies]]
name = "my_strategy"
type = "rust"
class = "Grid"
"#;
        let result = EngineConfig::from_toml(toml);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::DuplicateName(_)));
    }

    #[test]
    fn test_validation_missing_path_for_python() {
        let toml = r#"
[[strategies]]
name = "grid_eth"
type = "python"
class = "GridStrategy"
"#;
        let result = EngineConfig::from_toml(toml);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::MissingField { field, .. } if field == "path"));
    }

    #[test]
    fn test_validation_empty_class() {
        let toml = r#"
[[strategies]]
name = "my_strategy"
type = "rust"
class = ""
"#;
        let result = EngineConfig::from_toml(toml);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::MissingField { field, .. } if field == "class"));
    }

    #[test]
    fn test_get_strategy_by_name() {
        let toml = r#"
[[strategies]]
name = "dual_thrust_btc"
type = "rust"
class = "DualThrust"

[[strategies]]
name = "grid_eth"
type = "python"
path = "./strategies/grid.py"
class = "GridStrategy"
"#;
        let config = EngineConfig::from_toml(toml).unwrap();

        let strategy = config.get_strategy("dual_thrust_btc");
        assert!(strategy.is_some());
        assert_eq!(strategy.unwrap().class, "DualThrust");

        let strategy = config.get_strategy("nonexistent");
        assert!(strategy.is_none());
    }

    #[test]
    fn test_strategy_type_display() {
        assert_eq!(format!("{}", StrategyType::Rust), "rust");
        assert_eq!(format!("{}", StrategyType::Python), "python");
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = EngineConfig {
            python: PythonConfig {
                venv_dir: Some(PathBuf::from("/path/to/venv")),
                python_paths: vec![PathBuf::from("./strategies")],
            },
            strategies: vec![StrategyConfig {
                name: "test_strategy".to_string(),
                strategy_type: StrategyType::Rust,
                class: "TestClass".to_string(),
                path: None,
                params: serde_json::json!({"key": "value"}),
            }],
        };

        let toml_str = toml::to_string(&config).unwrap();
        let parsed: EngineConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.strategies.len(), 1);
        assert_eq!(parsed.strategies[0].name, "test_strategy");
        assert_eq!(parsed.python.venv_dir, Some(PathBuf::from("/path/to/venv")));
    }

    #[test]
    fn test_parse_invalid_toml() {
        let toml = r#"
[[strategies]
name = "broken"
"#;
        let result = EngineConfig::from_toml(toml);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::ParseError(_)));
    }

    #[test]
    fn test_parse_invalid_strategy_type() {
        let toml = r#"
[[strategies]]
name = "test"
type = "invalid"
class = "Test"
"#;
        let result = EngineConfig::from_toml(toml);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::ParseError(_)));
    }

    #[test]
    fn test_validation_empty_name() {
        let toml = r#"
[[strategies]]
name = ""
type = "rust"
class = "DualThrust"
"#;
        let result = EngineConfig::from_toml(toml);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::ValidationError(_)));
    }

    #[test]
    fn test_error_messages_are_descriptive() {
        // Test duplicate name error message
        let toml = r#"
[[strategies]]
name = "my_strategy"
type = "rust"
class = "DualThrust"

[[strategies]]
name = "my_strategy"
type = "rust"
class = "Grid"
"#;
        let err = EngineConfig::from_toml(toml).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("my_strategy"));
        assert!(msg.contains("duplicate"));

        // Test missing field error message
        let toml = r#"
[[strategies]]
name = "grid_eth"
type = "python"
class = "GridStrategy"
"#;
        let err = EngineConfig::from_toml(toml).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("path"));
        assert!(msg.contains("grid_eth"));
    }

    #[test]
    fn test_from_file_nonexistent() {
        let result = EngineConfig::from_file("/nonexistent/path/config.toml");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::ReadError { .. }));
    }

    #[test]
    fn test_rust_strategy_with_path_is_valid() {
        // Rust strategies can optionally have a path (it's just ignored)
        let toml = r#"
[[strategies]]
name = "dual_thrust_btc"
type = "rust"
class = "DualThrust"
path = "./optional/path.rs"
"#;
        let config = EngineConfig::from_toml(toml).unwrap();
        assert_eq!(config.strategies.len(), 1);
        assert_eq!(
            config.strategies[0].path,
            Some(PathBuf::from("./optional/path.rs"))
        );
    }

    #[test]
    fn test_strategy_with_complex_params() {
        let toml = r#"
[[strategies]]
name = "complex_strategy"
type = "rust"
class = "Complex"
[strategies.params]
string_param = "hello"
int_param = 42
float_param = 3.14
bool_param = true
array_param = [1, 2, 3]
[strategies.params.nested]
key = "value"
"#;
        let config = EngineConfig::from_toml(toml).unwrap();
        let params = &config.strategies[0].params;

        assert_eq!(params["string_param"], "hello");
        assert_eq!(params["int_param"], 42);
        assert_eq!(params["bool_param"], true);
        assert_eq!(params["array_param"][0], 1);
        assert_eq!(params["nested"]["key"], "value");
    }
}
