//! Portfolio Management Module.
//!
//! This module provides strategy portfolio management functionality including:
//! - [`Portfolio`] - Strategy portfolio containing multiple strategies
//! - [`StrategyAllocation`] - Strategy allocation with weight and multiplier
//! - [`PortfolioRiskLimits`] - Portfolio-level risk limits
//! - [`PortfolioManager`] - Portfolio management operations
//! - [`PortfolioMetrics`] - Portfolio performance metrics
//!
//! # Architecture
//!
//! The portfolio manager implements the M+1+N execution architecture:
//! - M strategies generate signals
//! - 1 signal aggregator merges positions (with self-trading prevention)
//! - N execution channels handle order routing
//!
//! # Example
//!
//! ```ignore
//! use zephyr_engine::portfolio::{Portfolio, PortfolioManager, PortfolioManagerImpl};
//!
//! let manager = PortfolioManagerImpl::new();
//! let portfolio_id = manager.create_portfolio(config).await?;
//! let signals = manager.merge_signals(&portfolio_id, strategy_signals);
//! ```

mod manager;
mod merger;
mod metrics;
mod types;

pub use manager::{PortfolioError, PortfolioManager, PortfolioManagerImpl};
pub use merger::{ExecutionSignal, MergeConfig, SignalMerger};
pub use metrics::{PortfolioMetrics, StrategyAttribution};
pub use types::{
    Portfolio, PortfolioConfig, PortfolioId, PortfolioRiskLimits, PortfolioStatus,
    StrategyAllocation, StrategyId,
};
