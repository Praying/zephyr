//! Python wrappers for core Zephyr types.
//!
//! This module provides Python-compatible wrappers for the core NewType
//! types used throughout Zephyr.

use pyo3::prelude::*;
use pyo3::types::PyType;
use rust_decimal::Decimal;
use std::str::FromStr;

use zephyr_core::types as core_types;

use crate::error::PyZephyrError;

/// Price type - represents asset prices with decimal precision.
///
/// # Examples
///
/// ```python
/// price = Price(42000.50)
/// price = Price.from_str("42000.50")
/// print(float(price))  # 42000.5
/// print(str(price))    # "42000.50"
/// ```
#[pyclass(name = "Price", module = "zephyr_py")]
#[derive(Debug, Clone)]
pub struct PyPrice(pub(crate) core_types::Price);

#[pymethods]
impl PyPrice {
    /// Creates a new Price from a float value.
    ///
    /// # Arguments
    ///
    /// * `value` - The price value (must be non-negative)
    ///
    /// # Raises
    ///
    /// * `ValidationException` - If the value is negative
    #[new]
    pub fn new(value: f64) -> PyResult<Self> {
        let decimal = Decimal::try_from(value)
            .map_err(|e| PyZephyrError::Validation(format!("Invalid decimal: {e}")))?;
        let price = core_types::Price::new(decimal).map_err(PyZephyrError::from)?;
        Ok(Self(price))
    }

    /// Creates a Price from a string representation.
    ///
    /// # Arguments
    ///
    /// * `s` - String representation of the price
    #[classmethod]
    pub fn from_str(_cls: &Bound<'_, PyType>, s: &str) -> PyResult<Self> {
        let decimal = Decimal::from_str(s)
            .map_err(|e| PyZephyrError::Validation(format!("Invalid decimal string: {e}")))?;
        let price = core_types::Price::new(decimal).map_err(PyZephyrError::from)?;
        Ok(Self(price))
    }

    /// Returns the price as a float.
    pub fn __float__(&self) -> f64 {
        self.0.as_decimal().try_into().unwrap_or(0.0)
    }

    /// Returns the string representation.
    pub fn __str__(&self) -> String {
        self.0.to_string()
    }

    /// Returns the debug representation.
    pub fn __repr__(&self) -> String {
        format!("Price({})", self.0)
    }

    /// Returns true if the price is zero.
    #[getter]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Adds two prices.
    pub fn __add__(&self, other: &PyPrice) -> Self {
        Self(self.0 + other.0)
    }

    /// Subtracts two prices (returns float for difference).
    pub fn __sub__(&self, other: &PyPrice) -> f64 {
        let diff = self.0 - other.0;
        diff.try_into().unwrap_or(0.0)
    }

    /// Compares two prices for equality.
    pub fn __eq__(&self, other: &PyPrice) -> bool {
        self.0 == other.0
    }

    /// Compares two prices.
    pub fn __lt__(&self, other: &PyPrice) -> bool {
        self.0 < other.0
    }

    /// Compares two prices.
    pub fn __le__(&self, other: &PyPrice) -> bool {
        self.0 <= other.0
    }

    /// Compares two prices.
    pub fn __gt__(&self, other: &PyPrice) -> bool {
        self.0 > other.0
    }

    /// Compares two prices.
    pub fn __ge__(&self, other: &PyPrice) -> bool {
        self.0 >= other.0
    }

    /// Returns hash for use in dicts/sets.
    pub fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.0.hash(&mut hasher);
        hasher.finish()
    }
}

/// Quantity type - represents trading quantities.
///
/// Unlike Price, quantities can be negative (for short positions).
///
/// # Examples
///
/// ```python
/// qty = Quantity(10.5)
/// qty = Quantity(-5.0)  # Short position
/// print(qty.is_negative)  # True
/// print(qty.abs())  # Quantity(5.0)
/// ```
#[pyclass(name = "Quantity", module = "zephyr_py")]
#[derive(Debug, Clone)]
pub struct PyQuantity(pub(crate) core_types::Quantity);

#[pymethods]
impl PyQuantity {
    /// Creates a new Quantity from a float value.
    #[new]
    pub fn new(value: f64) -> PyResult<Self> {
        let decimal = Decimal::try_from(value)
            .map_err(|e| PyZephyrError::Validation(format!("Invalid decimal: {e}")))?;
        let qty = core_types::Quantity::new(decimal).map_err(PyZephyrError::from)?;
        Ok(Self(qty))
    }

    /// Creates a Quantity from a string representation.
    #[classmethod]
    pub fn from_str(_cls: &Bound<'_, PyType>, s: &str) -> PyResult<Self> {
        let decimal = Decimal::from_str(s)
            .map_err(|e| PyZephyrError::Validation(format!("Invalid decimal string: {e}")))?;
        let qty = core_types::Quantity::new(decimal).map_err(PyZephyrError::from)?;
        Ok(Self(qty))
    }

