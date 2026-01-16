//! Strategy runner actors for isolated execution.
//!
//! This module provides the `StrategyRunner` actor that manages a strategy's
//! lifecycle and processes events in isolation.
//!
//! # Architecture
//!
//! Each strategy runs in its own Tokio task, receiving commands via an mpsc channel.
//! This provides:
//! - **Isolation**: Failures in one strategy don't affect others
//! - **Concurrency**: Multiple strategies can run in parallel
//! - **Control**: Strategies can be started, stopped, and reloaded independently
//!
//! # Commands
//!
//! The runner accepts the following commands:
//! - `OnTick`: Process a tick update
//! - `OnBar`: Process a bar update
//! - `OnOrderStatus`: Handle order status change
//! - `Reload`: Hot reload Python strategy (future)
//! - `Stop`: Gracefully stop the strategy
//!
//! # Hot Reload (requires `hot-reload` feature)
//!
//! The `HotReloader` monitors Python strategy files for changes and sends
//! reload commands to the corresponding runners. Enable with:
//!
//! ```toml
//! [dependencies]
//! zephyr-strategy = { version = "0.1", features = ["hot-reload"] }
//! ```
//!
//! # Example
//!
//! ```ignore
//! use zephyr_strategy::runner::{StrategyRunner, RunnerCommand};
//! use tokio::sync::mpsc;
//!
//! // Create command channel
//! let (cmd_tx, cmd_rx) = mpsc::channel(100);
//!
//! // Create runner
//! let runner = StrategyRunner::new(strategy, ctx, cmd_rx, false);
//!
//! // Spawn the runner task
//! tokio::spawn(runner.run());
//!
//! // Send commands
//! cmd_tx.send(RunnerCommand::tick(tick)).await?;
//! cmd_tx.send(RunnerCommand::stop()).await?;
//! ```

mod command;
#[allow(clippy::module_inception)]
mod runner;

#[cfg(feature = "hot-reload")]
mod hot_reload;

pub use command::RunnerCommand;
pub use runner::{DEFAULT_ERROR_THRESHOLD, StrategyRunner};

#[cfg(feature = "hot-reload")]
pub use hot_reload::{DEFAULT_DEBOUNCE_DURATION, HotReloadError, HotReloader};
