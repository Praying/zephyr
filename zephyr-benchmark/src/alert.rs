//! Performance alerting with threshold monitoring.
//!
//! This module provides:
//! - Threshold-based performance monitoring
//! - Alert generation for performance degradation
//! - Configurable alert severity levels
//!
//! ## Features
//!
//! - Multiple alert types (latency, throughput, error rate)
//! - Configurable thresholds per metric
//! - Alert callbacks for integration with notification systems

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

/// Alert severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AlertSeverity {
    /// Informational alert.
    Info,
    /// Warning alert.
    Warning,
    /// Critical alert requiring immediate attention.
    Critical,
}

/// Types of performance alerts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    /// Latency exceeded threshold.
    LatencyThreshold {
        /// Metric name.
        metric: String,
        /// Threshold that was exceeded.
        threshold: Duration,
        /// Actual value.
        actual: Duration,
    },
    /// Throughput dropped below threshold.
    ThroughputThreshold {
        /// Metric name.
        metric: String,
        /// Minimum expected throughput.
        threshold: f64,
        /// Actual throughput.
        actual: f64,
    },
    /// Performance regression detected.
    Regression {
        /// Metric name.
        metric: String,
        /// Percentage degradation.
        degradation_percent: f64,
    },
    /// Trend degradation detected.
    TrendDegradation {
        /// Metric name.
        metric: String,
        /// Number of consecutive degrading samples.
        consecutive_samples: usize,
    },
    /// Custom alert.
    Custom {
        /// Alert name.
        name: String,
        /// Alert message.
        message: String,
    },
}

/// A performance alert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAlert {
    /// Unique alert ID.
    pub id: u64,
    /// Alert type.
    pub alert_type: AlertType,
    /// Alert severity.
    pub severity: AlertSeverity,
    /// Alert timestamp.
    pub timestamp: DateTime<Utc>,
    /// Alert message.
    pub message: String,
    /// Additional context.
    pub context: HashMap<String, String>,
    /// Whether the alert has been acknowledged.
    pub acknowledged: bool,
}

impl PerformanceAlert {
    /// Create a new performance alert.
    #[must_use]
    pub fn new(
        id: u64,
        alert_type: AlertType,
        severity: AlertSeverity,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id,
            alert_type,
            severity,
            timestamp: Utc::now(),
            message: message.into(),
            context: HashMap::new(),
            acknowledged: false,
        }
    }

    /// Add context to the alert.
    #[must_use]
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    /// Acknowledge the alert.
    pub fn acknowledge(&mut self) {
        self.acknowledged = true;
    }
}

/// Configuration for a latency threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyThresholdConfig {
    /// Warning threshold.
    pub warning: Duration,
    /// Critical threshold.
    pub critical: Duration,
}

/// Configuration for a throughput threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputThresholdConfig {
    /// Warning threshold (minimum acceptable).
    pub warning: f64,
    /// Critical threshold (minimum acceptable).
    pub critical: f64,
}

/// Configuration for threshold monitoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdConfig {
    /// Latency thresholds by metric name.
    pub latency: HashMap<String, LatencyThresholdConfig>,
    /// Throughput thresholds by metric name.
    pub throughput: HashMap<String, ThroughputThresholdConfig>,
    /// Regression threshold percentage.
    pub regression_threshold_percent: f64,
    /// Number of consecutive degrading samples to trigger trend alert.
    pub trend_degradation_samples: usize,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        let mut latency = HashMap::new();
        latency.insert(
            "order_submission_p99".to_string(),
            LatencyThresholdConfig {
                warning: Duration::from_millis(100),
                critical: Duration::from_millis(500),
            },
        );
        latency.insert(
            "tick_processing_p99".to_string(),
            LatencyThresholdConfig {
                warning: Duration::from_micros(500),
                critical: Duration::from_millis(5),
            },
        );

        let mut throughput = HashMap::new();
        throughput.insert(
            "order_submission".to_string(),
            ThroughputThresholdConfig {
                warning: 100.0,
                critical: 50.0,
            },
        );
        throughput.insert(
            "tick_processing".to_string(),
            ThroughputThresholdConfig {
                warning: 10000.0,
                critical: 5000.0,
            },
        );

        Self {
            latency,
            throughput,
            regression_threshold_percent: 20.0,
            trend_degradation_samples: 5,
        }
    }
}

/// Alert configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    /// Whether alerting is enabled.
    pub enabled: bool,
    /// Threshold configuration.
    pub thresholds: ThresholdConfig,
    /// Maximum number of alerts to retain.
    pub max_alerts: usize,
    /// Cooldown period between duplicate alerts.
    pub cooldown: Duration,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            thresholds: ThresholdConfig::default(),
            max_alerts: 1000,
            cooldown: Duration::from_secs(60),
        }
    }
}

