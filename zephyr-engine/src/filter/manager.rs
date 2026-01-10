//! Filter manager for executing filter chains.
//!
//! This module provides the `FilterManager` which orchestrates the execution
//! of multiple filters in priority order.

#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_possible_truncation)]

use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::ExecutionSignal;

use super::stats::{FilterEvent, FilterEventType, FilterStats};
use super::types::{
    FilterContext, FilterId, FilterModification, FilterRejection, FilterResult, SignalFilter,
};

/// Configuration for the filter manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterManagerConfig {
    /// Whether filtering is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Whether to continue processing after a filter modifies a signal.
    #[serde(default = "default_true")]
    pub continue_on_modify: bool,
    /// Whether to log all filter decisions.
    #[serde(default)]
    pub log_all_decisions: bool,
    /// Maximum number of modifications allowed per signal.
    #[serde(default = "default_max_modifications")]
    pub max_modifications: usize,
    /// Whether to collect statistics.
    #[serde(default = "default_true")]
    pub collect_stats: bool,
}

fn default_true() -> bool {
    true
}

fn default_max_modifications() -> usize {
    10
}

impl Default for FilterManagerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            continue_on_modify: true,
            log_all_decisions: false,
            max_modifications: 10,
            collect_stats: true,
        }
    }
}

/// Result of processing a signal through the filter chain.
#[derive(Debug, Clone)]
pub struct FilterChainResult {
    /// Final result after all filters.
    pub result: FilterResult,
    /// List of modifications applied (if any).
    pub modifications: Vec<FilterModification>,
    /// Number of filters that passed the signal.
    pub filters_passed: usize,
    /// Total number of filters executed.
    pub filters_executed: usize,
    /// Processing time in microseconds.
    pub processing_time_us: u64,
}

impl FilterChainResult {
    /// Returns true if the signal was ultimately passed.
    #[must_use]
    pub fn is_passed(&self) -> bool {
        matches!(
            self.result,
            FilterResult::Pass | FilterResult::Modify { .. }
        )
    }

    /// Returns true if the signal was rejected.
    #[must_use]
    pub fn is_rejected(&self) -> bool {
        matches!(self.result, FilterResult::Reject(_))
    }

    /// Returns true if the signal was modified.
    #[must_use]
    pub fn was_modified(&self) -> bool {
        !self.modifications.is_empty()
    }

    /// Gets the final signal (original or modified).
    #[must_use]
    pub fn final_signal(&self) -> Option<&ExecutionSignal> {
        match &self.result {
            FilterResult::Modify { signal, .. } => Some(signal),
            _ => None,
        }
    }

    /// Gets the rejection reason if rejected.
    #[must_use]
    pub fn rejection(&self) -> Option<&FilterRejection> {
        match &self.result {
            FilterResult::Reject(r) => Some(r),
            _ => None,
        }
    }
}

/// Filter manager that executes filters in priority order.
///
/// The filter manager maintains a list of filters sorted by priority
/// and executes them sequentially on each signal.
pub struct FilterManager {
    /// Configuration.
    config: FilterManagerConfig,
    /// Registered filters (sorted by priority).
    filters: RwLock<Vec<Arc<dyn SignalFilter>>>,
    /// Statistics collector.
    stats: Arc<FilterStats>,
}

impl FilterManager {
    /// Creates a new filter manager with default configuration.
    #[must_use]
    pub fn new(config: FilterManagerConfig) -> Self {
        Self {
            config,
            filters: RwLock::new(Vec::new()),
            stats: Arc::new(FilterStats::new()),
        }
    }

    /// Adds a filter to the manager.
    ///
    /// Filters are automatically sorted by priority after adding.
    pub fn add_filter(&self, filter: Arc<dyn SignalFilter>) {
        let mut filters = self.filters.write();
        filters.push(filter);
        filters.sort_by_key(|f| f.priority());

        info!(filter_count = filters.len(), "Filter added to manager");
    }

    /// Removes a filter by ID.
    ///
    /// Returns true if a filter was removed.
    pub fn remove_filter(&self, filter_id: &FilterId) -> bool {
        let mut filters = self.filters.write();
        let initial_len = filters.len();
        filters.retain(|f| f.id() != filter_id);
        let removed = filters.len() < initial_len;

        if removed {
            info!(filter_id = %filter_id, "Filter removed from manager");
        }

        removed
    }

    /// Returns the number of registered filters.
    #[must_use]
    pub fn filter_count(&self) -> usize {
        self.filters.read().len()
    }

    /// Returns the IDs of all registered filters.
    #[must_use]
    pub fn filter_ids(&self) -> Vec<FilterId> {
        self.filters.read().iter().map(|f| f.id().clone()).collect()
    }

