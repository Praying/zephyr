//! Backtest error types.

use thiserror::Error;
use zephyr_core::types::Symbol;

/// Backtest error type.
#[derive(Error, Debug)]
pub enum BacktestError {
    /// No data available for replay
    #[error("no data available for symbol {0}")]
    NoData(Symbol),

    /// Data is not sorted chronologically
    #[error("data is not sorted chronologically at index {index}: {current} > {next}")]
    UnsortedData {
        /// Index where the error occurred
        index: usize,
        /// Current timestamp
        current: i64,
        /// Next timestamp (should be >= current)
        next: i64,
    },

    /// Invalid configuration
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// Order validation failed
    #[error("order validation failed: {0}")]
    OrderValidation(String),

    /// Insufficient balance
    #[error("insufficient balance: required {required}, available {available}")]
    InsufficientBalance {
        /// Required amount
        required: String,
        /// Available amount
        available: String,
    },

    /// Position not found
    #[error("position not found for symbol {0}")]
    PositionNotFound(Symbol),

    /// Invalid order state
    #[error("invalid order state: {0}")]
    InvalidOrderState(String),

    /// Data error
    #[error("data error: {0}")]
    Data(#[from] zephyr_core::data::DataValidationError),

    /// Internal error
    #[error("internal error: {0}")]
    Internal(String),
}
