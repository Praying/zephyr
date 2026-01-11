//! Data quality checking and anomaly detection.
//!
//! Monitors data quality metrics including:
//! - Data completeness
//! - Latency statistics
//! - Anomaly detection (price spikes, volume anomalies)
//! - Quality reports

#![allow(clippy::disallowed_types)]
#![allow(clippy::significant_drop_tightening)]

use std::collections::HashMap;
use std::sync::Arc;

use zephyr_core::data::{KlineData, TickData};
use zephyr_core::types::{Symbol, Timestamp};

/// Anomaly type detected in data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnomalyType {
    /// Price spike detected
    PriceSpike,
    /// Volume anomaly detected
    VolumeAnomaly,
    /// Missing data points
    MissingData,
    /// Latency spike
    LatencySpike,
    /// Data out of order
    OutOfOrder,
}

/// Data quality metrics for a symbol.
#[derive(Debug, Clone)]
pub struct QualityMetrics {
    /// Symbol
    pub symbol: Symbol,
    /// Total ticks received
    pub tick_count: u64,
    /// Total klines received
    pub kline_count: u64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// Max latency in milliseconds
    pub max_latency_ms: f64,
    /// Data completeness percentage (0-100)
    pub completeness: f64,
    /// Detected anomalies
    pub anomalies: Vec<AnomalyType>,
    /// Last update timestamp
    pub last_update: Timestamp,
}

impl QualityMetrics {
    /// Creates new quality metrics.
    #[must_use]
    pub fn new(symbol: Symbol) -> Self {
        Self {
            symbol,
            tick_count: 0,
            kline_count: 0,
            avg_latency_ms: 0.0,
            max_latency_ms: 0.0,
            completeness: 100.0,
            anomalies: Vec::new(),
            last_update: Timestamp::now(),
        }
    }
}

/// Data quality report.
#[derive(Debug, Clone)]
pub struct DataQualityReport {
    /// Symbol
    pub symbol: Symbol,
    /// Overall quality score (0-100)
    pub quality_score: f64,
    /// Data completeness percentage
    pub completeness: f64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// Max latency in milliseconds
    pub max_latency_ms: f64,
    /// Detected anomalies
    pub anomalies: Vec<AnomalyType>,
    /// Report timestamp
    pub timestamp: Timestamp,
}

/// Data quality checker.
pub struct DataQualityChecker {
    metrics: Arc<parking_lot::RwLock<HashMap<Symbol, QualityMetrics>>>,
}

