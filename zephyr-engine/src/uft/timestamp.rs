//! Hardware Timestamp Support for UFT Strategies.
//!
//! This module provides nanosecond-level timestamp support and latency
//! measurement utilities for ultra-high frequency trading.
//!
//! # Features
//!
//! - TSC (Time Stamp Counter) calibration for `x86_64`
//! - Nanosecond precision timestamps
//! - Latency measurement and statistics
//! - Low-overhead timing operations
//!
//! # Example
//!
//! ```ignore
//! use zephyr_engine::uft::timestamp::{LatencyTracker, TscCalibration};
//!
//! // Calibrate TSC frequency
//! let calibration = TscCalibration::calibrate();
//!
//! // Track latency
//! let mut tracker = LatencyTracker::new(1000);
//! let start = tracker.start();
//! // ... do work ...
//! tracker.record(start);
//!
//! // Get statistics
//! let stats = tracker.stats();
//! println!("Mean latency: {} ns", stats.mean_ns);
//! ```

#![allow(clippy::doc_markdown)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::inline_always)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::explicit_iter_loop)]
#![allow(clippy::uninlined_format_args)]

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// TSC (Time Stamp Counter) calibration data.
///
/// On x86_64 systems, the TSC provides a high-resolution counter
/// that can be used for nanosecond-precision timing. This struct
/// stores the calibration data needed to convert TSC ticks to nanoseconds.
#[derive(Debug, Clone, Copy)]
pub struct TscCalibration {
    /// TSC frequency in Hz.
    pub frequency_hz: u64,
    /// Nanoseconds per TSC tick (scaled by 2^32 for precision).
    pub ns_per_tick_scaled: u64,
    /// TSC value at calibration time.
    pub base_tsc: u64,
    /// System time at calibration (nanoseconds since epoch).
    pub base_time_ns: i64,
}

impl TscCalibration {
    /// Calibrates the TSC by measuring its frequency.
    ///
    /// This performs a short calibration period to determine the
    /// TSC frequency relative to the system clock.
    #[must_use]
    pub fn calibrate() -> Self {
        Self::calibrate_with_duration(Duration::from_millis(10))
    }

    /// Calibrates the TSC with a custom calibration duration.
    ///
    /// Longer durations provide more accurate calibration but
    /// take more time.
    #[must_use]
    pub fn calibrate_with_duration(duration: Duration) -> Self {
        let start_instant = Instant::now();
        let start_tsc = Self::read_tsc();
        let start_time_ns = Self::system_time_ns();

        // Wait for calibration period
        std::thread::sleep(duration);

        let end_instant = Instant::now();
        let end_tsc = Self::read_tsc();

        // Calculate frequency
        let elapsed_ns = end_instant.duration_since(start_instant).as_nanos() as u64;
        let tsc_delta = end_tsc.saturating_sub(start_tsc);

        // Avoid division by zero
        let frequency_hz = if elapsed_ns > 0 {
            (tsc_delta as u128 * 1_000_000_000 / elapsed_ns as u128) as u64
        } else {
            // Fallback to a reasonable default (3 GHz)
            3_000_000_000
        };

        // Calculate ns per tick (scaled by 2^32 for integer math)
        let ns_per_tick_scaled = if frequency_hz > 0 {
            ((1_000_000_000u128 << 32) / frequency_hz as u128) as u64
        } else {
            (1u64 << 32) / 3 // ~0.33 ns per tick at 3 GHz
        };

        Self {
            frequency_hz,
            ns_per_tick_scaled,
            base_tsc: start_tsc,
            base_time_ns: start_time_ns,
        }
    }

    /// Converts a TSC value to nanoseconds since epoch.
    #[must_use]
    pub fn tsc_to_ns(&self, tsc: u64) -> i64 {
        let tsc_delta = tsc.saturating_sub(self.base_tsc);
        let ns_delta = ((tsc_delta as u128 * self.ns_per_tick_scaled as u128) >> 32) as i64;
        self.base_time_ns.saturating_add(ns_delta)
    }

    /// Gets the current time in nanoseconds using TSC.
    #[must_use]
    pub fn now_ns(&self) -> i64 {
        self.tsc_to_ns(Self::read_tsc())
    }

