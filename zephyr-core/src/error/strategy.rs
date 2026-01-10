//! Strategy-related error types.
//!
//! This module provides error types for strategy operations including
//! invalid signals, position conflicts, and calculation errors.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Strategy error type covering invalid signals, position conflicts,
/// and calculation errors.
///
/// # Examples
///
/// ```
/// use zephyr_core::error::StrategyError;
///
/// let error = StrategyError::InvalidSignal {
///     strategy: "momentum".to_string(),
///     reason: "Signal value out of range".to_string(),
/// };
/// assert!(error.to_string().contains("momentum"));
/// ```
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrategyError {
    /// Strategy generated an invalid signal.
    #[error("[Strategy] Invalid signal from '{strategy}': {reason}")]
    InvalidSignal {
        /// Strategy name that generated the invalid signal.
        strategy: String,
        /// Reason why the signal is invalid.
        reason: String,
    },

    /// Position conflict detected.
    #[error("[Strategy] Position conflict for '{symbol}': {reason}")]
    PositionConflict {
        /// Symbol with the position conflict.
        symbol: String,
        /// Reason for the conflict.
        reason: String,
    },

    /// Calculation error in strategy logic.
    #[error("[Strategy] Calculation error in '{strategy}': {reason}")]
    CalculationError {
        /// Strategy name where the error occurred.
        strategy: String,
        /// Reason for the calculation error.
        reason: String,
    },

    /// Strategy initialization failed.
    #[error("[Strategy] Initialization failed for '{strategy}': {reason}")]
    InitializationFailed {
        /// Strategy name that failed to initialize.
        strategy: String,
        /// Reason for the initialization failure.
        reason: String,
    },

    /// Strategy is not found.
    #[error("[Strategy] Strategy not found: {name}")]
    NotFound {
        /// Name of the strategy that was not found.
        name: String,
    },

    /// Strategy is already running.
    #[error("[Strategy] Strategy '{name}' is already running")]
    AlreadyRunning {
        /// Name of the strategy.
        name: String,
    },

    /// Strategy is not running.
    #[error("[Strategy] Strategy '{name}' is not running")]
    NotRunning {
        /// Name of the strategy.
        name: String,
    },

    /// Strategy parameter is invalid.
    #[error("[Strategy] Invalid parameter '{param}' for '{strategy}': {reason}")]
    InvalidParameter {
        /// Strategy name.
        strategy: String,
        /// Parameter name.
        param: String,
        /// Reason why the parameter is invalid.
        reason: String,
    },

    /// Strategy execution timeout.
    #[error("[Strategy] Execution timeout for '{strategy}' after {timeout_ms}ms")]
    ExecutionTimeout {
        /// Strategy name.
        strategy: String,
        /// Timeout duration in milliseconds.
        timeout_ms: u64,
    },

    /// Strategy callback error (e.g., from Python strategy).
    #[error("[Strategy] Callback error in '{strategy}': {reason}")]
    CallbackError {
        /// Strategy name.
        strategy: String,
        /// Reason for the callback error.
        reason: String,
    },

    /// Risk limit exceeded.
    #[error("[Strategy] Risk limit exceeded for '{strategy}': {reason}")]
    RiskLimitExceeded {
        /// Strategy name.
        strategy: String,
        /// Reason for the risk limit breach.
        reason: String,
    },

    /// Insufficient data for strategy execution.
    #[error("[Strategy] Insufficient data for '{strategy}': {reason}")]
    InsufficientData {
        /// Strategy name.
        strategy: String,
        /// Reason describing what data is missing.
        reason: String,
    },
}

impl StrategyError {
    /// Returns true if this error is recoverable.
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::ExecutionTimeout { .. }
                | Self::InsufficientData { .. }
                | Self::RiskLimitExceeded { .. }
        )
    }

    /// Returns the severity level of this error.
    #[must_use]
    pub fn severity(&self) -> super::ErrorSeverity {
        use super::ErrorSeverity;
        match self {
            Self::InitializationFailed { .. } => ErrorSeverity::Fatal,
            Self::ExecutionTimeout { .. } | Self::InsufficientData { .. } => {
                ErrorSeverity::Recoverable
            }
            Self::InvalidSignal { .. }
            | Self::PositionConflict { .. }
            | Self::CalculationError { .. }
            | Self::InvalidParameter { .. }
            | Self::CallbackError { .. }
            | Self::RiskLimitExceeded { .. } => ErrorSeverity::Warning,
            Self::NotFound { .. } | Self::AlreadyRunning { .. } | Self::NotRunning { .. } => {
                ErrorSeverity::Info
            }
        }
    }

    /// Returns a suggested retry delay in milliseconds, if applicable.
    #[must_use]
    pub fn suggested_retry_delay_ms(&self) -> Option<u64> {
        match self {
            Self::ExecutionTimeout { timeout_ms, .. } => Some(*timeout_ms / 2),
            Self::InsufficientData { .. } => Some(1000),
            Self::RiskLimitExceeded { .. } => Some(5000),
            _ => None,
        }
    }

    /// Returns the strategy name associated with this error, if any.
    #[must_use]
    pub fn strategy_name(&self) -> Option<&str> {
        match self {
            Self::InvalidSignal { strategy, .. }
            | Self::CalculationError { strategy, .. }
            | Self::InitializationFailed { strategy, .. }
            | Self::InvalidParameter { strategy, .. }
            | Self::ExecutionTimeout { strategy, .. }
            | Self::CallbackError { strategy, .. }
            | Self::RiskLimitExceeded { strategy, .. }
            | Self::InsufficientData { strategy, .. } => Some(strategy),
            Self::NotFound { name } | Self::AlreadyRunning { name } | Self::NotRunning { name } => {
                Some(name)
            }
            Self::PositionConflict { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_signal() {
        let error = StrategyError::InvalidSignal {
            strategy: "momentum".to_string(),
            reason: "Signal value out of range".to_string(),
        };
        assert!(error.to_string().contains("momentum"));
        assert_eq!(error.strategy_name(), Some("momentum"));
    }

    #[test]
    fn test_position_conflict() {
        let error = StrategyError::PositionConflict {
            symbol: "BTC-USDT".to_string(),
            reason: "Conflicting signals".to_string(),
        };
        assert!(error.to_string().contains("BTC-USDT"));
        assert_eq!(error.strategy_name(), None);
    }

    #[test]
    fn test_is_recoverable() {
        let timeout = StrategyError::ExecutionTimeout {
            strategy: "test".to_string(),
            timeout_ms: 5000,
        };
        assert!(timeout.is_recoverable());

        let invalid = StrategyError::InvalidSignal {
            strategy: "test".to_string(),
            reason: "bad".to_string(),
        };
        assert!(!invalid.is_recoverable());
    }

    #[test]
    fn test_serde_roundtrip() {
        let error = StrategyError::CalculationError {
            strategy: "mean_reversion".to_string(),
            reason: "Division by zero".to_string(),
        };
        let json = serde_json::to_string(&error).unwrap();
        let parsed: StrategyError = serde_json::from_str(&json).unwrap();
        assert_eq!(error, parsed);
    }
}
