//! Python strategy adapter using `PyO3`.
//!
//! This module provides the `PyStrategyAdapter` struct that wraps a Python strategy
//! object and implements the Rust `Strategy` trait, enabling Python strategies to
//! be used seamlessly within the Zephyr trading system.
//!
//! # Architecture
//!
//! The adapter delegates all strategy method calls to the underlying Python object
//! via `PyO3`. Each call acquires the GIL (Global Interpreter Lock) to safely interact
//! with the Python interpreter.
//!
//! # Error Handling
//!
//! Python exceptions are caught and logged, allowing the strategy to continue
//! execution. This ensures that a single Python error doesn't crash the entire
//! trading system.

use crate::context::StrategyContext;
use crate::loader::py_bindings::{PyBar, PyContextWrapper, PyTick};
use crate::r#trait::{DataType, Strategy, Subscription, Timeframe};
use crate::types::{Bar, Tick};
use anyhow::{Result, anyhow};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use rust_decimal::prelude::ToPrimitive;
use tracing::{debug, error};
use zephyr_core::data::OrderStatus;
use zephyr_core::types::Symbol;

/// Wraps a Python strategy object to implement the Rust `Strategy` trait.
///
/// The adapter holds a reference to the Python strategy instance and delegates
/// all trait method calls to the corresponding Python methods.
///
/// # Thread Safety
///
/// The adapter is `Send + Sync` because `PyO3`'s `Py<PyAny>` is thread-safe.
/// All Python interactions acquire the GIL, ensuring safe concurrent access.
///
/// # Hot Reload Support
///
/// The adapter supports hot reload through the `extract_state()` and `restore_state()`
/// methods, which attempt to preserve strategy state across reloads.
///
/// # Example Python Strategy
///
/// ```python
/// class MyStrategy:
///     def __init__(self, config: str):
///         self.config = json.loads(config)
///         self.name = self.config.get("name", "my_strategy")
///
///     def required_subscriptions(self) -> list:
///         return [("BTC-USDT", "tick"), ("ETH-USDT", "bar_1h")]
///
///     def on_init(self, ctx):
///         pass
///
///     def on_tick(self, tick, ctx):
///         # Process tick data
///         pass
///
///     def on_bar(self, bar, ctx):
///         # Process bar data
///         pass
///
///     def on_stop(self, ctx):
///         pass
///
///     # Optional: For hot reload state preservation
///     def get_state(self) -> dict:
///         return {"position": self.position, "signals": self.signals}
///
///     def set_state(self, state: dict):
///         self.position = state.get("position", 0)
///         self.signals = state.get("signals", [])
/// ```
pub struct PyStrategyAdapter {
    /// Reference to the Python strategy instance
    py_instance: Py<PyAny>,
    /// Cached strategy name
    name: String,
    /// Cached subscriptions (extracted once during construction)
    subscriptions: Vec<Subscription>,
}

impl PyStrategyAdapter {
    /// Creates a new `PyStrategyAdapter` wrapping the given Python object.
    ///
    /// # Arguments
    ///
    /// * `py_instance` - The Python strategy instance
    /// * `name` - The strategy name
    /// * `subscriptions` - Pre-extracted subscriptions from the Python strategy
    ///
    /// # Returns
    ///
    /// A new `PyStrategyAdapter` instance
    #[must_use]
    pub fn new(py_instance: Py<PyAny>, name: String, subscriptions: Vec<Subscription>) -> Self {
        Self {
            py_instance,
            name,
            subscriptions,
        }
    }

    /// Returns a reference to the underlying Python object.
    ///
    /// This can be used for advanced operations like state extraction
    /// during hot reload.
    #[must_use]
    pub fn py_instance(&self) -> &Py<PyAny> {
        &self.py_instance
    }

    /// Replaces the underlying Python instance.
    ///
    /// Used during hot reload to swap in a new Python strategy instance.
    pub fn replace_instance(&mut self, new_instance: Py<PyAny>, subscriptions: Vec<Subscription>) {
        self.py_instance = new_instance;
        self.subscriptions = subscriptions;
    }

