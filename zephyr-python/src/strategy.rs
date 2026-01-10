//! Python strategy context interfaces.
//!
//! This module provides Python-compatible wrappers for strategy contexts,
//! allowing Python strategies to interact with the Zephyr trading engine.

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::data::{PyKlineData, PyKlinePeriod, PyTickData};
use crate::numpy_conv;
use crate::types::{PyOrderId, PyPrice, PyQuantity, PySymbol, PyTimestamp};

/// Order execution flags for HFT strategies.
#[pyclass(name = "OrderFlag", module = "zephyr_py", eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyOrderFlag {
    /// Normal order execution.
    Normal = 0,
    /// Fill and Kill - fill what's available immediately, cancel the rest.
    Fak = 1,
    /// Fill or Kill - fill completely or cancel entirely.
    Fok = 2,
    /// Post-only - only add liquidity, reject if would take.
    PostOnly = 3,
    /// Reduce-only - only reduce existing position.
    ReduceOnly = 4,
}

#[pymethods]
impl PyOrderFlag {
    /// Returns the string representation.
    pub fn __str__(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Fak => "fak",
            Self::Fok => "fok",
            Self::PostOnly => "post_only",
            Self::ReduceOnly => "reduce_only",
        }
    }

    /// Returns the debug representation.
    pub fn __repr__(&self) -> String {
        format!("OrderFlag.{}", self.name())
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Fak => "Fak",
            Self::Fok => "Fok",
            Self::PostOnly => "PostOnly",
            Self::ReduceOnly => "ReduceOnly",
        }
    }
}

impl PyOrderFlag {
    /// Converts to core OrderFlag.
    #[allow(dead_code)]
    pub(crate) fn to_core(self) -> zephyr_core::traits::OrderFlag {
        match self {
            Self::Normal => zephyr_core::traits::OrderFlag::Normal,
            Self::Fak => zephyr_core::traits::OrderFlag::Fak,
            Self::Fok => zephyr_core::traits::OrderFlag::Fok,
            Self::PostOnly => zephyr_core::traits::OrderFlag::PostOnly,
            Self::ReduceOnly => zephyr_core::traits::OrderFlag::ReduceOnly,
        }
    }
}

/// Log level for strategy logging.
#[pyclass(name = "LogLevel", module = "zephyr_py", eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyLogLevel {
    /// Trace level - most verbose.
    Trace = 0,
    /// Debug level.
    Debug = 1,
    /// Info level.
    Info = 2,
    /// Warning level.
    Warn = 3,
    /// Error level.
    Error = 4,
}

#[pymethods]
impl PyLogLevel {
    /// Returns the string representation.
    pub fn __str__(&self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }

    /// Returns the debug representation.
    pub fn __repr__(&self) -> String {
        format!("LogLevel.{}", self.name())
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Trace => "Trace",
            Self::Debug => "Debug",
            Self::Info => "Info",
            Self::Warn => "Warn",
            Self::Error => "Error",
        }
    }
}

impl PyLogLevel {
    /// Converts to core LogLevel.
    pub(crate) fn to_core(self) -> zephyr_core::traits::LogLevel {
        match self {
            Self::Trace => zephyr_core::traits::LogLevel::Trace,
            Self::Debug => zephyr_core::traits::LogLevel::Debug,
            Self::Info => zephyr_core::traits::LogLevel::Info,
            Self::Warn => zephyr_core::traits::LogLevel::Warn,
            Self::Error => zephyr_core::traits::LogLevel::Error,
        }
    }
}

/// CTA Strategy Context for Python strategies.
///
/// Provides data access and position management for CTA strategies.
/// CTA strategies use a "set position" model where they declare their
/// target position and the engine handles order execution.
///
/// # Examples
///
/// ```python
/// class MyStrategy:
///     def on_bar(self, ctx: CtaStrategyContext, bar: KlineData):
///         # Get current position
///         pos = ctx.get_position(bar.symbol)
///         
///         # Get historical bars as numpy arrays
///         bars = ctx.get_bars_array(bar.symbol, "1h", 20)
///         closes = bars["close"]
///         
///         # Simple momentum strategy
///         if bar.close > bar.open:
///             ctx.set_position(bar.symbol, Quantity(1.0), "long_signal")
///         else:
///             ctx.set_position(bar.symbol, Quantity(0.0), "exit_signal")
/// ```
#[pyclass(name = "CtaStrategyContext", module = "zephyr_py", subclass)]
#[derive(Debug)]
pub struct PyCtaStrategyContext {
    /// Strategy name
    name: String,
    /// Mock positions for testing (in real impl, this would delegate to Rust context)
    positions: std::collections::HashMap<String, f64>,
    /// Mock prices for testing
    prices: std::collections::HashMap<String, f64>,
    /// Mock kline cache for testing
    klines: std::collections::HashMap<String, Vec<zephyr_core::data::KlineData>>,
    /// Mock tick cache for testing
    ticks: std::collections::HashMap<String, Vec<zephyr_core::data::TickData>>,
    /// Current timestamp
    current_time: i64,
}

