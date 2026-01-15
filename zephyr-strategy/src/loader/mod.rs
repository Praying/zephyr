//! Strategy loaders for Rust and Python strategies.
//!
//! This module provides the infrastructure for loading strategies from
//! different sources:
//! - Rust strategies via compile-time registry
//! - Python strategies via PyO3 (requires `python` feature)
//!
//! # Configuration
//!
//! Strategies are configured via TOML files:
//!
//! ```toml
//! [python]
//! venv_dir = "/path/to/venv"
//! python_paths = ["./strategies"]
//!
//! [[strategies]]
//! name = "dual_thrust_btc"
//! type = "rust"
//! class = "DualThrust"
//! [strategies.params]
//! n = 20
//! k1 = 0.5
//! ```
//!
//! # Rust Strategy Loading
//!
//! Rust strategies are automatically discovered at compile time using the
//! `inventory` crate. To register a strategy:
//!
//! ```ignore
//! use zephyr_strategy::loader::{StrategyBuilder, register_strategy};
//!
//! struct MyStrategyBuilder;
//!
//! impl StrategyBuilder for MyStrategyBuilder {
//!     fn name(&self) -> &'static str {
//!         "MyStrategy"
//!     }
//!
//!     fn build(&self, config: serde_json::Value) -> anyhow::Result<Box<dyn Strategy>> {
//!         // Parse config and create strategy
//!         Ok(Box::new(MyStrategy::new(config)?))
//!     }
//! }
//!
//! // Auto-register at compile time
//! register_strategy!(MyStrategyBuilder);
//! ```

mod config;
mod rust_loader;
mod unified;

#[cfg(feature = "python")]
mod python_adapter;

#[cfg(feature = "python")]
mod python_loader;

#[cfg(feature = "python")]
mod py_bindings;

pub use config::{ConfigError, EngineConfig, PythonConfig, StrategyConfig, StrategyType};
pub use rust_loader::{
    StrategyBuilder, is_rust_strategy_registered, list_rust_strategies, load_rust_strategy,
};
pub use unified::{
    LoadError, load_all_strategies, load_python_strategy_with_config, load_strategies,
    load_strategy,
};

#[cfg(feature = "python")]
pub use python_adapter::{PyStrategyAdapter, extract_subscriptions};

#[cfg(feature = "python")]
pub use python_loader::{PythonLoadError, PythonLoader};

#[cfg(feature = "python")]
pub use py_bindings::{PyBar, PyContextWrapper, PyTick};

/// Trait for types that can be constructed as a const default.
///
/// This is used by the `register_strategy!` macro to create static
/// instances of strategy builders.
pub trait ConstDefault {
    /// The default value as a const.
    const DEFAULT: Self;
}

/// Macro for registering a Rust strategy builder.
///
/// This macro uses the `inventory` crate to automatically register
/// strategy builders at compile time. The builder type must have a
/// const constructor or be a unit struct.
///
/// # Example
///
/// ```ignore
/// use zephyr_strategy::register_strategy;
///
/// struct MyBuilder;
/// impl StrategyBuilder for MyBuilder {
///     // ...
/// }
///
/// register_strategy!(MyBuilder);
/// ```
#[macro_export]
macro_rules! register_strategy {
    ($builder:ty) => {
        const _: () = {
            static BUILDER: $builder = <$builder as $crate::loader::ConstDefault>::DEFAULT;
            ::inventory::submit! {
                &BUILDER as &'static dyn $crate::loader::StrategyBuilder
            }
        };
    };
}