    /// Extracts state from the Python strategy for hot reload.
    ///
    /// Attempts to call `get_state()` first, then falls back to `__dict__` serialization.
    /// Returns `None` if state extraction fails or the strategy doesn't support it.
    ///
    /// # State Extraction Order
    ///
    /// 1. Try calling `get_state()` method if defined
    /// 2. Fall back to copying `__dict__` (excluding non-picklable objects)
    /// 3. Return `None` if both fail
    #[must_use]
    pub fn extract_state(&self) -> Option<Py<PyAny>> {
        Python::with_gil(|py| {
            let instance = self.py_instance.bind(py);

            // Try get_state() first
            if instance.hasattr("get_state").unwrap_or(false) {
                match instance.call_method0("get_state") {
                    Ok(state) => {
                        tracing::debug!(
                            strategy = %self.name,
                            "Extracted state via get_state()"
                        );
                        return Some(state.unbind());
                    }
                    Err(e) => {
                        tracing::warn!(
                            strategy = %self.name,
                            error = %e,
                            "get_state() failed, trying __dict__"
                        );
                    }
                }
            }

            // Fall back to __dict__
            match instance.getattr("__dict__") {
                Ok(dict) => {
                    // Try to make a shallow copy of the dict to avoid issues
                    // with non-picklable objects
                    match dict.call_method0("copy") {
                        Ok(copied) => {
                            tracing::debug!(
                                strategy = %self.name,
                                "Extracted state via __dict__.copy()"
                            );
                            Some(copied.unbind())
                        }
                        Err(e) => {
                            tracing::warn!(
                                strategy = %self.name,
                                error = %e,
                                "Failed to copy __dict__"
                            );
                            None
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        strategy = %self.name,
                        error = %e,
                        "Failed to access __dict__"
                    );
                    None
                }
            }
        })
    }

    /// Restores state to the Python strategy after hot reload.
    ///
    /// Attempts to call `set_state()` first, then falls back to `__dict__` update.
    ///
    /// # Arguments
    ///
    /// * `state` - The state object previously extracted via `extract_state()`
    ///
    /// # Returns
    ///
    /// `Ok(())` if state was restored successfully, `Err` with description otherwise.
    pub fn restore_state(&mut self, state: &Py<PyAny>) -> Result<(), String> {
        Python::with_gil(|py| {
            let instance = self.py_instance.bind(py);
            let state_bound = state.bind(py);

            // Try set_state() first
            if instance.hasattr("set_state").unwrap_or(false) {
                match instance.call_method1("set_state", (state_bound,)) {
                    Ok(_) => {
                        tracing::debug!(
                            strategy = %self.name,
                            "Restored state via set_state()"
                        );
                        return Ok(());
                    }
                    Err(e) => {
                        tracing::warn!(
                            strategy = %self.name,
                            error = %e,
                            "set_state() failed, trying __dict__ update"
                        );
                    }
                }
            }

            // Fall back to __dict__ update
            match instance.getattr("__dict__") {
                Ok(dict) => {
                    // Check if state is a dict
                    if state_bound.is_instance_of::<PyDict>() {
                        match dict.call_method1("update", (state_bound,)) {
                            Ok(_) => {
                                tracing::debug!(
                                    strategy = %self.name,
                                    "Restored state via __dict__.update()"
                                );
                                Ok(())
                            }
                            Err(e) => Err(format!("Failed to update __dict__: {e}")),
                        }
                    } else {
                        Err("State is not a dict, cannot restore via __dict__".to_string())
                    }
                }
                Err(e) => Err(format!("Failed to access __dict__: {e}")),
            }
        })
    }
}

/// Extracts subscriptions from a Python strategy instance.
///
/// The Python strategy should implement `required_subscriptions()` returning
/// a list of tuples: `[(symbol, data_type), ...]`
///
/// Where `data_type` is one of:
/// - `"tick"` - Real-time tick data
/// - `"bar_1m"`, `"bar_5m"`, etc. - Bar data with timeframe
/// - `"orderbook"` - Order book depth data
///
/// # Errors
///
/// Returns an error if:
/// - The Python method doesn't exist
/// - The return value is not a list
/// - Individual items are not valid (symbol, `data_type`) tuples
pub fn extract_subscriptions(
    _py: Python<'_>,
    instance: &Bound<'_, PyAny>,
) -> Result<Vec<Subscription>> {
    let mut subscriptions = Vec::new();

    // Check if the method exists
    if !instance.hasattr("required_subscriptions")? {
        debug!("Python strategy does not have required_subscriptions method, using empty list");
        return Ok(subscriptions);
    }

    let result = instance.call_method0("required_subscriptions")?;
    let list: &Bound<'_, PyList> = result
        .downcast()
        .map_err(|_| anyhow!("required_subscriptions must return a list"))?;

    for item in list.iter() {
        // Each item should be a tuple (symbol, data_type)
        let tuple: Vec<String> = item.extract().map_err(|e| {
            anyhow!("Invalid subscription format, expected (symbol, data_type) tuple: {e}")
        })?;

        if tuple.len() != 2 {
            return Err(anyhow!(
                "Invalid subscription format, expected (symbol, data_type) tuple, got {} elements",
                tuple.len()
            ));
        }

        let symbol = Symbol::new_unchecked(&tuple[0]);
        let data_type = parse_data_type(&tuple[1])?;

        subscriptions.push(Subscription::new(symbol, data_type));
    }

    Ok(subscriptions)
}

/// Parses a data type string into a `DataType` enum.
///
/// Supported formats:
/// - `"tick"` -> `DataType::Tick`
/// - `"bar_1m"`, `"bar_5m"`, etc. -> `DataType::Bar(Timeframe)`
/// - `"orderbook"` -> `DataType::OrderBook`
fn parse_data_type(s: &str) -> Result<DataType> {
    match s.to_lowercase().as_str() {
        "tick" => Ok(DataType::Tick),
        "orderbook" => Ok(DataType::OrderBook),
        s if s.starts_with("bar_") => {
            let tf_str = &s[4..];
            let timeframe = match tf_str {
                "1m" => Timeframe::M1,
                "5m" => Timeframe::M5,
                "15m" => Timeframe::M15,
                "30m" => Timeframe::M30,
                "1h" => Timeframe::H1,
                "4h" => Timeframe::H4,
                "1d" => Timeframe::D1,
                "1w" => Timeframe::W1,
                _ => return Err(anyhow!("Unknown timeframe: {tf_str}")),
            };
            Ok(DataType::Bar(timeframe))
        }
        _ => Err(anyhow!("Unknown data type: {s}")),
    }
}

// SAFETY: PyO3's Py<T> is Send + Sync when T: Send
// The GIL ensures safe access to Python objects
#[allow(unsafe_code)]
unsafe impl Send for PyStrategyAdapter {}
#[allow(unsafe_code)]
unsafe impl Sync for PyStrategyAdapter {}

impl Strategy for PyStrategyAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn required_subscriptions(&self) -> Vec<Subscription> {
        self.subscriptions.clone()
    }

