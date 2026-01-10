//! Throughput testing for orders and market data.
//!
//! This module provides throughput measurement for:
//! - Order submission rate (orders per second)
//! - Market data processing rate (ticks per second)
//!
//! ## Features
//!
//! - Configurable test duration and warmup period
//! - Statistical analysis of throughput results
//! - Support for burst and sustained load testing

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

/// Throughput statistics from a test run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputStats {
    /// Total number of operations completed.
    pub total_operations: u64,
    /// Test duration.
    pub duration: Duration,
    /// Operations per second.
    pub ops_per_second: f64,
    /// Peak operations per second (measured in 1-second windows).
    pub peak_ops_per_second: f64,
    /// Minimum operations per second.
    pub min_ops_per_second: f64,
    /// Average operations per second.
    pub avg_ops_per_second: f64,
}

impl Default for ThroughputStats {
    fn default() -> Self {
        Self {
            total_operations: 0,
            duration: Duration::ZERO,
            ops_per_second: 0.0,
            peak_ops_per_second: 0.0,
            min_ops_per_second: 0.0,
            avg_ops_per_second: 0.0,
        }
    }
}

/// Result of a throughput test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputResult {
    /// Test name.
    pub name: String,
    /// Throughput statistics.
    pub stats: ThroughputStats,
    /// Per-second throughput samples.
    pub samples: Vec<u64>,
    /// Test start time (Unix timestamp).
    pub start_time: i64,
    /// Test end time (Unix timestamp).
    pub end_time: i64,
}

/// Configuration for throughput testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputConfig {
    /// Test duration.
    pub duration: Duration,
    /// Warmup duration (operations during warmup are not counted).
    pub warmup: Duration,
    /// Sample interval for per-second measurements.
    pub sample_interval: Duration,
}

impl Default for ThroughputConfig {
    fn default() -> Self {
        Self {
            duration: Duration::from_secs(10),
            warmup: Duration::from_secs(1),
            sample_interval: Duration::from_secs(1),
        }
    }
}

/// A throughput tester for measuring operation rates.
pub struct ThroughputTester {
    name: String,
    config: ThroughputConfig,
    operation_count: AtomicU64,
    samples: RwLock<Vec<u64>>,
    start_time: RwLock<Option<Instant>>,
    last_sample_time: RwLock<Option<Instant>>,
    last_sample_count: AtomicU64,
    is_warmup: RwLock<bool>,
}

