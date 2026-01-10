//! # Zephyr Benchmark
//!
//! Performance benchmarking framework for the Zephyr cryptocurrency trading system.
//!
//! This crate provides:
//! - Latency measurement for orders and market data
//! - Throughput testing for orders and market data processing
//! - Performance reporting with baseline comparison
//! - Performance alerting with threshold monitoring
//!
//! ## Features
//!
//! - **Latency Measurement**: Precise timing for order submission and market data processing
//! - **Throughput Testing**: Measure orders/second and ticks/second capacity
//! - **Performance Reports**: Generate reports with statistics and trend analysis
//! - **Alerting**: Monitor for performance degradation and emit alerts
//!
//! ## Example
//!
//! ```rust,ignore
//! use zephyr_benchmark::{LatencyTracker, ThroughputTester, PerformanceReporter};
//!
//! // Track order latency
//! let tracker = LatencyTracker::new("order_submission");
//! let measurement = tracker.start();
//! // ... perform operation ...
//! measurement.stop();
//!
//! // Get statistics
//! let stats = tracker.statistics();
//! println!("P99 latency: {:?}", stats.p99);
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::module_name_repetitions)]

pub mod alert;
pub mod latency;
pub mod report;
pub mod throughput;

pub use alert::{
    AlertConfig, AlertSeverity, AlertType, PerformanceAlert, PerformanceMonitor, ThresholdConfig,
};
pub use latency::{
    LatencyBucket, LatencyMeasurement, LatencyStats, LatencyTracker, MarketDataLatencyTracker,
    OrderLatencyTracker,
};
pub use report::{
    BaselineComparison, PerformanceReport, PerformanceReporter, ReportConfig, TrendAnalysis,
    TrendDirection,
};
pub use throughput::{
    MarketDataThroughputTester, OrderThroughputTester, ThroughputResult, ThroughputStats,
    ThroughputTester,
};