/// Callback for alert notifications.
pub trait AlertCallback: Send + Sync {
    /// Called when an alert is triggered.
    fn on_alert(&self, alert: &PerformanceAlert);
}

/// Performance monitor for threshold-based alerting.
pub struct PerformanceMonitor {
    config: AlertConfig,
    alerts: RwLock<Vec<PerformanceAlert>>,
    alert_counter: AtomicU64,
    last_alert_times: RwLock<HashMap<String, DateTime<Utc>>>,
    callbacks: RwLock<Vec<Box<dyn AlertCallback>>>,
}

impl PerformanceMonitor {
    /// Create a new performance monitor.
    #[must_use]
    pub fn new(config: AlertConfig) -> Self {
        Self {
            config,
            alerts: RwLock::new(Vec::new()),
            alert_counter: AtomicU64::new(0),
            last_alert_times: RwLock::new(HashMap::new()),
            callbacks: RwLock::new(Vec::new()),
        }
    }

    /// Register an alert callback.
    pub fn register_callback(&self, callback: Box<dyn AlertCallback>) {
        self.callbacks.write().push(callback);
    }

    /// Check latency against thresholds.
    pub fn check_latency(&self, metric: &str, value: Duration) {
        if !self.config.enabled {
            return;
        }

        if let Some(threshold) = self.config.thresholds.latency.get(metric) {
            if value >= threshold.critical {
                self.trigger_alert(
                    AlertType::LatencyThreshold {
                        metric: metric.to_string(),
                        threshold: threshold.critical,
                        actual: value,
                    },
                    AlertSeverity::Critical,
                    format!(
                        "Critical latency threshold exceeded for {}: {:?} >= {:?}",
                        metric, value, threshold.critical
                    ),
                );
            } else if value >= threshold.warning {
                self.trigger_alert(
                    AlertType::LatencyThreshold {
                        metric: metric.to_string(),
                        threshold: threshold.warning,
                        actual: value,
                    },
                    AlertSeverity::Warning,
                    format!(
                        "Warning latency threshold exceeded for {}: {:?} >= {:?}",
                        metric, value, threshold.warning
                    ),
                );
            }
        }
    }

    /// Check throughput against thresholds.
    pub fn check_throughput(&self, metric: &str, value: f64) {
        if !self.config.enabled {
            return;
        }

        if let Some(threshold) = self.config.thresholds.throughput.get(metric) {
            if value <= threshold.critical {
                self.trigger_alert(
                    AlertType::ThroughputThreshold {
                        metric: metric.to_string(),
                        threshold: threshold.critical,
                        actual: value,
                    },
                    AlertSeverity::Critical,
                    format!(
                        "Critical throughput threshold breached for {}: {:.2} <= {:.2}",
                        metric, value, threshold.critical
                    ),
                );
            } else if value <= threshold.warning {
                self.trigger_alert(
                    AlertType::ThroughputThreshold {
                        metric: metric.to_string(),
                        threshold: threshold.warning,
                        actual: value,
                    },
                    AlertSeverity::Warning,
                    format!(
                        "Warning throughput threshold breached for {}: {:.2} <= {:.2}",
                        metric, value, threshold.warning
                    ),
                );
            }
        }
    }

    /// Check for regression.
    pub fn check_regression(&self, metric: &str, degradation_percent: f64) {
        if !self.config.enabled {
            return;
        }

        if degradation_percent >= self.config.thresholds.regression_threshold_percent {
            let severity = if degradation_percent
                >= self.config.thresholds.regression_threshold_percent * 2.0
            {
                AlertSeverity::Critical
            } else {
                AlertSeverity::Warning
            };

            self.trigger_alert(
                AlertType::Regression {
                    metric: metric.to_string(),
                    degradation_percent,
                },
                severity,
                format!(
                    "Performance regression detected for {}: {:.2}% degradation",
                    metric, degradation_percent
                ),
            );
        }
    }

    /// Check for trend degradation.
    pub fn check_trend(&self, metric: &str, consecutive_degrading: usize) {
        if !self.config.enabled {
            return;
        }

        if consecutive_degrading >= self.config.thresholds.trend_degradation_samples {
            let severity =
                if consecutive_degrading >= self.config.thresholds.trend_degradation_samples * 2 {
                    AlertSeverity::Critical
                } else {
                    AlertSeverity::Warning
                };

            self.trigger_alert(
                AlertType::TrendDegradation {
                    metric: metric.to_string(),
                    consecutive_samples: consecutive_degrading,
                },
                severity,
                format!(
                    "Trend degradation detected for {}: {} consecutive degrading samples",
                    metric, consecutive_degrading
                ),
            );
        }
    }