    fn on_init(&mut self, ctx: &StrategyContext) -> Result<()> {
        Python::with_gil(|py| {
            // For on_init, we use a read-only dict context since we can't get a mutable ref
            let py_ctx = create_py_context_readonly(py, ctx)?;

            // Check if on_init method exists
            let instance = self.py_instance.bind(py);
            if !instance.hasattr("on_init")? {
                debug!(strategy = %self.name, "Python strategy does not have on_init method");
                return Ok(());
            }

            self.py_instance
                .call_method1(py, "on_init", (py_ctx,))
                .map_err(|e| {
                    error!(strategy = %self.name, error = %e, "Python on_init error");
                    anyhow!("Python on_init error: {e}")
                })?;

            Ok(())
        })
    }

    fn on_tick(&mut self, tick: &Tick, ctx: &mut StrategyContext) {
        Python::with_gil(|py| {
            // Convert tick to PyTick
            let py_tick: PyTick = tick.into();
            let py_tick_obj = Py::new(py, py_tick);
            let py_tick_obj = match py_tick_obj {
                Ok(t) => t,
                Err(e) => {
                    error!(strategy = %self.name, error = %e, "Failed to convert tick to Python");
                    return;
                }
            };

            // Create PyContextWrapper with mutable access
            let py_ctx = PyContextWrapper::new(ctx);
            let py_ctx_obj = match Py::new(py, py_ctx) {
                Ok(c) => c,
                Err(e) => {
                    error!(strategy = %self.name, error = %e, "Failed to create Python context");
                    return;
                }
            };

            if let Err(e) = self
                .py_instance
                .call_method1(py, "on_tick", (py_tick_obj, py_ctx_obj))
            {
                error!(strategy = %self.name, error = %e, "Python on_tick error");
            }
        });
    }

