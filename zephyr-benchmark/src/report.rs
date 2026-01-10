//! Performance reporting with baseline comparison and trend analysis.
//!
//! This module provides:
//! - Performance report generation
//! - Baseline comparison for regression detection
//! - Trend analysis over time
//!
//! ## Features
//!
//! - JSON and human-readable report formats
//! - Historical baseline storage and comparison
//! - Statistical trend analysis

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::latency::{MarketDataLatencyStats, OrderLatencyStats};
use crate::throughput::{MarketDataThroughputResults, OrderThroughputResults};

/// Configuration for performance reporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportConfig {
    /// Report name/identifier.
    pub name: String,
    /// Include detailed histogram data.
    pub include_histograms: bool,
    /// Include per-second samples.
    pub include_samples: bool,
    /// Baseline comparison threshold (percentage).
    pub regression_threshold_percent: f64,
    /// Number of historical reports to retain for trend analysis.
    pub history_size: usize,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            name: "performance_report".to_string(),
            include_histograms: true,
            include_samples: false,
            regression_threshold_percent: 10.0,
            history_size: 100,
        }
    }
}

/// Direction of a performance trend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendDirection {
    /// Performance is improving.
    Improving,
    /// Performance is stable.
    Stable,
    /// Performance is degrading.
    Degrading,
}

/// Trend analysis for a metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    /// Metric name.
    pub metric: String,
    /// Trend direction.
    pub direction: TrendDirection,
    /// Percentage change from baseline.
    pub change_percent: f64,
    /// Number of data points analyzed.
    pub sample_count: usize,
    /// Confidence level (0.0 to 1.0).
    pub confidence: f64,
}

/// Comparison against a baseline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineComparison {
    /// Metric name.
    pub metric: String,
    /// Current value.
    pub current: f64,
    /// Baseline value.
    pub baseline: f64,
    /// Percentage difference (positive = worse, negative = better for latency).
    pub diff_percent: f64,
    /// Whether this represents a regression.
    pub is_regression: bool,
}

/// A complete performance report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    /// Report name.
    pub name: String,
    /// Report generation timestamp.
    pub timestamp: DateTime<Utc>,
    /// Report duration.
    pub duration: Duration,
    /// Order latency statistics.
    pub order_latency: Option<OrderLatencyStats>,
    /// Market data latency statistics.
    pub market_data_latency: Option<MarketDataLatencyStats>,
    /// Order throughput results.
    pub order_throughput: Option<OrderThroughputResults>,
    /// Market data throughput results.
    pub market_data_throughput: Option<MarketDataThroughputResults>,
    /// Baseline comparisons.
    pub comparisons: Vec<BaselineComparison>,
    /// Trend analyses.
    pub trends: Vec<TrendAnalysis>,
    /// Custom metrics.
    pub custom_metrics: HashMap<String, f64>,
    /// Summary text.
    pub summary: String,
}

