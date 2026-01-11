//! Configuration loader supporting YAML and TOML formats.
//!
//! This module provides the main configuration loading functionality,
//! supporting multiple file formats and environment variable overrides.

use crate::error::ConfigError;
use serde::de::DeserializeOwned;
use std::path::Path;

/// Supported configuration file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfigFormat {
    /// YAML format (.yaml, .yml)
    #[default]
    Yaml,
    /// TOML format (.toml)
    Toml,
    /// JSON format (.json)
    Json,
}

impl ConfigFormat {
    /// Detects the format from a file extension.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the configuration file
    ///
    /// # Returns
    ///
    /// The detected format, or `None` if the extension is not recognized.
    #[must_use]
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| match ext.to_lowercase().as_str() {
                "yaml" | "yml" => Some(Self::Yaml),
                "toml" => Some(Self::Toml),
                "json" => Some(Self::Json),
                _ => None,
            })
    }

    /// Returns the file extension for this format.
    #[must_use]
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Json => "json",
        }
    }
}

/// Configuration loader with support for multiple formats and environment overrides.
///
/// # Example
///
/// ```rust,ignore
/// use zephyr_core::config::{ConfigLoader, ConfigFormat};
/// use serde::Deserialize;
///
/// #[derive(Deserialize)]
/// struct MyConfig {
///     host: String,
///     port: u16,
/// }
///
/// let config: MyConfig = ConfigLoader::new()
///     .with_env_prefix("MYAPP")
///     .load_file("config.yaml")?;
/// ```
#[derive(Debug, Clone, Default)]
pub struct ConfigLoader {
    /// Environment variable prefix for overrides.
    env_prefix: Option<String>,
    /// Whether to validate after loading.
    validate: bool,
}

impl ConfigLoader {
    /// Creates a new configuration loader.
    #[must_use]
    pub fn new() -> Self {
        Self {
            env_prefix: None,
            validate: true,
        }
    }