    fn on_bar(&mut self, bar: &Bar, ctx: &mut StrategyContext) {
        Python::with_gil(|py| {
            // Convert bar to PyBar
            let py_bar: PyBar = bar.into();
            let py_bar_obj = Py::new(py, py_bar);
            let py_bar_obj = match py_bar_obj {
                Ok(b) => b,
                Err(e) => {
                    error!(strategy = %self.name, error = %e, "Failed to convert bar to Python");
                    return;
                }
            };

            // Create PyContextWrapper with mutable access
            let py_ctx = PyContextWrapper::new(ctx);
            let py_ctx_obj = match Py::new(py, py_ctx) {
                Ok(c) => c,
                Err(e) => {
                    error!(strategy = %self.name, error = %e, "Failed to create Python context");
                    return;
                }
            };

            if let Err(e) = self
                .py_instance
                .call_method1(py, "on_bar", (py_bar_obj, py_ctx_obj))
            {
                error!(strategy = %self.name, error = %e, "Python on_bar error");
            }
        });
    }

    fn on_order_status(&mut self, status: &OrderStatus, ctx: &mut StrategyContext) {
        Python::with_gil(|py| {
            // Check if on_order_status method exists
            let instance = self.py_instance.bind(py);
            if instance.hasattr("on_order_status").unwrap_or(false) {
                let py_status = create_py_order_status(py, *status);

                // Create PyContextWrapper with mutable access
                let py_ctx = PyContextWrapper::new(ctx);
                let py_ctx_obj = match Py::new(py, py_ctx) {
                    Ok(c) => c,
                    Err(e) => {
                        error!(strategy = %self.name, error = %e, "Failed to create Python context");
                        return;
                    }
                };

                if let Err(e) =
                    self.py_instance
                        .call_method1(py, "on_order_status", (py_status, py_ctx_obj))
                {
                    error!(strategy = %self.name, error = %e, "Python on_order_status error");
                }
            }
        });
    }

    fn on_stop(&mut self, ctx: &mut StrategyContext) -> Result<()> {
        Python::with_gil(|py| {
            // Check if on_stop method exists
            let instance = self.py_instance.bind(py);
            if !instance.hasattr("on_stop")? {
                debug!(strategy = %self.name, "Python strategy does not have on_stop method");
                return Ok(());
            }

            // Create PyContextWrapper with mutable access
            let py_ctx = PyContextWrapper::new(ctx);
            let py_ctx_obj = Py::new(py, py_ctx)?;

            self.py_instance
                .call_method1(py, "on_stop", (py_ctx_obj,))
                .map_err(|e| {
                    error!(strategy = %self.name, error = %e, "Python on_stop error");
                    anyhow!("Python on_stop error: {e}")
                })?;

            Ok(())
        })
    }
}

/// Creates a Python dictionary representing a Tick.
/// Kept for backward compatibility - prefer using `PyTick` directly.
#[allow(dead_code)]
fn create_py_tick<'py>(py: Python<'py>, tick: &Tick) -> Result<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("symbol", tick.symbol.as_str())?;
    dict.set_item("timestamp", tick.timestamp.as_millis())?;
    dict.set_item("price", tick.price.as_decimal().to_f64().unwrap_or(0.0))?;
    dict.set_item("volume", tick.volume.as_decimal().to_f64().unwrap_or(0.0))?;

    // Add best bid/ask if available
    if let Some(bid) = tick.best_bid() {
        dict.set_item("best_bid", bid.as_decimal().to_f64().unwrap_or(0.0))?;
    }
    if let Some(ask) = tick.best_ask() {
        dict.set_item("best_ask", ask.as_decimal().to_f64().unwrap_or(0.0))?;
    }
    if let Some(spread) = tick.spread() {
        dict.set_item("spread", spread.to_f64().unwrap_or(0.0))?;
    }

    Ok(dict)
}

