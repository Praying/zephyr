//! Core filter types and traits.
//!
//! This module defines the fundamental types for the signal filtering system.

#![allow(clippy::disallowed_types)]
#![allow(clippy::unnecessary_literal_bound)]

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use zephyr_core::data::Order;
use zephyr_core::types::{Amount, Price, Quantity, Symbol, Timestamp};

use crate::ExecutionSignal;

/// Unique identifier for a filter.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FilterId(String);

impl FilterId {
    /// Creates a new filter ID.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FilterId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for FilterId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for FilterId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Filter priority - lower numbers execute first.
///
/// Filters are executed in ascending priority order.
/// Use standard priority levels or custom values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FilterPriority(pub u32);

impl FilterPriority {
    /// Highest priority - executes first (e.g., kill switch).
    pub const CRITICAL: Self = Self(0);
    /// High priority - risk checks.
    pub const HIGH: Self = Self(100);
    /// Normal priority - standard filters.
    pub const NORMAL: Self = Self(500);
    /// Low priority - logging/monitoring filters.
    pub const LOW: Self = Self(900);
    /// Lowest priority - executes last.
    pub const LOWEST: Self = Self(1000);

    /// Creates a custom priority.
    #[must_use]
    pub const fn custom(value: u32) -> Self {
        Self(value)
    }
}

impl Default for FilterPriority {
    fn default() -> Self {
        Self::NORMAL
    }
}

impl fmt::Display for FilterPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Context provided to filters for decision making.
///
/// Contains current market state, positions, and recent order history
/// that filters can use to make filtering decisions.
#[derive(Debug, Clone)]
pub struct FilterContext {
    /// Current positions per symbol.
    pub current_positions: HashMap<Symbol, Quantity>,
    /// Pending (unfilled) orders.
    pub pending_orders: Vec<Order>,
    /// Recent orders (for frequency limiting).
    pub recent_orders: Vec<Order>,
    /// Current account balance.
    pub account_balance: Amount,
    /// Current timestamp.
    pub current_time: Timestamp,
    /// Current prices per symbol (for deviation checks).
    pub current_prices: HashMap<Symbol, Price>,
    /// Additional context data.
    pub metadata: HashMap<String, String>,
}

impl FilterContext {
    /// Creates a new filter context.
    #[must_use]
    pub fn new(account_balance: Amount, current_time: Timestamp) -> Self {
        Self {
            current_positions: HashMap::new(),
            pending_orders: Vec::new(),
            recent_orders: Vec::new(),
            account_balance,
            current_time,
            current_prices: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    /// Creates a builder for `FilterContext`.
    #[must_use]
    pub fn builder() -> FilterContextBuilder {
        FilterContextBuilder::default()
    }

    /// Gets the current position for a symbol.
    #[must_use]
    pub fn get_position(&self, symbol: &Symbol) -> Quantity {
        self.current_positions
            .get(symbol)
            .copied()
            .unwrap_or(Quantity::ZERO)
    }

    /// Gets the current price for a symbol.
    #[must_use]
    pub fn get_price(&self, symbol: &Symbol) -> Option<Price> {
        self.current_prices.get(symbol).copied()
    }

    /// Gets pending orders for a symbol.
    #[must_use]
    pub fn get_pending_orders(&self, symbol: &Symbol) -> Vec<&Order> {
        self.pending_orders
            .iter()
            .filter(|o| &o.symbol == symbol)
            .collect()
    }

    /// Gets recent orders for a symbol.
    #[must_use]
    pub fn get_recent_orders(&self, symbol: &Symbol) -> Vec<&Order> {
        self.recent_orders
            .iter()
            .filter(|o| &o.symbol == symbol)
            .collect()
    }

    /// Counts orders within a time window.
    #[must_use]
    pub fn count_orders_since(&self, since: Timestamp) -> usize {
        self.recent_orders
            .iter()
            .filter(|o| o.create_time >= since)
            .count()
    }

    /// Counts orders for a symbol within a time window.
    #[must_use]
    pub fn count_orders_for_symbol_since(&self, symbol: &Symbol, since: Timestamp) -> usize {
        self.recent_orders
            .iter()
            .filter(|o| &o.symbol == symbol && o.create_time >= since)
            .count()
    }
}

impl Default for FilterContext {
    fn default() -> Self {
        Self::new(Amount::ZERO, Timestamp::now())
    }
}

/// Builder for `FilterContext`.
#[derive(Debug, Default)]
pub struct FilterContextBuilder {
    current_positions: HashMap<Symbol, Quantity>,
    pending_orders: Vec<Order>,
    recent_orders: Vec<Order>,
    account_balance: Amount,
    current_time: Option<Timestamp>,
    current_prices: HashMap<Symbol, Price>,
    metadata: HashMap<String, String>,
}

impl FilterContextBuilder {
    /// Sets the current positions.
    #[must_use]
    pub fn positions(mut self, positions: HashMap<Symbol, Quantity>) -> Self {
        self.current_positions = positions;
        self
    }

    /// Adds a position.
    #[must_use]
    pub fn add_position(mut self, symbol: Symbol, quantity: Quantity) -> Self {
        self.current_positions.insert(symbol, quantity);
        self
    }

    /// Sets the pending orders.
    #[must_use]
    pub fn pending_orders(mut self, orders: Vec<Order>) -> Self {
        self.pending_orders = orders;
        self
    }

    /// Sets the recent orders.
    #[must_use]
    pub fn recent_orders(mut self, orders: Vec<Order>) -> Self {
        self.recent_orders = orders;
        self
    }

    /// Sets the account balance.
    #[must_use]
    pub fn account_balance(mut self, balance: Amount) -> Self {
        self.account_balance = balance;
        self
    }

    /// Sets the current time.
    #[must_use]
    pub fn current_time(mut self, time: Timestamp) -> Self {
        self.current_time = Some(time);
        self
    }

    /// Sets the current prices.
    #[must_use]
    pub fn prices(mut self, prices: HashMap<Symbol, Price>) -> Self {
        self.current_prices = prices;
        self
    }

    /// Adds a price.
    #[must_use]
    pub fn add_price(mut self, symbol: Symbol, price: Price) -> Self {
        self.current_prices.insert(symbol, price);
        self
    }

    /// Adds metadata.
    #[must_use]
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Builds the `FilterContext`.
    #[must_use]
    pub fn build(self) -> FilterContext {
        FilterContext {
            current_positions: self.current_positions,
            pending_orders: self.pending_orders,
            recent_orders: self.recent_orders,
            account_balance: self.account_balance,
            current_time: self.current_time.unwrap_or_else(Timestamp::now),
            current_prices: self.current_prices,
            metadata: self.metadata,
        }
    }
}

/// Details about why a signal was rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterRejection {
    /// Filter that rejected the signal.
    pub filter_id: FilterId,
    /// Human-readable rejection reason.
    pub reason: String,
    /// Error code for programmatic handling.
    pub code: Option<String>,
    /// Additional details.
    pub details: HashMap<String, String>,
}

impl FilterRejection {
    /// Creates a new rejection.
    #[must_use]
    pub fn new(filter_id: FilterId, reason: impl Into<String>) -> Self {
        Self {
            filter_id,
            reason: reason.into(),
            code: None,
            details: HashMap::new(),
        }
    }