impl ThroughputTester {
    /// Create a new throughput tester with the given name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self::with_config(name, ThroughputConfig::default())
    }

    /// Create a new throughput tester with custom configuration.
    #[must_use]
    pub fn with_config(name: impl Into<String>, config: ThroughputConfig) -> Self {
        Self {
            name: name.into(),
            config,
            operation_count: AtomicU64::new(0),
            samples: RwLock::new(Vec::new()),
            start_time: RwLock::new(None),
            last_sample_time: RwLock::new(None),
            last_sample_count: AtomicU64::new(0),
            is_warmup: RwLock::new(true),
        }
    }

    /// Get the tester name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Start the throughput test.
    pub fn start(&self) {
        let now = Instant::now();
        *self.start_time.write() = Some(now);
        *self.last_sample_time.write() = Some(now);
        *self.is_warmup.write() = true;
        self.operation_count.store(0, Ordering::Relaxed);
        self.last_sample_count.store(0, Ordering::Relaxed);
        self.samples.write().clear();
    }

    /// Record an operation completion.
    pub fn record_operation(&self) {
        self.operation_count.fetch_add(1, Ordering::Relaxed);
        self.maybe_sample();
    }

    /// Record multiple operation completions.
    pub fn record_operations(&self, count: u64) {
        self.operation_count.fetch_add(count, Ordering::Relaxed);
        self.maybe_sample();
    }

    /// Check if warmup period has ended and take samples if needed.
    fn maybe_sample(&self) {
        let start = match *self.start_time.read() {
            Some(t) => t,
            None => return,
        };

        let now = Instant::now();
        let elapsed = now.duration_since(start);

        // Check if warmup period has ended (only if warmup is configured)
        if *self.is_warmup.read() && !self.config.warmup.is_zero() && elapsed >= self.config.warmup
        {
            *self.is_warmup.write() = false;
            // Reset counters after warmup
            self.operation_count.store(0, Ordering::Relaxed);
            self.last_sample_count.store(0, Ordering::Relaxed);
            *self.last_sample_time.write() = Some(now);
        } else if *self.is_warmup.read() && self.config.warmup.is_zero() {
            // No warmup configured, immediately exit warmup mode
            *self.is_warmup.write() = false;
        }

        // Take sample if interval has passed
        let last_sample = match *self.last_sample_time.read() {
            Some(t) => t,
            None => return,
        };

        if now.duration_since(last_sample) >= self.config.sample_interval && !*self.is_warmup.read()
        {
            let current_count = self.operation_count.load(Ordering::Relaxed);
            let last_count = self
                .last_sample_count
                .swap(current_count, Ordering::Relaxed);
            let ops_in_interval = current_count.saturating_sub(last_count);

            self.samples.write().push(ops_in_interval);
            *self.last_sample_time.write() = Some(now);
        }
    }

    /// Stop the test and return results.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn stop(&self) -> ThroughputResult {
        let start = self.start_time.read().unwrap_or_else(Instant::now);
        let end = Instant::now();
        let duration = end.duration_since(start).saturating_sub(self.config.warmup);

        let total_operations = self.operation_count.load(Ordering::Relaxed);
        let samples = self.samples.read().clone();

        let ops_per_second = if duration.as_secs_f64() > 0.0 {
            total_operations as f64 / duration.as_secs_f64()
        } else {
            0.0
        };

        let (peak, min, avg) = if samples.is_empty() {
            (ops_per_second, ops_per_second, ops_per_second)
        } else {
            let peak = *samples.iter().max().unwrap_or(&0) as f64;
            let min = *samples.iter().min().unwrap_or(&0) as f64;
            let avg = samples.iter().sum::<u64>() as f64 / samples.len() as f64;
            (peak, min, avg)
        };

        let now = chrono::Utc::now();
        let start_time =
            (now - chrono::Duration::from_std(duration).unwrap_or_default()).timestamp();
        let end_time = now.timestamp();

        ThroughputResult {
            name: self.name.clone(),
            stats: ThroughputStats {
                total_operations,
                duration,
                ops_per_second,
                peak_ops_per_second: peak,
                min_ops_per_second: min,
                avg_ops_per_second: avg,
            },
            samples,
            start_time,
            end_time,
        }
    }

    /// Check if the test duration has elapsed.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        let start = match *self.start_time.read() {
            Some(t) => t,
            None => return false,
        };

        let elapsed = Instant::now().duration_since(start);
        elapsed >= self.config.warmup + self.config.duration
    }

    /// Get current operations per second (live measurement).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn current_ops_per_second(&self) -> f64 {
        let start = match *self.start_time.read() {
            Some(t) => t,
            None => return 0.0,
        };

        let elapsed = Instant::now().duration_since(start);
        let effective_elapsed = elapsed.saturating_sub(self.config.warmup);

        if effective_elapsed.as_secs_f64() > 0.0 {
            self.operation_count.load(Ordering::Relaxed) as f64 / effective_elapsed.as_secs_f64()
        } else {
            0.0
        }
    }

    /// Reset the tester for a new test.
    pub fn reset(&self) {
        self.operation_count.store(0, Ordering::Relaxed);
        self.last_sample_count.store(0, Ordering::Relaxed);
        *self.start_time.write() = None;
        *self.last_sample_time.write() = None;
        *self.is_warmup.write() = true;
        self.samples.write().clear();
    }
}

/// Order throughput tester for measuring order submission rates.
pub struct OrderThroughputTester {
    /// Tester for order submissions.
    pub submission: ThroughputTester,
    /// Tester for order cancellations.
    pub cancellation: ThroughputTester,
    /// Tester for order modifications.
    pub modification: ThroughputTester,
}

