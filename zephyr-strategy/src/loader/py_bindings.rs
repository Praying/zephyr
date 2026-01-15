//! Python bindings for strategy context and market data types.
//!
//! This module provides PyO3 classes that wrap Rust types for use in Python strategies:
//! - `PyContextWrapper`: Wraps `StrategyContext` for Python access
//! - `PyTick`: Wraps `TickData` for Python access
//! - `PyBar`: Wraps `KlineData` for Python access
//!
//! These wrappers enable Python strategies to:
//! - Emit trading signals
//! - Access portfolio state
//! - Log messages
//! - Process market data

use crate::Decimal;
use crate::context::{LogLevel, StrategyContext};
use crate::signal::{Signal, Urgency};
use crate::types::{Bar, Tick};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use rust_decimal::prelude::ToPrimitive;
use std::collections::HashMap;
use zephyr_core::data::{OrderSide, OrderType, TimeInForce};
use zephyr_core::types::{OrderId, Symbol};

/// Python wrapper for `StrategyContext`.
///
/// Provides Python strategies with controlled access to system functionality:
/// - Signal emission for trading intents
/// - Portfolio state access
/// - Logging interface
/// - Time access
///
/// # Example (Python)
///
/// ```python
/// def on_tick(self, tick, ctx):
///     # Emit a trading signal
///     signal_id = ctx.emit_target_position("BTC-USDT", 1.0, "medium")
///     
///     # Log a message
///     ctx.log_info(f"Emitted signal {signal_id}")
///     
///     # Access portfolio
///     portfolio = ctx.portfolio()
///     print(f"Cash: {portfolio['cash']}")
/// ```
#[pyclass(name = "StrategyContext", unsendable)]
pub struct PyContextWrapper {
    /// Pointer to the underlying StrategyContext
    /// SAFETY: This is only valid during the lifetime of the on_tick/on_bar call
    ctx_ptr: *mut StrategyContext,
}

// Note: PyContextWrapper uses `unsendable` attribute which handles thread safety

impl PyContextWrapper {
    /// Creates a new `PyContextWrapper` from a mutable reference.
    ///
    /// # Safety
    ///
    /// The returned wrapper is only valid for the duration of the current
    /// Python callback. Do not store or use it after the callback returns.
    #[must_use]
    pub fn new(ctx: &mut StrategyContext) -> Self {
        Self {
            ctx_ptr: ctx as *mut StrategyContext,
        }
    }

    /// Gets a mutable reference to the underlying context.
    ///
    /// # Safety
    ///
    /// This is only safe to call during the lifetime of the on_tick/on_bar callback.
    fn ctx_mut(&mut self) -> &mut StrategyContext {
        // SAFETY: The pointer is valid during the callback lifetime
        unsafe { &mut *self.ctx_ptr }
    }

    /// Gets an immutable reference to the underlying context.
    fn ctx(&self) -> &StrategyContext {
        // SAFETY: The pointer is valid during the callback lifetime
        unsafe { &*self.ctx_ptr }
    }
}

#[pymethods]
impl PyContextWrapper {
    /// Returns the current engine time in milliseconds.
    ///
    /// In live trading, this returns the system time.
    /// In backtesting, this returns the simulated time.
    #[getter]
    fn now(&self) -> i64 {
        self.ctx().now().as_millis()
    }

