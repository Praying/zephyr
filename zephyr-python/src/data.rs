//! Python wrappers for market data structures.
//!
//! This module provides Python-compatible wrappers for market data
//! structures like TickData and KlineData.

use pyo3::prelude::*;
use rust_decimal::Decimal;

use zephyr_core::data as core_data;
use zephyr_core::types as core_types;

use crate::error::PyZephyrError;
use crate::types::{PyPrice, PyQuantity, PySymbol, PyTimestamp};

/// K-line period enumeration.
///
/// Represents the time period for candlestick data.
#[pyclass(name = "KlinePeriod", module = "zephyr_py", eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyKlinePeriod {
    /// 1 minute
    Minute1 = 1,
    /// 5 minutes
    Minute5 = 5,
    /// 15 minutes
    Minute15 = 15,
    /// 30 minutes
    Minute30 = 30,
    /// 1 hour
    Hour1 = 60,
    /// 4 hours
    Hour4 = 240,
    /// 1 day
    Day1 = 1440,
    /// 1 week
    Week1 = 10080,
}

#[pymethods]
impl PyKlinePeriod {
    /// Returns the duration in seconds.
    #[getter]
    pub fn secs(&self) -> i64 {
        self.to_core().secs()
    }

    /// Returns the duration in milliseconds.
    #[getter]
    pub fn millis(&self) -> i64 {
        self.to_core().millis()
    }

    /// Returns the string representation (e.g., "1m", "1h").
    pub fn __str__(&self) -> &'static str {
        self.to_core().as_str()
    }

    /// Returns the debug representation.
    pub fn __repr__(&self) -> String {
        format!("KlinePeriod.{}", self.name())
    }

    /// Returns the enum variant name.
    fn name(&self) -> &'static str {
        match self {
            Self::Minute1 => "Minute1",
            Self::Minute5 => "Minute5",
            Self::Minute15 => "Minute15",
            Self::Minute30 => "Minute30",
            Self::Hour1 => "Hour1",
            Self::Hour4 => "Hour4",
            Self::Day1 => "Day1",
            Self::Week1 => "Week1",
        }
    }
}

impl PyKlinePeriod {
    /// Converts to core KlinePeriod.
    pub(crate) fn to_core(self) -> core_data::KlinePeriod {
        match self {
            Self::Minute1 => core_data::KlinePeriod::Minute1,
            Self::Minute5 => core_data::KlinePeriod::Minute5,
            Self::Minute15 => core_data::KlinePeriod::Minute15,
            Self::Minute30 => core_data::KlinePeriod::Minute30,
            Self::Hour1 => core_data::KlinePeriod::Hour1,
            Self::Hour4 => core_data::KlinePeriod::Hour4,
            Self::Day1 => core_data::KlinePeriod::Day1,
            Self::Week1 => core_data::KlinePeriod::Week1,
        }
    }

    /// Converts from core KlinePeriod.
    pub(crate) fn from_core(period: core_data::KlinePeriod) -> Self {
        match period {
            core_data::KlinePeriod::Minute1 => Self::Minute1,
            core_data::KlinePeriod::Minute5 => Self::Minute5,
            core_data::KlinePeriod::Minute15 => Self::Minute15,
            core_data::KlinePeriod::Minute30 => Self::Minute30,
            core_data::KlinePeriod::Hour1 => Self::Hour1,
            core_data::KlinePeriod::Hour4 => Self::Hour4,
            core_data::KlinePeriod::Day1 => Self::Day1,
            core_data::KlinePeriod::Week1 => Self::Week1,
        }
    }

    /// Parses a period from string (e.g., "1m", "1h", "1d").
    pub fn from_str(s: &str) -> PyResult<Self> {
        match s.to_lowercase().as_str() {
            "1m" => Ok(Self::Minute1),
            "5m" => Ok(Self::Minute5),
            "15m" => Ok(Self::Minute15),
            "30m" => Ok(Self::Minute30),
            "1h" => Ok(Self::Hour1),
            "4h" => Ok(Self::Hour4),
            "1d" => Ok(Self::Day1),
            "1w" => Ok(Self::Week1),
            _ => Err(PyZephyrError::Validation(format!("Invalid period: {s}")).into()),
        }
    }
}

