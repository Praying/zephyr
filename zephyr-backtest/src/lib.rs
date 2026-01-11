//! # Zephyr Backtest
//!
//! Backtesting engine for the Zephyr trading system.
//!
//! This crate provides:
//! - Historical data replay with chronological ordering
//! - Simulated order matching with slippage
//! - Performance metrics calculation (PnL, Sharpe, Max Drawdown)
//! - Deterministic backtest execution
//! - Multi-asset portfolio backtesting with correlation analysis
//! - Advanced metrics (Sortino, Calmar, Information ratio)
//! - Monte Carlo simulation for robustness testing
//! - Walk-forward optimization with out-of-sample validation

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
// Allow various code style issues that match project conventions
#![allow(clippy::too_many_lines)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::struct_field_names)]
#![allow(clippy::bool_assert_comparison)]
#![allow(clippy::branches_sharing_code)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::explicit_iter_loop)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::doc_markdown)]
// Allow async issues
#![allow(clippy::unused_async)]
#![allow(clippy::await_holding_lock)]
#![allow(clippy::future_not_send)]
// Allow cast issues
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_lossless)]
// Allow lifetime and reference issues
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::needless_pass_by_ref_mut)]
#![allow(clippy::trivially_copy_pass_by_ref)]
// Allow more issues
#![allow(clippy::wildcard_imports)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::map_identity)]
#![allow(clippy::use_self)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::must_use_unit)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::unused_self)]
// Allow test-only issues
#![cfg_attr(test, allow(clippy::float_cmp))]
#![cfg_attr(test, allow(clippy::indexing_slicing))]

/// Advanced performance metrics module.
pub mod advanced_metrics;
mod error;
mod event;
/// Order matching module.
pub mod matching;
mod metrics;
/// Monte Carlo simulation module.
pub mod monte_carlo;
/// Multi-asset portfolio backtesting module.
pub mod portfolio;
mod replayer;
/// Walk-forward optimization module.
pub mod walk_forward;

pub use advanced_metrics::{AdvancedMetrics, AdvancedMetricsCalculator, AdvancedMetricsConfig};
pub use error::BacktestError;
pub use event::{BacktestEvent, EventType};
pub use matching::{MatchEngine, MatchEngineConfig, MatchResult, SlippageModel};
pub use metrics::{BacktestMetrics, PerformanceStats, TradeRecord};
pub use monte_carlo::{
    MonteCarloConfig, MonteCarloError, MonteCarloResults, MonteCarloSimulator, SimulationPath,
};
pub use portfolio::{
    AssetPosition, CorrelationMatrix, PortfolioBacktestConfig, PortfolioBacktester,
    PortfolioMetrics,
};
pub use replayer::{DataReplayer, ReplayConfig};
pub use walk_forward::{
    AggregateMetrics, WalkForwardConfig, WalkForwardError, WalkForwardOptimizer,
    WalkForwardResults, WalkForwardWindow, WindowMetrics, WindowResult,
};

/// Prelude module for convenient imports.
pub mod prelude {
    pub use crate::advanced_metrics::{
        AdvancedMetrics, AdvancedMetricsCalculator, AdvancedMetricsConfig,
    };
    pub use crate::error::BacktestError;
    pub use crate::event::{BacktestEvent, EventType};
    pub use crate::matching::{MatchEngine, MatchResult, SlippageModel};
    pub use crate::metrics::{BacktestMetrics, PerformanceStats, TradeRecord};
    pub use crate::monte_carlo::{
        MonteCarloConfig, MonteCarloError, MonteCarloResults, MonteCarloSimulator, SimulationPath,
    };
    pub use crate::portfolio::{
        AssetPosition, CorrelationMatrix, PortfolioBacktestConfig, PortfolioBacktester,
        PortfolioMetrics,
    };
    pub use crate::replayer::{DataReplayer, ReplayConfig};
    pub use crate::walk_forward::{
        AggregateMetrics, WalkForwardConfig, WalkForwardError, WalkForwardOptimizer,
        WalkForwardResults, WalkForwardWindow, WindowMetrics, WindowResult,
    };
}