    /// Sets the error code.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Adds a detail.
    #[must_use]
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }
}

impl fmt::Display for FilterRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.filter_id, self.reason)
    }
}

/// Details about how a signal was modified.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterModification {
    /// Filter that modified the signal.
    pub filter_id: FilterId,
    /// Description of the modification.
    pub description: String,
    /// Original values before modification.
    pub original_values: HashMap<String, String>,
    /// New values after modification.
    pub new_values: HashMap<String, String>,
}

impl FilterModification {
    /// Creates a new modification record.
    #[must_use]
    pub fn new(filter_id: FilterId, description: impl Into<String>) -> Self {
        Self {
            filter_id,
            description: description.into(),
            original_values: HashMap::new(),
            new_values: HashMap::new(),
        }
    }

    /// Records a value change.
    #[must_use]
    pub fn with_change(
        mut self,
        field: impl Into<String>,
        original: impl Into<String>,
        new: impl Into<String>,
    ) -> Self {
        let field = field.into();
        self.original_values.insert(field.clone(), original.into());
        self.new_values.insert(field, new.into());
        self
    }
}

impl fmt::Display for FilterModification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.filter_id, self.description)
    }
}

/// Result of filtering a signal.
///
/// A filter can pass, reject, or modify a signal.
#[derive(Debug, Clone)]
pub enum FilterResult {
    /// Signal passes unchanged.
    Pass,
    /// Signal is rejected with a reason.
    Reject(FilterRejection),
    /// Signal is modified.
    Modify {
        /// The modified signal.
        signal: ExecutionSignal,
        /// Details about the modification.
        modification: FilterModification,
    },
}

impl FilterResult {
    /// Creates a pass result.
    #[must_use]
    pub fn pass() -> Self {
        Self::Pass
    }

    /// Creates a reject result.
    #[must_use]
    pub fn reject(filter_id: FilterId, reason: impl Into<String>) -> Self {
        Self::Reject(FilterRejection::new(filter_id, reason))
    }

    /// Creates a reject result with details.
    #[must_use]
    pub fn reject_with(rejection: FilterRejection) -> Self {
        Self::Reject(rejection)
    }

    /// Creates a modify result.
    #[must_use]
    pub fn modify(signal: ExecutionSignal, modification: FilterModification) -> Self {
        Self::Modify {
            signal,
            modification,
        }
    }

    /// Returns true if the result is a pass.
    #[must_use]
    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Pass)
    }

    /// Returns true if the result is a rejection.
    #[must_use]
    pub fn is_reject(&self) -> bool {
        matches!(self, Self::Reject(_))
    }

    /// Returns true if the result is a modification.
    #[must_use]
    pub fn is_modify(&self) -> bool {
        matches!(self, Self::Modify { .. })
    }

    /// Gets the rejection if this is a reject result.
    #[must_use]
    pub fn rejection(&self) -> Option<&FilterRejection> {
        match self {
            Self::Reject(r) => Some(r),
            _ => None,
        }
    }
}

