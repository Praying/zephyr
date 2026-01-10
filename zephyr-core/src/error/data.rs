//! Data-related error types.
//!
//! This module provides error types for data operations including
//! parsing failures, validation errors, missing data, and data corruption.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Data error type covering parsing failures, validation errors,
/// missing data, and data corruption.
///
/// # Examples
///
/// ```
/// use zephyr_core::error::DataError;
///
/// let error = DataError::ParseFailed {
///     field: "price".to_string(),
///     reason: "Invalid decimal format".to_string(),
/// };
/// assert!(error.to_string().contains("price"));
/// ```
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataError {
    /// Failed to parse data.
    #[error("[Data] Parse failed for field '{field}': {reason}")]
    ParseFailed {
        /// Field that failed to parse.
        field: String,
        /// Reason for the parse failure.
        reason: String,
    },

    /// Data validation failed.
    #[error("[Data] Validation failed: {field} - {reason}")]
    ValidationFailed {
        /// Field that failed validation.
        field: String,
        /// Reason for the validation failure.
        reason: String,
    },

    /// Required data is missing.
    #[error("[Data] Missing data: {description}")]
    MissingData {
        /// Description of the missing data.
        description: String,
    },

    /// Data is corrupted.
    #[error("[Data] Data corruption detected: {reason}")]
    Corruption {
        /// Reason or description of the corruption.
        reason: String,
    },

    /// Checksum verification failed.
    #[error("[Data] Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// Expected checksum value.
        expected: String,
        /// Actual checksum value.
        actual: String,
    },

    /// Data format is not supported.
    #[error("[Data] Unsupported format: {format}")]
    UnsupportedFormat {
        /// Format that is not supported.
        format: String,
    },

    /// Data is out of expected range.
    #[error("[Data] Out of range: {field} value {value} not in [{min}, {max}]")]
    OutOfRange {
        /// Field that is out of range.
        field: String,
        /// Actual value.
        value: String,
        /// Minimum allowed value.
        min: String,
        /// Maximum allowed value.
        max: String,
    },

    /// JSON serialization/deserialization error.
    #[error("[Data] JSON error: {reason}")]
    JsonError {
        /// Reason for the JSON error.
        reason: String,
    },

    /// Data schema version mismatch.
    #[error("[Data] Schema version mismatch: expected {expected}, got {actual}")]
    SchemaMismatch {
        /// Expected schema version.
        expected: String,
        /// Actual schema version.
        actual: String,
    },

    /// Empty data where non-empty was expected.
    #[error("[Data] Empty data: {description}")]
    EmptyData {
        /// Description of what data was expected.
        description: String,
    },

    /// Duplicate data detected.
    #[error("[Data] Duplicate data: {description}")]
    DuplicateData {
        /// Description of the duplicate.
        description: String,
    },
}

impl DataError {
    /// Returns true if this error is recoverable.
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        // Most data errors are not recoverable without fixing the source
        false
    }

    /// Returns the severity level of this error.
    #[must_use]
    pub fn severity(&self) -> super::ErrorSeverity {
        use super::ErrorSeverity;
        match self {
            Self::Corruption { .. } | Self::ChecksumMismatch { .. } => ErrorSeverity::Fatal,
            Self::ParseFailed { .. }
            | Self::ValidationFailed { .. }
            | Self::OutOfRange { .. }
            | Self::JsonError { .. }
            | Self::SchemaMismatch { .. } => ErrorSeverity::Warning,
            Self::MissingData { .. }
            | Self::UnsupportedFormat { .. }
            | Self::EmptyData { .. }
            | Self::DuplicateData { .. } => ErrorSeverity::Info,
        }
    }

    /// Returns a suggested retry delay in milliseconds, if applicable.
    #[must_use]
    pub fn suggested_retry_delay_ms(&self) -> Option<u64> {
        // Data errors are generally not recoverable by retrying
        None
    }

    /// Creates a parse error for a specific field.
    #[must_use]
    pub fn parse_error(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ParseFailed {
            field: field.into(),
            reason: reason.into(),
        }
    }

    /// Creates a validation error for a specific field.
    #[must_use]
    pub fn validation_error(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ValidationFailed {
            field: field.into(),
            reason: reason.into(),
        }
    }

    /// Creates a missing data error.
    #[must_use]
    pub fn missing(description: impl Into<String>) -> Self {
        Self::MissingData {
            description: description.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_failed() {
        let error = DataError::ParseFailed {
            field: "price".to_string(),
            reason: "Invalid decimal".to_string(),
        };
        assert!(error.to_string().contains("price"));
        assert!(error.to_string().contains("Invalid decimal"));
    }

    #[test]
    fn test_validation_failed() {
        let error = DataError::ValidationFailed {
            field: "quantity".to_string(),
            reason: "Must be positive".to_string(),
        };
        assert!(error.to_string().contains("quantity"));
    }

    #[test]
    fn test_checksum_mismatch() {
        let error = DataError::ChecksumMismatch {
            expected: "abc123".to_string(),
            actual: "def456".to_string(),
        };
        assert!(error.to_string().contains("abc123"));
        assert!(error.to_string().contains("def456"));
    }

    #[test]
    fn test_helper_methods() {
        let error = DataError::parse_error("timestamp", "Invalid format");
        assert!(matches!(error, DataError::ParseFailed { .. }));

        let error = DataError::validation_error("price", "Negative value");
        assert!(matches!(error, DataError::ValidationFailed { .. }));

        let error = DataError::missing("Historical data for BTC-USDT");
        assert!(matches!(error, DataError::MissingData { .. }));
    }

    #[test]
    fn test_serde_roundtrip() {
        let error = DataError::Corruption {
            reason: "Invalid header".to_string(),
        };
        let json = serde_json::to_string(&error).unwrap();
        let parsed: DataError = serde_json::from_str(&json).unwrap();
        assert_eq!(error, parsed);
    }
}