    /// Trigger a custom alert.
    pub fn trigger_custom_alert(&self, name: &str, message: &str, severity: AlertSeverity) {
        if !self.config.enabled {
            return;
        }

        self.trigger_alert(
            AlertType::Custom {
                name: name.to_string(),
                message: message.to_string(),
            },
            severity,
            message.to_string(),
        );
    }

    /// Trigger an alert.
    fn trigger_alert(&self, alert_type: AlertType, severity: AlertSeverity, message: String) {
        // Check cooldown
        let alert_key = format!("{:?}", alert_type);
        let now = Utc::now();

        {
            let last_times = self.last_alert_times.read();
            if let Some(last_time) = last_times.get(&alert_key) {
                let elapsed = now.signed_duration_since(*last_time);
                if elapsed < chrono::Duration::from_std(self.config.cooldown).unwrap_or_default() {
                    return; // Still in cooldown
                }
            }
        }

        // Update last alert time
        self.last_alert_times.write().insert(alert_key, now);

        // Create alert
        let id = self.alert_counter.fetch_add(1, Ordering::Relaxed);
        let alert = PerformanceAlert::new(id, alert_type, severity, message);

        // Store alert
        {
            let mut alerts = self.alerts.write();
            alerts.push(alert.clone());
            if alerts.len() > self.config.max_alerts {
                alerts.remove(0);
            }
        }

        // Notify callbacks
        let callbacks = self.callbacks.read();
        for callback in callbacks.iter() {
            callback.on_alert(&alert);
        }
    }

    /// Get all alerts.
    #[must_use]
    pub fn alerts(&self) -> Vec<PerformanceAlert> {
        self.alerts.read().clone()
    }

    /// Get unacknowledged alerts.
    #[must_use]
    pub fn unacknowledged_alerts(&self) -> Vec<PerformanceAlert> {
        self.alerts
            .read()
            .iter()
            .filter(|a| !a.acknowledged)
            .cloned()
            .collect()
    }

    /// Get alerts by severity.
    #[must_use]
    pub fn alerts_by_severity(&self, severity: AlertSeverity) -> Vec<PerformanceAlert> {
        self.alerts
            .read()
            .iter()
            .filter(|a| a.severity == severity)
            .cloned()
            .collect()
    }

    /// Acknowledge an alert by ID.
    pub fn acknowledge_alert(&self, id: u64) -> bool {
        let mut alerts = self.alerts.write();
        if let Some(alert) = alerts.iter_mut().find(|a| a.id == id) {
            alert.acknowledge();
            true
        } else {
            false
        }
    }

    /// Acknowledge all alerts.
    pub fn acknowledge_all(&self) {
        let mut alerts = self.alerts.write();
        for alert in alerts.iter_mut() {
            alert.acknowledge();
        }
    }

    /// Clear all alerts.
    pub fn clear_alerts(&self) {
        self.alerts.write().clear();
    }

    /// Get alert count.
    #[must_use]
    pub fn alert_count(&self) -> usize {
        self.alerts.read().len()
    }