    /// Returns the quantity as a float.
    pub fn __float__(&self) -> f64 {
        self.0.as_decimal().try_into().unwrap_or(0.0)
    }

    /// Returns the string representation.
    pub fn __str__(&self) -> String {
        self.0.to_string()
    }

    /// Returns the debug representation.
    pub fn __repr__(&self) -> String {
        format!("Quantity({})", self.0)
    }

    /// Returns true if the quantity is zero.
    #[getter]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Returns true if the quantity is positive.
    #[getter]
    pub fn is_positive(&self) -> bool {
        self.0.is_positive()
    }

    /// Returns true if the quantity is negative.
    #[getter]
    pub fn is_negative(&self) -> bool {
        self.0.is_negative()
    }

    /// Returns the absolute value.
    pub fn abs(&self) -> Self {
        Self(self.0.abs())
    }

    /// Adds two quantities.
    pub fn __add__(&self, other: &PyQuantity) -> Self {
        Self(self.0 + other.0)
    }

    /// Subtracts two quantities.
    pub fn __sub__(&self, other: &PyQuantity) -> Self {
        Self(self.0 - other.0)
    }

    /// Negates the quantity.
    pub fn __neg__(&self) -> Self {
        Self(-self.0)
    }

    /// Compares two quantities for equality.
    pub fn __eq__(&self, other: &PyQuantity) -> bool {
        self.0 == other.0
    }

    /// Compares two quantities.
    pub fn __lt__(&self, other: &PyQuantity) -> bool {
        self.0 < other.0
    }

    /// Compares two quantities.
    pub fn __le__(&self, other: &PyQuantity) -> bool {
        self.0 <= other.0
    }

    /// Compares two quantities.
    pub fn __gt__(&self, other: &PyQuantity) -> bool {
        self.0 > other.0
    }

    /// Compares two quantities.
    pub fn __ge__(&self, other: &PyQuantity) -> bool {
        self.0 >= other.0
    }

    /// Returns hash for use in dicts/sets.
    pub fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.0.hash(&mut hasher);
        hasher.finish()
    }
}

/// Amount type - represents monetary amounts (price × quantity).
#[pyclass(name = "Amount", module = "zephyr_py")]
#[derive(Debug, Clone)]
pub struct PyAmount(pub(crate) core_types::Amount);

#[pymethods]
impl PyAmount {
    /// Creates a new Amount from a float value.
    #[new]
    pub fn new(value: f64) -> PyResult<Self> {
        let decimal = Decimal::try_from(value)
            .map_err(|e| PyZephyrError::Validation(format!("Invalid decimal: {e}")))?;
        let amount = core_types::Amount::new(decimal).map_err(PyZephyrError::from)?;
        Ok(Self(amount))
    }

    /// Creates an Amount from a string representation.
    #[classmethod]
    pub fn from_str(_cls: &Bound<'_, PyType>, s: &str) -> PyResult<Self> {
        let decimal = Decimal::from_str(s)
            .map_err(|e| PyZephyrError::Validation(format!("Invalid decimal string: {e}")))?;
        let amount = core_types::Amount::new(decimal).map_err(PyZephyrError::from)?;
        Ok(Self(amount))
    }

    /// Returns the amount as a float.
    pub fn __float__(&self) -> f64 {
        self.0.as_decimal().try_into().unwrap_or(0.0)
    }

    /// Returns the string representation.
    pub fn __str__(&self) -> String {
        self.0.to_string()
    }

    /// Returns the debug representation.
    pub fn __repr__(&self) -> String {
        format!("Amount({})", self.0)
    }

    /// Returns true if the amount is zero.
    #[getter]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Compares two amounts for equality.
    pub fn __eq__(&self, other: &PyAmount) -> bool {
        self.0 == other.0
    }

    /// Returns hash for use in dicts/sets.
    pub fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.0.hash(&mut hasher);
        hasher.finish()
    }
}

/// Symbol type - represents trading pair identifiers.
///
/// # Examples
///
/// ```python
/// symbol = Symbol("BTC-USDT")
/// print(symbol.base_asset)   # "BTC"
/// print(symbol.quote_asset)  # "USDT"
/// ```
#[pyclass(name = "Symbol", module = "zephyr_py")]
#[derive(Debug, Clone)]
pub struct PySymbol(pub(crate) core_types::Symbol);

#[pymethods]
impl PySymbol {
    /// Creates a new Symbol from a string.
    ///
    /// # Arguments
    ///
    /// * `value` - The symbol string (e.g., "BTC-USDT")
    ///
    /// # Raises
    ///
    /// * `ValidationException` - If the symbol format is invalid
    #[new]
    pub fn new(value: &str) -> PyResult<Self> {
        let symbol = core_types::Symbol::new(value).map_err(PyZephyrError::from)?;
        Ok(Self(symbol))
    }

    /// Returns the symbol as a string.
    pub fn __str__(&self) -> String {
        self.0.to_string()
    }

    /// Returns the debug representation.
    pub fn __repr__(&self) -> String {
        format!("Symbol(\"{}\")", self.0)
    }