impl PerformanceReport {
    /// Create a new empty performance report.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            timestamp: Utc::now(),
            duration: Duration::ZERO,
            order_latency: None,
            market_data_latency: None,
            order_throughput: None,
            market_data_throughput: None,
            comparisons: Vec::new(),
            trends: Vec::new(),
            custom_metrics: HashMap::new(),
            summary: String::new(),
        }
    }

    /// Set the report duration.
    #[must_use]
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Set order latency statistics.
    #[must_use]
    pub fn with_order_latency(mut self, stats: OrderLatencyStats) -> Self {
        self.order_latency = Some(stats);
        self
    }

    /// Set market data latency statistics.
    #[must_use]
    pub fn with_market_data_latency(mut self, stats: MarketDataLatencyStats) -> Self {
        self.market_data_latency = Some(stats);
        self
    }

    /// Set order throughput results.
    #[must_use]
    pub fn with_order_throughput(mut self, results: OrderThroughputResults) -> Self {
        self.order_throughput = Some(results);
        self
    }

    /// Set market data throughput results.
    #[must_use]
    pub fn with_market_data_throughput(mut self, results: MarketDataThroughputResults) -> Self {
        self.market_data_throughput = Some(results);
        self
    }

    /// Add a baseline comparison.
    pub fn add_comparison(&mut self, comparison: BaselineComparison) {
        self.comparisons.push(comparison);
    }

    /// Add a trend analysis.
    pub fn add_trend(&mut self, trend: TrendAnalysis) {
        self.trends.push(trend);
    }

    /// Add a custom metric.
    pub fn add_custom_metric(&mut self, name: impl Into<String>, value: f64) {
        self.custom_metrics.insert(name.into(), value);
    }

    /// Set the summary text.
    pub fn set_summary(&mut self, summary: impl Into<String>) {
        self.summary = summary.into();
    }

    /// Check if any regressions were detected.
    #[must_use]
    pub fn has_regressions(&self) -> bool {
        self.comparisons.iter().any(|c| c.is_regression)
    }

    /// Get all regressions.
    #[must_use]
    pub fn regressions(&self) -> Vec<&BaselineComparison> {
        self.comparisons
            .iter()
            .filter(|c| c.is_regression)
            .collect()
    }

    /// Serialize to JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Generate a human-readable summary.
    #[must_use]
    pub fn to_summary(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!("Performance Report: {}", self.name));
        lines.push(format!(
            "Generated: {}",
            self.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        lines.push(format!("Duration: {:?}", self.duration));
        lines.push(String::new());

        // Order latency summary
        if let Some(ref stats) = self.order_latency {
            lines.push("Order Latency:".to_string());
            lines.push(format!("  Submission P99: {:?}", stats.submission.p99));
            lines.push(format!(
                "  Acknowledgment P99: {:?}",
                stats.acknowledgment.p99
            ));
            lines.push(format!("  Fill P99: {:?}", stats.fill.p99));
            lines.push(String::new());
        }

        // Market data latency summary
        if let Some(ref stats) = self.market_data_latency {
            lines.push("Market Data Latency:".to_string());
            lines.push(format!("  Tick P99: {:?}", stats.tick.p99));
            lines.push(format!("  Orderbook P99: {:?}", stats.orderbook.p99));
            lines.push(String::new());
        }

        // Order throughput summary
        if let Some(ref results) = self.order_throughput {
            lines.push("Order Throughput:".to_string());
            lines.push(format!(
                "  Submission: {:.2} ops/sec",
                results.submission.stats.ops_per_second
            ));
            lines.push(format!(
                "  Cancellation: {:.2} ops/sec",
                results.cancellation.stats.ops_per_second
            ));
            lines.push(String::new());
        }

        // Market data throughput summary
        if let Some(ref results) = self.market_data_throughput {
            lines.push("Market Data Throughput:".to_string());
            lines.push(format!(
                "  Tick: {:.2} ops/sec",
                results.tick.stats.ops_per_second
            ));
            lines.push(format!(
                "  Orderbook: {:.2} ops/sec",
                results.orderbook.stats.ops_per_second
            ));
            lines.push(String::new());
        }

        // Regressions
        let regressions: Vec<_> = self
            .comparisons
            .iter()
            .filter(|c| c.is_regression)
            .collect();
        if !regressions.is_empty() {
            lines.push("⚠️ REGRESSIONS DETECTED:".to_string());
            for regression in regressions {
                lines.push(format!(
                    "  {}: {:.2}% worse (current: {:.2}, baseline: {:.2})",
                    regression.metric,
                    regression.diff_percent,
                    regression.current,
                    regression.baseline
                ));
            }
            lines.push(String::new());
        }

        // Trends
        if !self.trends.is_empty() {
            lines.push("Trends:".to_string());
            for trend in &self.trends {
                let direction = match trend.direction {
                    TrendDirection::Improving => "↑ Improving",
                    TrendDirection::Stable => "→ Stable",
                    TrendDirection::Degrading => "↓ Degrading",
                };
                lines.push(format!(
                    "  {}: {} ({:+.2}%)",
                    trend.metric, direction, trend.change_percent
                ));
            }
        }

        lines.join("\n")
    }
}

