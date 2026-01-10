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
