//! Latency measurement for orders and market data.
//!
//! This module provides precise latency tracking for:
//! - Order submission to acknowledgment
//! - Market data reception to processing completion
//!
//! ## Features
//!
//! - Nanosecond precision timing
//! - Statistical analysis (min, max, mean, percentiles)
//! - Histogram bucketing for distribution analysis
//! - Thread-safe concurrent access

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

/// Maximum number of samples to retain for statistics.
const MAX_SAMPLES: usize = 10_000;

/// Default histogram bucket boundaries in microseconds.
const DEFAULT_BUCKETS_US: [u64; 12] = [
    10, 50, 100, 250, 500, 1_000, 2_500, 5_000, 10_000, 25_000, 50_000, 100_000,
];

/// Latency statistics computed from measurements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyStats {
    /// Number of measurements.
    pub count: u64,
    /// Minimum latency observed.
    pub min: Duration,
    /// Maximum latency observed.
    pub max: Duration,
    /// Mean latency.
    pub mean: Duration,
    /// Median latency (P50).
    pub median: Duration,
    /// 90th percentile latency.
    pub p90: Duration,
    /// 95th percentile latency.
    pub p95: Duration,
    /// 99th percentile latency.
    pub p99: Duration,
    /// 99.9th percentile latency.
    pub p999: Duration,
    /// Standard deviation.
    pub std_dev: Duration,
}

impl Default for LatencyStats {
    fn default() -> Self {
        Self {
            count: 0,
            min: Duration::ZERO,
            max: Duration::ZERO,
            mean: Duration::ZERO,
            median: Duration::ZERO,
            p90: Duration::ZERO,
            p95: Duration::ZERO,
            p99: Duration::ZERO,
            p999: Duration::ZERO,
            std_dev: Duration::ZERO,
        }
    }
}

/// A histogram bucket for latency distribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyBucket {
    /// Upper bound of the bucket in microseconds.
    pub upper_bound_us: u64,
    /// Number of samples in this bucket.
    pub count: u64,
    /// Percentage of total samples.
    pub percentage: f64,
}

/// An active latency measurement.
///
/// Created by [`LatencyTracker::start`] and completed by calling [`stop`](Self::stop).
pub struct LatencyMeasurement<'a> {
    tracker: &'a LatencyTracker,
    start: Instant,
}

impl<'a> LatencyMeasurement<'a> {
    /// Stop the measurement and record the latency.
    pub fn stop(self) -> Duration {
        let elapsed = self.start.elapsed();
        self.tracker.record(elapsed);
        elapsed
    }

    /// Stop the measurement with a custom end time.
    pub fn stop_at(self, end: Instant) -> Duration {
        let elapsed = end.duration_since(self.start);
        self.tracker.record(elapsed);
        elapsed
    }
}

/// A latency tracker for measuring operation timing.
///
/// Thread-safe and supports concurrent measurements.
pub struct LatencyTracker {
    name: String,
    samples: RwLock<VecDeque<Duration>>,
    count: AtomicU64,
    sum_nanos: AtomicU64,
    min_nanos: AtomicU64,
    max_nanos: AtomicU64,
    bucket_bounds_us: Vec<u64>,
    bucket_counts: Vec<AtomicU64>,
}