/// Performance reporter for generating and managing reports.
pub struct PerformanceReporter {
    config: ReportConfig,
    history: Vec<PerformanceReport>,
    baseline: Option<PerformanceReport>,
}

impl PerformanceReporter {
    /// Create a new performance reporter.
    #[must_use]
    pub fn new(config: ReportConfig) -> Self {
        Self {
            config,
            history: Vec::new(),
            baseline: None,
        }
    }

    /// Set the baseline report for comparison.
    pub fn set_baseline(&mut self, baseline: PerformanceReport) {
        self.baseline = Some(baseline);
    }

    /// Generate a report with baseline comparison and trend analysis.
    #[must_use]
    pub fn generate_report(&mut self, mut report: PerformanceReport) -> PerformanceReport {
        // Add baseline comparisons
        if let Some(ref baseline) = self.baseline {
            self.add_comparisons(&mut report, baseline);
        }

        // Add trend analysis
        if !self.history.is_empty() {
            self.add_trends(&mut report);
        }

        // Generate summary
        report.set_summary(self.generate_summary(&report));

        // Add to history
        self.history.push(report.clone());
        if self.history.len() > self.config.history_size {
            self.history.remove(0);
        }

        report
    }

    /// Add baseline comparisons to a report.
    fn add_comparisons(&self, report: &mut PerformanceReport, baseline: &PerformanceReport) {
        // Compare order latency
        if let (Some(current), Some(base)) = (&report.order_latency, &baseline.order_latency) {
            let submission_current = current.submission.p99;
            let submission_base = base.submission.p99;
            let ack_current = current.acknowledgment.p99;
            let ack_base = base.acknowledgment.p99;
            let fill_current = current.fill.p99;
            let fill_base = base.fill.p99;

            self.compare_latency(
                report,
                "order_submission_p99",
                submission_current,
                submission_base,
            );
            self.compare_latency(report, "order_acknowledgment_p99", ack_current, ack_base);
            self.compare_latency(report, "order_fill_p99", fill_current, fill_base);
        }

        // Compare market data latency
        if let (Some(current), Some(base)) =
            (&report.market_data_latency, &baseline.market_data_latency)
        {
            let tick_current = current.tick.p99;
            let tick_base = base.tick.p99;
            let orderbook_current = current.orderbook.p99;
            let orderbook_base = base.orderbook.p99;

            self.compare_latency(report, "tick_processing_p99", tick_current, tick_base);
            self.compare_latency(
                report,
                "orderbook_processing_p99",
                orderbook_current,
                orderbook_base,
            );
        }

        // Compare order throughput
        if let (Some(current), Some(base)) = (&report.order_throughput, &baseline.order_throughput)
        {
            let submission_current = current.submission.stats.ops_per_second;
            let submission_base = base.submission.stats.ops_per_second;

            self.compare_throughput(
                report,
                "order_submission_throughput",
                submission_current,
                submission_base,
            );
        }

        // Compare market data throughput
        if let (Some(current), Some(base)) = (
            &report.market_data_throughput,
            &baseline.market_data_throughput,
        ) {
            let tick_current = current.tick.stats.ops_per_second;
            let tick_base = base.tick.stats.ops_per_second;

            self.compare_throughput(report, "tick_throughput", tick_current, tick_base);
        }
    }

