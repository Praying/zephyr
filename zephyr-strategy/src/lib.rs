//! # Zephyr Strategy
//!
//! Strategy loading and execution framework for Zephyr trading system.
//!
//! This crate provides:
//! - `Strategy` trait for implementing trading strategies
//! - `StrategyContext` for strategy-system interaction
//! - Signal types for expressing trading intents
//! - Loaders for Rust and Python strategies
//! - Strategy runner actors for isolated execution
//!
//! ## Architecture
//!
//! The strategy framework follows a "Dual-Loop Architecture":
//! - **Fast Loop**: Strategies receive market data and emit trading intents
//! - **Slow Loop**: Execution layer validates and executes orders
//!
//! ## Example
//!
//! ```ignore
//! use zephyr_strategy::prelude::*;
//!
//! struct MyStrategy {
//!     name: String,
//! }
//!
//! impl Strategy for MyStrategy {
//!     fn name(&self) -> &str {
//!         &self.name
//!     }
//!
//!     fn required_subscriptions(&self) -> Vec<Subscription> {
//!         vec![Subscription::new(Symbol::new_unchecked("BTC-USDT"), DataType::Tick)]
//!     }
//!
//!     fn on_init(&mut self, _ctx: &StrategyContext) -> anyhow::Result<()> {
//!         Ok(())
//!     }
//!
//!     fn on_tick(&mut self, tick: &Tick, ctx: &mut StrategyContext) {
//!         // Process tick data
//!     }
//!
//!     fn on_bar(&mut self, bar: &Bar, ctx: &mut StrategyContext) {
//!         // Process bar data
//!     }
//!
//!     fn on_stop(&mut self, _ctx: &mut StrategyContext) -> anyhow::Result<()> {
//!         Ok(())
//!     }
//! }
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::module_name_repetitions)]

/// Core strategy trait definition
pub mod r#trait;

/// Strategy context (sandbox interface)
pub mod context;

/// Signal types for trading intents
pub mod signal;

/// Strategy loaders (Rust and Python)
pub mod loader;

/// Strategy runner actors
pub mod runner;

/// Built-in strategy implementations
pub mod strategies;

/// Strategy engine orchestrator
pub mod engine;

/// Subscription routing for market data
pub mod subscription;

/// Re-export core types for convenience
pub use zephyr_core::data::{KlineData, KlinePeriod, OrderStatus, TickData};
pub use zephyr_core::types::{OrderId, Price, Quantity, Symbol, Timestamp};

/// Type alias for Decimal (re-exported from `rust_decimal`)
pub type Decimal = rust_decimal::Decimal;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::Decimal;
    pub use crate::context::{Clock, EmitError, StrategyContext};
    pub use crate::engine::{
        DEFAULT_COMMAND_CHANNEL_CAPACITY, DEFAULT_SIGNAL_CHANNEL_CAPACITY,
        DEFAULT_SUBSCRIPTION_CHANNEL_CAPACITY, EngineBuilder, EngineError, EngineHandle,
        StrategyEngine, StrategyInfo,
    };
    pub use crate::loader::{
        EngineConfig, LoadError, StrategyBuilder, StrategyConfig, StrategyType,
        load_all_strategies, load_rust_strategy, load_strategies, load_strategy,
    };
    pub use crate::runner::{DEFAULT_ERROR_THRESHOLD, RunnerCommand, StrategyRunner};
    pub use crate::signal::{Signal, SignalId, Urgency};
    pub use crate::strategies::{DualThrust, DualThrustBuilder, DualThrustParams};
    pub use crate::subscription::{SubscriptionManager, SubscriptionRouter};
    pub use crate::r#trait::{DataType, Strategy, Subscription, Timeframe};
    pub use crate::{
        KlineData, KlinePeriod, OrderId, OrderStatus, Price, Quantity, Symbol, TickData, Timestamp,
    };

    #[cfg(feature = "hot-reload")]
    pub use crate::runner::{DEFAULT_DEBOUNCE_DURATION, HotReloadError, HotReloader};
}

/// Simplified type aliases for market data
pub mod types {
    pub use zephyr_core::data::KlineData as Bar;
    pub use zephyr_core::data::TickData as Tick;
}
