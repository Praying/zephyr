//! Filter statistics and reporting.
//!
//! This module provides statistics collection and reporting for the filter system.

#![allow(clippy::disallowed_types)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::struct_field_names)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::info;

use zephyr_core::types::{Symbol, Timestamp};

use super::types::FilterId;

/// Type of filter event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FilterEventType {
    /// Signal passed the filter.
    Passed,
    /// Signal was rejected.
    Rejected {
        /// Rejection reason.
        reason: String,
    },
    /// Signal was modified.
    Modified {
        /// Modification description.
        description: String,
    },
}

impl FilterEventType {
    /// Returns true if this is a pass event.
    #[must_use]
    pub fn is_passed(&self) -> bool {
        matches!(self, Self::Passed)
    }

    /// Returns true if this is a rejection event.
    #[must_use]
    pub fn is_rejected(&self) -> bool {
        matches!(self, Self::Rejected { .. })
    }

    /// Returns true if this is a modification event.
    #[must_use]
    pub fn is_modified(&self) -> bool {
        matches!(self, Self::Modified { .. })
    }
}

/// A filter event for logging and statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterEvent {
    /// Filter that generated the event.
    pub filter_id: FilterId,
    /// Symbol being filtered.
    pub symbol: Symbol,
    /// Type of event.
    pub event_type: FilterEventType,
    /// Timestamp of the event.
    pub timestamp: Timestamp,
}

impl FilterEvent {
    /// Creates a new pass event.
    #[must_use]
    pub fn passed(filter_id: FilterId, symbol: Symbol, timestamp: Timestamp) -> Self {
        Self {
            filter_id,
            symbol,
            event_type: FilterEventType::Passed,
            timestamp,
        }
    }

    /// Creates a new rejection event.
    #[must_use]
    pub fn rejected(
        filter_id: FilterId,
        symbol: Symbol,
        reason: impl Into<String>,
        timestamp: Timestamp,
    ) -> Self {
        Self {
            filter_id,
            symbol,
            event_type: FilterEventType::Rejected {
                reason: reason.into(),
            },
            timestamp,
        }
    }

    /// Creates a new modification event.
    #[must_use]
    pub fn modified(
        filter_id: FilterId,
        symbol: Symbol,
        description: impl Into<String>,
        timestamp: Timestamp,
    ) -> Self {
        Self {
            filter_id,
            symbol,
            event_type: FilterEventType::Modified {
                description: description.into(),
            },
            timestamp,
        }
    }
}

/// Statistics for a single filter.
#[derive(Debug, Default)]
struct FilterStatEntry {
    /// Total signals processed.
    total: AtomicU64,
    /// Signals passed.
    passed: AtomicU64,
    /// Signals rejected.
    rejected: AtomicU64,
    /// Signals modified.
    modified: AtomicU64,
}