    /// Compare latency values (lower is better).
    fn compare_latency(
        &self,
        report: &mut PerformanceReport,
        metric: &str,
        current: Duration,
        baseline: Duration,
    ) {
        let current_us = current.as_micros() as f64;
        let baseline_us = baseline.as_micros() as f64;

        if baseline_us > 0.0 {
            let diff_percent = ((current_us - baseline_us) / baseline_us) * 100.0;
            let is_regression = diff_percent > self.config.regression_threshold_percent;

            report.add_comparison(BaselineComparison {
                metric: metric.to_string(),
                current: current_us,
                baseline: baseline_us,
                diff_percent,
                is_regression,
            });
        }
    }

    /// Compare throughput values (higher is better).
    fn compare_throughput(
        &self,
        report: &mut PerformanceReport,
        metric: &str,
        current: f64,
        baseline: f64,
    ) {
        if baseline > 0.0 {
            // For throughput, negative diff means regression (lower is worse)
            let diff_percent = ((current - baseline) / baseline) * 100.0;
            let is_regression = diff_percent < -self.config.regression_threshold_percent;

            report.add_comparison(BaselineComparison {
                metric: metric.to_string(),
                current,
                baseline,
                diff_percent,
                is_regression,
            });
        }
    }

    /// Add trend analysis to a report.
    #[allow(clippy::cast_precision_loss)]
    fn add_trends(&self, report: &mut PerformanceReport) {
        // Analyze order submission latency trend
        let latencies: Vec<f64> = self
            .history
            .iter()
            .filter_map(|r| r.order_latency.as_ref())
            .map(|s| s.submission.p99.as_micros() as f64)
            .collect();

        if latencies.len() >= 3 {
            report.add_trend(self.analyze_trend("order_submission_p99", &latencies, true));
        }

        // Analyze tick processing latency trend
        let tick_latencies: Vec<f64> = self
            .history
            .iter()
            .filter_map(|r| r.market_data_latency.as_ref())
            .map(|s| s.tick.p99.as_micros() as f64)
            .collect();

        if tick_latencies.len() >= 3 {
            report.add_trend(self.analyze_trend("tick_processing_p99", &tick_latencies, true));
        }

        // Analyze order throughput trend
        let throughputs: Vec<f64> = self
            .history
            .iter()
            .filter_map(|r| r.order_throughput.as_ref())
            .map(|r| r.submission.stats.ops_per_second)
            .collect();

        if throughputs.len() >= 3 {
            report.add_trend(self.analyze_trend(
                "order_submission_throughput",
                &throughputs,
                false,
            ));
        }
    }

    /// Analyze trend for a metric.
    fn analyze_trend(&self, metric: &str, values: &[f64], lower_is_better: bool) -> TrendAnalysis {
        let n = values.len();
        if n < 2 {
            return TrendAnalysis {
                metric: metric.to_string(),
                direction: TrendDirection::Stable,
                change_percent: 0.0,
                sample_count: n,
                confidence: 0.0,
            };
        }

        // Calculate simple linear regression slope
        let (slope, r_squared) = linear_regression(values);

        // Calculate percentage change from first to last
        let first = values.first().copied().unwrap_or(0.0);
        let last = values.last().copied().unwrap_or(0.0);
        let change_percent = if first > 0.0 {
            ((last - first) / first) * 100.0
        } else {
            0.0
        };

        // Determine direction based on slope and whether lower is better
        let direction = if slope.abs() < 0.01 * first {
            TrendDirection::Stable
        } else if lower_is_better {
            if slope < 0.0 {
                TrendDirection::Improving
            } else {
                TrendDirection::Degrading
            }
        } else if slope > 0.0 {
            TrendDirection::Improving
        } else {
            TrendDirection::Degrading
        };

        TrendAnalysis {
            metric: metric.to_string(),
            direction,
            change_percent,
            sample_count: n,
            confidence: r_squared,
        }
    }