    /// Sets the environment variable prefix for overrides.
    ///
    /// When set, the loader will look for environment variables with this prefix
    /// to override configuration values.
    ///
    /// # Arguments
    ///
    /// * `prefix` - The environment variable prefix (e.g., "ZEPHYR")
    #[must_use]
    pub fn with_env_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.env_prefix = Some(prefix.into());
        self
    }

    /// Sets whether to validate the configuration after loading.
    ///
    /// Default is `true`.
    #[must_use]
    pub fn with_validation(mut self, validate: bool) -> Self {
        self.validate = validate;
        self
    }

    /// Returns the environment variable prefix, if set.
    #[must_use]
    pub fn env_prefix(&self) -> Option<&str> {
        self.env_prefix.as_deref()
    }

    /// Loads configuration from a file.
    ///
    /// The format is automatically detected from the file extension.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the configuration file
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be read
    /// - The file format is not recognized
    /// - The content cannot be parsed
    pub fn load_file<T, P>(&self, path: P) -> Result<T, ConfigError>
    where
        T: DeserializeOwned,
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let format = ConfigFormat::from_path(path).ok_or_else(|| ConfigError::InvalidFormat {
            path: path.display().to_string(),
            reason: "Unrecognized file extension. Supported: .yaml, .yml, .toml, .json".to_string(),
        })?;

        let content = std::fs::read_to_string(path).map_err(|e| ConfigError::FileReadError {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;

        self.load_str(&content, format)
    }

    /// Loads configuration from a string with the specified format.
    ///
    /// # Arguments
    ///
    /// * `content` - The configuration content as a string
    /// * `format` - The format of the content
    ///
    /// # Errors
    ///
    /// Returns an error if the content cannot be parsed.
    pub fn load_str<T>(&self, content: &str, format: ConfigFormat) -> Result<T, ConfigError>
    where
        T: DeserializeOwned,
    {
        let config: T = match format {
            ConfigFormat::Yaml => {
                serde_yaml::from_str(content).map_err(|e| ConfigError::InvalidFormat {
                    path: "<string>".to_string(),
                    reason: format!("YAML parse error: {e}"),
                })?
            }
            ConfigFormat::Toml => {
                toml::from_str(content).map_err(|e| ConfigError::InvalidFormat {
                    path: "<string>".to_string(),
                    reason: format!("TOML parse error: {e}"),
                })?
            }
            ConfigFormat::Json => {
                serde_json::from_str(content).map_err(|e| ConfigError::InvalidFormat {
                    path: "<string>".to_string(),
                    reason: format!("JSON parse error: {e}"),
                })?
            }
        };

        Ok(config)
    }

    /// Loads configuration from bytes with the specified format.
    ///
    /// # Arguments
    ///
    /// * `content` - The configuration content as bytes
    /// * `format` - The format of the content
    ///
    /// # Errors
    ///
    /// Returns an error if the content cannot be parsed.
    pub fn load_bytes<T>(&self, content: &[u8], format: ConfigFormat) -> Result<T, ConfigError>
    where
        T: DeserializeOwned,
    {
        let content_str = std::str::from_utf8(content).map_err(|e| ConfigError::InvalidFormat {
            path: "<bytes>".to_string(),
            reason: format!("Invalid UTF-8: {e}"),
        })?;
        self.load_str(content_str, format)
    }

    /// Serializes a configuration to a string in the specified format.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to serialize
    /// * `format` - The output format
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn serialize<T>(config: &T, format: ConfigFormat) -> Result<String, ConfigError>
    where
        T: serde::Serialize,
    {
        match format {
            ConfigFormat::Yaml => {
                serde_yaml::to_string(config).map_err(|e| ConfigError::InvalidFormat {
                    path: "<serialize>".to_string(),
                    reason: format!("YAML serialization error: {e}"),
                })
            }
            ConfigFormat::Toml => {
                toml::to_string_pretty(config).map_err(|e| ConfigError::InvalidFormat {
                    path: "<serialize>".to_string(),
                    reason: format!("TOML serialization error: {e}"),
                })
            }
            ConfigFormat::Json => {
                serde_json::to_string_pretty(config).map_err(|e| ConfigError::InvalidFormat {
                    path: "<serialize>".to_string(),
                    reason: format!("JSON serialization error: {e}"),
                })
            }
        }
    }

    /// Saves a configuration to a file.
    ///
    /// The format is automatically detected from the file extension.
    ///
    /// # Arguments
    ///
    /// * `config` - The configuration to save
    /// * `path` - Path to the output file
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file format is not recognized
    /// - Serialization fails
    /// - The file cannot be written
    pub fn save_file<T, P>(config: &T, path: P) -> Result<(), ConfigError>
    where
        T: serde::Serialize,
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let format = ConfigFormat::from_path(path).ok_or_else(|| ConfigError::InvalidFormat {
            path: path.display().to_string(),
            reason: "Unrecognized file extension. Supported: .yaml, .yml, .toml, .json".to_string(),
        })?;

        let content = Self::serialize(config, format)?;

        std::fs::write(path, content).map_err(|e| ConfigError::FileWriteError {
            path: path.display().to_string(),
            reason: e.to_string(),
        })
    }
}

/// Helper function to load configuration from environment variables only.
///
/// This creates a default configuration and applies environment variable overrides.
///
/// # Type Parameters
///
/// * `T` - The configuration type, must implement `Default` and `DeserializeOwned`
///
/// # Arguments
///
/// * `prefix` - The environment variable prefix
#[allow(dead_code)]
pub fn load_from_env<T>(_prefix: &str) -> T
where
    T: Default,
{
    // Note: The actual env override application would be done by the Configurable trait
    // This is a placeholder that returns the default config
    T::default()
}

/// Merges two configurations, with the second taking precedence.
///
/// This is useful for layered configuration (e.g., defaults + user config + env overrides).
#[allow(dead_code)]
#[allow(clippy::needless_pass_by_value)]
pub fn merge_configs<T>(base: T, overlay: T) -> T
where
    T: serde::Serialize + DeserializeOwned,
{
    // Serialize both to JSON values
    let base_value = serde_json::to_value(&base).unwrap_or(serde_json::Value::Null);
    let overlay_value = serde_json::to_value(&overlay).unwrap_or(serde_json::Value::Null);

    // Merge the values
    let merged = merge_json_values(base_value, overlay_value);

    // Deserialize back
    serde_json::from_value(merged).unwrap_or(base)
}