    /// Reads the TSC value.
    #[cfg(target_arch = "x86_64")]
    #[inline(always)]
    fn read_tsc() -> u64 {
        // Use RDTSC instruction for lowest latency
        // Note: In production, consider using RDTSCP for serialization
        #[cfg(target_feature = "sse2")]
        unsafe {
            core::arch::x86_64::_rdtsc()
        }
        #[cfg(not(target_feature = "sse2"))]
        {
            Self::system_time_ns() as u64
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    #[inline(always)]
    fn read_tsc() -> u64 {
        // Fallback for non-x86_64 architectures
        Self::system_time_ns() as u64
    }

    /// Gets the system time in nanoseconds since epoch.
    fn system_time_ns() -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0)
    }
}

impl Default for TscCalibration {
    fn default() -> Self {
        Self::calibrate()
    }
}

/// A single latency measurement.
#[derive(Debug, Clone, Copy)]
pub struct LatencyMeasurement {
    /// Start timestamp in nanoseconds.
    pub start_ns: i64,
    /// End timestamp in nanoseconds.
    pub end_ns: i64,
    /// Latency in nanoseconds.
    pub latency_ns: i64,
}

impl LatencyMeasurement {
    /// Creates a new latency measurement.
    #[must_use]
    pub fn new(start_ns: i64, end_ns: i64) -> Self {
        Self {
            start_ns,
            end_ns,
            latency_ns: end_ns.saturating_sub(start_ns),
        }
    }
}

/// Latency statistics.
#[derive(Debug, Clone, Copy, Default)]
pub struct LatencyStats {
    /// Number of measurements.
    pub count: u64,
    /// Minimum latency in nanoseconds.
    pub min_ns: i64,
    /// Maximum latency in nanoseconds.
    pub max_ns: i64,
    /// Mean latency in nanoseconds.
    pub mean_ns: i64,
    /// 50th percentile (median) latency in nanoseconds.
    pub p50_ns: i64,
    /// 90th percentile latency in nanoseconds.
    pub p90_ns: i64,
    /// 99th percentile latency in nanoseconds.
    pub p99_ns: i64,
    /// 99.9th percentile latency in nanoseconds.
    pub p999_ns: i64,
    /// Standard deviation in nanoseconds.
    pub stddev_ns: i64,
}

impl LatencyStats {
    /// Returns true if no measurements have been recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// Latency tracker for measuring operation latencies.
///
/// Uses a ring buffer to store recent latency measurements
/// and provides statistical analysis.
pub struct LatencyTracker {
    /// Ring buffer of latency measurements (in nanoseconds).
    measurements: Box<[AtomicI64]>,
    /// Current write index.
    write_index: AtomicU64,
    /// Buffer capacity.
    capacity: usize,
    /// Total count of measurements (may exceed capacity).
    total_count: AtomicU64,
    /// Running sum for mean calculation.
    running_sum: AtomicI64,
    /// Minimum latency seen.
    min_latency: AtomicI64,
    /// Maximum latency seen.
    max_latency: AtomicI64,
}

impl LatencyTracker {
    /// Creates a new latency tracker with the specified capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let measurements: Vec<AtomicI64> = (0..capacity).map(|_| AtomicI64::new(0)).collect();

        Self {
            measurements: measurements.into_boxed_slice(),
            write_index: AtomicU64::new(0),
            capacity,
            total_count: AtomicU64::new(0),
            running_sum: AtomicI64::new(0),
            min_latency: AtomicI64::new(i64::MAX),
            max_latency: AtomicI64::new(i64::MIN),
        }
    }

    /// Records the start of an operation.
    ///
    /// Returns the start timestamp in nanoseconds.
    #[must_use]
    #[inline(always)]
    pub fn start(&self) -> i64 {
        Self::now_ns()
    }

    /// Records the end of an operation and stores the latency.
    ///
    /// # Arguments
    ///
    /// * `start_ns` - The start timestamp from `start()`
    ///
    /// Returns the latency in nanoseconds.
    #[inline(always)]
    pub fn record(&self, start_ns: i64) -> i64 {
        let end_ns = Self::now_ns();
        let latency = end_ns.saturating_sub(start_ns);
        self.record_latency(latency);
        latency
    }