impl LatencyTracker {
    /// Create a new latency tracker with the given name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self::with_buckets(name, &DEFAULT_BUCKETS_US)
    }

    /// Create a new latency tracker with custom histogram buckets.
    #[must_use]
    pub fn with_buckets(name: impl Into<String>, bucket_bounds_us: &[u64]) -> Self {
        let bucket_counts = (0..=bucket_bounds_us.len())
            .map(|_| AtomicU64::new(0))
            .collect();

        Self {
            name: name.into(),
            samples: RwLock::new(VecDeque::with_capacity(MAX_SAMPLES)),
            count: AtomicU64::new(0),
            sum_nanos: AtomicU64::new(0),
            min_nanos: AtomicU64::new(u64::MAX),
            max_nanos: AtomicU64::new(0),
            bucket_bounds_us: bucket_bounds_us.to_vec(),
            bucket_counts,
        }
    }

    /// Get the tracker name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Start a new latency measurement.
    #[must_use]
    pub fn start(&self) -> LatencyMeasurement<'_> {
        LatencyMeasurement {
            tracker: self,
            start: Instant::now(),
        }
    }

    /// Record a latency duration directly.
    pub fn record(&self, duration: Duration) {
        let nanos = duration.as_nanos() as u64;

        // Update atomic counters
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum_nanos.fetch_add(nanos, Ordering::Relaxed);

        // Update min (using compare-and-swap loop)
        let mut current_min = self.min_nanos.load(Ordering::Relaxed);
        while nanos < current_min {
            match self.min_nanos.compare_exchange_weak(
                current_min,
                nanos,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_min = x,
            }
        }

        // Update max
        let mut current_max = self.max_nanos.load(Ordering::Relaxed);
        while nanos > current_max {
            match self.max_nanos.compare_exchange_weak(
                current_max,
                nanos,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_max = x,
            }
        }

        // Update histogram bucket
        let micros = nanos / 1_000;
        let bucket_idx = self
            .bucket_bounds_us
            .iter()
            .position(|&bound| micros <= bound)
            .unwrap_or(self.bucket_bounds_us.len());
        self.bucket_counts[bucket_idx].fetch_add(1, Ordering::Relaxed);

        // Store sample for percentile calculation
        let mut samples = self.samples.write();
        if samples.len() >= MAX_SAMPLES {
            samples.pop_front();
        }
        samples.push_back(duration);
    }

    /// Get the current count of measurements.
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Compute statistics from recorded measurements.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn statistics(&self) -> LatencyStats {
        let count = self.count.load(Ordering::Relaxed);
        if count == 0 {
            return LatencyStats::default();
        }

        let sum_nanos = self.sum_nanos.load(Ordering::Relaxed);
        let min_nanos = self.min_nanos.load(Ordering::Relaxed);
        let max_nanos = self.max_nanos.load(Ordering::Relaxed);

        let mean_nanos = sum_nanos / count;

        // Get sorted samples for percentile calculation
        let samples = self.samples.read();
        let mut sorted: Vec<Duration> = samples.iter().copied().collect();
        sorted.sort();

        let percentile = |p: f64| -> Duration {
            if sorted.is_empty() {
                return Duration::ZERO;
            }
            let idx = ((sorted.len() as f64 * p) as usize).min(sorted.len() - 1);
            sorted[idx]
        };

        // Calculate standard deviation
        let mean = Duration::from_nanos(mean_nanos);
        let variance: f64 = sorted
            .iter()
            .map(|&d| {
                let diff = d.as_nanos() as f64 - mean_nanos as f64;
                diff * diff
            })
            .sum::<f64>()
            / sorted.len().max(1) as f64;
        let std_dev = Duration::from_nanos(variance.sqrt() as u64);

        LatencyStats {
            count,
            min: Duration::from_nanos(if min_nanos == u64::MAX { 0 } else { min_nanos }),
            max: Duration::from_nanos(max_nanos),
            mean,
            median: percentile(0.5),
            p90: percentile(0.9),
            p95: percentile(0.95),
            p99: percentile(0.99),
            p999: percentile(0.999),
            std_dev,
        }
    }

    /// Get histogram buckets with counts.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn histogram(&self) -> Vec<LatencyBucket> {
        let total: u64 = self
            .bucket_counts
            .iter()
            .map(|c| c.load(Ordering::Relaxed))
            .sum();
        if total == 0 {
            return Vec::new();
        }

        let mut buckets = Vec::with_capacity(self.bucket_bounds_us.len() + 1);

        for (i, &bound) in self.bucket_bounds_us.iter().enumerate() {
            let count = self.bucket_counts[i].load(Ordering::Relaxed);
            buckets.push(LatencyBucket {
                upper_bound_us: bound,
                count,
                percentage: (count as f64 / total as f64) * 100.0,
            });
        }

        // Add overflow bucket
        let overflow_count =
            self.bucket_counts[self.bucket_bounds_us.len()].load(Ordering::Relaxed);
        buckets.push(LatencyBucket {
            upper_bound_us: u64::MAX,
            count: overflow_count,
            percentage: (overflow_count as f64 / total as f64) * 100.0,
        });

        buckets
    }

    /// Reset all measurements.
    pub fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
        self.sum_nanos.store(0, Ordering::Relaxed);
        self.min_nanos.store(u64::MAX, Ordering::Relaxed);
        self.max_nanos.store(0, Ordering::Relaxed);

        for bucket in &self.bucket_counts {
            bucket.store(0, Ordering::Relaxed);
        }

        self.samples.write().clear();
    }
}

/// Order latency tracker for measuring order submission latency.
///
/// Tracks latency from order submission to acknowledgment.
pub struct OrderLatencyTracker {
    /// Tracker for order submission latency.
    pub submission: LatencyTracker,
    /// Tracker for order acknowledgment latency.
    pub acknowledgment: LatencyTracker,
    /// Tracker for order fill latency.
    pub fill: LatencyTracker,
    /// Tracker for order cancellation latency.
    pub cancellation: LatencyTracker,
}