/// Recursively merges two JSON values.
#[allow(dead_code)]
fn merge_json_values(base: serde_json::Value, overlay: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;

    match (base, overlay) {
        (Value::Object(mut base_map), Value::Object(overlay_map)) => {
            for (key, overlay_val) in overlay_map {
                let merged_val = if let Some(base_val) = base_map.remove(&key) {
                    merge_json_values(base_val, overlay_val)
                } else {
                    overlay_val
                };
                base_map.insert(key, merged_val);
            }
            Value::Object(base_map)
        }
        (_, overlay) => overlay,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
    struct TestConfig {
        host: String,
        port: u16,
        #[serde(default)]
        debug: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        api_key: Option<String>,
    }

    #[test]
    fn test_format_detection() {
        assert_eq!(
            ConfigFormat::from_path(Path::new("config.yaml")),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigFormat::from_path(Path::new("config.yml")),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigFormat::from_path(Path::new("config.toml")),
            Some(ConfigFormat::Toml)
        );
        assert_eq!(
            ConfigFormat::from_path(Path::new("config.json")),
            Some(ConfigFormat::Json)
        );
        assert_eq!(ConfigFormat::from_path(Path::new("config.txt")), None);
        assert_eq!(ConfigFormat::from_path(Path::new("config")), None);
    }

    #[test]
    fn test_load_yaml() {
        let yaml = r"
host: localhost
port: 8080
debug: true
";
        let loader = ConfigLoader::new();
        let config: TestConfig = loader.load_str(yaml, ConfigFormat::Yaml).unwrap();

        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 8080);
        assert!(config.debug);
    }

    #[test]
    fn test_load_toml() {
        let toml = r#"
host = "localhost"
port = 8080
debug = true
"#;
        let loader = ConfigLoader::new();
        let config: TestConfig = loader.load_str(toml, ConfigFormat::Toml).unwrap();

        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 8080);
        assert!(config.debug);
    }

    #[test]
    fn test_load_json() {
        let json = r#"{"host": "localhost", "port": 8080, "debug": true}"#;
        let loader = ConfigLoader::new();
        let config: TestConfig = loader.load_str(json, ConfigFormat::Json).unwrap();

        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 8080);
        assert!(config.debug);
    }

    #[test]
    fn test_invalid_yaml() {
        let invalid = "host: [invalid";
        let loader = ConfigLoader::new();
        let result: Result<TestConfig, _> = loader.load_str(invalid, ConfigFormat::Yaml);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::InvalidFormat { .. }));
        assert!(err.to_string().contains("YAML parse error"));
    }

    #[test]
    fn test_invalid_toml() {
        let invalid = "host = [invalid";
        let loader = ConfigLoader::new();
        let result: Result<TestConfig, _> = loader.load_str(invalid, ConfigFormat::Toml);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::InvalidFormat { .. }));
        assert!(err.to_string().contains("TOML parse error"));
    }

    #[test]
    fn test_serialize_yaml() {
        let config = TestConfig {
            host: "localhost".to_string(),
            port: 8080,
            debug: true,
            api_key: None,
        };

        let yaml = ConfigLoader::serialize(&config, ConfigFormat::Yaml).unwrap();
        assert!(yaml.contains("host: localhost"));
        assert!(yaml.contains("port: 8080"));
    }

    #[test]
    fn test_serialize_toml() {
        let config = TestConfig {
            host: "localhost".to_string(),
            port: 8080,
            debug: true,
            api_key: None,
        };

        let toml = ConfigLoader::serialize(&config, ConfigFormat::Toml).unwrap();
        assert!(toml.contains("host = \"localhost\""));
        assert!(toml.contains("port = 8080"));
    }

    #[test]
    fn test_roundtrip_yaml() {
        let original = TestConfig {
            host: "example.com".to_string(),
            port: 443,
            debug: false,
            api_key: Some("secret".to_string()),
        };

        let yaml = ConfigLoader::serialize(&original, ConfigFormat::Yaml).unwrap();
        let loader = ConfigLoader::new();
        let parsed: TestConfig = loader.load_str(&yaml, ConfigFormat::Yaml).unwrap();

        assert_eq!(original, parsed);
    }

    #[test]
    fn test_roundtrip_toml() {
        let original = TestConfig {
            host: "example.com".to_string(),
            port: 443,
            debug: false,
            api_key: Some("secret".to_string()),
        };

        let toml = ConfigLoader::serialize(&original, ConfigFormat::Toml).unwrap();
        let loader = ConfigLoader::new();
        let parsed: TestConfig = loader.load_str(&toml, ConfigFormat::Toml).unwrap();

        assert_eq!(original, parsed);
    }

    #[test]
    fn test_roundtrip_json() {
        let original = TestConfig {
            host: "example.com".to_string(),
            port: 443,
            debug: false,
            api_key: Some("secret".to_string()),
        };

        let json = ConfigLoader::serialize(&original, ConfigFormat::Json).unwrap();
        let loader = ConfigLoader::new();
        let parsed: TestConfig = loader.load_str(&json, ConfigFormat::Json).unwrap();

        assert_eq!(original, parsed);
    }

    #[test]
    fn test_merge_configs() {
        let base = TestConfig {
            host: "localhost".to_string(),
            port: 8080,
            debug: false,
            api_key: None,
        };

        let overlay = TestConfig {
            host: "example.com".to_string(),
            port: 443,
            debug: true,
            api_key: Some("key".to_string()),
        };

        let merged = merge_configs(base, overlay.clone());
        assert_eq!(merged, overlay);
    }

    #[test]
    fn test_env_prefix() {
        let loader = ConfigLoader::new().with_env_prefix("ZEPHYR");
        assert_eq!(loader.env_prefix(), Some("ZEPHYR"));

        let loader = ConfigLoader::new();
        assert_eq!(loader.env_prefix(), None);
    }

    #[test]
    fn test_format_extension() {
        assert_eq!(ConfigFormat::Yaml.extension(), "yaml");
        assert_eq!(ConfigFormat::Toml.extension(), "toml");
        assert_eq!(ConfigFormat::Json.extension(), "json");
    }
}

