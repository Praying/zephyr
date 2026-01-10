//! Cryptocurrency-specific features and calculations.
//!
//! This module provides functionality specific to cryptocurrency trading:
//! - Funding rate calculations for perpetual contracts
//! - Margin and liquidation price calculations
//! - Contract information management
//!
//! # Modules
//!
//! - [`funding`] - Funding rate calculations and position cost adjustments
//! - [`margin`] - Margin, liquidation, and unrealized PnL calculations
//! - [`contract`] - Contract information management and symbol normalization

pub mod contract;
pub mod funding;
pub mod margin;

pub use contract::{ContractInfo, ContractManager, ContractType, MarginMode, SymbolNormalizer};
pub use funding::{FundingCalculator, FundingPayment, FundingSchedule};
pub use margin::{LiquidationCalculator, MarginCalculator, MarginRequirement};