    /// Returns the base asset (e.g., "BTC" from "BTC-USDT").
    #[getter]
    pub fn base_asset(&self) -> Option<String> {
        self.0.base_asset().map(String::from)
    }

    /// Returns the quote asset (e.g., "USDT" from "BTC-USDT").
    #[getter]
    pub fn quote_asset(&self) -> Option<String> {
        self.0.quote_asset().map(String::from)
    }

    /// Compares two symbols for equality.
    pub fn __eq__(&self, other: &PySymbol) -> bool {
        self.0 == other.0
    }

    /// Returns hash for use in dicts/sets.
    pub fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.0.hash(&mut hasher);
        hasher.finish()
    }
}

/// Timestamp type - represents Unix millisecond timestamps.
///
/// # Examples
///
/// ```python
/// ts = Timestamp.now()
/// ts = Timestamp(1704067200000)
/// print(ts.as_secs)  # Seconds since epoch
/// dt = ts.to_datetime()  # Convert to datetime
/// ```
#[pyclass(name = "Timestamp", module = "zephyr_py")]
#[derive(Debug, Clone, Copy)]
pub struct PyTimestamp(pub(crate) core_types::Timestamp);

#[pymethods]
impl PyTimestamp {
    /// Creates a new Timestamp from milliseconds since Unix epoch.
    #[new]
    pub fn new(millis: i64) -> PyResult<Self> {
        let ts = core_types::Timestamp::new(millis).map_err(PyZephyrError::from)?;
        Ok(Self(ts))
    }

    /// Returns the current timestamp.
    #[classmethod]
    pub fn now(_cls: &Bound<'_, PyType>) -> Self {
        Self(core_types::Timestamp::now())
    }

    /// Creates a Timestamp from seconds since Unix epoch.
    #[classmethod]
    pub fn from_secs(_cls: &Bound<'_, PyType>, secs: i64) -> PyResult<Self> {
        let ts = core_types::Timestamp::from_secs(secs).map_err(PyZephyrError::from)?;
        Ok(Self(ts))
    }

    /// Returns the timestamp as milliseconds.
    #[getter]
    pub fn as_millis(&self) -> i64 {
        self.0.as_millis()
    }

    /// Returns the timestamp as seconds.
    #[getter]
    pub fn as_secs(&self) -> i64 {
        self.0.as_secs()
    }

    /// Returns true if the timestamp is zero.
    #[getter]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Returns the string representation.
    pub fn __str__(&self) -> String {
        self.0.to_string()
    }

    /// Returns the debug representation.
    pub fn __repr__(&self) -> String {
        format!("Timestamp({})", self.0.as_millis())
    }

    /// Returns the integer value (milliseconds).
    pub fn __int__(&self) -> i64 {
        self.0.as_millis()
    }

    /// Compares two timestamps for equality.
    pub fn __eq__(&self, other: &PyTimestamp) -> bool {
        self.0 == other.0
    }

    /// Compares two timestamps.
    pub fn __lt__(&self, other: &PyTimestamp) -> bool {
        self.0 < other.0
    }

    /// Compares two timestamps.
    pub fn __le__(&self, other: &PyTimestamp) -> bool {
        self.0 <= other.0
    }

    /// Compares two timestamps.
    pub fn __gt__(&self, other: &PyTimestamp) -> bool {
        self.0 > other.0
    }

    /// Compares two timestamps.
    pub fn __ge__(&self, other: &PyTimestamp) -> bool {
        self.0 >= other.0
    }

    /// Returns hash for use in dicts/sets.
    pub fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.0.hash(&mut hasher);
        hasher.finish()
    }
}

/// OrderId type - represents order identifiers.
#[pyclass(name = "OrderId", module = "zephyr_py")]
#[derive(Debug, Clone)]
pub struct PyOrderId(pub(crate) core_types::OrderId);

#[pymethods]
impl PyOrderId {
    /// Creates a new OrderId from a string.
    #[new]
    pub fn new(value: &str) -> PyResult<Self> {
        let order_id = core_types::OrderId::new(value).map_err(PyZephyrError::from)?;
        Ok(Self(order_id))
    }

    /// Returns the order ID as a string.
    pub fn __str__(&self) -> String {
        self.0.as_str().to_string()
    }

    /// Returns the debug representation.
    pub fn __repr__(&self) -> String {
        format!("OrderId(\"{}\")", self.0.as_str())
    }

    /// Compares two order IDs for equality.
    pub fn __eq__(&self, other: &PyOrderId) -> bool {
        self.0 == other.0
    }

    /// Returns hash for use in dicts/sets.
    pub fn __hash__(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.0.hash(&mut hasher);
        hasher.finish()
    }
}

/// Registers type classes with the Python module.
pub fn register_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPrice>()?;
    m.add_class::<PyQuantity>()?;
    m.add_class::<PyAmount>()?;
    m.add_class::<PySymbol>()?;
    m.add_class::<PyTimestamp>()?;
    m.add_class::<PyOrderId>()?;
    Ok(())
}
