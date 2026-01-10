//! Execution unit framework for algorithmic order execution.
//!
//! This module provides the execution unit framework that enables
//! pluggable execution algorithms for order slicing and execution.
//!
//! # Architecture
//!
//! The execution framework follows a factory pattern:
//! - [`ExecuteUnit`] - Trait for execution algorithm implementations
//! - [`ExecuteContext`] - Provides market data and order submission
//! - [`ExecuteUnitFactory`] - Creates execution unit instances
//!
//! # Built-in Execution Units
//!
//! - [`TwapExecutor`] - Time-Weighted Average Price execution
//! - [`VwapExecutor`] - Volume-Weighted Average Price execution
//! - [`MinImpactExecutor`] - Minimum market impact execution
//!
//! # Example
//!
//! ```ignore
//! use zephyr_engine::execute::{ExecuteUnit, ExecuteContext, TwapConfig};
//!
//! let config = TwapConfig {
//!     slices: 10,
//!     interval: Duration::from_secs(60),
//!     ..Default::default()
//! };
//!
//! let mut executor = TwapExecutor::new(config);
//! executor.init(ctx, &symbol, &exec_config).await;
//! executor.set_position(target_qty).await;
//! ```

mod config;
mod context;
mod factory;
mod min_impact;
mod traits;
mod twap;
mod vwap;

pub use config::{ExecuteConfig, SlippageConfig};
pub use context::ExecuteContextImpl;
pub use factory::{DefaultExecuteUnitFactory, ExecuteUnitFactory};
pub use min_impact::{MinImpactConfig, MinImpactExecutor};
pub use traits::{ExecuteContext, ExecuteUnit, ExecutionMetrics, ExecutionProgress, SliceOrder};
pub use twap::{TwapConfig, TwapExecutor};
pub use vwap::{VwapConfig, VwapExecutor};