impl OrderThroughputTester {
    /// Create a new order throughput tester.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(ThroughputConfig::default())
    }

    /// Create a new order throughput tester with custom configuration.
    #[must_use]
    pub fn with_config(config: ThroughputConfig) -> Self {
        Self {
            submission: ThroughputTester::with_config("order_submission", config.clone()),
            cancellation: ThroughputTester::with_config("order_cancellation", config.clone()),
            modification: ThroughputTester::with_config("order_modification", config),
        }
    }

    /// Start all throughput tests.
    pub fn start(&self) {
        self.submission.start();
        self.cancellation.start();
        self.modification.start();
    }

    /// Record an order submission.
    pub fn record_submission(&self) {
        self.submission.record_operation();
    }

    /// Record an order cancellation.
    pub fn record_cancellation(&self) {
        self.cancellation.record_operation();
    }

    /// Record an order modification.
    pub fn record_modification(&self) {
        self.modification.record_operation();
    }

    /// Stop all tests and return results.
    #[must_use]
    pub fn stop(&self) -> OrderThroughputResults {
        OrderThroughputResults {
            submission: self.submission.stop(),
            cancellation: self.cancellation.stop(),
            modification: self.modification.stop(),
        }
    }

    /// Check if all tests are complete.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.submission.is_complete()
            && self.cancellation.is_complete()
            && self.modification.is_complete()
    }

    /// Reset all testers.
    pub fn reset(&self) {
        self.submission.reset();
        self.cancellation.reset();
        self.modification.reset();
    }
}

impl Default for OrderThroughputTester {
    fn default() -> Self {
        Self::new()
    }
}

/// Results from order throughput testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderThroughputResults {
    /// Order submission throughput results.
    pub submission: ThroughputResult,
    /// Order cancellation throughput results.
    pub cancellation: ThroughputResult,
    /// Order modification throughput results.
    pub modification: ThroughputResult,
}

/// Market data throughput tester for measuring data processing rates.
pub struct MarketDataThroughputTester {
    /// Tester for tick data processing.
    pub tick: ThroughputTester,
    /// Tester for kline data processing.
    pub kline: ThroughputTester,
    /// Tester for orderbook updates.
    pub orderbook: ThroughputTester,
}

impl MarketDataThroughputTester {
    /// Create a new market data throughput tester.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(ThroughputConfig::default())
    }

    /// Create a new market data throughput tester with custom configuration.
    #[must_use]
    pub fn with_config(config: ThroughputConfig) -> Self {
        Self {
            tick: ThroughputTester::with_config("tick_processing", config.clone()),
            kline: ThroughputTester::with_config("kline_processing", config.clone()),
            orderbook: ThroughputTester::with_config("orderbook_processing", config),
        }
    }

    /// Start all throughput tests.
    pub fn start(&self) {
        self.tick.start();
        self.kline.start();
        self.orderbook.start();
    }

    /// Record a tick processed.
    pub fn record_tick(&self) {
        self.tick.record_operation();
    }

    /// Record multiple ticks processed.
    pub fn record_ticks(&self, count: u64) {
        self.tick.record_operations(count);
    }

    /// Record a kline processed.
    pub fn record_kline(&self) {
        self.kline.record_operation();
    }

    /// Record an orderbook update processed.
    pub fn record_orderbook(&self) {
        self.orderbook.record_operation();
    }

    /// Stop all tests and return results.
    #[must_use]
    pub fn stop(&self) -> MarketDataThroughputResults {
        MarketDataThroughputResults {
            tick: self.tick.stop(),
            kline: self.kline.stop(),
            orderbook: self.orderbook.stop(),
        }
    }

    /// Check if all tests are complete.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.tick.is_complete() && self.kline.is_complete() && self.orderbook.is_complete()
    }

    /// Reset all testers.
    pub fn reset(&self) {
        self.tick.reset();
        self.kline.reset();
        self.orderbook.reset();
    }
}

impl Default for MarketDataThroughputTester {
    fn default() -> Self {
        Self::new()
    }
}