#[pymethods]
impl PyCtaStrategyContext {
    /// Creates a new CTA strategy context (for testing).
    #[new]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            positions: std::collections::HashMap::new(),
            prices: std::collections::HashMap::new(),
            klines: std::collections::HashMap::new(),
            ticks: std::collections::HashMap::new(),
            current_time: zephyr_core::types::Timestamp::now().as_millis(),
        }
    }

    /// Gets the current position for a symbol.
    ///
    /// Returns the theoretical position quantity (positive for long, negative for short).
    pub fn get_position(&self, symbol: &PySymbol) -> PyQuantity {
        let qty = self
            .positions
            .get(symbol.0.as_str())
            .copied()
            .unwrap_or(0.0);
        let decimal = rust_decimal::Decimal::try_from(qty).unwrap_or_default();
        PyQuantity(
            zephyr_core::types::Quantity::new(decimal)
                .unwrap_or(zephyr_core::types::Quantity::ZERO),
        )
    }

    /// Sets the target position for a symbol.
    ///
    /// The engine will calculate the required orders to reach the target position.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `qty` - Target position quantity
    /// * `tag` - A tag for identifying this position change
    pub fn set_position(&mut self, symbol: &PySymbol, qty: &PyQuantity, tag: &str) -> PyResult<()> {
        let qty_f64: f64 = qty.0.as_decimal().try_into().unwrap_or(0.0);
        self.positions
            .insert(symbol.0.as_str().to_string(), qty_f64);
        tracing::info!(
            strategy = %self.name,
            symbol = %symbol.0,
            position = qty_f64,
            tag = %tag,
            "Position set"
        );
        Ok(())
    }

    /// Gets the current price for a symbol.
    pub fn get_price(&self, symbol: &PySymbol) -> Option<PyPrice> {
        self.prices.get(symbol.0.as_str()).map(|&p| {
            let decimal = rust_decimal::Decimal::try_from(p).unwrap_or_default();
            PyPrice(
                zephyr_core::types::Price::new(decimal).unwrap_or(zephyr_core::types::Price::ZERO),
            )
        })
    }

    /// Gets historical K-line data for a symbol as Python objects.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `period` - The K-line period (e.g., "1m", "1h", "1d")
    /// * `count` - Number of bars to retrieve
    pub fn get_bars(
        &self,
        symbol: &PySymbol,
        period: &str,
        count: usize,
    ) -> PyResult<Vec<PyKlineData>> {
        let _period = PyKlinePeriod::from_str(period)?;
        let key = format!("{}_{}", symbol.0.as_str(), period);

        Ok(self
            .klines
            .get(&key)
            .map(|klines| {
                let start = klines.len().saturating_sub(count);
                klines[start..]
                    .iter()
                    .map(|k| PyKlineData::from_core(k.clone()))
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Gets historical K-line data as NumPy arrays.
    ///
    /// Returns a dictionary with keys: "timestamp", "open", "high", "low", "close", "volume", "turnover"
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `period` - The K-line period (e.g., "1m", "1h", "1d")
    /// * `count` - Number of bars to retrieve
    pub fn get_bars_array<'py>(
        &self,
        py: Python<'py>,
        symbol: &PySymbol,
        period: &str,
        count: usize,
    ) -> PyResult<Bound<'py, PyDict>> {
        let _period = PyKlinePeriod::from_str(period)?;
        let key = format!("{}_{}", symbol.0.as_str(), period);

        let klines = self
            .klines
            .get(&key)
            .map(|klines| {
                let start = klines.len().saturating_sub(count);
                &klines[start..]
            })
            .unwrap_or(&[]);

        numpy_conv::klines_to_numpy(py, klines)
    }

    /// Gets recent tick data for a symbol.
    pub fn get_ticks(&self, symbol: &PySymbol, count: usize) -> Vec<PyTickData> {
        self.ticks
            .get(symbol.0.as_str())
            .map(|ticks| {
                let start = ticks.len().saturating_sub(count);
                ticks[start..]
                    .iter()
                    .map(|t| PyTickData::from_core(t.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Gets recent tick data as NumPy arrays.
    pub fn get_ticks_array<'py>(
        &self,
        py: Python<'py>,
        symbol: &PySymbol,
        count: usize,
    ) -> PyResult<Bound<'py, PyDict>> {
        let ticks = self
            .ticks
            .get(symbol.0.as_str())
            .map(|ticks| {
                let start = ticks.len().saturating_sub(count);
                &ticks[start..]
            })
            .unwrap_or(&[]);

        numpy_conv::ticks_to_numpy(py, ticks)
    }

    /// Subscribes to tick data for a symbol.
    pub fn subscribe_ticks(&self, symbol: &PySymbol) {
        tracing::debug!(symbol = %symbol.0, "Subscribed to ticks");
    }

    /// Unsubscribes from tick data for a symbol.
    pub fn unsubscribe_ticks(&self, symbol: &PySymbol) {
        tracing::debug!(symbol = %symbol.0, "Unsubscribed from ticks");
    }

    /// Gets the current timestamp.
    pub fn current_time(&self) -> PyTimestamp {
        PyTimestamp(zephyr_core::types::Timestamp::new_unchecked(
            self.current_time,
        ))
    }

    /// Logs a message with the specified level.
    pub fn log(&self, level: PyLogLevel, message: &str) {
        let core_level = level.to_core();
        match core_level {
            zephyr_core::traits::LogLevel::Trace => {
                tracing::trace!(strategy = %self.name, "{}", message)
            }
            zephyr_core::traits::LogLevel::Debug => {
                tracing::debug!(strategy = %self.name, "{}", message)
            }
            zephyr_core::traits::LogLevel::Info => {
                tracing::info!(strategy = %self.name, "{}", message)
            }
            zephyr_core::traits::LogLevel::Warn => {
                tracing::warn!(strategy = %self.name, "{}", message)
            }
            zephyr_core::traits::LogLevel::Error => {
                tracing::error!(strategy = %self.name, "{}", message)
            }
        }
    }

    /// Gets the strategy name.
    #[getter]
    pub fn strategy_name(&self) -> &str {
        &self.name
    }

    /// Gets all symbols the strategy is trading.
    pub fn symbols(&self) -> Vec<PySymbol> {
        self.positions
            .keys()
            .filter_map(|s| zephyr_core::types::Symbol::new(s).ok())
            .map(PySymbol)
            .collect()
    }

    // Test helper methods

    /// Sets a price for testing.
    pub fn _set_price(&mut self, symbol: &PySymbol, price: f64) {
        self.prices.insert(symbol.0.as_str().to_string(), price);
    }

    /// Sets the current time for testing.
    pub fn _set_current_time(&mut self, millis: i64) {
        self.current_time = millis;
    }
}

/// HFT Strategy Context for Python strategies.
///
/// Provides direct order submission for HFT strategies.
/// HFT strategies have direct control over order placement and cancellation.
///
/// # Examples
///
/// ```python
/// class MyHftStrategy:
///     def on_tick(self, ctx: HftStrategyContext, tick: TickData):
///         # Get current position
///         pos = ctx.get_position(tick.symbol)
///         
///         # Submit a buy order
///         order_id = ctx.buy(
///             tick.symbol,
///             Price(tick.price - 1),  # 1 below market
///             Quantity(0.1),
///             OrderFlag.PostOnly
///         )
///         
///         # Cancel an order
///         ctx.cancel(order_id)
/// ```
#[pyclass(name = "HftStrategyContext", module = "zephyr_py", subclass)]
#[derive(Debug)]
pub struct PyHftStrategyContext {
    /// Strategy name
    name: String,
    /// Mock positions for testing
    positions: std::collections::HashMap<String, f64>,
    /// Mock prices for testing
    prices: std::collections::HashMap<String, f64>,
    /// Order counter for generating IDs
    order_counter: u64,
    /// Current timestamp
    current_time: i64,
}

#[pymethods]
impl PyHftStrategyContext {
    /// Creates a new HFT strategy context (for testing).
    #[new]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            positions: std::collections::HashMap::new(),
            prices: std::collections::HashMap::new(),
            order_counter: 0,
            current_time: zephyr_core::types::Timestamp::now().as_millis(),
        }
    }

    /// Submits a buy order.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `price` - Limit price
    /// * `qty` - Order quantity
    /// * `flag` - Order execution flag
    ///
    /// # Returns
    ///
    /// Returns the order ID on success.
    pub fn buy(
        &mut self,
        symbol: &PySymbol,
        price: &PyPrice,
        qty: &PyQuantity,
        flag: PyOrderFlag,
    ) -> PyResult<PyOrderId> {
        self.order_counter += 1;
        let order_id = format!("{}-{}", self.name, self.order_counter);

        tracing::info!(
            strategy = %self.name,
            order_id = %order_id,
            symbol = %symbol.0,
            side = "buy",
            price = %price.0,
            quantity = %qty.0,
            flag = %flag.__str__(),
            "Order submitted"
        );

        Ok(PyOrderId(zephyr_core::types::OrderId::new_unchecked(
            order_id,
        )))
    }

    /// Submits a sell order.
    pub fn sell(
        &mut self,
        symbol: &PySymbol,
        price: &PyPrice,
        qty: &PyQuantity,
        flag: PyOrderFlag,
    ) -> PyResult<PyOrderId> {
        self.order_counter += 1;
        let order_id = format!("{}-{}", self.name, self.order_counter);

        tracing::info!(
            strategy = %self.name,
            order_id = %order_id,
            symbol = %symbol.0,
            side = "sell",
            price = %price.0,
            quantity = %qty.0,
            flag = %flag.__str__(),
            "Order submitted"
        );

        Ok(PyOrderId(zephyr_core::types::OrderId::new_unchecked(
            order_id,
        )))
    }

    /// Cancels an order.
    pub fn cancel(&self, order_id: &PyOrderId) -> PyResult<bool> {
        tracing::info!(
            strategy = %self.name,
            order_id = %order_id.0.as_str(),
            "Order cancelled"
        );
        Ok(true)
    }

    /// Cancels all orders for a symbol.
    pub fn cancel_all(&self, symbol: &PySymbol) -> PyResult<usize> {
        tracing::info!(
            strategy = %self.name,
            symbol = %symbol.0,
            "All orders cancelled"
        );
        Ok(0)
    }

    /// Gets the current position for a symbol.
    pub fn get_position(&self, symbol: &PySymbol) -> PyQuantity {
        let qty = self
            .positions
            .get(symbol.0.as_str())
            .copied()
            .unwrap_or(0.0);
        let decimal = rust_decimal::Decimal::try_from(qty).unwrap_or_default();
        PyQuantity(
            zephyr_core::types::Quantity::new(decimal)
                .unwrap_or(zephyr_core::types::Quantity::ZERO),
        )
    }

    /// Gets the current price for a symbol.
    pub fn get_price(&self, symbol: &PySymbol) -> Option<PyPrice> {
        self.prices.get(symbol.0.as_str()).map(|&p| {
            let decimal = rust_decimal::Decimal::try_from(p).unwrap_or_default();
            PyPrice(
                zephyr_core::types::Price::new(decimal).unwrap_or(zephyr_core::types::Price::ZERO),
            )
        })
    }

    /// Gets the current timestamp.
    pub fn current_time(&self) -> PyTimestamp {
        PyTimestamp(zephyr_core::types::Timestamp::new_unchecked(
            self.current_time,
        ))
    }

    /// Logs a message with the specified level.
    pub fn log(&self, level: PyLogLevel, message: &str) {
        let core_level = level.to_core();
        match core_level {
            zephyr_core::traits::LogLevel::Trace => {
                tracing::trace!(strategy = %self.name, "{}", message)
            }
            zephyr_core::traits::LogLevel::Debug => {
                tracing::debug!(strategy = %self.name, "{}", message)
            }
            zephyr_core::traits::LogLevel::Info => {
                tracing::info!(strategy = %self.name, "{}", message)
            }
            zephyr_core::traits::LogLevel::Warn => {
                tracing::warn!(strategy = %self.name, "{}", message)
            }
            zephyr_core::traits::LogLevel::Error => {
                tracing::error!(strategy = %self.name, "{}", message)
            }
        }
    }

    /// Gets the strategy name.
    #[getter]
    pub fn strategy_name(&self) -> &str {
        &self.name
    }

    // Test helper methods

    /// Sets a position for testing.
    pub fn _set_position(&mut self, symbol: &PySymbol, qty: f64) {
        self.positions.insert(symbol.0.as_str().to_string(), qty);
    }

    /// Sets a price for testing.
    pub fn _set_price(&mut self, symbol: &PySymbol, price: f64) {
        self.prices.insert(symbol.0.as_str().to_string(), price);
    }

    /// Sets the current time for testing.
    pub fn _set_current_time(&mut self, millis: i64) {
        self.current_time = millis;
    }
}

/// Registers strategy classes with the Python module.
pub fn register_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyOrderFlag>()?;
    m.add_class::<PyLogLevel>()?;
    m.add_class::<PyCtaStrategyContext>()?;
    m.add_class::<PyHftStrategyContext>()?;
    Ok(())
}
