//! Risk module error types.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use zephyr_core::types::{Amount, Quantity, Symbol};

/// Risk-related errors.
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskError {
    /// Position limit exceeded for a symbol.
    #[error("position limit exceeded for {symbol}: current {current}, limit {limit}")]
    PositionLimitExceeded {
        /// The symbol that exceeded the limit.
        symbol: Symbol,
        /// Current position quantity.
        current: Quantity,
        /// Maximum allowed position.
        limit: Quantity,
    },

    /// Account-wide position limit exceeded.
    #[error("account position limit exceeded: current {current}, limit {limit}")]
    AccountPositionLimitExceeded {
        /// Current total position value.
        current: Amount,
        /// Maximum allowed position value.
        limit: Amount,
    },

    /// Order frequency limit exceeded.
    #[error("order frequency limit exceeded: {count} orders in {window_secs}s, limit {limit}")]
    FrequencyLimitExceeded {
        /// Number of orders in the window.
        count: u32,
        /// Time window in seconds.
        window_secs: u64,
        /// Maximum allowed orders.
        limit: u32,
    },

    /// Daily loss limit exceeded.
    #[error("daily loss limit exceeded: loss {loss}, limit {limit}")]
    DailyLossLimitExceeded {
        /// Current daily loss.
        loss: Amount,
        /// Maximum allowed daily loss.
        limit: Amount,
    },

    /// Trading is paused.
    #[error("trading is paused: {reason}")]
    TradingPaused {
        /// Reason for the pause.
        reason: String,
    },

    /// Circuit breaker is open.
    #[error("circuit breaker open for {service}: {reason}")]
    CircuitBreakerOpen {
        /// The service that triggered the circuit breaker.
        service: String,
        /// Reason the circuit breaker opened.
        reason: String,
    },

    /// Order would exceed risk limits.
    #[error("order rejected: {reason}")]
    OrderRejected {
        /// Reason for rejection.
        reason: String,
    },

    /// Invalid risk configuration.
    #[error("invalid risk configuration: {reason}")]
    InvalidConfig {
        /// Reason the configuration is invalid.
        reason: String,
    },
}

impl RiskError {
    /// Returns true if this error indicates a temporary condition that may resolve.
    #[must_use]
    pub const fn is_temporary(&self) -> bool {
        matches!(
            self,
            Self::FrequencyLimitExceeded { .. }
                | Self::CircuitBreakerOpen { .. }
                | Self::TradingPaused { .. }
        )
    }

    /// Returns true if this error indicates a limit was exceeded.
    #[must_use]
    pub const fn is_limit_exceeded(&self) -> bool {
        matches!(
            self,
            Self::PositionLimitExceeded { .. }
                | Self::AccountPositionLimitExceeded { .. }
                | Self::FrequencyLimitExceeded { .. }
                | Self::DailyLossLimitExceeded { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_position_limit_error_display() {
        let err = RiskError::PositionLimitExceeded {
            symbol: Symbol::new("BTC-USDT").unwrap(),
            current: Quantity::new(dec!(1.5)).unwrap(),
            limit: Quantity::new(dec!(1.0)).unwrap(),
        };
        let msg = err.to_string();
        assert!(msg.contains("BTC-USDT"));
        assert!(msg.contains("1.5"));
        assert!(msg.contains("1.0"));
    }

    #[test]
    fn test_is_temporary() {
        let temp = RiskError::FrequencyLimitExceeded {
            count: 100,
            window_secs: 60,
            limit: 50,
        };
        assert!(temp.is_temporary());

        let perm = RiskError::PositionLimitExceeded {
            symbol: Symbol::new("BTC-USDT").unwrap(),
            current: Quantity::new(dec!(1.5)).unwrap(),
            limit: Quantity::new(dec!(1.0)).unwrap(),
        };
        assert!(!perm.is_temporary());
    }

    #[test]
    fn test_is_limit_exceeded() {
        let limit_err = RiskError::DailyLossLimitExceeded {
            loss: Amount::new(dec!(1000)).unwrap(),
            limit: Amount::new(dec!(500)).unwrap(),
        };
        assert!(limit_err.is_limit_exceeded());

        let other_err = RiskError::TradingPaused {
            reason: "manual".to_string(),
        };
        assert!(!other_err.is_limit_exceeded());
    }

    #[test]
    fn test_serde_roundtrip() {
        let err = RiskError::PositionLimitExceeded {
            symbol: Symbol::new("ETH-USDT").unwrap(),
            current: Quantity::new(dec!(10)).unwrap(),
            limit: Quantity::new(dec!(5)).unwrap(),
        };
        let json = serde_json::to_string(&err).unwrap();
        let parsed: RiskError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, parsed);
    }
}