    /// Processes a signal through all filters.
    ///
    /// Returns the final result after all filters have been applied.
    pub fn process(&self, signal: &ExecutionSignal, context: &FilterContext) -> FilterChainResult {
        let start = std::time::Instant::now();

        if !self.config.enabled {
            return FilterChainResult {
                result: FilterResult::Pass,
                modifications: Vec::new(),
                filters_passed: 0,
                filters_executed: 0,
                processing_time_us: 0,
            };
        }

        let filters = self.filters.read();
        let mut current_signal = signal.clone();
        let mut modifications = Vec::new();
        let mut filters_passed = 0;
        let mut filters_executed = 0;

        for filter in filters.iter() {
            if !filter.is_enabled() {
                continue;
            }

            filters_executed += 1;
            let filter_id = filter.id().clone();
            let filter_name = filter.name().to_string();

            debug!(
                filter_id = %filter_id,
                filter_name = %filter_name,
                symbol = %current_signal.symbol,
                "Executing filter"
            );

            let result = filter.filter(&current_signal, context);

            // Record stats
            if self.config.collect_stats {
                self.record_filter_event(&filter_id, &current_signal, &result);
            }

            match result {
                FilterResult::Pass => {
                    filters_passed += 1;
                    if self.config.log_all_decisions {
                        debug!(
                            filter_id = %filter_id,
                            "Signal passed filter"
                        );
                    }
                }
                FilterResult::Reject(rejection) => {
                    warn!(
                        filter_id = %filter_id,
                        reason = %rejection.reason,
                        symbol = %current_signal.symbol,
                        "Signal rejected by filter"
                    );

                    let elapsed = start.elapsed();
                    return FilterChainResult {
                        result: FilterResult::Reject(rejection),
                        modifications,
                        filters_passed,
                        filters_executed,
                        processing_time_us: elapsed.as_micros() as u64,
                    };
                }
                FilterResult::Modify {
                    signal: modified_signal,
                    modification,
                } => {
                    info!(
                        filter_id = %filter_id,
                        description = %modification.description,
                        symbol = %current_signal.symbol,
                        "Signal modified by filter"
                    );

                    modifications.push(modification);
                    current_signal = modified_signal;
                    filters_passed += 1;

                    // Check modification limit
                    if modifications.len() >= self.config.max_modifications {
                        warn!(
                            max = self.config.max_modifications,
                            "Maximum modifications reached, stopping filter chain"
                        );
                        break;
                    }

                    if !self.config.continue_on_modify {
                        break;
                    }
                }
            }
        }

        let elapsed = start.elapsed();
        let final_result = if modifications.is_empty() {
            FilterResult::Pass
        } else {
            // Return the last modification
            let last_mod = modifications.last().cloned().unwrap();
            FilterResult::Modify {
                signal: current_signal,
                modification: last_mod,
            }
        };

        FilterChainResult {
            result: final_result,
            modifications,
            filters_passed,
            filters_executed,
            processing_time_us: elapsed.as_micros() as u64,
        }
    }

    /// Records a filter event for statistics.
    fn record_filter_event(
        &self,
        filter_id: &FilterId,
        signal: &ExecutionSignal,
        result: &FilterResult,
    ) {
        let event_type = match result {
            FilterResult::Pass => FilterEventType::Passed,
            FilterResult::Reject(r) => FilterEventType::Rejected {
                reason: r.reason.clone(),
            },
            FilterResult::Modify { modification, .. } => FilterEventType::Modified {
                description: modification.description.clone(),
            },
        };

        let event = FilterEvent {
            filter_id: filter_id.clone(),
            symbol: signal.symbol.clone(),
            event_type,
            timestamp: signal.timestamp,
        };

        self.stats.record_event(event);
    }

    /// Returns the statistics collector.
    #[must_use]
    pub fn stats(&self) -> Arc<FilterStats> {
        Arc::clone(&self.stats)
    }

    /// Clears all filters.
    pub fn clear(&self) {
        self.filters.write().clear();
        info!("All filters cleared");
    }

    /// Returns the configuration.
    #[must_use]
    pub fn config(&self) -> &FilterManagerConfig {
        &self.config
    }

    /// Updates the configuration.
    pub fn set_config(&mut self, config: FilterManagerConfig) {
        self.config = config;
    }
}

