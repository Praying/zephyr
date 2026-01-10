//! # Zephyr Python
//!
//! Python bindings for the Zephyr trading system using PyO3.
//!
//! This crate provides:
//! - Core type wrappers (Price, Quantity, Symbol, etc.)
//! - Market data structures (TickData, KlineData)
//! - Strategy context interfaces (CTA and HFT)
//! - NumPy array integration for efficient data access
//! - Exception handling with Python-native errors
//! - Strategy decorators for defining Python strategies
//!
//! # Usage
//!
//! ```python
//! import zephyr_py as zephyr
//!
//! # Create core types
//! price = zephyr.Price(42000.50)
//! qty = zephyr.Quantity(1.5)
//! symbol = zephyr.Symbol("BTC-USDT")
//!
//! # Access K-line data as numpy arrays
//! bars = ctx.get_bars_array(symbol, "1h", 100)
//! closes = bars["close"]  # numpy array
//!
//! # Define a CTA strategy using decorators
//! @zephyr.cta_strategy(name="my_strategy", symbols=["BTC-USDT"])
//! class MyStrategy:
//!     def on_bar(self, ctx, bar):
//!         pass
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::module_name_repetitions)]

mod data;
mod decorators;
mod error;
mod numpy_conv;
mod strategy;
mod types;

use pyo3::prelude::*;

/// Zephyr Python module.
///
/// Provides Python bindings for the Zephyr cryptocurrency trading system.
#[pymodule]
fn zephyr_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Register error types
    error::register_module(m)?;

    // Register core types
    types::register_module(m)?;

    // Register data structures
    data::register_module(m)?;

    // Register strategy interfaces
    strategy::register_module(m)?;

    // Register decorators
    decorators::register_module(m)?;

    // Add module metadata
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add(
        "__doc__",
        "Zephyr cryptocurrency trading system Python bindings",
    )?;

    Ok(())
}
