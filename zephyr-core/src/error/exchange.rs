//! Exchange-related error types.
//!
//! This module provides error types for exchange API operations including
//! authentication failures, rate limiting, insufficient balance, and order rejections.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Exchange error type covering authentication failures, rate limiting,
/// insufficient balance, invalid parameters, and order rejection reasons.
///
/// # Examples
///
/// ```
/// use zephyr_core::error::ExchangeError;
/// use rust_decimal_macros::dec;
///
/// let error = ExchangeError::RateLimited { retry_after_ms: 1000 };
/// assert!(error.to_string().contains("1000ms"));
/// ```
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExchangeError {
    /// Authentication with the exchange failed.
    #[error("[Exchange] Authentication failed: {reason}")]
    AuthenticationFailed {
        /// Reason for the authentication failure.
        reason: String,
    },

    /// API rate limit exceeded.
    #[error("[Exchange] Rate limited, retry after {retry_after_ms}ms")]
    RateLimited {
        /// Time to wait before retrying in milliseconds.
        retry_after_ms: u64,
    },

    /// Insufficient balance for the operation.
    #[error("[Exchange] Insufficient balance: required {required}, available {available}")]
    InsufficientBalance {
        /// Required amount for the operation.
        required: Decimal,
        /// Available balance.
        available: Decimal,
    },

    /// Invalid parameter provided.
    #[error("[Exchange] Invalid parameter: {param} - {reason}")]
    InvalidParameter {
        /// Parameter name that was invalid.
        param: String,
        /// Reason why the parameter is invalid.
        reason: String,
    },

    /// Order was rejected by the exchange.
    #[error("[Exchange] Order rejected: {reason}")]
    OrderRejected {
        /// Reason for the order rejection.
        reason: String,
        /// Optional error code from the exchange.
        code: Option<i32>,
    },

    /// Position not found.
    #[error("[Exchange] Position not found: {symbol}")]
    PositionNotFound {
        /// Symbol for which position was not found.
        symbol: String,
    },

    /// Order not found.
    #[error("[Exchange] Order not found: {order_id}")]
    OrderNotFound {
        /// Order ID that was not found.
        order_id: String,
    },

    /// Exchange is under maintenance.
    #[error("[Exchange] Exchange under maintenance: {message}")]
    Maintenance {
        /// Maintenance message from the exchange.
        message: String,
    },

    /// Exchange returned an unknown error.
    #[error("[Exchange] Unknown error: code={code}, message={message}")]
    Unknown {
        /// Error code from the exchange.
        code: i32,
        /// Error message from the exchange.
        message: String,
    },

    /// API key permissions are insufficient.
    #[error("[Exchange] Insufficient permissions: {required_permission}")]
    InsufficientPermissions {
        /// Permission that is required but not granted.
        required_permission: String,
    },

    /// Symbol is not supported or invalid.
    #[error("[Exchange] Invalid symbol: {symbol}")]
    InvalidSymbol {
        /// Symbol that is invalid.
        symbol: String,
    },

    /// Leverage setting is invalid.
    #[error("[Exchange] Invalid leverage: {leverage} (max: {max_leverage})")]
    InvalidLeverage {
        /// Requested leverage.
        leverage: u8,
        /// Maximum allowed leverage.
        max_leverage: u8,
    },
}

impl ExchangeError {
    /// Returns true if this error is recoverable (can be retried).
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        matches!(self, Self::RateLimited { .. } | Self::Maintenance { .. })
    }

    /// Returns the severity level of this error.
    #[must_use]
    pub fn severity(&self) -> super::ErrorSeverity {
        use super::ErrorSeverity;
        match self {
            Self::AuthenticationFailed { .. } | Self::InsufficientPermissions { .. } => {
                ErrorSeverity::Fatal
            }
            Self::RateLimited { .. } | Self::Maintenance { .. } => ErrorSeverity::Recoverable,
            Self::InsufficientBalance { .. }
            | Self::OrderRejected { .. }
            | Self::InvalidParameter { .. }
            | Self::InvalidSymbol { .. }
            | Self::InvalidLeverage { .. }
            | Self::Unknown { .. } => ErrorSeverity::Warning,
            Self::PositionNotFound { .. } | Self::OrderNotFound { .. } => ErrorSeverity::Info,
        }
    }

    /// Returns a suggested retry delay in milliseconds, if applicable.
    #[must_use]
    pub fn suggested_retry_delay_ms(&self) -> Option<u64> {
        match self {
            Self::RateLimited { retry_after_ms } => Some(*retry_after_ms),
            Self::Maintenance { .. } => Some(60_000), // 1 minute
            _ => None,
        }
    }

    /// Returns the error code if available.
    #[must_use]
    pub fn error_code(&self) -> Option<i32> {
        match self {
            Self::OrderRejected { code, .. } => *code,
            Self::Unknown { code, .. } => Some(*code),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_authentication_failed() {
        let error = ExchangeError::AuthenticationFailed {
            reason: "Invalid API key".to_string(),
        };
        assert!(error.to_string().contains("Invalid API key"));
        assert!(!error.is_recoverable());
    }

    #[test]
    fn test_rate_limited() {
        let error = ExchangeError::RateLimited {
            retry_after_ms: 1000,
        };
        assert!(error.to_string().contains("1000ms"));
        assert!(error.is_recoverable());
        assert_eq!(error.suggested_retry_delay_ms(), Some(1000));
    }

    #[test]
    fn test_insufficient_balance() {
        let error = ExchangeError::InsufficientBalance {
            required: dec!(1000),
            available: dec!(500),
        };
        assert!(error.to_string().contains("1000"));
        assert!(error.to_string().contains("500"));
        assert!(!error.is_recoverable());
    }

    #[test]
    fn test_order_rejected() {
        let error = ExchangeError::OrderRejected {
            reason: "Price out of range".to_string(),
            code: Some(-1013),
        };
        assert!(error.to_string().contains("Price out of range"));
        assert_eq!(error.error_code(), Some(-1013));
    }

    #[test]
    fn test_serde_roundtrip() {
        let error = ExchangeError::RateLimited {
            retry_after_ms: 5000,
        };
        let json = serde_json::to_string(&error).unwrap();
        let parsed: ExchangeError = serde_json::from_str(&json).unwrap();
        assert_eq!(error, parsed);
    }
}