impl FilterStatEntry {
    fn record(&self, event_type: &FilterEventType) {
        self.total.fetch_add(1, Ordering::Relaxed);
        match event_type {
            FilterEventType::Passed => {
                self.passed.fetch_add(1, Ordering::Relaxed);
            }
            FilterEventType::Rejected { .. } => {
                self.rejected.fetch_add(1, Ordering::Relaxed);
            }
            FilterEventType::Modified { .. } => {
                self.modified.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn to_report(&self) -> FilterStatReport {
        let total = self.total.load(Ordering::Relaxed);
        let passed = self.passed.load(Ordering::Relaxed);
        let rejected = self.rejected.load(Ordering::Relaxed);
        let modified = self.modified.load(Ordering::Relaxed);

        let pass_rate = if total > 0 {
            (passed as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        let reject_rate = if total > 0 {
            (rejected as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        FilterStatReport {
            total,
            passed,
            rejected,
            modified,
            pass_rate,
            reject_rate,
        }
    }
}

/// Statistics report for a single filter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterStatReport {
    /// Total signals processed.
    pub total: u64,
    /// Signals passed.
    pub passed: u64,
    /// Signals rejected.
    pub rejected: u64,
    /// Signals modified.
    pub modified: u64,
    /// Pass rate as percentage.
    pub pass_rate: f64,
    /// Reject rate as percentage.
    pub reject_rate: f64,
}

/// Statistics collector for the filter system.
pub struct FilterStats {
    /// Per-filter statistics.
    filter_stats: RwLock<HashMap<FilterId, FilterStatEntry>>,
    /// Per-symbol statistics.
    symbol_stats: RwLock<HashMap<Symbol, FilterStatEntry>>,
    /// Recent events (for debugging).
    recent_events: RwLock<Vec<FilterEvent>>,
    /// Maximum recent events to keep.
    max_recent_events: usize,
    /// Global counters.
    global_total: AtomicU64,
    global_passed: AtomicU64,
    global_rejected: AtomicU64,
    global_modified: AtomicU64,
}

impl FilterStats {
    /// Creates a new statistics collector.
    #[must_use]
    pub fn new() -> Self {
        Self::with_max_events(1000)
    }

    /// Creates a new statistics collector with custom event limit.
    #[must_use]
    pub fn with_max_events(max_recent_events: usize) -> Self {
        Self {
            filter_stats: RwLock::new(HashMap::new()),
            symbol_stats: RwLock::new(HashMap::new()),
            recent_events: RwLock::new(Vec::new()),
            max_recent_events,
            global_total: AtomicU64::new(0),
            global_passed: AtomicU64::new(0),
            global_rejected: AtomicU64::new(0),
            global_modified: AtomicU64::new(0),
        }
    }

    /// Records a filter event.
    pub fn record_event(&self, event: FilterEvent) {
        // Update global counters
        self.global_total.fetch_add(1, Ordering::Relaxed);
        match &event.event_type {
            FilterEventType::Passed => {
                self.global_passed.fetch_add(1, Ordering::Relaxed);
            }
            FilterEventType::Rejected { .. } => {
                self.global_rejected.fetch_add(1, Ordering::Relaxed);
            }
            FilterEventType::Modified { .. } => {
                self.global_modified.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Update per-filter stats
        {
            let mut filter_stats = self.filter_stats.write();
            filter_stats
                .entry(event.filter_id.clone())
                .or_default()
                .record(&event.event_type);
        }

        // Update per-symbol stats
        {
            let mut symbol_stats = self.symbol_stats.write();
            symbol_stats
                .entry(event.symbol.clone())
                .or_default()
                .record(&event.event_type);
        }

        // Store recent event
        {
            let mut recent = self.recent_events.write();
            recent.push(event);
            if recent.len() > self.max_recent_events {
                recent.remove(0);
            }
        }
    }

    /// Gets statistics for a specific filter.
    #[must_use]
    pub fn get_filter_stats(&self, filter_id: &FilterId) -> Option<FilterStatReport> {
        self.filter_stats
            .read()
            .get(filter_id)
            .map(FilterStatEntry::to_report)
    }

    /// Gets statistics for a specific symbol.
    #[must_use]
    pub fn get_symbol_stats(&self, symbol: &Symbol) -> Option<FilterStatReport> {
        self.symbol_stats
            .read()
            .get(symbol)
            .map(FilterStatEntry::to_report)
    }

    /// Gets recent events.
    #[must_use]
    pub fn get_recent_events(&self, limit: usize) -> Vec<FilterEvent> {
        let recent = self.recent_events.read();
        let start = recent.len().saturating_sub(limit);
        recent[start..].to_vec()
    }

    /// Gets recent rejection events.
    #[must_use]
    pub fn get_recent_rejections(&self, limit: usize) -> Vec<FilterEvent> {
        let recent = self.recent_events.read();
        recent
            .iter()
            .filter(|e| e.event_type.is_rejected())
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    /// Generates a full statistics report.
    #[must_use]
    pub fn generate_report(&self) -> FilterStatsReport {
        let total = self.global_total.load(Ordering::Relaxed);
        let passed = self.global_passed.load(Ordering::Relaxed);
        let rejected = self.global_rejected.load(Ordering::Relaxed);
        let modified = self.global_modified.load(Ordering::Relaxed);

        let pass_rate = if total > 0 {
            (passed as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        let reject_rate = if total > 0 {
            (rejected as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        let filter_stats: HashMap<FilterId, FilterStatReport> = self
            .filter_stats
            .read()
            .iter()
            .map(|(k, v)| (k.clone(), v.to_report()))
            .collect();

        let symbol_stats: HashMap<Symbol, FilterStatReport> = self
            .symbol_stats
            .read()
            .iter()
            .map(|(k, v)| (k.clone(), v.to_report()))
            .collect();

        FilterStatsReport {
            total_signals: total,
            passed_signals: passed,
            rejected_signals: rejected,
            modified_signals: modified,
            pass_rate,
            reject_rate,
            filter_stats,
            symbol_stats,
            generated_at: Timestamp::now(),
        }
    }

    /// Logs a summary of the statistics.
    pub fn log_summary(&self) {
        let report = self.generate_report();
        info!(
            total = report.total_signals,
            passed = report.passed_signals,
            rejected = report.rejected_signals,
            modified = report.modified_signals,
            pass_rate = format!("{:.2}%", report.pass_rate),
            reject_rate = format!("{:.2}%", report.reject_rate),
            "Filter statistics summary"
        );
    }

    /// Resets all statistics.
    pub fn reset(&self) {
        self.filter_stats.write().clear();
        self.symbol_stats.write().clear();
        self.recent_events.write().clear();
        self.global_total.store(0, Ordering::Relaxed);
        self.global_passed.store(0, Ordering::Relaxed);
        self.global_rejected.store(0, Ordering::Relaxed);
        self.global_modified.store(0, Ordering::Relaxed);
    }
}

impl Default for FilterStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Full statistics report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterStatsReport {
    /// Total signals processed.
    pub total_signals: u64,
    /// Signals passed.
    pub passed_signals: u64,
    /// Signals rejected.
    pub rejected_signals: u64,
    /// Signals modified.
    pub modified_signals: u64,
    /// Pass rate as percentage.
    pub pass_rate: f64,
    /// Reject rate as percentage.
    pub reject_rate: f64,
    /// Per-filter statistics.
    pub filter_stats: HashMap<FilterId, FilterStatReport>,
    /// Per-symbol statistics.
    pub symbol_stats: HashMap<Symbol, FilterStatReport>,
    /// Report generation timestamp.
    pub generated_at: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(
        filter_id: &str,
        symbol: &str,
        event_type: FilterEventType,
    ) -> FilterEvent {
        FilterEvent {
            filter_id: FilterId::new(filter_id),
            symbol: Symbol::new(symbol).unwrap(),
            event_type,
            timestamp: Timestamp::now(),
        }
    }

    #[test]
    fn test_filter_event_type() {
        assert!(FilterEventType::Passed.is_passed());
        assert!(!FilterEventType::Passed.is_rejected());

        let rejected = FilterEventType::Rejected {
            reason: "test".to_string(),
        };
        assert!(rejected.is_rejected());
        assert!(!rejected.is_passed());

        let modified = FilterEventType::Modified {
            description: "test".to_string(),
        };
        assert!(modified.is_modified());
        assert!(!modified.is_passed());
    }

    #[test]
    fn test_filter_event_constructors() {
        let filter_id = FilterId::new("test");
        let symbol = Symbol::new("BTC-USDT").unwrap();
        let ts = Timestamp::now();

        let passed = FilterEvent::passed(filter_id.clone(), symbol.clone(), ts);
        assert!(passed.event_type.is_passed());

        let rejected = FilterEvent::rejected(filter_id.clone(), symbol.clone(), "reason", ts);
        assert!(rejected.event_type.is_rejected());

        let modified = FilterEvent::modified(filter_id, symbol, "description", ts);
        assert!(modified.event_type.is_modified());
    }

    #[test]
    fn test_filter_stats_new() {
        let stats = FilterStats::new();
        let report = stats.generate_report();
        assert_eq!(report.total_signals, 0);
        assert_eq!(report.passed_signals, 0);
    }

    #[test]
    fn test_filter_stats_record_event() {
        let stats = FilterStats::new();

        stats.record_event(create_test_event("f1", "BTC-USDT", FilterEventType::Passed));
        stats.record_event(create_test_event("f1", "BTC-USDT", FilterEventType::Passed));
        stats.record_event(create_test_event(
            "f1",
            "BTC-USDT",
            FilterEventType::Rejected {
                reason: "test".to_string(),
            },
        ));

        let report = stats.generate_report();
        assert_eq!(report.total_signals, 3);
        assert_eq!(report.passed_signals, 2);
        assert_eq!(report.rejected_signals, 1);
    }

    #[test]
    fn test_filter_stats_per_filter() {
        let stats = FilterStats::new();

        stats.record_event(create_test_event("f1", "BTC-USDT", FilterEventType::Passed));
        stats.record_event(create_test_event("f2", "BTC-USDT", FilterEventType::Passed));
        stats.record_event(create_test_event(
            "f1",
            "BTC-USDT",
            FilterEventType::Rejected {
                reason: "test".to_string(),
            },
        ));

        let f1_stats = stats.get_filter_stats(&FilterId::new("f1")).unwrap();
        assert_eq!(f1_stats.total, 2);
        assert_eq!(f1_stats.passed, 1);
        assert_eq!(f1_stats.rejected, 1);

        let f2_stats = stats.get_filter_stats(&FilterId::new("f2")).unwrap();
        assert_eq!(f2_stats.total, 1);
        assert_eq!(f2_stats.passed, 1);
    }

    #[test]
    fn test_filter_stats_per_symbol() {
        let stats = FilterStats::new();

        stats.record_event(create_test_event("f1", "BTC-USDT", FilterEventType::Passed));
        stats.record_event(create_test_event("f1", "ETH-USDT", FilterEventType::Passed));
        stats.record_event(create_test_event("f1", "BTC-USDT", FilterEventType::Passed));

        let btc_stats = stats
            .get_symbol_stats(&Symbol::new("BTC-USDT").unwrap())
            .unwrap();
        assert_eq!(btc_stats.total, 2);

        let eth_stats = stats
            .get_symbol_stats(&Symbol::new("ETH-USDT").unwrap())
            .unwrap();
        assert_eq!(eth_stats.total, 1);
    }

    #[test]
    fn test_filter_stats_recent_events() {
        let stats = FilterStats::with_max_events(5);

        for i in 0..10 {
            stats.record_event(create_test_event(
                &format!("f{i}"),
                "BTC-USDT",
                FilterEventType::Passed,
            ));
        }

        let recent = stats.get_recent_events(10);
        assert_eq!(recent.len(), 5); // Limited to max_recent_events
    }

    #[test]
    fn test_filter_stats_recent_rejections() {
        let stats = FilterStats::new();

        stats.record_event(create_test_event("f1", "BTC-USDT", FilterEventType::Passed));
        stats.record_event(create_test_event(
            "f2",
            "BTC-USDT",
            FilterEventType::Rejected {
                reason: "reason1".to_string(),
            },
        ));
        stats.record_event(create_test_event("f3", "BTC-USDT", FilterEventType::Passed));
        stats.record_event(create_test_event(
            "f4",
            "BTC-USDT",
            FilterEventType::Rejected {
                reason: "reason2".to_string(),
            },
        ));

        let rejections = stats.get_recent_rejections(10);
        assert_eq!(rejections.len(), 2);
    }

    #[test]
    fn test_filter_stats_reset() {
        let stats = FilterStats::new();

        stats.record_event(create_test_event("f1", "BTC-USDT", FilterEventType::Passed));
        stats.record_event(create_test_event("f1", "BTC-USDT", FilterEventType::Passed));

        let report = stats.generate_report();
        assert_eq!(report.total_signals, 2);

        stats.reset();

        let report = stats.generate_report();
        assert_eq!(report.total_signals, 0);
    }

    #[test]
    fn test_filter_stats_report_rates() {
        let stats = FilterStats::new();

        // 3 passed, 1 rejected = 75% pass rate, 25% reject rate
        stats.record_event(create_test_event("f1", "BTC-USDT", FilterEventType::Passed));
        stats.record_event(create_test_event("f1", "BTC-USDT", FilterEventType::Passed));
        stats.record_event(create_test_event("f1", "BTC-USDT", FilterEventType::Passed));
        stats.record_event(create_test_event(
            "f1",
            "BTC-USDT",
            FilterEventType::Rejected {
                reason: "test".to_string(),
            },
        ));

        let report = stats.generate_report();
        assert!((report.pass_rate - 75.0).abs() < 0.01);
        assert!((report.reject_rate - 25.0).abs() < 0.01);
    }
}