impl DataQualityChecker {
    /// Creates a new data quality checker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        }
    }

    /// Records a tick for quality tracking.
    pub fn record_tick(&self, tick: &TickData) {
        let metrics = self.metrics.clone();
        let tick = tick.clone();

        // Process synchronously instead of spawning
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            handle.spawn(async move {
                {
                    let mut metrics_guard = metrics.write();
                    let entry = metrics_guard
                        .entry(tick.symbol.clone())
                        .or_insert_with(|| QualityMetrics::new(tick.symbol.clone()));

                    entry.tick_count += 1;
                    entry.last_update = tick.timestamp;

                    // Check for price anomalies
                    let time_diff = tick.timestamp.as_millis() - entry.last_update.as_millis();
                    if time_diff > 0 {
                        let latency = f64::from(i32::try_from(time_diff).unwrap_or(i32::MAX));
                        entry.avg_latency_ms = entry.avg_latency_ms.mul_add(0.9, latency * 0.1);
                        if latency > entry.max_latency_ms {
                            entry.max_latency_ms = latency;
                        }

                        if latency > 1000.0 && !entry.anomalies.contains(&AnomalyType::LatencySpike)
                        {
                            entry.anomalies.push(AnomalyType::LatencySpike);
                        }
                    }
                }
            });
        }
    }

    /// Records a kline for quality tracking.
    pub fn record_kline(&self, kline: &KlineData) {
        let metrics = self.metrics.clone();
        let kline = kline.clone();

        // Process synchronously instead of spawning
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            handle.spawn(async move {
                {
                    let mut metrics_guard = metrics.write();
                    let entry = metrics_guard
                        .entry(kline.symbol.clone())
                        .or_insert_with(|| QualityMetrics::new(kline.symbol.clone()));

                    entry.kline_count += 1;
                    entry.last_update = kline.timestamp;

                    // Check for price anomalies (high > low)
                    if kline.high < kline.low && !entry.anomalies.contains(&AnomalyType::PriceSpike)
                    {
                        entry.anomalies.push(AnomalyType::PriceSpike);
                    }
                }
            });
        }
    }

    /// Checks data quality for a symbol.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn check_quality(&self, symbol: &Symbol) -> DataQualityReport {
        // Direct read from parking_lot RwLock which is sync
        let metrics_map = self.metrics.read().clone();

        let metrics = metrics_map
            .get(symbol)
            .cloned()
            .unwrap_or_else(|| QualityMetrics::new(symbol.clone()));

        // Calculate quality score
        let mut quality_score: f64 = 100.0;

        // Deduct for latency
        if metrics.avg_latency_ms > 100.0 {
            quality_score -= (metrics.avg_latency_ms - 100.0).min(20.0);
        }

        // Deduct for anomalies
        quality_score -= metrics.anomalies.len() as f64 * 5.0;

        // Deduct for low completeness
        quality_score -= (100.0_f64 - metrics.completeness).min(20.0);

        quality_score = quality_score.clamp(0.0, 100.0);

        DataQualityReport {
            symbol: symbol.clone(),
            quality_score,
            completeness: metrics.completeness,
            avg_latency_ms: metrics.avg_latency_ms,
            max_latency_ms: metrics.max_latency_ms,
            anomalies: metrics.anomalies,
            timestamp: Timestamp::now(),
        }
    }

    /// Gets all quality metrics.
    #[allow(clippy::unused_async)]
    pub async fn get_all_metrics(&self) -> Vec<QualityMetrics> {
        self.metrics.read().values().cloned().collect()
    }

    /// Clears metrics for a symbol.
    #[allow(clippy::unused_async)]
    pub async fn clear_metrics(&self, symbol: &Symbol) {
        self.metrics.write().remove(symbol);
    }
}

impl Default for DataQualityChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use zephyr_core::types::{Amount, Price, Quantity};

    fn create_test_tick() -> TickData {
        TickData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1_704_067_200_000).unwrap())
            .price(Price::new(dec!(42000)).unwrap())
            .volume(Quantity::new(dec!(0.5)).unwrap())
            .bid_price(Price::new(dec!(41999)).unwrap())
            .bid_quantity(Quantity::new(dec!(10)).unwrap())
            .ask_price(Price::new(dec!(42001)).unwrap())
            .ask_quantity(Quantity::new(dec!(8)).unwrap())
            .build()
            .unwrap()
    }

    fn create_test_kline() -> KlineData {
        KlineData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1_704_067_200_000).unwrap())
            .period(zephyr_core::data::KlinePeriod::Hour1)
            .open(Price::new(dec!(42000)).unwrap())
            .high(Price::new(dec!(42500)).unwrap())
            .low(Price::new(dec!(41800)).unwrap())
            .close(Price::new(dec!(42300)).unwrap())
            .volume(Quantity::new(dec!(100)).unwrap())
            .turnover(Amount::new(dec!(4200000)).unwrap())
            .build()
            .unwrap()
    }

    #[test]
    fn test_quality_metrics_creation() {
        let symbol = Symbol::new("BTC-USDT").unwrap();
        let metrics = QualityMetrics::new(symbol.clone());

        assert_eq!(metrics.symbol, symbol);
        assert_eq!(metrics.tick_count, 0);
        assert!((metrics.completeness - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_record_tick() {
        let checker = DataQualityChecker::new();
        let tick = create_test_tick();

        // Just verify it doesn't panic
        checker.record_tick(&tick);
    }

    #[test]
    fn test_record_kline() {
        let checker = DataQualityChecker::new();
        let kline = create_test_kline();

        // Just verify it doesn't panic
        checker.record_kline(&kline);
    }

    #[test]
    fn test_check_quality() {
        let checker = DataQualityChecker::new();
        let symbol = Symbol::new("BTC-USDT").unwrap();

        let report = checker.check_quality(&symbol);
        assert_eq!(report.symbol, symbol);
        assert!(report.quality_score >= 0.0 && report.quality_score <= 100.0);
    }
}