    /// Records a latency value directly.
    #[inline(always)]
    pub fn record_latency(&self, latency_ns: i64) {
        // Store in ring buffer
        let index = self.write_index.fetch_add(1, Ordering::Relaxed) as usize % self.capacity;

        // Get old value for running sum update
        let old_value = self.measurements[index].swap(latency_ns, Ordering::Relaxed);

        // Update running sum (subtract old, add new)
        let count = self.total_count.fetch_add(1, Ordering::Relaxed);
        if count >= self.capacity as u64 {
            // Buffer is full, subtract old value
            self.running_sum.fetch_sub(old_value, Ordering::Relaxed);
        }
        self.running_sum.fetch_add(latency_ns, Ordering::Relaxed);

        // Update min/max
        self.update_min(latency_ns);
        self.update_max(latency_ns);
    }

    /// Updates the minimum latency atomically.
    fn update_min(&self, latency: i64) {
        let mut current = self.min_latency.load(Ordering::Relaxed);
        while latency < current {
            match self.min_latency.compare_exchange_weak(
                current,
                latency,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(c) => current = c,
            }
        }
    }

    /// Updates the maximum latency atomically.
    fn update_max(&self, latency: i64) {
        let mut current = self.max_latency.load(Ordering::Relaxed);
        while latency > current {
            match self.max_latency.compare_exchange_weak(
                current,
                latency,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(c) => current = c,
            }
        }
    }

