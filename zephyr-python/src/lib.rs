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
// Allow various code style issues that match project conventions
#![allow(clippy::too_many_lines)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::struct_field_names)]
#![allow(clippy::bool_assert_comparison)]
#![allow(clippy::branches_sharing_code)]
#![allow(clippy::collapsible_if)]
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
#![allow(clippy::or_fun_call)]
#![allow(clippy::let_underscore_must_use)]
#![allow(clippy::missing_trait_methods)]
#![allow(clippy::must_use_candidate)]
// Allow test-only issues
#![cfg_attr(test, allow(clippy::float_cmp))]
#![cfg_attr(test, allow(clippy::indexing_slicing))]

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
