//! Signal filtering module.
//!
//! This module provides signal filtering functionality for the trading engine.
//! Filters can be used to validate, modify, or reject execution signals before
//! they are sent to the execution layer.
//!
//! # Architecture
//!
//! The filter system uses a chain-of-responsibility pattern where signals
//! pass through multiple filters in priority order. Each filter can:
//! - Pass the signal unchanged
//! - Modify the signal
//! - Reject the signal with a reason
//!
//! # Built-in Filters
//!
//! - [`PositionLimitFilter`] - Enforces position limits per symbol
//! - [`FrequencyFilter`] - Limits order frequency
//! - [`PriceDeviationFilter`] - Rejects signals with excessive price deviation
//! - [`TimeFilter`] - Restricts trading to specific time windows
//!
//! # Example
//!
//! ```ignore
//! use zephyr_engine::filter::{FilterManager, FilterManagerConfig, PositionLimitFilter};
//!
//! let mut manager = FilterManager::new(FilterManagerConfig::default());
//! manager.add_filter(Box::new(PositionLimitFilter::new()));
//!
//! let result = manager.process(&signal, &context);
//! match result {
//!     FilterResult::Pass => { /* execute signal */ }
//!     FilterResult::Reject(reason) => { /* log rejection */ }
//!     FilterResult::Modify(new_signal) => { /* execute modified signal */ }
//! }
//! ```

mod filters;
mod manager;
mod stats;
mod types;

pub use filters::{
    FrequencyFilter, FrequencyFilterConfig, PositionLimitConfig, PositionLimitFilter,
    PriceDeviationConfig, PriceDeviationFilter, TimeFilter, TimeFilterConfig, TimeWindow,
};
pub use manager::{FilterChainResult, FilterManager, FilterManagerConfig};
pub use stats::{FilterEvent, FilterEventType, FilterStats, FilterStatsReport};
pub use types::{
    FilterContext, FilterId, FilterModification, FilterPriority, FilterRejection, FilterResult,
    SignalFilter,
};
