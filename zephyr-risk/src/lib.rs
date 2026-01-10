//! # Zephyr Risk
//!
//! Risk control and management for the Zephyr trading system.
//!
//! This crate provides:
//! - [`RiskController`] - Main risk control engine
//! - Position limit enforcement
//! - Order frequency limiting
//! - Daily loss limits
//! - [`CircuitBreaker`] - Circuit breaker for external dependencies
//! - [`RetryPolicy`] - Retry strategies with exponential backoff
//!
//! # Example
//!
//! ```
//! use zephyr_risk::{RiskController, RiskConfig, RiskLimits};
//! use zephyr_core::types::{Symbol, Quantity, Amount};
//! use rust_decimal_macros::dec;
//!
//! let config = RiskConfig::default();
//! let controller = RiskController::new(config);
//!
//! // Check if an order passes risk checks
//! let symbol = Symbol::new("BTC-USDT").unwrap();
//! let quantity = Quantity::new(dec!(0.1)).unwrap();
//! // controller.check_order(&symbol, quantity, ...);
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

pub mod approval;
mod circuit_breaker;
mod controller;
mod error;
pub mod kill_switch;
mod limits;
mod retry;
pub mod stress;
pub mod var;

pub use approval::{
    ApprovalConfig, ApprovalError, ApprovalLevel, ApprovalRequest, ApprovalStatus, ApprovalType,
    ApprovalWorkflow, Approver,
};
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};
pub use controller::{RiskCheckResult, RiskController};
pub use error::RiskError;
pub use kill_switch::{
    KillSwitch, KillSwitchConfig, KillSwitchState, KillSwitchTrigger, LiquidationOrder,
    LiquidationStatus, PositionToLiquidate,
};
pub use limits::{FrequencyLimit, RiskConfig, RiskLimits, SymbolLimits};
pub use retry::{BackoffStrategy, RetryConfig, RetryPolicy, RetryPolicyBuilder};
pub use stress::{
    ScenarioType, StressPortfolio, StressPosition, StressScenario, StressTestResult, StressTester,
    StressTesterConfig,
};
pub use var::{VarCalculator, VarConfig, VarMethod, VarResult};