impl Default for FilterManager {
    fn default() -> Self {
        Self::new(FilterManagerConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portfolio::StrategyId;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;
    use zephyr_core::data::OrderSide;
    use zephyr_core::types::{Amount, Quantity, Symbol, Timestamp};

    /// Test filter that always passes.
    struct PassFilter {
        id: FilterId,
    }

    impl SignalFilter for PassFilter {
        fn id(&self) -> &FilterId {
            &self.id
        }

        fn name(&self) -> &str {
            "pass-filter"
        }

        fn filter(&self, _signal: &ExecutionSignal, _context: &FilterContext) -> FilterResult {
            FilterResult::Pass
        }
    }

    /// Test filter that always rejects.
    struct RejectFilter {
        id: FilterId,
        reason: String,
    }

    impl SignalFilter for RejectFilter {
        fn id(&self) -> &FilterId {
            &self.id
        }

        fn name(&self) -> &str {
            "reject-filter"
        }

        fn filter(&self, _signal: &ExecutionSignal, _context: &FilterContext) -> FilterResult {
            FilterResult::reject(self.id.clone(), &self.reason)
        }
    }

    fn create_test_signal() -> ExecutionSignal {
        ExecutionSignal {
            symbol: Symbol::new("BTC-USDT").unwrap(),
            current_position: Quantity::ZERO,
            target_position: Quantity::new(dec!(1.0)).unwrap(),
            order_quantity: Quantity::new(dec!(1.0)).unwrap(),
            order_side: OrderSide::Buy,
            strategy_contributions: HashMap::new(),
            timestamp: Timestamp::now(),
            tag: "test".to_string(),
        }
    }

    fn create_test_context() -> FilterContext {
        FilterContext::builder()
            .account_balance(Amount::new(dec!(10000)).unwrap())
            .build()
    }

    #[test]
    fn test_filter_manager_new() {
        let manager = FilterManager::new(FilterManagerConfig::default());
        assert_eq!(manager.filter_count(), 0);
        assert!(manager.config().enabled);
    }

    #[test]
    fn test_filter_manager_add_filter() {
        let manager = FilterManager::new(FilterManagerConfig::default());

        manager.add_filter(Arc::new(PassFilter {
            id: FilterId::new("pass-1"),
        }));

        assert_eq!(manager.filter_count(), 1);
        assert_eq!(manager.filter_ids(), vec![FilterId::new("pass-1")]);
    }

    #[test]
    fn test_filter_manager_remove_filter() {
        let manager = FilterManager::new(FilterManagerConfig::default());

        manager.add_filter(Arc::new(PassFilter {
            id: FilterId::new("pass-1"),
        }));
        manager.add_filter(Arc::new(PassFilter {
            id: FilterId::new("pass-2"),
        }));

        assert_eq!(manager.filter_count(), 2);

        let removed = manager.remove_filter(&FilterId::new("pass-1"));
        assert!(removed);
        assert_eq!(manager.filter_count(), 1);

        let not_removed = manager.remove_filter(&FilterId::new("nonexistent"));
        assert!(!not_removed);
    }

    #[test]
    fn test_filter_manager_process_pass() {
        let manager = FilterManager::new(FilterManagerConfig::default());

        manager.add_filter(Arc::new(PassFilter {
            id: FilterId::new("pass-1"),
        }));
        manager.add_filter(Arc::new(PassFilter {
            id: FilterId::new("pass-2"),
        }));

        let signal = create_test_signal();
        let context = create_test_context();

        let result = manager.process(&signal, &context);

        assert!(result.is_passed());
        assert!(!result.is_rejected());
        assert!(!result.was_modified());
        assert_eq!(result.filters_executed, 2);
        assert_eq!(result.filters_passed, 2);
    }

    #[test]
    fn test_filter_manager_process_reject() {
        let manager = FilterManager::new(FilterManagerConfig::default());

        manager.add_filter(Arc::new(PassFilter {
            id: FilterId::new("pass-1"),
        }));
        manager.add_filter(Arc::new(RejectFilter {
            id: FilterId::new("reject-1"),
            reason: "Test rejection".to_string(),
        }));
        manager.add_filter(Arc::new(PassFilter {
            id: FilterId::new("pass-2"),
        }));

        let signal = create_test_signal();
        let context = create_test_context();

        let result = manager.process(&signal, &context);

        assert!(!result.is_passed());
        assert!(result.is_rejected());
        assert_eq!(result.filters_executed, 2); // Stopped at reject
        assert_eq!(result.filters_passed, 1);

        let rejection = result.rejection().unwrap();
        assert_eq!(rejection.reason, "Test rejection");
    }

    #[test]
    fn test_filter_manager_disabled() {
        let config = FilterManagerConfig {
            enabled: false,
            ..Default::default()
        };
        let manager = FilterManager::new(config);

        manager.add_filter(Arc::new(RejectFilter {
            id: FilterId::new("reject-1"),
            reason: "Should not reject".to_string(),
        }));

        let signal = create_test_signal();
        let context = create_test_context();

        let result = manager.process(&signal, &context);

        // Should pass because filtering is disabled
        assert!(result.is_passed());
        assert_eq!(result.filters_executed, 0);
    }

    #[test]
    fn test_filter_manager_clear() {
        let manager = FilterManager::new(FilterManagerConfig::default());

        manager.add_filter(Arc::new(PassFilter {
            id: FilterId::new("pass-1"),
        }));
        manager.add_filter(Arc::new(PassFilter {
            id: FilterId::new("pass-2"),
        }));

        assert_eq!(manager.filter_count(), 2);

        manager.clear();

        assert_eq!(manager.filter_count(), 0);
    }

    #[test]
    fn test_filter_chain_result() {
        let result = FilterChainResult {
            result: FilterResult::Pass,
            modifications: Vec::new(),
            filters_passed: 3,
            filters_executed: 3,
            processing_time_us: 100,
        };

        assert!(result.is_passed());
        assert!(!result.is_rejected());
        assert!(!result.was_modified());
        assert!(result.final_signal().is_none());
        assert!(result.rejection().is_none());
    }
}