impl OrderLatencyTracker {
    /// Create a new order latency tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            submission: LatencyTracker::new("order_submission"),
            acknowledgment: LatencyTracker::new("order_acknowledgment"),
            fill: LatencyTracker::new("order_fill"),
            cancellation: LatencyTracker::new("order_cancellation"),
        }
    }

    /// Start measuring order submission latency.
    #[must_use]
    pub fn start_submission(&self) -> LatencyMeasurement<'_> {
        self.submission.start()
    }

    /// Start measuring order acknowledgment latency.
    #[must_use]
    pub fn start_acknowledgment(&self) -> LatencyMeasurement<'_> {
        self.acknowledgment.start()
    }

    /// Start measuring order fill latency.
    #[must_use]
    pub fn start_fill(&self) -> LatencyMeasurement<'_> {
        self.fill.start()
    }

    /// Start measuring order cancellation latency.
    #[must_use]
    pub fn start_cancellation(&self) -> LatencyMeasurement<'_> {
        self.cancellation.start()
    }

    /// Get statistics for all order latency types.
    #[must_use]
    pub fn all_statistics(&self) -> OrderLatencyStats {
        OrderLatencyStats {
            submission: self.submission.statistics(),
            acknowledgment: self.acknowledgment.statistics(),
            fill: self.fill.statistics(),
            cancellation: self.cancellation.statistics(),
        }
    }

    /// Reset all trackers.
    pub fn reset(&self) {
        self.submission.reset();
        self.acknowledgment.reset();
        self.fill.reset();
        self.cancellation.reset();
    }
}

impl Default for OrderLatencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for all order latency types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderLatencyStats {
    /// Order submission latency statistics.
    pub submission: LatencyStats,
    /// Order acknowledgment latency statistics.
    pub acknowledgment: LatencyStats,
    /// Order fill latency statistics.
    pub fill: LatencyStats,
    /// Order cancellation latency statistics.
    pub cancellation: LatencyStats,
}

/// Market data latency tracker for measuring market data processing latency.
///
/// Tracks latency for tick data, kline data, and orderbook updates.
pub struct MarketDataLatencyTracker {
    /// Tracker for tick data processing latency.
    pub tick: LatencyTracker,
    /// Tracker for kline data processing latency.
    pub kline: LatencyTracker,
    /// Tracker for orderbook update latency.
    pub orderbook: LatencyTracker,
    /// Tracker for end-to-end market data latency (exchange to strategy).
    pub end_to_end: LatencyTracker,
}

impl MarketDataLatencyTracker {
    /// Create a new market data latency tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tick: LatencyTracker::new("tick_processing"),
            kline: LatencyTracker::new("kline_processing"),
            orderbook: LatencyTracker::new("orderbook_processing"),
            end_to_end: LatencyTracker::new("market_data_end_to_end"),
        }
    }

    /// Start measuring tick processing latency.
    #[must_use]
    pub fn start_tick(&self) -> LatencyMeasurement<'_> {
        self.tick.start()
    }

    /// Start measuring kline processing latency.
    #[must_use]
    pub fn start_kline(&self) -> LatencyMeasurement<'_> {
        self.kline.start()
    }

    /// Start measuring orderbook update latency.
    #[must_use]
    pub fn start_orderbook(&self) -> LatencyMeasurement<'_> {
        self.orderbook.start()
    }

    /// Start measuring end-to-end market data latency.
    #[must_use]
    pub fn start_end_to_end(&self) -> LatencyMeasurement<'_> {
        self.end_to_end.start()
    }

    /// Record tick processing latency directly.
    pub fn record_tick(&self, duration: Duration) {
        self.tick.record(duration);
    }

    /// Record kline processing latency directly.
    pub fn record_kline(&self, duration: Duration) {
        self.kline.record(duration);
    }

    /// Record orderbook update latency directly.
    pub fn record_orderbook(&self, duration: Duration) {
        self.orderbook.record(duration);
    }

    /// Record end-to-end market data latency directly.
    pub fn record_end_to_end(&self, duration: Duration) {
        self.end_to_end.record(duration);
    }

    /// Get statistics for all market data latency types.
    #[must_use]
    pub fn all_statistics(&self) -> MarketDataLatencyStats {
        MarketDataLatencyStats {
            tick: self.tick.statistics(),
            kline: self.kline.statistics(),
            orderbook: self.orderbook.statistics(),
            end_to_end: self.end_to_end.statistics(),
        }
    }

    /// Reset all trackers.
    pub fn reset(&self) {
        self.tick.reset();
        self.kline.reset();
        self.orderbook.reset();
        self.end_to_end.reset();
    }
}

