//! NewType wrappers for financial primitives.
//!
//! This module provides type-safe wrappers around decimal values
//! to prevent mixing incompatible types at compile time.
//!
//! # Types
//!
//! - [`Price`] - Asset price values
//! - [`Quantity`] - Trading quantities
//! - [`Amount`] - Monetary amounts (price × quantity)
//! - [`Symbol`] - Trading pair identifiers
//! - [`OrderId`] - Order identifiers
//! - [`Timestamp`] - Unix millisecond timestamps
//! - [`Leverage`] - Leverage multipliers
//! - [`FundingRate`] - Perpetual contract funding rates
//! - [`MarkPrice`] - Mark price values

mod amount;
mod funding_rate;
mod leverage;
mod mark_price;
mod order_id;
mod price;
mod quantity;
mod symbol;
mod timestamp;

pub use amount::Amount;
pub use funding_rate::FundingRate;
pub use leverage::Leverage;
pub use mark_price::MarkPrice;
pub use order_id::OrderId;
pub use price::Price;
pub use quantity::Quantity;
pub use symbol::Symbol;
pub use timestamp::Timestamp;

/// Validation error for `NewType` construction.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    /// Price value is negative
    #[error("price cannot be negative: {0}")]
    NegativePrice(rust_decimal::Decimal),

    /// Quantity value is negative
    #[error("quantity cannot be negative: {0}")]
    NegativeQuantity(rust_decimal::Decimal),

    /// Amount value is negative
    #[error("amount cannot be negative: {0}")]
    NegativeAmount(rust_decimal::Decimal),

    /// Symbol format is invalid
    #[error("invalid symbol format: {0}")]
    InvalidSymbol(String),

    /// Symbol is empty
    #[error("symbol cannot be empty")]
    EmptySymbol,

    /// Order ID is empty
    #[error("order ID cannot be empty")]
    EmptyOrderId,

    /// Timestamp is invalid (zero or negative)
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(i64),

    /// Leverage exceeds maximum allowed value
    #[error("leverage {0} exceeds maximum {1}")]
    LeverageExceedsMax(u8, u8),

    /// Leverage is zero
    #[error("leverage cannot be zero")]
    ZeroLeverage,

    /// Mark price is negative
    #[error("mark price cannot be negative: {0}")]
    NegativeMarkPrice(rust_decimal::Decimal),
}

/// Strategy type enumeration.
///
/// Defines the different types of trading strategies supported by the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StrategyType {
    /// CTA (Commodity Trading Advisor) strategy - trend following, medium/low frequency
    Cta,
    /// HFT (High-Frequency Trading) strategy - market making, arbitrage
    Hft,
    /// UFT (Ultra-Fast Trading) strategy - ultra-low latency trading
    Uft,
    /// SEL (Selection/Scheduled) strategy - portfolio rebalancing, periodic execution
    Sel,
}

impl std::fmt::Display for StrategyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cta => write!(f, "cta"),
            Self::Hft => write!(f, "hft"),
            Self::Uft => write!(f, "uft"),
            Self::Sel => write!(f, "sel"),
        }
    }
}
