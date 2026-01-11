//! Error types and handling framework.
//!
//! This module provides a hierarchical error type system with
//! domain-specific error categories for Zephyr trading system.
//!
//! # Error Hierarchy
//!
//! The error system is organized hierarchically:
//! - ZephyrError - Top-level error type
//!   - NetworkError - Network and connection errors
//!   - ExchangeError - Exchange API errors
//!   - DataError - Data parsing and validation errors
//!   - StrategyError - Strategy execution errors
//!   - ConfigError - Configuration errors
//!   - StorageError - Storage and I/O errors
//!
//! # Error Context
//!
//! Use ErrorContext and ContextualError to attach metadata to errors:
//!
//! ```
//! use zephyr_core::error::{ErrorContext, NetworkError, ZephyrError, WithContext};
//!
//! let error = NetworkError::Timeout { timeout_ms: 5000 };
//! let contextual = ZephyrError::from(error)
//!     .with_context(ErrorContext::new("Gateway", "connect"));
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

// Define ErrorSeverity first so submodules can use it
/// Error severity levels for categorizing errors.
///
/// Severity levels help determine the appropriate response to an error:
/// - `Fatal`: Unrecoverable errors that require immediate attention
/// - `Recoverable`: Errors that can be retried or recovered from
/// - `Warning`: Non-critical issues that should be logged
/// - `Info`: Informational messages about expected conditions
///
/// # Examples
///
/// ```
/// use zephyr_core::error::ErrorSeverity;
///
/// let severity = ErrorSeverity::Recoverable;
/// assert!(severity.is_recoverable());
/// assert!(!severity.is_fatal());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ErrorSeverity {
    /// Unrecoverable error requiring immediate attention.
    /// The system cannot continue normal operation.
    Fatal,

    /// Error that can potentially be recovered from through retry or fallback.
    /// The operation failed but the system can continue.
    #[default]
    Recoverable,

    /// Non-critical issue that should be logged but doesn't prevent operation.
    /// May indicate degraded functionality.
    Warning,

    /// Informational message about an expected or handled condition.
    /// Not a true error, but worth noting.
    Info,
}

impl ErrorSeverity {
    /// Returns true if this error is recoverable (not fatal).
    #[must_use]
    pub const fn is_recoverable(&self) -> bool {
        !matches!(self, Self::Fatal)
    }

    /// Returns true if this error is fatal (unrecoverable).
    #[must_use]
    pub const fn is_fatal(&self) -> bool {
        matches!(self, Self::Fatal)
    }

    /// Returns true if this is a warning level severity.
    #[must_use]
    pub const fn is_warning(&self) -> bool {
        matches!(self, Self::Warning)
    }

    /// Returns true if this is an info level severity.
    #[must_use]
    pub const fn is_info(&self) -> bool {
        matches!(self, Self::Info)
    }

    /// Returns the severity as a static string.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Fatal => "FATAL",
            Self::Recoverable => "RECOVERABLE",
            Self::Warning => "WARNING",
            Self::Info => "INFO",
        }
    }
}

impl fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

mod config;
mod context;
mod data;
mod exchange;
mod network;
mod storage;
mod strategy;

pub use config::ConfigError;
pub use context::{ContextualError, ErrorContext};
pub use data::DataError;
pub use exchange::ExchangeError;
pub use network::NetworkError;
pub use storage::StorageError;
pub use strategy::StrategyError;