    /// Returns the portfolio snapshot as a dictionary.
    ///
    /// Returns:
    ///     dict: Portfolio state with keys:
    ///         - cash: Available cash balance
    ///         - equity: Total equity
    ///         - margin_used: Margin currently in use
    ///         - positions: Dict of symbol -> position info
    fn portfolio(&self) -> PyResult<HashMap<String, PyObject>> {
        Python::with_gil(|py| {
            let portfolio = self.ctx().portfolio();
            let mut result = HashMap::new();

            result.insert(
                "cash".to_string(),
                portfolio
                    .cash
                    .to_f64()
                    .unwrap_or(0.0)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
            );
            result.insert(
                "equity".to_string(),
                portfolio
                    .equity
                    .to_f64()
                    .unwrap_or(0.0)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
            );
            result.insert(
                "margin_used".to_string(),
                portfolio
                    .margin_used
                    .to_f64()
                    .unwrap_or(0.0)
                    .into_pyobject(py)?
                    .into_any()
                    .unbind(),
            );

            // Build positions dict
            let mut positions = HashMap::new();
            for (symbol, pos) in &portfolio.positions {
                let mut pos_dict = HashMap::new();
                pos_dict.insert("quantity".to_string(), pos.quantity.to_f64().unwrap_or(0.0));
                pos_dict.insert(
                    "avg_price".to_string(),
                    pos.avg_price.to_f64().unwrap_or(0.0),
                );
                pos_dict.insert(
                    "unrealized_pnl".to_string(),
                    pos.unrealized_pnl.to_f64().unwrap_or(0.0),
                );
                positions.insert(symbol.as_str().to_string(), pos_dict);
            }
            result.insert(
                "positions".to_string(),
                positions.into_pyobject(py)?.into_any().unbind(),
            );

            Ok(result)
        })
    }

    /// Emits a target position signal.
    ///
    /// This is a high-level intent: "I want to hold X quantity".
    /// The execution layer handles the how (buy/sell, algo selection).
    ///
    /// Args:
    ///     symbol: Trading pair symbol (e.g., "BTC-USDT")
    ///     target_qty: Target position quantity (positive = long, negative = short)
    ///     urgency: How urgently to reach the target ("high", "medium", "low")
    ///
    /// Returns:
    ///     int: Signal ID for tracking
    ///
    /// Raises:
    ///     ValueError: If the signal channel is full or closed
    #[pyo3(signature = (symbol, target_qty, urgency = "medium"))]
    fn emit_target_position(
        &mut self,
        symbol: &str,
        target_qty: f64,
        urgency: &str,
    ) -> PyResult<u64> {
        let urgency = parse_urgency(urgency)?;
        let signal = Signal::SetTargetPosition {
            symbol: Symbol::new_unchecked(symbol),
            target_qty: Decimal::try_from(target_qty)
                .map_err(|e| PyValueError::new_err(format!("Invalid quantity: {e}")))?,
            urgency,
        };

        self.ctx_mut()
            .emit_signal(signal)
            .map(|id| id.as_u64())
            .map_err(|e| PyValueError::new_err(format!("Failed to emit signal: {e}")))
    }

    /// Emits a limit order signal.
    ///
    /// Args:
    ///     symbol: Trading pair symbol
    ///     side: Order side ("buy" or "sell")
    ///     price: Limit price
    ///     qty: Order quantity
    ///     time_in_force: Time in force ("gtc", "ioc", "fok")
    ///
    /// Returns:
    ///     int: Signal ID for tracking
    #[pyo3(signature = (symbol, side, price, qty, time_in_force = "gtc"))]
    fn emit_limit_order(
        &mut self,
        symbol: &str,
        side: &str,
        price: f64,
        qty: f64,
        time_in_force: &str,
    ) -> PyResult<u64> {
        let side = parse_side(side)?;
        let tif = parse_time_in_force(time_in_force)?;

        let signal = Signal::PlaceOrder {
            symbol: Symbol::new_unchecked(symbol),
            side,
            price: Decimal::try_from(price)
                .map_err(|e| PyValueError::new_err(format!("Invalid price: {e}")))?,
            qty: Decimal::try_from(qty)
                .map_err(|e| PyValueError::new_err(format!("Invalid quantity: {e}")))?,
            order_type: OrderType::Limit,
            time_in_force: tif,
        };

        self.ctx_mut()
            .emit_signal(signal)
            .map(|id| id.as_u64())
            .map_err(|e| PyValueError::new_err(format!("Failed to emit signal: {e}")))
    }