/// Signal filter trait.
///
/// Implement this trait to create custom filters that can validate,
/// modify, or reject execution signals.
///
/// # Example
///
/// ```ignore
/// use zephyr_engine::filter::{SignalFilter, FilterResult, FilterContext, FilterId, FilterPriority};
/// use zephyr_engine::ExecutionSignal;
///
/// struct MyFilter {
///     id: FilterId,
/// }
///
/// impl SignalFilter for MyFilter {
///     fn id(&self) -> &FilterId {
///         &self.id
///     }
///
///     fn name(&self) -> &str {
///         "my-filter"
///     }
///
///     fn filter(&self, signal: &ExecutionSignal, context: &FilterContext) -> FilterResult {
///         // Custom filtering logic
///         FilterResult::Pass
///     }
///
///     fn priority(&self) -> FilterPriority {
///         FilterPriority::NORMAL
///     }
/// }
/// ```
pub trait SignalFilter: Send + Sync {
    /// Returns the unique filter ID.
    fn id(&self) -> &FilterId;

    /// Returns the filter name for display/logging.
    fn name(&self) -> &str;

    /// Filters a signal.
    ///
    /// Returns:
    /// - `FilterResult::Pass` - Signal passes unchanged
    /// - `FilterResult::Reject(reason)` - Signal is rejected
    /// - `FilterResult::Modify(signal, modification)` - Signal is modified
    fn filter(&self, signal: &ExecutionSignal, context: &FilterContext) -> FilterResult;

    /// Returns the filter priority (lower executes first).
    fn priority(&self) -> FilterPriority {
        FilterPriority::NORMAL
    }

    /// Returns true if the filter is enabled.
    fn is_enabled(&self) -> bool {
        true
    }

    /// Returns a description of the filter.
    fn description(&self) -> &str {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_filter_id() {
        let id = FilterId::new("test-filter");
        assert_eq!(id.as_str(), "test-filter");
        assert_eq!(format!("{id}"), "test-filter");
    }

    #[test]
    fn test_filter_priority_ordering() {
        assert!(FilterPriority::CRITICAL < FilterPriority::HIGH);
        assert!(FilterPriority::HIGH < FilterPriority::NORMAL);
        assert!(FilterPriority::NORMAL < FilterPriority::LOW);
        assert!(FilterPriority::LOW < FilterPriority::LOWEST);
    }

    #[test]
    fn test_filter_context_builder() {
        let symbol = Symbol::new("BTC-USDT").unwrap();
        let qty = Quantity::new(dec!(1.0)).unwrap();
        let price = Price::new(dec!(50000)).unwrap();
        let balance = Amount::new(dec!(10000)).unwrap();

        let ctx = FilterContext::builder()
            .add_position(symbol.clone(), qty)
            .add_price(symbol.clone(), price)
            .account_balance(balance)
            .add_metadata("key", "value")
            .build();

        assert_eq!(ctx.get_position(&symbol), qty);
        assert_eq!(ctx.get_price(&symbol), Some(price));
        assert_eq!(ctx.account_balance, balance);
        assert_eq!(ctx.metadata.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_filter_context_default() {
        let ctx = FilterContext::default();
        assert!(ctx.current_positions.is_empty());
        assert!(ctx.pending_orders.is_empty());
        assert_eq!(ctx.account_balance, Amount::ZERO);
    }

    #[test]
    fn test_filter_rejection() {
        let rejection = FilterRejection::new(FilterId::new("test"), "Position limit exceeded")
            .with_code("POSITION_LIMIT")
            .with_detail("limit", "100")
            .with_detail("requested", "150");

        assert_eq!(rejection.filter_id.as_str(), "test");
        assert_eq!(rejection.reason, "Position limit exceeded");
        assert_eq!(rejection.code, Some("POSITION_LIMIT".to_string()));
        assert_eq!(rejection.details.get("limit"), Some(&"100".to_string()));
    }

    #[test]
    fn test_filter_modification() {
        let modification = FilterModification::new(FilterId::new("test"), "Reduced quantity")
            .with_change("quantity", "100", "50");

        assert_eq!(modification.filter_id.as_str(), "test");
        assert_eq!(modification.description, "Reduced quantity");
        assert_eq!(
            modification.original_values.get("quantity"),
            Some(&"100".to_string())
        );
        assert_eq!(
            modification.new_values.get("quantity"),
            Some(&"50".to_string())
        );
    }

    #[test]
    fn test_filter_result_pass() {
        let result = FilterResult::pass();
        assert!(result.is_pass());
        assert!(!result.is_reject());
        assert!(!result.is_modify());
    }

    #[test]
    fn test_filter_result_reject() {
        let result = FilterResult::reject(FilterId::new("test"), "Test rejection");
        assert!(!result.is_pass());
        assert!(result.is_reject());
        assert!(!result.is_modify());

        let rejection = result.rejection().unwrap();
        assert_eq!(rejection.reason, "Test rejection");
    }
}