/// K-line (candlestick) data structure.
///
/// Represents OHLCV data for a specific time period.
///
/// # Examples
///
/// ```python
/// kline = KlineData(
///     symbol=Symbol("BTC-USDT"),
///     timestamp=Timestamp.now(),
///     period=KlinePeriod.Hour1,
///     open=Price(42000),
///     high=Price(42500),
///     low=Price(41800),
///     close=Price(42300),
///     volume=Quantity(100),
///     turnover=Amount(4200000)
/// )
/// print(kline.is_bullish)  # True if close > open
/// ```
#[pyclass(name = "KlineData", module = "zephyr_py")]
#[derive(Debug, Clone)]
pub struct PyKlineData(pub(crate) core_data::KlineData);

#[pymethods]
impl PyKlineData {
    /// Creates a new KlineData.
    #[new]
    #[pyo3(signature = (symbol, timestamp, period, open, high, low, close, volume, turnover))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        symbol: &PySymbol,
        timestamp: &PyTimestamp,
        period: PyKlinePeriod,
        open: &PyPrice,
        high: &PyPrice,
        low: &PyPrice,
        close: &PyPrice,
        volume: &PyQuantity,
        turnover: f64,
    ) -> PyResult<Self> {
        let turnover_decimal = Decimal::try_from(turnover)
            .map_err(|e| PyZephyrError::Validation(format!("Invalid turnover: {e}")))?;
        let turnover_amount =
            core_types::Amount::new(turnover_decimal).map_err(PyZephyrError::from)?;

        let kline = core_data::KlineData::builder()
            .symbol(symbol.0.clone())
            .timestamp(timestamp.0)
            .period(period.to_core())
            .open(open.0)
            .high(high.0)
            .low(low.0)
            .close(close.0)
            .volume(volume.0)
            .turnover(turnover_amount)
            .build()
            .map_err(|e| PyZephyrError::Data(e.to_string()))?;

        Ok(Self(kline))
    }

    /// Trading pair symbol.
    #[getter]
    pub fn symbol(&self) -> PySymbol {
        PySymbol(self.0.symbol.clone())
    }

    /// K-line open timestamp.
    #[getter]
    pub fn timestamp(&self) -> PyTimestamp {
        PyTimestamp(self.0.timestamp)
    }

    /// K-line period.
    #[getter]
    pub fn period(&self) -> PyKlinePeriod {
        PyKlinePeriod::from_core(self.0.period)
    }

    /// Opening price.
    #[getter]
    pub fn open(&self) -> PyPrice {
        PyPrice(self.0.open)
    }

    /// Highest price.
    #[getter]
    pub fn high(&self) -> PyPrice {
        PyPrice(self.0.high)
    }

    /// Lowest price.
    #[getter]
    pub fn low(&self) -> PyPrice {
        PyPrice(self.0.low)
    }

    /// Closing price.
    #[getter]
    pub fn close(&self) -> PyPrice {
        PyPrice(self.0.close)
    }

    /// Trading volume.
    #[getter]
    pub fn volume(&self) -> PyQuantity {
        PyQuantity(self.0.volume)
    }

    /// Turnover (quote currency volume).
    #[getter]
    pub fn turnover(&self) -> f64 {
        self.0.turnover.as_decimal().try_into().unwrap_or(0.0)
    }

    /// Returns true if this is a bullish (green) candle.
    #[getter]
    pub fn is_bullish(&self) -> bool {
        self.0.is_bullish()
    }

    /// Returns true if this is a bearish (red) candle.
    #[getter]
    pub fn is_bearish(&self) -> bool {
        self.0.is_bearish()
    }

    /// Returns the price range (high - low).
    #[getter]
    pub fn range(&self) -> f64 {
        self.0.range().try_into().unwrap_or(0.0)
    }

    /// Returns the body size (|close - open|).
    #[getter]
    pub fn body(&self) -> f64 {
        self.0.body().try_into().unwrap_or(0.0)
    }

    /// Returns the string representation.
    pub fn __str__(&self) -> String {
        format!(
            "KlineData({} {} O:{} H:{} L:{} C:{} V:{})",
            self.0.symbol,
            self.0.period,
            self.0.open,
            self.0.high,
            self.0.low,
            self.0.close,
            self.0.volume
        )
    }

    /// Returns the debug representation.
    pub fn __repr__(&self) -> String {
        self.__str__()
    }
}