impl Default for MarketDataLatencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for all market data latency types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataLatencyStats {
    /// Tick data processing latency statistics.
    pub tick: LatencyStats,
    /// Kline data processing latency statistics.
    pub kline: LatencyStats,
    /// Orderbook update latency statistics.
    pub orderbook: LatencyStats,
    /// End-to-end market data latency statistics.
    pub end_to_end: LatencyStats,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_latency_tracker_basic() {
        let tracker = LatencyTracker::new("test");

        // Record some latencies
        tracker.record(Duration::from_micros(100));
        tracker.record(Duration::from_micros(200));
        tracker.record(Duration::from_micros(150));

        assert_eq!(tracker.count(), 3);

        let stats = tracker.statistics();
        assert_eq!(stats.count, 3);
        assert_eq!(stats.min, Duration::from_micros(100));
        assert_eq!(stats.max, Duration::from_micros(200));
    }

    #[test]
    fn test_latency_measurement() {
        let tracker = LatencyTracker::new("test");

        let measurement = tracker.start();
        thread::sleep(Duration::from_millis(1));
        let elapsed = measurement.stop();

        assert!(elapsed >= Duration::from_millis(1));
        assert_eq!(tracker.count(), 1);
    }

    #[test]
    fn test_latency_histogram() {
        let tracker = LatencyTracker::new("test");

        // Record latencies in different buckets
        tracker.record(Duration::from_micros(5)); // <= 10us bucket
        tracker.record(Duration::from_micros(25)); // <= 50us bucket
        tracker.record(Duration::from_micros(75)); // <= 100us bucket

        let histogram = tracker.histogram();
        assert!(!histogram.is_empty());

        // First bucket (<=10us) should have 1 sample
        assert_eq!(histogram[0].count, 1);
    }

    #[test]
    fn test_latency_reset() {
        let tracker = LatencyTracker::new("test");

        tracker.record(Duration::from_micros(100));
        assert_eq!(tracker.count(), 1);

        tracker.reset();
        assert_eq!(tracker.count(), 0);

        let stats = tracker.statistics();
        assert_eq!(stats.count, 0);
    }

    #[test]
    fn test_order_latency_tracker() {
        let tracker = OrderLatencyTracker::new();

        tracker.submission.record(Duration::from_micros(100));
        tracker.acknowledgment.record(Duration::from_micros(200));
        tracker.fill.record(Duration::from_micros(500));
        tracker.cancellation.record(Duration::from_micros(150));

        let stats = tracker.all_statistics();
        assert_eq!(stats.submission.count, 1);
        assert_eq!(stats.acknowledgment.count, 1);
        assert_eq!(stats.fill.count, 1);
        assert_eq!(stats.cancellation.count, 1);
    }

    #[test]
    fn test_market_data_latency_tracker() {
        let tracker = MarketDataLatencyTracker::new();

        tracker.record_tick(Duration::from_micros(50));
        tracker.record_kline(Duration::from_micros(100));
        tracker.record_orderbook(Duration::from_micros(75));
        tracker.record_end_to_end(Duration::from_micros(200));

        let stats = tracker.all_statistics();
        assert_eq!(stats.tick.count, 1);
        assert_eq!(stats.kline.count, 1);
        assert_eq!(stats.orderbook.count, 1);
        assert_eq!(stats.end_to_end.count, 1);
    }

    #[test]
    fn test_percentile_calculation() {
        let tracker = LatencyTracker::new("test");

        // Record 100 samples with increasing latency
        for i in 1..=100 {
            tracker.record(Duration::from_micros(i * 10));
        }

        let stats = tracker.statistics();
        assert_eq!(stats.count, 100);

        // P50 should be around 500us (50th sample)
        assert!(stats.median >= Duration::from_micros(400));
        assert!(stats.median <= Duration::from_micros(600));

        // P99 should be around 990us (99th sample)
        assert!(stats.p99 >= Duration::from_micros(900));
    }

    #[test]
    fn test_concurrent_recording() {
        use std::sync::Arc;

        let tracker = Arc::new(LatencyTracker::new("test"));
        let mut handles = vec![];

        // Spawn multiple threads recording latencies
        for _ in 0..4 {
            let tracker = Arc::clone(&tracker);
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    tracker.record(Duration::from_micros(i * 10));
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(tracker.count(), 400);
    }
}