/// Results from market data throughput testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataThroughputResults {
    /// Tick data throughput results.
    pub tick: ThroughputResult,
    /// Kline data throughput results.
    pub kline: ThroughputResult,
    /// Orderbook update throughput results.
    pub orderbook: ThroughputResult,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_throughput_tester_basic() {
        let config = ThroughputConfig {
            duration: Duration::from_millis(100),
            warmup: Duration::from_millis(10),
            sample_interval: Duration::from_millis(50),
        };
        let tester = ThroughputTester::with_config("test", config);

        tester.start();

        // Record operations
        for _ in 0..100 {
            tester.record_operation();
            thread::sleep(Duration::from_micros(100));
        }

        let result = tester.stop();
        assert!(result.stats.total_operations > 0);
        assert!(result.stats.ops_per_second > 0.0);
    }

    #[test]
    fn test_throughput_tester_batch() {
        // Use no warmup for simpler testing
        let config = ThroughputConfig {
            duration: Duration::from_millis(100),
            warmup: Duration::ZERO,
            sample_interval: Duration::from_millis(25),
        };
        let tester = ThroughputTester::with_config("test", config);

        tester.start();
        tester.record_operations(1000);
        thread::sleep(Duration::from_millis(50));

        let result = tester.stop();
        assert!(
            result.stats.total_operations >= 1000,
            "Expected >= 1000 operations, got {}",
            result.stats.total_operations
        );
    }

    #[test]
    fn test_throughput_is_complete() {
        let config = ThroughputConfig {
            duration: Duration::from_millis(50),
            warmup: Duration::from_millis(10),
            sample_interval: Duration::from_millis(25),
        };
        let tester = ThroughputTester::with_config("test", config);

        tester.start();
        assert!(!tester.is_complete());

        thread::sleep(Duration::from_millis(70));
        assert!(tester.is_complete());
    }

    #[test]
    fn test_order_throughput_tester() {
        // Use no warmup for simpler testing
        let config = ThroughputConfig {
            duration: Duration::from_millis(100),
            warmup: Duration::ZERO,
            sample_interval: Duration::from_millis(25),
        };
        let tester = OrderThroughputTester::with_config(config);

        tester.start();

        tester.record_submission();
        tester.record_cancellation();
        tester.record_modification();

        thread::sleep(Duration::from_millis(50));

        let results = tester.stop();
        assert!(
            results.submission.stats.total_operations >= 1,
            "Expected >= 1 submission, got {}",
            results.submission.stats.total_operations
        );
        assert!(
            results.cancellation.stats.total_operations >= 1,
            "Expected >= 1 cancellation, got {}",
            results.cancellation.stats.total_operations
        );
        assert!(
            results.modification.stats.total_operations >= 1,
            "Expected >= 1 modification, got {}",
            results.modification.stats.total_operations
        );
    }

    #[test]
    fn test_market_data_throughput_tester() {
        // Use no warmup for simpler testing
        let config = ThroughputConfig {
            duration: Duration::from_millis(100),
            warmup: Duration::ZERO,
            sample_interval: Duration::from_millis(25),
        };
        let tester = MarketDataThroughputTester::with_config(config);

        tester.start();

        tester.record_tick();
        tester.record_ticks(10);
        tester.record_kline();
        tester.record_orderbook();

        thread::sleep(Duration::from_millis(50));

        let results = tester.stop();
        assert!(
            results.tick.stats.total_operations >= 11,
            "Expected >= 11 ticks, got {}",
            results.tick.stats.total_operations
        );
        assert!(
            results.kline.stats.total_operations >= 1,
            "Expected >= 1 kline, got {}",
            results.kline.stats.total_operations
        );
        assert!(
            results.orderbook.stats.total_operations >= 1,
            "Expected >= 1 orderbook, got {}",
            results.orderbook.stats.total_operations
        );
    }

    #[test]
    fn test_throughput_reset() {
        let tester = ThroughputTester::new("test");

        tester.start();
        tester.record_operations(100);

        tester.reset();

        // After reset, should be able to start fresh
        tester.start();
        tester.record_operation();

        let result = tester.stop();
        // Should only have 1 operation after reset
        assert!(result.stats.total_operations <= 1);
    }
}