impl PyKlineData {
    /// Creates from core KlineData.
    pub(crate) fn from_core(kline: core_data::KlineData) -> Self {
        Self(kline)
    }
}

/// Tick data structure representing real-time market data.
///
/// Contains the latest price, volume, and order book snapshot.
///
/// # Examples
///
/// ```python
/// tick = TickData(
///     symbol=Symbol("BTC-USDT"),
///     timestamp=Timestamp.now(),
///     price=Price(42000),
///     volume=Quantity(0.5),
///     bid_prices=[Price(41999)],
///     bid_quantities=[Quantity(10)],
///     ask_prices=[Price(42001)],
///     ask_quantities=[Quantity(8)]
/// )
/// print(tick.spread)  # 2.0
/// print(tick.mid_price)  # Price(42000)
/// ```
#[pyclass(name = "TickData", module = "zephyr_py")]
#[derive(Debug, Clone)]
pub struct PyTickData(pub(crate) core_data::TickData);

#[pymethods]
impl PyTickData {
    /// Creates a new TickData.
    #[new]
    #[pyo3(signature = (symbol, timestamp, price, volume, bid_prices=vec![], bid_quantities=vec![], ask_prices=vec![], ask_quantities=vec![], open_interest=None, funding_rate=None, mark_price=None))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        symbol: &PySymbol,
        timestamp: &PyTimestamp,
        price: &PyPrice,
        volume: &PyQuantity,
        bid_prices: Vec<PyPrice>,
        bid_quantities: Vec<PyQuantity>,
        ask_prices: Vec<PyPrice>,
        ask_quantities: Vec<PyQuantity>,
        open_interest: Option<f64>,
        funding_rate: Option<f64>,
        mark_price: Option<f64>,
    ) -> PyResult<Self> {
        let mut builder = core_data::TickData::builder()
            .symbol(symbol.0.clone())
            .timestamp(timestamp.0)
            .price(price.0)
            .volume(volume.0)
            .bid_prices(bid_prices.iter().map(|p| p.0).collect())
            .bid_quantities(bid_quantities.iter().map(|q| q.0).collect())
            .ask_prices(ask_prices.iter().map(|p| p.0).collect())
            .ask_quantities(ask_quantities.iter().map(|q| q.0).collect());

        if let Some(oi) = open_interest {
            let decimal = Decimal::try_from(oi)
                .map_err(|e| PyZephyrError::Validation(format!("Invalid open_interest: {e}")))?;
            let qty = core_types::Quantity::new(decimal).map_err(PyZephyrError::from)?;
            builder = builder.open_interest(qty);
        }

        if let Some(fr) = funding_rate {
            let decimal = Decimal::try_from(fr)
                .map_err(|e| PyZephyrError::Validation(format!("Invalid funding_rate: {e}")))?;
            let rate = core_types::FundingRate::new(decimal).map_err(PyZephyrError::from)?;
            builder = builder.funding_rate(rate);
        }

        if let Some(mp) = mark_price {
            let decimal = Decimal::try_from(mp)
                .map_err(|e| PyZephyrError::Validation(format!("Invalid mark_price: {e}")))?;
            let price = core_types::MarkPrice::new(decimal).map_err(PyZephyrError::from)?;
            builder = builder.mark_price(price);
        }

        let tick = builder
            .build()
            .map_err(|e| PyZephyrError::Data(e.to_string()))?;

        Ok(Self(tick))
    }

    /// Trading pair symbol.
    #[getter]
    pub fn symbol(&self) -> PySymbol {
        PySymbol(self.0.symbol.clone())
    }

    /// Tick timestamp.
    #[getter]
    pub fn timestamp(&self) -> PyTimestamp {
        PyTimestamp(self.0.timestamp)
    }

    /// Last traded price.
    #[getter]
    pub fn price(&self) -> PyPrice {
        PyPrice(self.0.price)
    }

    /// Last traded volume.
    #[getter]
    pub fn volume(&self) -> PyQuantity {
        PyQuantity(self.0.volume)
    }

    /// Best bid prices.
    #[getter]
    pub fn bid_prices(&self) -> Vec<PyPrice> {
        self.0.bid_prices.iter().map(|p| PyPrice(*p)).collect()
    }

    /// Best bid quantities.
    #[getter]
    pub fn bid_quantities(&self) -> Vec<PyQuantity> {
        self.0
            .bid_quantities
            .iter()
            .map(|q| PyQuantity(*q))
            .collect()
    }

    /// Best ask prices.
    #[getter]
    pub fn ask_prices(&self) -> Vec<PyPrice> {
        self.0.ask_prices.iter().map(|p| PyPrice(*p)).collect()
    }

    /// Best ask quantities.
    #[getter]
    pub fn ask_quantities(&self) -> Vec<PyQuantity> {
        self.0
            .ask_quantities
            .iter()
            .map(|q| PyQuantity(*q))
            .collect()
    }

    /// Open interest (for futures/perpetuals).
    #[getter]
    pub fn open_interest(&self) -> Option<f64> {
        self.0
            .open_interest
            .map(|q| q.as_decimal().try_into().unwrap_or(0.0))
    }

    /// Funding rate (for perpetuals).
    #[getter]
    pub fn funding_rate(&self) -> Option<f64> {
        self.0
            .funding_rate
            .map(|r| r.as_decimal().try_into().unwrap_or(0.0))
    }

    /// Mark price (for futures/perpetuals).
    #[getter]
    pub fn mark_price(&self) -> Option<f64> {
        self.0
            .mark_price
            .map(|p| p.as_decimal().try_into().unwrap_or(0.0))
    }

    /// Returns the best bid price.
    pub fn best_bid(&self) -> Option<PyPrice> {
        self.0.best_bid().map(PyPrice)
    }

    /// Returns the best ask price.
    pub fn best_ask(&self) -> Option<PyPrice> {
        self.0.best_ask().map(PyPrice)
    }

    /// Returns the spread (best ask - best bid).
    #[getter]
    pub fn spread(&self) -> Option<f64> {
        self.0.spread().map(|d| d.try_into().unwrap_or(0.0))
    }

    /// Returns the mid price.
    pub fn mid_price(&self) -> Option<PyPrice> {
        self.0.mid_price().map(PyPrice)
    }

    /// Returns the string representation.
    pub fn __str__(&self) -> String {
        format!(
            "TickData({} {} P:{} V:{})",
            self.0.symbol, self.0.timestamp, self.0.price, self.0.volume
        )
    }

    /// Returns the debug representation.
    pub fn __repr__(&self) -> String {
        self.__str__()
    }
}

impl PyTickData {
    /// Creates from core TickData.
    pub(crate) fn from_core(tick: core_data::TickData) -> Self {
        Self(tick)
    }
}

/// Registers data classes with the Python module.
pub fn register_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyKlinePeriod>()?;
    m.add_class::<PyKlineData>()?;
    m.add_class::<PyTickData>()?;
    Ok(())
}