    /// Generate a summary for the report.
    fn generate_summary(&self, report: &PerformanceReport) -> String {
        let mut parts = Vec::new();

        if report.has_regressions() {
            let count = report.regressions().len();
            parts.push(format!("{count} regression(s) detected"));
        } else {
            parts.push("No regressions detected".to_string());
        }

        let degrading: Vec<_> = report
            .trends
            .iter()
            .filter(|t| t.direction == TrendDirection::Degrading)
            .collect();

        if !degrading.is_empty() {
            parts.push(format!(
                "{} metric(s) showing degrading trend",
                degrading.len()
            ));
        }

        parts.join(". ")
    }

    /// Get the report history.
    #[must_use]
    pub fn history(&self) -> &[PerformanceReport] {
        &self.history
    }

    /// Clear the report history.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }
}

/// Simple linear regression returning (slope, r_squared).
#[allow(clippy::cast_precision_loss)]
fn linear_regression(values: &[f64]) -> (f64, f64) {
    let n = values.len() as f64;
    if n < 2.0 {
        return (0.0, 0.0);
    }

    let x_mean = (n - 1.0) / 2.0;
    let y_mean: f64 = values.iter().sum::<f64>() / n;

    let mut ss_xy = 0.0;
    let mut ss_xx = 0.0;
    let mut ss_yy = 0.0;

    for (i, &y) in values.iter().enumerate() {
        let x = i as f64;
        ss_xy += (x - x_mean) * (y - y_mean);
        ss_xx += (x - x_mean) * (x - x_mean);
        ss_yy += (y - y_mean) * (y - y_mean);
    }

    let slope = if ss_xx > 0.0 { ss_xy / ss_xx } else { 0.0 };
    let r_squared = if ss_xx > 0.0 && ss_yy > 0.0 {
        (ss_xy * ss_xy) / (ss_xx * ss_yy)
    } else {
        0.0
    };

    (slope, r_squared)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_report_creation() {
        let report = PerformanceReport::new("test_report").with_duration(Duration::from_secs(60));

        assert_eq!(report.name, "test_report");
        assert_eq!(report.duration, Duration::from_secs(60));
        assert!(!report.has_regressions());
    }

    #[test]
    fn test_baseline_comparison() {
        let mut report = PerformanceReport::new("test");

        report.add_comparison(BaselineComparison {
            metric: "latency".to_string(),
            current: 100.0,
            baseline: 80.0,
            diff_percent: 25.0,
            is_regression: true,
        });

        assert!(report.has_regressions());
        assert_eq!(report.regressions().len(), 1);
    }

    #[test]
    fn test_trend_analysis() {
        let trend = TrendAnalysis {
            metric: "latency".to_string(),
            direction: TrendDirection::Degrading,
            change_percent: 15.0,
            sample_count: 10,
            confidence: 0.85,
        };

        assert_eq!(trend.direction, TrendDirection::Degrading);
        assert_eq!(trend.change_percent, 15.0);
    }

    #[test]
    fn test_linear_regression() {
        // Increasing values
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let (slope, r_squared) = linear_regression(&values);
        assert!((slope - 1.0).abs() < 0.01);
        assert!((r_squared - 1.0).abs() < 0.01);

        // Constant values
        let constant = vec![5.0, 5.0, 5.0, 5.0];
        let (slope, _) = linear_regression(&constant);
        assert!(slope.abs() < 0.01);
    }

    #[test]
    fn test_performance_reporter() {
        let config = ReportConfig::default();
        let mut reporter = PerformanceReporter::new(config);

        let report = PerformanceReport::new("test");
        let generated = reporter.generate_report(report);

        assert!(!generated.summary.is_empty());
        assert_eq!(reporter.history().len(), 1);
    }

    #[test]
    fn test_report_to_json() {
        let report = PerformanceReport::new("test").with_duration(Duration::from_secs(60));

        let json = report.to_json().unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("duration"));
    }

    #[test]
    fn test_report_summary() {
        let report = PerformanceReport::new("test").with_duration(Duration::from_secs(60));

        let summary = report.to_summary();
        assert!(summary.contains("Performance Report: test"));
    }
}