    /// Gets the current timestamp in nanoseconds.
    #[inline(always)]
    fn now_ns() -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0)
    }

    /// Returns the number of measurements recorded.
    #[must_use]
    pub fn count(&self) -> u64 {
        self.total_count.load(Ordering::Relaxed)
    }

    /// Returns the number of measurements in the buffer.
    #[must_use]
    pub fn buffer_count(&self) -> usize {
        let count = self.total_count.load(Ordering::Relaxed) as usize;
        count.min(self.capacity)
    }

    /// Computes latency statistics from the recorded measurements.
    #[must_use]
    #[allow(clippy::similar_names)]
    pub fn stats(&self) -> LatencyStats {
        let count = self.total_count.load(Ordering::Relaxed);
        if count == 0 {
            return LatencyStats::default();
        }

        let buffer_count = (count as usize).min(self.capacity);

        // Collect measurements into a sorted vector
        let mut latencies: Vec<i64> = self.measurements[..buffer_count]
            .iter()
            .map(|a| a.load(Ordering::Relaxed))
            .collect();
        latencies.sort_unstable();

        // Calculate statistics
        let min_ns = self.min_latency.load(Ordering::Relaxed);
        let max_ns = self.max_latency.load(Ordering::Relaxed);
        let sum = self.running_sum.load(Ordering::Relaxed);
        let mean_ns = sum / buffer_count as i64;

        // Percentiles
        let p50_ns = Self::percentile(&latencies, 50);
        let p90_ns = Self::percentile(&latencies, 90);
        let p99_ns = Self::percentile(&latencies, 99);
        let p999_ns = Self::percentile(&latencies, 999);

        // Standard deviation
        let variance: i64 = latencies
            .iter()
            .map(|&x| {
                let diff = x - mean_ns;
                diff.saturating_mul(diff)
            })
            .sum::<i64>()
            / buffer_count as i64;
        let stddev_ns = (variance as f64).sqrt() as i64;

        LatencyStats {
            count,
            min_ns,
            max_ns,
            mean_ns,
            p50_ns,
            p90_ns,
            p99_ns,
            p999_ns,
            stddev_ns,
        }
    }

    /// Calculates a percentile from sorted data.
    fn percentile(sorted: &[i64], percentile: usize) -> i64 {
        if sorted.is_empty() {
            return 0;
        }
        // For p50, p90, p99 we use percentile values 50, 90, 99
        // For p999 we use 999 (99.9th percentile)
        let divisor = if percentile > 100 { 1000 } else { 100 };
        let index = (sorted.len() * percentile / divisor).min(sorted.len() - 1);
        sorted[index]
    }

    /// Resets all measurements.
    pub fn reset(&self) {
        self.write_index.store(0, Ordering::Relaxed);
        self.total_count.store(0, Ordering::Relaxed);
        self.running_sum.store(0, Ordering::Relaxed);
        self.min_latency.store(i64::MAX, Ordering::Relaxed);
        self.max_latency.store(i64::MIN, Ordering::Relaxed);
        for m in self.measurements.iter() {
            m.store(0, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tsc_calibration() {
        let calibration = TscCalibration::calibrate_with_duration(Duration::from_millis(5));

        // Frequency should be reasonable (100 MHz to 10 GHz)
        assert!(calibration.frequency_hz > 100_000_000);
        assert!(calibration.frequency_hz < 10_000_000_000);

        // Base time should be recent
        assert!(calibration.base_time_ns > 0);
    }

    #[test]
    fn test_tsc_now_ns() {
        let calibration = TscCalibration::calibrate_with_duration(Duration::from_millis(5));

        let ns1 = calibration.now_ns();
        std::thread::sleep(Duration::from_micros(100));
        let ns2 = calibration.now_ns();

        // Time should advance
        assert!(ns2 > ns1);

        // Should be roughly 100+ microseconds apart
        let diff = ns2 - ns1;
        assert!(diff >= 100_000, "Expected at least 100us, got {} ns", diff);
    }

    #[test]
    fn test_latency_measurement() {
        let m = LatencyMeasurement::new(1000, 2500);
        assert_eq!(m.start_ns, 1000);
        assert_eq!(m.end_ns, 2500);
        assert_eq!(m.latency_ns, 1500);
    }

    #[test]
    fn test_latency_tracker_basic() {
        let tracker = LatencyTracker::new(100);

        assert_eq!(tracker.count(), 0);
        assert!(tracker.stats().is_empty());

        // Record some latencies
        tracker.record_latency(1000);
        tracker.record_latency(2000);
        tracker.record_latency(3000);

        assert_eq!(tracker.count(), 3);

        let stats = tracker.stats();
        assert_eq!(stats.count, 3);
        assert_eq!(stats.min_ns, 1000);
        assert_eq!(stats.max_ns, 3000);
        assert_eq!(stats.mean_ns, 2000);
    }

    #[test]
    fn test_latency_tracker_start_record() {
        let tracker = LatencyTracker::new(100);

        let start = tracker.start();
        std::thread::sleep(Duration::from_micros(100));
        let latency = tracker.record(start);

        // Latency should be at least 100 microseconds
        assert!(
            latency >= 100_000,
            "Expected at least 100us, got {} ns",
            latency
        );
        assert_eq!(tracker.count(), 1);
    }

    #[test]
    fn test_latency_tracker_ring_buffer() {
        let tracker = LatencyTracker::new(5);

        // Fill buffer
        for i in 1..=5 {
            tracker.record_latency(i * 1000);
        }

        assert_eq!(tracker.count(), 5);
        assert_eq!(tracker.buffer_count(), 5);

        // Overflow buffer
        for i in 6..=10 {
            tracker.record_latency(i * 1000);
        }

        assert_eq!(tracker.count(), 10);
        assert_eq!(tracker.buffer_count(), 5);

        // Stats should reflect recent values
        let stats = tracker.stats();
        assert_eq!(stats.count, 10);
        // Min/max track all-time values
        assert_eq!(stats.min_ns, 1000);
        assert_eq!(stats.max_ns, 10000);
    }

    #[test]
    fn test_latency_tracker_percentiles() {
        let tracker = LatencyTracker::new(100);

        // Record 100 values from 1 to 100
        for i in 1..=100 {
            tracker.record_latency(i * 1000);
        }

        let stats = tracker.stats();

        // Check percentiles (approximate due to discrete values)
        assert!(stats.p50_ns >= 49_000 && stats.p50_ns <= 51_000);
        assert!(stats.p90_ns >= 89_000 && stats.p90_ns <= 91_000);
        assert!(stats.p99_ns >= 98_000 && stats.p99_ns <= 100_000);
    }

    #[test]
    fn test_latency_tracker_reset() {
        let tracker = LatencyTracker::new(100);

        tracker.record_latency(1000);
        tracker.record_latency(2000);

        assert_eq!(tracker.count(), 2);

        tracker.reset();

        assert_eq!(tracker.count(), 0);
        assert!(tracker.stats().is_empty());
    }

    #[test]
    fn test_latency_stats_default() {
        let stats = LatencyStats::default();
        assert!(stats.is_empty());
        assert_eq!(stats.count, 0);
    }
}