    /// Emits a market order signal.
    ///
    /// Args:
    ///     symbol: Trading pair symbol
    ///     side: Order side ("buy" or "sell")
    ///     qty: Order quantity
    ///
    /// Returns:
    ///     int: Signal ID for tracking
    fn emit_market_order(&mut self, symbol: &str, side: &str, qty: f64) -> PyResult<u64> {
        let side = parse_side(side)?;

        let signal = Signal::PlaceOrder {
            symbol: Symbol::new_unchecked(symbol),
            side,
            price: Decimal::ZERO,
            qty: Decimal::try_from(qty)
                .map_err(|e| PyValueError::new_err(format!("Invalid quantity: {e}")))?,
            order_type: OrderType::Market,
            time_in_force: TimeInForce::Ioc,
        };

        self.ctx_mut()
            .emit_signal(signal)
            .map(|id| id.as_u64())
            .map_err(|e| PyValueError::new_err(format!("Failed to emit signal: {e}")))
    }

    /// Cancels a specific order.
    ///
    /// Args:
    ///     order_id: The order ID to cancel
    ///
    /// Returns:
    ///     int: Signal ID for tracking
    fn emit_cancel_order(&mut self, order_id: &str) -> PyResult<u64> {
        let signal = Signal::CancelOrder {
            order_id: OrderId::new_unchecked(order_id),
        };

        self.ctx_mut()
            .emit_signal(signal)
            .map(|id| id.as_u64())
            .map_err(|e| PyValueError::new_err(format!("Failed to emit signal: {e}")))
    }

    /// Cancels all orders for a symbol.
    ///
    /// Args:
    ///     symbol: Trading pair symbol
    ///
    /// Returns:
    ///     int: Signal ID for tracking
    fn emit_cancel_all(&mut self, symbol: &str) -> PyResult<u64> {
        let signal = Signal::CancelAll {
            symbol: Symbol::new_unchecked(symbol),
        };

        self.ctx_mut()
            .emit_signal(signal)
            .map(|id| id.as_u64())
            .map_err(|e| PyValueError::new_err(format!("Failed to emit signal: {e}")))
    }

    /// Logs an info message.
    ///
    /// Args:
    ///     msg: Message to log
    fn log_info(&self, msg: &str) {
        self.ctx().log(LogLevel::Info, msg);
    }

    /// Logs a warning message.
    ///
    /// Args:
    ///     msg: Message to log
    fn log_warn(&self, msg: &str) {
        self.ctx().log(LogLevel::Warn, msg);
    }

    /// Logs an error message.
    ///
    /// Args:
    ///     msg: Message to log
    fn log_error(&self, msg: &str) {
        self.ctx().log(LogLevel::Error, msg);
    }

    /// Logs a debug message.
    ///
    /// Args:
    ///     msg: Message to log
    fn log_debug(&self, msg: &str) {
        self.ctx().log(LogLevel::Debug, msg);
    }
}

/// Parses an urgency string into an `Urgency` enum.
fn parse_urgency(s: &str) -> PyResult<Urgency> {
    match s.to_lowercase().as_str() {
        "high" => Ok(Urgency::High),
        "medium" => Ok(Urgency::Medium),
        "low" => Ok(Urgency::Low),
        _ => Err(PyValueError::new_err(format!(
            "Invalid urgency '{}', expected 'high', 'medium', or 'low'",
            s
        ))),
    }
}

/// Parses a side string into an `OrderSide` enum.
fn parse_side(s: &str) -> PyResult<OrderSide> {
    match s.to_lowercase().as_str() {
        "buy" => Ok(OrderSide::Buy),
        "sell" => Ok(OrderSide::Sell),
        _ => Err(PyValueError::new_err(format!(
            "Invalid side '{}', expected 'buy' or 'sell'",
            s
        ))),
    }
}

/// Parses a time-in-force string into a `TimeInForce` enum.
fn parse_time_in_force(s: &str) -> PyResult<TimeInForce> {
    match s.to_lowercase().as_str() {
        "gtc" => Ok(TimeInForce::Gtc),
        "ioc" => Ok(TimeInForce::Ioc),
        "fok" => Ok(TimeInForce::Fok),
        _ => Err(PyValueError::new_err(format!(
            "Invalid time_in_force '{}', expected 'gtc', 'ioc', or 'fok'",
            s
        ))),
    }
}