/// Creates a Python dictionary representing a Bar.
/// Kept for backward compatibility - prefer using `PyBar` directly.
#[allow(dead_code)]
fn create_py_bar<'py>(py: Python<'py>, bar: &Bar) -> Result<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("symbol", bar.symbol.as_str())?;
    dict.set_item("timestamp", bar.timestamp.as_millis())?;
    dict.set_item("open", bar.open.as_decimal().to_f64().unwrap_or(0.0))?;
    dict.set_item("high", bar.high.as_decimal().to_f64().unwrap_or(0.0))?;
    dict.set_item("low", bar.low.as_decimal().to_f64().unwrap_or(0.0))?;
    dict.set_item("close", bar.close.as_decimal().to_f64().unwrap_or(0.0))?;
    dict.set_item("volume", bar.volume.as_decimal().to_f64().unwrap_or(0.0))?;
    dict.set_item("period", bar.period.as_str())?;
    Ok(dict)
}

/// Creates a Python string representing an `OrderStatus` enum.
fn create_py_order_status(py: Python<'_>, status: OrderStatus) -> Bound<'_, PyAny> {
    // OrderStatus is an enum, so we just convert it to a string
    let status_str = format!("{status:?}");
    status_str
        .into_pyobject(py)
        .expect("string conversion should not fail")
        .into_any()
}

/// Creates a Python dictionary representing a read-only `StrategyContext`.
/// Used for `on_init` where we only have an immutable reference.
fn create_py_context_readonly<'py>(
    py: Python<'py>,
    ctx: &StrategyContext,
) -> Result<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("now", ctx.now().as_millis())?;

    // Add portfolio snapshot
    let portfolio = ctx.portfolio();
    let portfolio_dict = PyDict::new(py);
    portfolio_dict.set_item("cash", portfolio.cash.to_f64().unwrap_or(0.0))?;
    portfolio_dict.set_item("equity", portfolio.equity.to_f64().unwrap_or(0.0))?;
    portfolio_dict.set_item("margin_used", portfolio.margin_used.to_f64().unwrap_or(0.0))?;

    // Add positions
    let positions_dict = PyDict::new(py);
    for (symbol, position) in &portfolio.positions {
        let pos_dict = PyDict::new(py);
        pos_dict.set_item("quantity", position.quantity.to_f64().unwrap_or(0.0))?;
        pos_dict.set_item("avg_price", position.avg_price.to_f64().unwrap_or(0.0))?;
        pos_dict.set_item(
            "unrealized_pnl",
            position.unrealized_pnl.to_f64().unwrap_or(0.0),
        )?;
        positions_dict.set_item(symbol.as_str(), pos_dict)?;
    }
    portfolio_dict.set_item("positions", positions_dict)?;
    dict.set_item("portfolio", portfolio_dict)?;

    Ok(dict)
}

/// Creates a Python dictionary representing a mutable `StrategyContext`.
/// Kept for backward compatibility - prefer using `PyContextWrapper` directly.
#[allow(dead_code)]
fn create_py_context_mut<'py>(
    py: Python<'py>,
    ctx: &StrategyContext,
) -> Result<Bound<'py, PyDict>> {
    create_py_context_readonly(py, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_data_type_tick() {
        assert_eq!(parse_data_type("tick").unwrap(), DataType::Tick);
        assert_eq!(parse_data_type("TICK").unwrap(), DataType::Tick);
    }

    #[test]
    fn test_parse_data_type_orderbook() {
        assert_eq!(parse_data_type("orderbook").unwrap(), DataType::OrderBook);
    }

    #[test]
    fn test_parse_data_type_bar() {
        assert_eq!(
            parse_data_type("bar_1m").unwrap(),
            DataType::Bar(Timeframe::M1)
        );
        assert_eq!(
            parse_data_type("bar_1h").unwrap(),
            DataType::Bar(Timeframe::H1)
        );
        assert_eq!(
            parse_data_type("bar_1d").unwrap(),
            DataType::Bar(Timeframe::D1)
        );
    }

    #[test]
    fn test_parse_data_type_invalid() {
        assert!(parse_data_type("invalid").is_err());
        assert!(parse_data_type("bar_2m").is_err());
    }
}