/// Top-level error type for the Zephyr trading system.
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZephyrError {
    /// Network-related error.
    #[error("{0}")]
    Network(#[from] NetworkError),

    /// Exchange API error.
    #[error("{0}")]
    Exchange(#[from] ExchangeError),

    /// Data parsing or validation error.
    #[error("{0}")]
    Data(#[from] DataError),

    /// Strategy execution error.
    #[error("{0}")]
    Strategy(#[from] StrategyError),

    /// Configuration error.
    #[error("{0}")]
    Config(#[from] ConfigError),

    /// Storage or I/O error.
    #[error("{0}")]
    Storage(#[from] StorageError),
}

impl ZephyrError {
    /// Returns the severity level of this error.
    #[must_use]
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::Network(e) => e.severity(),
            Self::Exchange(e) => e.severity(),
            Self::Data(e) => e.severity(),
            Self::Strategy(e) => e.severity(),
            Self::Config(e) => e.severity(),
            Self::Storage(e) => e.severity(),
        }
    }

    /// Returns true if this error is recoverable.
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        self.severity().is_recoverable()
    }

    /// Wraps this error with context information.
    #[must_use]
    pub fn with_context(self, context: ErrorContext) -> ContextualError {
        ContextualError::new(self, context)
    }

    /// Returns true if this is a network error.
    #[must_use]
    pub fn is_network_error(&self) -> bool {
        matches!(self, Self::Network(_))
    }

    /// Returns true if this is an exchange error.
    #[must_use]
    pub fn is_exchange_error(&self) -> bool {
        matches!(self, Self::Exchange(_))
    }

    /// Returns true if this is a data error.
    #[must_use]
    pub fn is_data_error(&self) -> bool {
        matches!(self, Self::Data(_))
    }

    /// Returns true if this is a strategy error.
    #[must_use]
    pub fn is_strategy_error(&self) -> bool {
        matches!(self, Self::Strategy(_))
    }

    /// Returns true if this is a config error.
    #[must_use]
    pub fn is_config_error(&self) -> bool {
        matches!(self, Self::Config(_))
    }

    /// Returns true if this is a storage error.
    #[must_use]
    pub fn is_storage_error(&self) -> bool {
        matches!(self, Self::Storage(_))
    }

    /// Returns a suggested retry delay in milliseconds, if applicable.
    #[must_use]
    pub fn suggested_retry_delay_ms(&self) -> Option<u64> {
        match self {
            Self::Network(e) => e.suggested_retry_delay_ms(),
            Self::Exchange(e) => e.suggested_retry_delay_ms(),
            Self::Data(e) => e.suggested_retry_delay_ms(),
            Self::Strategy(e) => e.suggested_retry_delay_ms(),
            Self::Config(e) => e.suggested_retry_delay_ms(),
            Self::Storage(e) => e.suggested_retry_delay_ms(),
        }
    }

    /// Returns the error category as a string.
    #[must_use]
    pub fn category(&self) -> &'static str {
        match self {
            Self::Network(_) => "network",
            Self::Exchange(_) => "exchange",
            Self::Data(_) => "data",
            Self::Strategy(_) => "strategy",
            Self::Config(_) => "config",
            Self::Storage(_) => "storage",
        }
    }

    /// Returns the inner network error, if this is a network error.
    #[must_use]
    pub fn as_network_error(&self) -> Option<&NetworkError> {
        match self {
            Self::Network(e) => Some(e),
            _ => None,
        }
    }

    /// Returns the inner exchange error, if this is an exchange error.
    #[must_use]
    pub fn as_exchange_error(&self) -> Option<&ExchangeError> {
        match self {
            Self::Exchange(e) => Some(e),
            _ => None,
        }
    }

    /// Returns the inner data error, if this is a data error.
    #[must_use]
    pub fn as_data_error(&self) -> Option<&DataError> {
        match self {
            Self::Data(e) => Some(e),
            _ => None,
        }
    }

    /// Returns the inner strategy error, if this is a strategy error.
    #[must_use]
    pub fn as_strategy_error(&self) -> Option<&StrategyError> {
        match self {
            Self::Strategy(e) => Some(e),
            _ => None,
        }
    }

    /// Returns the inner config error, if this is a config error.
    #[must_use]
    pub fn as_config_error(&self) -> Option<&ConfigError> {
        match self {
            Self::Config(e) => Some(e),
            _ => None,
        }
    }

    /// Returns the inner storage error, if this is a storage error.
    #[must_use]
    pub fn as_storage_error(&self) -> Option<&StorageError> {
        match self {
            Self::Storage(e) => Some(e),
            _ => None,
        }
    }
}

/// Extension trait for adding context to errors.
pub trait WithContext {
    /// Wraps the error with context information.
    fn with_context(self, context: ErrorContext) -> ContextualError;
}

impl WithContext for ZephyrError {
    fn with_context(self, context: ErrorContext) -> ContextualError {
        ContextualError::new(self, context)
    }
}

/// A specialized Result type for Zephyr operations.
pub type Result<T> = std::result::Result<T, ZephyrError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_severity_display() {
        assert_eq!(ErrorSeverity::Fatal.to_string(), "FATAL");
        assert_eq!(ErrorSeverity::Recoverable.to_string(), "RECOVERABLE");
        assert_eq!(ErrorSeverity::Warning.to_string(), "WARNING");
        assert_eq!(ErrorSeverity::Info.to_string(), "INFO");
    }

    #[test]
    fn test_error_severity_is_recoverable() {
        assert!(!ErrorSeverity::Fatal.is_recoverable());
        assert!(ErrorSeverity::Recoverable.is_recoverable());
        assert!(ErrorSeverity::Warning.is_recoverable());
        assert!(ErrorSeverity::Info.is_recoverable());
    }

    #[test]
    fn test_error_severity_is_fatal() {
        assert!(ErrorSeverity::Fatal.is_fatal());
        assert!(!ErrorSeverity::Recoverable.is_fatal());
        assert!(!ErrorSeverity::Warning.is_fatal());
        assert!(!ErrorSeverity::Info.is_fatal());
    }

    #[test]
    fn test_network_error_conversion() {
        let network_err = NetworkError::Timeout { timeout_ms: 5000 };
        let zephyr_err: ZephyrError = network_err.clone().into();
        assert!(zephyr_err.is_network_error());
        assert_eq!(zephyr_err.category(), "network");
        assert_eq!(zephyr_err.as_network_error(), Some(&network_err));
    }

    #[test]
    fn test_exchange_error_conversion() {
        let exchange_err = ExchangeError::RateLimited {
            retry_after_ms: 1000,
        };
        let zephyr_err: ZephyrError = exchange_err.clone().into();
        assert!(zephyr_err.is_exchange_error());
        assert_eq!(zephyr_err.category(), "exchange");
        assert_eq!(zephyr_err.as_exchange_error(), Some(&exchange_err));
    }

    #[test]
    fn test_data_error_conversion() {
        let data_err = DataError::MissingData {
            description: "price field".to_string(),
        };
        let zephyr_err: ZephyrError = data_err.clone().into();
        assert!(zephyr_err.is_data_error());
        assert_eq!(zephyr_err.category(), "data");
        assert_eq!(zephyr_err.as_data_error(), Some(&data_err));
    }

    #[test]
    fn test_strategy_error_conversion() {
        let strategy_err = StrategyError::InvalidSignal {
            strategy: "momentum".to_string(),
            reason: "Invalid signal".to_string(),
        };
        let zephyr_err: ZephyrError = strategy_err.clone().into();
        assert!(zephyr_err.is_strategy_error());
        assert_eq!(zephyr_err.category(), "strategy");
        assert_eq!(zephyr_err.as_strategy_error(), Some(&strategy_err));
    }

    #[test]
    fn test_config_error_conversion() {
        let config_err = ConfigError::MissingField {
            field: "api_key".to_string(),
            section: None,
        };
        let zephyr_err: ZephyrError = config_err.clone().into();
        assert!(zephyr_err.is_config_error());
        assert_eq!(zephyr_err.category(), "config");
        assert_eq!(zephyr_err.as_config_error(), Some(&config_err));
    }

    #[test]
    fn test_storage_error_conversion() {
        let storage_err = StorageError::NotFound {
            path: "order_123".to_string(),
        };
        let zephyr_err: ZephyrError = storage_err.clone().into();
        assert!(zephyr_err.is_storage_error());
        assert_eq!(zephyr_err.category(), "storage");
        assert_eq!(zephyr_err.as_storage_error(), Some(&storage_err));
    }

    #[test]
    fn test_is_recoverable_delegates() {
        let recoverable = ZephyrError::Network(NetworkError::Timeout { timeout_ms: 5000 });
        assert!(recoverable.is_recoverable());

        let non_recoverable = ZephyrError::Network(NetworkError::DnsResolution {
            host: "example.com".to_string(),
        });
        assert!(!non_recoverable.is_recoverable());
    }

    #[test]
    fn test_suggested_retry_delay_delegates() {
        let err = ZephyrError::Exchange(ExchangeError::RateLimited {
            retry_after_ms: 2000,
        });
        assert_eq!(err.suggested_retry_delay_ms(), Some(2000));
    }

    #[test]
    fn test_serde_roundtrip() {
        let err = ZephyrError::Network(NetworkError::Timeout { timeout_ms: 3000 });
        let json = serde_json::to_string(&err).unwrap();
        let parsed: ZephyrError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, parsed);
    }

    #[test]
    fn test_as_methods_return_none_for_wrong_type() {
        let err = ZephyrError::Network(NetworkError::Timeout { timeout_ms: 1000 });
        assert!(err.as_network_error().is_some());
        assert!(err.as_exchange_error().is_none());
        assert!(err.as_data_error().is_none());
        assert!(err.as_strategy_error().is_none());
        assert!(err.as_config_error().is_none());
        assert!(err.as_storage_error().is_none());
    }

    #[test]
    fn test_display() {
        let err = ZephyrError::Network(NetworkError::Timeout { timeout_ms: 5000 });
        let display = format!("{err}");
        assert!(display.contains("5000ms"));
    }
}
