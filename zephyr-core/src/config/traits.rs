//! Configuration traits for validation and loading.
//!
//! This module defines traits that configuration types should implement
//! to support validation and environment variable overrides.

use crate::error::ConfigError;

/// Trait for types that can be validated.
///
/// Implement this trait to add custom validation logic to configuration types.
///
/// # Example
///
/// ```rust
/// use zephyr_core::config::Validatable;
/// use zephyr_core::error::ConfigError;
///
/// struct ServerConfig {
///     port: u16,
///     host: String,
/// }
///
/// impl Validatable for ServerConfig {
///     fn validate(&self) -> Result<(), ConfigError> {
///         if self.port == 0 {
///             return Err(ConfigError::invalid_value("port", "Port cannot be 0"));
///         }
///         if self.host.is_empty() {
///             return Err(ConfigError::missing_field("host"));
///         }
///         Ok(())
///     }
/// }
/// ```
pub trait Validatable {
    /// Validates the configuration.
    ///
    /// Returns `Ok(())` if the configuration is valid, or a `ConfigError`
    /// describing what is invalid.
    fn validate(&self) -> Result<(), ConfigError>;
}

/// Trait for types that support environment variable overrides.
///
/// Implement this trait to allow configuration values to be overridden
/// by environment variables.
///
/// # Example
///
/// ```rust,ignore
/// use zephyr_core::config::Configurable;
///
/// struct DatabaseConfig {
///     host: String,
///     port: u16,
///     password: Option<String>,
/// }
///
/// impl Configurable for DatabaseConfig {
///     fn apply_env_overrides(&mut self, prefix: &str) {
///         if let Ok(host) = std::env::var(format!("{prefix}_DATABASE_HOST")) {
///             self.host = host;
///         }
///         if let Ok(port) = std::env::var(format!("{prefix}_DATABASE_PORT")) {
///             if let Ok(p) = port.parse() {
///                 self.port = p;
///             }
///         }
///         if let Ok(password) = std::env::var(format!("{prefix}_DATABASE_PASSWORD")) {
///             self.password = Some(password);
///         }
///     }
///
///     fn env_var_names(prefix: &str) -> Vec<String> {
///         vec![
///             format!("{prefix}_DATABASE_HOST"),
///             format!("{prefix}_DATABASE_PORT"),
///             format!("{prefix}_DATABASE_PASSWORD"),
///         ]
///     }
/// }
/// ```
pub trait Configurable: Sized {
    /// Applies environment variable overrides to the configuration.
    ///
    /// # Arguments
    ///
    /// * `prefix` - The environment variable prefix (e.g., "ZEPHYR")
    fn apply_env_overrides(&mut self, prefix: &str);

    /// Returns a list of environment variable names that can override this configuration.
    ///
    /// # Arguments
    ///
    /// * `prefix` - The environment variable prefix (e.g., "ZEPHYR")
    fn env_var_names(prefix: &str) -> Vec<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestConfig {
        value: i32,
    }

    impl Validatable for TestConfig {
        fn validate(&self) -> Result<(), ConfigError> {
            if self.value < 0 {
                return Err(ConfigError::invalid_value(
                    "value",
                    "Value must be non-negative",
                ));
            }
            Ok(())
        }
    }

    #[test]
    fn test_validatable_success() {
        let config = TestConfig { value: 10 };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validatable_failure() {
        let config = TestConfig { value: -1 };
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("value"));
    }
}
