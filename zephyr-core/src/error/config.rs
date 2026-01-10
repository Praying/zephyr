//! Configuration-related error types.
//!
//! This module provides error types for configuration operations including
//! missing fields, invalid values, and file access errors.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Configuration error type covering missing fields, invalid values,
/// and file access errors.
///
/// # Examples
///
/// ```
/// use zephyr_core::error::ConfigError;
///
/// let error = ConfigError::MissingField {
///     field: "api_key".to_string(),
///     section: Some("binance".to_string()),
/// };
/// assert!(error.to_string().contains("api_key"));
/// ```
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigError {
    /// Required configuration field is missing.
    #[error("[Config] Missing field '{field}'{}", section.as_ref().map(|s| format!(" in section '{s}'")).unwrap_or_default())]
    MissingField {
        /// Name of the missing field.
        field: String,
        /// Optional section where the field should be.
        section: Option<String>,
    },

    /// Configuration value is invalid.
    #[error("[Config] Invalid value for '{field}': {reason}")]
    InvalidValue {
        /// Field with the invalid value.
        field: String,
        /// Reason why the value is invalid.
        reason: String,
    },

    /// Configuration file could not be read.
    #[error("[Config] Failed to read file '{path}': {reason}")]
    FileReadError {
        /// Path to the configuration file.
        path: String,
        /// Reason for the read failure.
        reason: String,
    },

    /// Configuration file could not be written.
    #[error("[Config] Failed to write file '{path}': {reason}")]
    FileWriteError {
        /// Path to the configuration file.
        path: String,
        /// Reason for the write failure.
        reason: String,
    },

    /// Configuration file format is invalid.
    #[error("[Config] Invalid format in '{path}': {reason}")]
    InvalidFormat {
        /// Path to the configuration file.
        path: String,
        /// Reason for the format error.
        reason: String,
    },

    /// Environment variable is missing.
    #[error("[Config] Missing environment variable: {name}")]
    MissingEnvVar {
        /// Name of the missing environment variable.
        name: String,
    },

    /// Environment variable has invalid value.
    #[error("[Config] Invalid environment variable '{name}': {reason}")]
    InvalidEnvVar {
        /// Name of the environment variable.
        name: String,
        /// Reason why the value is invalid.
        reason: String,
    },

    /// Configuration validation failed.
    #[error("[Config] Validation failed: {reason}")]
    ValidationFailed {
        /// Reason for the validation failure.
        reason: String,
    },

    /// Configuration section not found.
    #[error("[Config] Section not found: {section}")]
    SectionNotFound {
        /// Name of the missing section.
        section: String,
    },

    /// Configuration is locked and cannot be modified.
    #[error("[Config] Configuration is locked: {reason}")]
    Locked {
        /// Reason why the configuration is locked.
        reason: String,
    },

    /// Circular reference detected in configuration.
    #[error("[Config] Circular reference detected: {path}")]
    CircularReference {
        /// Path showing the circular reference.
        path: String,
    },
}

impl ConfigError {
    /// Returns true if this error is recoverable.
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        matches!(self, Self::Locked { .. })
    }

    /// Returns the severity level of this error.
    #[must_use]
    pub fn severity(&self) -> super::ErrorSeverity {
        use super::ErrorSeverity;
        match self {
            Self::MissingField { .. }
            | Self::InvalidFormat { .. }
            | Self::CircularReference { .. } => ErrorSeverity::Fatal,
            Self::Locked { .. } => ErrorSeverity::Recoverable,
            Self::InvalidValue { .. }
            | Self::FileReadError { .. }
            | Self::FileWriteError { .. }
            | Self::MissingEnvVar { .. }
            | Self::InvalidEnvVar { .. }
            | Self::ValidationFailed { .. } => ErrorSeverity::Warning,
            Self::SectionNotFound { .. } => ErrorSeverity::Info,
        }
    }

    /// Returns a suggested retry delay in milliseconds, if applicable.
    #[must_use]
    pub fn suggested_retry_delay_ms(&self) -> Option<u64> {
        match self {
            Self::Locked { .. } => Some(100),
            _ => None,
        }
    }

    /// Creates a missing field error.
    #[must_use]
    pub fn missing_field(field: impl Into<String>) -> Self {
        Self::MissingField {
            field: field.into(),
            section: None,
        }
    }

    /// Creates a missing field error with section.
    #[must_use]
    pub fn missing_field_in_section(field: impl Into<String>, section: impl Into<String>) -> Self {
        Self::MissingField {
            field: field.into(),
            section: Some(section.into()),
        }
    }

    /// Creates an invalid value error.
    #[must_use]
    pub fn invalid_value(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidValue {
            field: field.into(),
            reason: reason.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_field() {
        let error = ConfigError::MissingField {
            field: "api_key".to_string(),
            section: None,
        };
        assert!(error.to_string().contains("api_key"));
        assert!(!error.to_string().contains("section"));
    }

    #[test]
    fn test_missing_field_with_section() {
        let error = ConfigError::MissingField {
            field: "api_key".to_string(),
            section: Some("binance".to_string()),
        };
        assert!(error.to_string().contains("api_key"));
        assert!(error.to_string().contains("binance"));
    }

    #[test]
    fn test_invalid_value() {
        let error = ConfigError::InvalidValue {
            field: "leverage".to_string(),
            reason: "Must be between 1 and 125".to_string(),
        };
        assert!(error.to_string().contains("leverage"));
    }

    #[test]
    fn test_file_read_error() {
        let error = ConfigError::FileReadError {
            path: "/etc/zephyr/config.yaml".to_string(),
            reason: "Permission denied".to_string(),
        };
        assert!(error.to_string().contains("config.yaml"));
    }

    #[test]
    fn test_helper_methods() {
        let error = ConfigError::missing_field("api_secret");
        assert!(matches!(
            error,
            ConfigError::MissingField { section: None, .. }
        ));

        let error = ConfigError::missing_field_in_section("api_key", "okx");
        assert!(matches!(
            error,
            ConfigError::MissingField {
                section: Some(_),
                ..
            }
        ));

        let error = ConfigError::invalid_value("timeout", "Must be positive");
        assert!(matches!(error, ConfigError::InvalidValue { .. }));
    }

    #[test]
    fn test_serde_roundtrip() {
        let error = ConfigError::InvalidFormat {
            path: "config.yaml".to_string(),
            reason: "Invalid YAML syntax".to_string(),
        };
        let json = serde_json::to_string(&error).unwrap();
        let parsed: ConfigError = serde_json::from_str(&json).unwrap();
        assert_eq!(error, parsed);
    }
}