/// Python wrapper for `TickData`.
///
/// Provides Python strategies with access to tick (real-time trade) data.
///
/// # Example (Python)
///
/// ```python
/// def on_tick(self, tick, ctx):
///     print(f"Symbol: {tick.symbol}")
///     print(f"Price: {tick.price}")
///     print(f"Volume: {tick.volume}")
///     if tick.best_bid is not None:
///         print(f"Spread: {tick.best_ask - tick.best_bid}")
/// ```
#[pyclass(name = "Tick")]
#[derive(Clone)]
pub struct PyTick {
    /// Trading pair symbol
    #[pyo3(get)]
    pub symbol: String,
    /// Tick timestamp in milliseconds
    #[pyo3(get)]
    pub timestamp: i64,
    /// Last traded price
    #[pyo3(get)]
    pub price: f64,
    /// Last traded volume
    #[pyo3(get)]
    pub volume: f64,
    /// Best bid price (if available)
    #[pyo3(get)]
    pub best_bid: Option<f64>,
    /// Best ask price (if available)
    #[pyo3(get)]
    pub best_ask: Option<f64>,
    /// Spread (if bid/ask available)
    #[pyo3(get)]
    pub spread: Option<f64>,
}

#[pymethods]
impl PyTick {
    /// Creates a new PyTick instance.
    #[new]
    #[pyo3(signature = (symbol, timestamp, price, volume, best_bid = None, best_ask = None))]
    fn new(
        symbol: String,
        timestamp: i64,
        price: f64,
        volume: f64,
        best_bid: Option<f64>,
        best_ask: Option<f64>,
    ) -> Self {
        let spread = match (best_bid, best_ask) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        };
        Self {
            symbol,
            timestamp,
            price,
            volume,
            best_bid,
            best_ask,
            spread,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Tick(symbol='{}', price={}, volume={}, timestamp={})",
            self.symbol, self.price, self.volume, self.timestamp
        )
    }
}

impl From<&Tick> for PyTick {
    fn from(tick: &Tick) -> Self {
        let best_bid = tick
            .best_bid()
            .map(|p| p.as_decimal().to_f64().unwrap_or(0.0));
        let best_ask = tick
            .best_ask()
            .map(|p| p.as_decimal().to_f64().unwrap_or(0.0));
        let spread = tick.spread().and_then(|s| s.to_f64());

        Self {
            symbol: tick.symbol.as_str().to_string(),
            timestamp: tick.timestamp.as_millis(),
            price: tick.price.as_decimal().to_f64().unwrap_or(0.0),
            volume: tick.volume.as_decimal().to_f64().unwrap_or(0.0),
            best_bid,
            best_ask,
            spread,
        }
    }
}

/// Python wrapper for `KlineData` (Bar/Candlestick).
///
/// Provides Python strategies with access to OHLCV bar data.
///
/// # Example (Python)
///
/// ```python
/// def on_bar(self, bar, ctx):
///     print(f"Symbol: {bar.symbol}")
///     print(f"OHLCV: {bar.open}, {bar.high}, {bar.low}, {bar.close}, {bar.volume}")
///     if bar.is_bullish:
///         ctx.log_info("Bullish candle!")
/// ```
#[pyclass(name = "Bar")]
#[derive(Clone)]
pub struct PyBar {
    /// Trading pair symbol
    #[pyo3(get)]
    pub symbol: String,
    /// Bar timestamp in milliseconds (start of period)
    #[pyo3(get)]
    pub timestamp: i64,
    /// Bar period (e.g., "1m", "1h", "1d")
    #[pyo3(get)]
    pub period: String,
    /// Opening price
    #[pyo3(get)]
    pub open: f64,
    /// Highest price
    #[pyo3(get)]
    pub high: f64,
    /// Lowest price
    #[pyo3(get)]
    pub low: f64,
    /// Closing price
    #[pyo3(get)]
    pub close: f64,
    /// Trading volume
    #[pyo3(get)]
    pub volume: f64,
    /// Turnover (quote volume)
    #[pyo3(get)]
    pub turnover: f64,
}