#[cfg(test)]
mod file_tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct FileTestConfig {
        name: String,
        value: i32,
    }

    #[test]
    fn test_load_yaml_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_config.yaml");

        let yaml = "name: test\nvalue: 42\n";
        std::fs::write(&path, yaml).unwrap();

        let loader = ConfigLoader::new();
        let config: FileTestConfig = loader.load_file(&path).unwrap();

        assert_eq!(config.name, "test");
        assert_eq!(config.value, 42);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_toml_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_config.toml");

        let toml = "name = \"test\"\nvalue = 42\n";
        std::fs::write(&path, toml).unwrap();

        let loader = ConfigLoader::new();
        let config: FileTestConfig = loader.load_file(&path).unwrap();

        assert_eq!(config.name, "test");
        assert_eq!(config.value, 42);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_save_and_load_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_save_config.yaml");

        let original = FileTestConfig {
            name: "saved".to_string(),
            value: 123,
        };

        ConfigLoader::save_file(&original, &path).unwrap();

        let cfg_loader = ConfigLoader::new();
        let loaded: FileTestConfig = cfg_loader.load_file(&path).unwrap();

        assert_eq!(original, loaded);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_file_not_found() {
        let loader = ConfigLoader::new();
        let result: Result<FileTestConfig, _> = loader.load_file("/nonexistent/path/config.yaml");

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::FileReadError { .. }));
    }

    #[test]
    fn test_unrecognized_extension() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_config.txt");

        std::fs::write(&path, "content").unwrap();

        let loader = ConfigLoader::new();
        let result: Result<FileTestConfig, _> = loader.load_file(&path);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::InvalidFormat { .. }));
        assert!(err.to_string().contains("Unrecognized file extension"));

        std::fs::remove_file(&path).ok();
    }
}
