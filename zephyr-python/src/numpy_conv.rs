//! NumPy array conversion utilities.
//!
//! This module provides functions to convert Zephyr data structures
//! to NumPy arrays for efficient data processing in Python.

use numpy::PyArray1;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use zephyr_core::data::{KlineData, TickData};

/// Converts a slice of KlineData to a dictionary of NumPy arrays.
///
/// Returns a dictionary with keys: "timestamp", "open", "high", "low", "close", "volume", "turnover"
/// Each value is a 1D NumPy array of f64.
pub fn klines_to_numpy<'py>(py: Python<'py>, klines: &[KlineData]) -> PyResult<Bound<'py, PyDict>> {
    let n = klines.len();

    // Pre-allocate vectors
    let mut timestamps = Vec::with_capacity(n);
    let mut opens = Vec::with_capacity(n);
    let mut highs = Vec::with_capacity(n);
    let mut lows = Vec::with_capacity(n);
    let mut closes = Vec::with_capacity(n);
    let mut volumes = Vec::with_capacity(n);
    let mut turnovers = Vec::with_capacity(n);

    // Extract data
    for kline in klines {
        timestamps.push(kline.timestamp.as_millis() as f64);
        opens.push(kline.open.as_decimal().try_into().unwrap_or(0.0));
        highs.push(kline.high.as_decimal().try_into().unwrap_or(0.0));
        lows.push(kline.low.as_decimal().try_into().unwrap_or(0.0));
        closes.push(kline.close.as_decimal().try_into().unwrap_or(0.0));
        volumes.push(kline.volume.as_decimal().try_into().unwrap_or(0.0));
        turnovers.push(kline.turnover.as_decimal().try_into().unwrap_or(0.0));
    }

    // Create NumPy arrays
    let dict = PyDict::new(py);
    dict.set_item("timestamp", PyArray1::from_vec(py, timestamps))?;
    dict.set_item("open", PyArray1::from_vec(py, opens))?;
    dict.set_item("high", PyArray1::from_vec(py, highs))?;
    dict.set_item("low", PyArray1::from_vec(py, lows))?;
    dict.set_item("close", PyArray1::from_vec(py, closes))?;
    dict.set_item("volume", PyArray1::from_vec(py, volumes))?;
    dict.set_item("turnover", PyArray1::from_vec(py, turnovers))?;

    Ok(dict)
}

/// Converts a slice of TickData to a dictionary of NumPy arrays.
///
/// Returns a dictionary with keys: "timestamp", "price", "volume", "bid", "ask"
/// Each value is a 1D NumPy array of f64.
pub fn ticks_to_numpy<'py>(py: Python<'py>, ticks: &[TickData]) -> PyResult<Bound<'py, PyDict>> {
    let n = ticks.len();

    // Pre-allocate vectors
    let mut timestamps = Vec::with_capacity(n);
    let mut prices = Vec::with_capacity(n);
    let mut volumes = Vec::with_capacity(n);
    let mut bids = Vec::with_capacity(n);
    let mut asks = Vec::with_capacity(n);

    // Extract data
    for tick in ticks {
        timestamps.push(tick.timestamp.as_millis() as f64);
        prices.push(tick.price.as_decimal().try_into().unwrap_or(0.0));
        volumes.push(tick.volume.as_decimal().try_into().unwrap_or(0.0));
        bids.push(
            tick.best_bid()
                .map(|p| p.as_decimal().try_into().unwrap_or(0.0))
                .unwrap_or(0.0),
        );
        asks.push(
            tick.best_ask()
                .map(|p| p.as_decimal().try_into().unwrap_or(0.0))
                .unwrap_or(0.0),
        );
    }

    // Create NumPy arrays
    let dict = PyDict::new(py);
    dict.set_item("timestamp", PyArray1::from_vec(py, timestamps))?;
    dict.set_item("price", PyArray1::from_vec(py, prices))?;
    dict.set_item("volume", PyArray1::from_vec(py, volumes))?;
    dict.set_item("bid", PyArray1::from_vec(py, bids))?;
    dict.set_item("ask", PyArray1::from_vec(py, asks))?;

    Ok(dict)
}

/// Extracts close prices from KlineData as a NumPy array.
#[allow(dead_code)]
pub fn klines_close_array<'py>(py: Python<'py>, klines: &[KlineData]) -> Bound<'py, PyArray1<f64>> {
    let closes: Vec<f64> = klines
        .iter()
        .map(|k| k.close.as_decimal().try_into().unwrap_or(0.0))
        .collect();
    PyArray1::from_vec(py, closes)
}

/// Extracts prices from TickData as a NumPy array.
#[allow(dead_code)]
pub fn ticks_price_array<'py>(py: Python<'py>, ticks: &[TickData]) -> Bound<'py, PyArray1<f64>> {
    let prices: Vec<f64> = ticks
        .iter()
        .map(|t| t.price.as_decimal().try_into().unwrap_or(0.0))
        .collect();
    PyArray1::from_vec(py, prices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use zephyr_core::data::KlinePeriod;
    use zephyr_core::types::{Amount, Price, Quantity, Symbol, Timestamp};

    fn create_test_kline(close: f64) -> KlineData {
        let close_dec = rust_decimal::Decimal::try_from(close).unwrap();
        KlineData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1704067200000).unwrap())
            .period(KlinePeriod::Hour1)
            .open(Price::new(dec!(42000)).unwrap())
            .high(Price::new(dec!(42500)).unwrap())
            .low(Price::new(dec!(41800)).unwrap())
            .close(Price::new(close_dec).unwrap())
            .volume(Quantity::new(dec!(100)).unwrap())
            .turnover(Amount::new(dec!(4200000)).unwrap())
            .build()
            .unwrap()
    }

    #[test]
    fn test_klines_conversion() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let klines = vec![
                create_test_kline(42000.0),
                create_test_kline(42100.0),
                create_test_kline(42200.0),
            ];

            let dict = klines_to_numpy(py, &klines).unwrap();
            assert!(dict.contains("close").unwrap());
            assert!(dict.contains("open").unwrap());
            assert!(dict.contains("high").unwrap());
            assert!(dict.contains("low").unwrap());
            assert!(dict.contains("volume").unwrap());
        });
    }
}