#[pymethods]
impl PyBar {
    /// Creates a new PyBar instance.
    #[new]
    #[pyo3(signature = (symbol, timestamp, period, open, high, low, close, volume, turnover = 0.0))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        symbol: String,
        timestamp: i64,
        period: String,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: f64,
        turnover: f64,
    ) -> Self {
        Self {
            symbol,
            timestamp,
            period,
            open,
            high,
            low,
            close,
            volume,
            turnover,
        }
    }

    /// Returns true if this is a bullish (green) candle.
    #[getter]
    fn is_bullish(&self) -> bool {
        self.close >= self.open
    }

    /// Returns true if this is a bearish (red) candle.
    #[getter]
    fn is_bearish(&self) -> bool {
        self.close < self.open
    }

    /// Returns the price range (high - low).
    #[getter]
    fn range(&self) -> f64 {
        self.high - self.low
    }

    /// Returns the body size (|close - open|).
    #[getter]
    fn body(&self) -> f64 {
        (self.close - self.open).abs()
    }

    fn __repr__(&self) -> String {
        format!(
            "Bar(symbol='{}', period='{}', o={}, h={}, l={}, c={}, v={})",
            self.symbol, self.period, self.open, self.high, self.low, self.close, self.volume
        )
    }
}

impl From<&Bar> for PyBar {
    fn from(bar: &Bar) -> Self {
        Self {
            symbol: bar.symbol.as_str().to_string(),
            timestamp: bar.timestamp.as_millis(),
            period: bar.period.as_str().to_string(),
            open: bar.open.as_decimal().to_f64().unwrap_or(0.0),
            high: bar.high.as_decimal().to_f64().unwrap_or(0.0),
            low: bar.low.as_decimal().to_f64().unwrap_or(0.0),
            close: bar.close.as_decimal().to_f64().unwrap_or(0.0),
            volume: bar.volume.as_decimal().to_f64().unwrap_or(0.0),
            turnover: bar.turnover.as_decimal().to_f64().unwrap_or(0.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_urgency() {
        assert_eq!(parse_urgency("high").unwrap(), Urgency::High);
        assert_eq!(parse_urgency("MEDIUM").unwrap(), Urgency::Medium);
        assert_eq!(parse_urgency("Low").unwrap(), Urgency::Low);
        assert!(parse_urgency("invalid").is_err());
    }

    #[test]
    fn test_parse_side() {
        assert_eq!(parse_side("buy").unwrap(), OrderSide::Buy);
        assert_eq!(parse_side("SELL").unwrap(), OrderSide::Sell);
        assert!(parse_side("invalid").is_err());
    }

    #[test]
    fn test_parse_time_in_force() {
        assert_eq!(parse_time_in_force("gtc").unwrap(), TimeInForce::Gtc);
        assert_eq!(parse_time_in_force("IOC").unwrap(), TimeInForce::Ioc);
        assert_eq!(parse_time_in_force("fok").unwrap(), TimeInForce::Fok);
        assert!(parse_time_in_force("invalid").is_err());
    }

    #[test]
    fn test_py_tick_new() {
        let tick = PyTick::new(
            "BTC-USDT".to_string(),
            1704067200000,
            42000.0,
            1.5,
            Some(41999.0),
            Some(42001.0),
        );
        assert_eq!(tick.symbol, "BTC-USDT");
        assert_eq!(tick.price, 42000.0);
        assert_eq!(tick.spread, Some(2.0));
    }

    #[test]
    fn test_py_bar_new() {
        let bar = PyBar::new(
            "BTC-USDT".to_string(),
            1704067200000,
            "1h".to_string(),
            42000.0,
            42500.0,
            41800.0,
            42300.0,
            100.0,
            4200000.0,
        );
        assert_eq!(bar.symbol, "BTC-USDT");
        assert!(bar.is_bullish());
        assert!(!bar.is_bearish());
        assert_eq!(bar.range(), 700.0);
        assert_eq!(bar.body(), 300.0);
    }
}