    /// Get critical alert count.
    #[must_use]
    pub fn critical_alert_count(&self) -> usize {
        self.alerts
            .read()
            .iter()
            .filter(|a| a.severity == AlertSeverity::Critical)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    struct TestCallback {
        count: AtomicUsize,
    }

    impl AlertCallback for TestCallback {
        fn on_alert(&self, _alert: &PerformanceAlert) {
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn test_performance_alert_creation() {
        let alert = PerformanceAlert::new(
            1,
            AlertType::Custom {
                name: "test".to_string(),
                message: "test message".to_string(),
            },
            AlertSeverity::Warning,
            "Test alert",
        );

        assert_eq!(alert.id, 1);
        assert_eq!(alert.severity, AlertSeverity::Warning);
        assert!(!alert.acknowledged);
    }

    #[test]
    fn test_alert_acknowledge() {
        let mut alert = PerformanceAlert::new(
            1,
            AlertType::Custom {
                name: "test".to_string(),
                message: "test".to_string(),
            },
            AlertSeverity::Info,
            "Test",
        );

        assert!(!alert.acknowledged);
        alert.acknowledge();
        assert!(alert.acknowledged);
    }

    #[test]
    fn test_latency_threshold_check() {
        let config = AlertConfig {
            cooldown: Duration::from_millis(1), // Short cooldown for testing
            ..Default::default()
        };
        let monitor = PerformanceMonitor::new(config);

        // Should trigger warning
        monitor.check_latency("order_submission_p99", Duration::from_millis(200));
        assert_eq!(monitor.alert_count(), 1);

        // Wait for cooldown
        std::thread::sleep(Duration::from_millis(5));

        // Should trigger critical
        monitor.check_latency("order_submission_p99", Duration::from_millis(600));
        assert_eq!(monitor.alert_count(), 2);
    }

    #[test]
    fn test_throughput_threshold_check() {
        let config = AlertConfig {
            cooldown: Duration::from_millis(1),
            ..Default::default()
        };
        let monitor = PerformanceMonitor::new(config);

        // Should trigger warning (below 100)
        monitor.check_throughput("order_submission", 80.0);
        assert_eq!(monitor.alert_count(), 1);

        std::thread::sleep(Duration::from_millis(5));

        // Should trigger critical (below 50)
        monitor.check_throughput("order_submission", 40.0);
        assert_eq!(monitor.alert_count(), 2);
    }

    #[test]
    fn test_regression_check() {
        let config = AlertConfig {
            cooldown: Duration::from_millis(1),
            ..Default::default()
        };
        let monitor = PerformanceMonitor::new(config);

        // Should trigger warning (>= 20%)
        monitor.check_regression("latency", 25.0);
        assert_eq!(monitor.alert_count(), 1);

        std::thread::sleep(Duration::from_millis(5));

        // Should trigger critical (>= 40%)
        monitor.check_regression("latency", 45.0);
        assert_eq!(monitor.alert_count(), 2);
    }

    #[test]
    fn test_alert_callback() {
        let config = AlertConfig {
            cooldown: Duration::from_millis(1),
            ..Default::default()
        };
        let monitor = PerformanceMonitor::new(config);

        let callback = Arc::new(TestCallback {
            count: AtomicUsize::new(0),
        });
        let callback_clone = Arc::clone(&callback);

        // Create a wrapper that implements AlertCallback
        struct CallbackWrapper(Arc<TestCallback>);
        impl AlertCallback for CallbackWrapper {
            fn on_alert(&self, alert: &PerformanceAlert) {
                self.0.on_alert(alert);
            }
        }

        monitor.register_callback(Box::new(CallbackWrapper(callback_clone)));

        monitor.trigger_custom_alert("test", "test message", AlertSeverity::Info);

        assert_eq!(callback.count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_cooldown() {
        let config = AlertConfig {
            cooldown: Duration::from_secs(60), // Long cooldown
            ..Default::default()
        };
        let monitor = PerformanceMonitor::new(config);

        // First alert should trigger
        monitor.check_latency("order_submission_p99", Duration::from_millis(600));
        assert_eq!(monitor.alert_count(), 1);

        // Second alert should be suppressed (cooldown)
        monitor.check_latency("order_submission_p99", Duration::from_millis(600));
        assert_eq!(monitor.alert_count(), 1);
    }

    #[test]
    fn test_alerts_by_severity() {
        let config = AlertConfig {
            cooldown: Duration::from_millis(1),
            ..Default::default()
        };
        let monitor = PerformanceMonitor::new(config);

        monitor.trigger_custom_alert("info", "info", AlertSeverity::Info);
        std::thread::sleep(Duration::from_millis(5));
        monitor.trigger_custom_alert("warning", "warning", AlertSeverity::Warning);
        std::thread::sleep(Duration::from_millis(5));
        monitor.trigger_custom_alert("critical", "critical", AlertSeverity::Critical);

        assert_eq!(monitor.alerts_by_severity(AlertSeverity::Info).len(), 1);
        assert_eq!(monitor.alerts_by_severity(AlertSeverity::Warning).len(), 1);
        assert_eq!(monitor.alerts_by_severity(AlertSeverity::Critical).len(), 1);
    }

    #[test]
    fn test_acknowledge_alert() {
        let config = AlertConfig::default();
        let monitor = PerformanceMonitor::new(config);

        monitor.trigger_custom_alert("test", "test", AlertSeverity::Info);

        let alerts = monitor.alerts();
        assert!(!alerts[0].acknowledged);

        monitor.acknowledge_alert(alerts[0].id);

        let alerts = monitor.alerts();
        assert!(alerts[0].acknowledged);
    }

    #[test]
    fn test_unacknowledged_alerts() {
        let config = AlertConfig {
            cooldown: Duration::from_millis(1),
            ..Default::default()
        };
        let monitor = PerformanceMonitor::new(config);

        monitor.trigger_custom_alert("test1", "test1", AlertSeverity::Info);
        std::thread::sleep(Duration::from_millis(5));
        monitor.trigger_custom_alert("test2", "test2", AlertSeverity::Info);

        assert_eq!(monitor.unacknowledged_alerts().len(), 2);

        let alerts = monitor.alerts();
        monitor.acknowledge_alert(alerts[0].id);

        assert_eq!(monitor.unacknowledged_alerts().len(), 1);
    }

    #[test]
    fn test_clear_alerts() {
        let config = AlertConfig::default();
        let monitor = PerformanceMonitor::new(config);

        monitor.trigger_custom_alert("test", "test", AlertSeverity::Info);
        assert_eq!(monitor.alert_count(), 1);

        monitor.clear_alerts();
        assert_eq!(monitor.alert_count(), 0);
    }
}
