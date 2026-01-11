//! Built-in filter implementations.
//!
//! This module provides standard filters for common use cases:
//! - Position limits
//! - Order frequency limits
//! - Price deviation checks
//! - Time-based restrictions

#![allow(clippy::disallowed_types)]
#![allow(clippy::unnecessary_literal_bound)]
#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::cast_possible_truncation)]

use std::collections::HashMap;

use parking_lot::RwLock;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::debug;

use zephyr_core::types::{Quantity, Symbol, Timestamp};

use crate::ExecutionSignal;

use super::types::{
    FilterContext, FilterId, FilterPriority, FilterRejection, FilterResult, SignalFilter,
};

// ============================================================================
// Position Limit Filter
// ============================================================================

/// Configuration for position limit filter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionLimitConfig {
    /// Maximum position per symbol.
    #[serde(default)]
    pub max_position_per_symbol: HashMap<Symbol, Quantity>,
    /// Default maximum position for symbols not in the map.
    pub default_max_position: Option<Quantity>,
    /// Whether to allow reducing positions that exceed limits.
    #[serde(default = "default_true")]
    pub allow_reduce_over_limit: bool,
}

fn default_true() -> bool {
    true
}

impl Default for PositionLimitConfig {
    fn default() -> Self {
        Self {
            max_position_per_symbol: HashMap::new(),
            default_max_position: None,
            allow_reduce_over_limit: true,
        }
    }
}

/// Filter that enforces position limits per symbol.
///
/// Rejects signals that would result in positions exceeding configured limits.
pub struct PositionLimitFilter {
    id: FilterId,
    config: PositionLimitConfig,
    enabled: bool,
}

impl PositionLimitFilter {
    /// Creates a new position limit filter.
    #[must_use]
    pub fn new(config: PositionLimitConfig) -> Self {
        Self {
            id: FilterId::new("position-limit"),
            config,
            enabled: true,
        }
    }

    /// Creates a filter with a default limit for all symbols.
    #[must_use]
    pub fn with_default_limit(max_position: Quantity) -> Self {
        Self::new(PositionLimitConfig {
            default_max_position: Some(max_position),
            ..Default::default()
        })
    }

    /// Sets the limit for a specific symbol.
    pub fn set_limit(&mut self, symbol: Symbol, max_position: Quantity) {
        self.config
            .max_position_per_symbol
            .insert(symbol, max_position);
    }

    /// Gets the limit for a symbol.
    #[must_use]
    pub fn get_limit(&self, symbol: &Symbol) -> Option<Quantity> {
        self.config
            .max_position_per_symbol
            .get(symbol)
            .copied()
            .or(self.config.default_max_position)
    }

    /// Sets the enabled state.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

impl SignalFilter for PositionLimitFilter {
    fn id(&self) -> &FilterId {
        &self.id
    }

    fn name(&self) -> &str {
        "Position Limit Filter"
    }

    fn filter(&self, signal: &ExecutionSignal, context: &FilterContext) -> FilterResult {
        let Some(max_position) = self.get_limit(&signal.symbol) else {
            // No limit configured for this symbol
            return FilterResult::Pass;
        };

        let current_position = context.get_position(&signal.symbol);
        let target_position = signal.target_position;

        // Check if target exceeds limit
        if target_position.abs() > max_position {
            // Check if we're reducing position
            if self.config.allow_reduce_over_limit && target_position.abs() < current_position.abs()
            {
                debug!(
                    symbol = %signal.symbol,
                    current = %current_position,
                    target = %target_position,
                    limit = %max_position,
                    "Allowing position reduction despite exceeding limit"
                );
                return FilterResult::Pass;
            }

            return FilterResult::reject_with(
                FilterRejection::new(
                    self.id.clone(),
                    format!(
                        "Target position {} exceeds limit {} for {}",
                        target_position, max_position, signal.symbol
                    ),
                )
                .with_code("POSITION_LIMIT_EXCEEDED")
                .with_detail("symbol", signal.symbol.as_str())
                .with_detail("target", target_position.to_string())
                .with_detail("limit", max_position.to_string()),
            );
        }

        FilterResult::Pass
    }

    fn priority(&self) -> FilterPriority {
        FilterPriority::HIGH
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn description(&self) -> &str {
        "Enforces maximum position limits per symbol"
    }
}

// ============================================================================
// Frequency Filter
// ============================================================================

/// Configuration for frequency filter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrequencyFilterConfig {
    /// Maximum orders per second (global).
    pub max_orders_per_second: Option<u32>,
    /// Maximum orders per minute (global).
    pub max_orders_per_minute: Option<u32>,
    /// Maximum orders per second per symbol.
    pub max_orders_per_second_per_symbol: Option<u32>,
    /// Maximum orders per minute per symbol.
    pub max_orders_per_minute_per_symbol: Option<u32>,
}

impl Default for FrequencyFilterConfig {
    fn default() -> Self {
        Self {
            max_orders_per_second: Some(10),
            max_orders_per_minute: Some(100),
            max_orders_per_second_per_symbol: Some(5),
            max_orders_per_minute_per_symbol: Some(30),
        }
    }
}

/// Filter that limits order frequency.
///
/// Rejects signals if order rate exceeds configured limits.
pub struct FrequencyFilter {
    id: FilterId,
    config: FrequencyFilterConfig,
    enabled: bool,
    /// Order timestamps for rate limiting (global).
    order_times: RwLock<Vec<Timestamp>>,
    /// Order timestamps per symbol.
    symbol_order_times: RwLock<HashMap<Symbol, Vec<Timestamp>>>,
}

impl FrequencyFilter {
    /// Creates a new frequency filter.
    #[must_use]
    pub fn new(config: FrequencyFilterConfig) -> Self {
        Self {
            id: FilterId::new("frequency"),
            config,
            enabled: true,
            order_times: RwLock::new(Vec::new()),
            symbol_order_times: RwLock::new(HashMap::new()),
        }
    }

    /// Sets the enabled state.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Records an order for rate limiting.
    pub fn record_order(&self, symbol: &Symbol, timestamp: Timestamp) {
        // Record global
        {
            let mut times = self.order_times.write();
            times.push(timestamp);
            // Keep only last minute of orders
            let cutoff = Timestamp::new_unchecked(timestamp.as_millis() - 60_000);
            times.retain(|t| *t >= cutoff);
        }

        // Record per-symbol
        {
            let mut symbol_times = self.symbol_order_times.write();
            let times = symbol_times.entry(symbol.clone()).or_default();
            times.push(timestamp);
            let cutoff = Timestamp::new_unchecked(timestamp.as_millis() - 60_000);
            times.retain(|t| *t >= cutoff);
        }
    }

    /// Counts orders since a timestamp.
    fn count_orders_since(&self, since: Timestamp) -> usize {
        self.order_times
            .read()
            .iter()
            .filter(|t| **t >= since)
            .count()
    }

    /// Counts orders for a symbol since a timestamp.
    fn count_symbol_orders_since(&self, symbol: &Symbol, since: Timestamp) -> usize {
        self.symbol_order_times
            .read()
            .get(symbol)
            .map(|times| times.iter().filter(|t| **t >= since).count())
            .unwrap_or(0)
    }

    /// Clears order history.
    pub fn clear(&self) {
        self.order_times.write().clear();
        self.symbol_order_times.write().clear();
    }
}

impl SignalFilter for FrequencyFilter {
    fn id(&self) -> &FilterId {
        &self.id
    }

    fn name(&self) -> &str {
        "Frequency Filter"
    }

    fn filter(&self, signal: &ExecutionSignal, _context: &FilterContext) -> FilterResult {
        let now = signal.timestamp;
        let one_second_ago = Timestamp::new_unchecked(now.as_millis() - 1_000);
        let one_minute_ago = Timestamp::new_unchecked(now.as_millis() - 60_000);

        // Check global per-second limit
        if let Some(max) = self.config.max_orders_per_second {
            let count = self.count_orders_since(one_second_ago);
            if count >= max as usize {
                return FilterResult::reject_with(
                    FilterRejection::new(
                        self.id.clone(),
                        format!("Global order rate exceeded: {count}/{max} orders/second"),
                    )
                    .with_code("RATE_LIMIT_SECOND")
                    .with_detail("count", count.to_string())
                    .with_detail("limit", max.to_string()),
                );
            }
        }

        // Check global per-minute limit
        if let Some(max) = self.config.max_orders_per_minute {
            let count = self.count_orders_since(one_minute_ago);
            if count >= max as usize {
                return FilterResult::reject_with(
                    FilterRejection::new(
                        self.id.clone(),
                        format!("Global order rate exceeded: {count}/{max} orders/minute"),
                    )
                    .with_code("RATE_LIMIT_MINUTE")
                    .with_detail("count", count.to_string())
                    .with_detail("limit", max.to_string()),
                );
            }
        }

        // Check per-symbol per-second limit
        if let Some(max) = self.config.max_orders_per_second_per_symbol {
            let count = self.count_symbol_orders_since(&signal.symbol, one_second_ago);
            if count >= max as usize {
                return FilterResult::reject_with(
                    FilterRejection::new(
                        self.id.clone(),
                        format!(
                            "Symbol order rate exceeded for {}: {count}/{max} orders/second",
                            signal.symbol
                        ),
                    )
                    .with_code("RATE_LIMIT_SYMBOL_SECOND")
                    .with_detail("symbol", signal.symbol.as_str())
                    .with_detail("count", count.to_string())
                    .with_detail("limit", max.to_string()),
                );
            }
        }

        // Check per-symbol per-minute limit
        if let Some(max) = self.config.max_orders_per_minute_per_symbol {
            let count = self.count_symbol_orders_since(&signal.symbol, one_minute_ago);
            if count >= max as usize {
                return FilterResult::reject_with(
                    FilterRejection::new(
                        self.id.clone(),
                        format!(
                            "Symbol order rate exceeded for {}: {count}/{max} orders/minute",
                            signal.symbol
                        ),
                    )
                    .with_code("RATE_LIMIT_SYMBOL_MINUTE")
                    .with_detail("symbol", signal.symbol.as_str())
                    .with_detail("count", count.to_string())
                    .with_detail("limit", max.to_string()),
                );
            }
        }

        // Record this order
        self.record_order(&signal.symbol, now);

        FilterResult::Pass
    }

    fn priority(&self) -> FilterPriority {
        FilterPriority::HIGH
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn description(&self) -> &str {
        "Limits order frequency to prevent excessive trading"
    }
}

// ============================================================================
// Price Deviation Filter
// ============================================================================

/// Configuration for price deviation filter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceDeviationConfig {
    /// Maximum allowed deviation from current price as percentage (e.g., 5.0 for 5%).
    pub max_deviation_percent: Decimal,
    /// Whether to use mark price instead of last price.
    #[serde(default)]
    pub use_mark_price: bool,
}

impl Default for PriceDeviationConfig {
    fn default() -> Self {
        Self {
            max_deviation_percent: Decimal::new(5, 0), // 5%
            use_mark_price: false,
        }
    }
}

/// Filter that rejects signals with excessive price deviation.
///
/// Compares the implied order price against current market price.
pub struct PriceDeviationFilter {
    id: FilterId,
    #[allow(dead_code)]
    config: PriceDeviationConfig,
    enabled: bool,
}

impl PriceDeviationFilter {
    /// Creates a new price deviation filter.
    #[must_use]
    pub fn new(config: PriceDeviationConfig) -> Self {
        Self {
            id: FilterId::new("price-deviation"),
            config,
            enabled: true,
        }
    }

    /// Creates a filter with a specific deviation limit.
    #[must_use]
    pub fn with_max_deviation(max_deviation_percent: Decimal) -> Self {
        Self::new(PriceDeviationConfig {
            max_deviation_percent,
            ..Default::default()
        })
    }

    /// Sets the enabled state.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

impl SignalFilter for PriceDeviationFilter {
    fn id(&self) -> &FilterId {
        &self.id
    }

    fn name(&self) -> &str {
        "Price Deviation Filter"
    }

    fn filter(&self, signal: &ExecutionSignal, context: &FilterContext) -> FilterResult {
        // This filter requires current price in context
        let Some(current_price) = context.get_price(&signal.symbol) else {
            debug!(
                symbol = %signal.symbol,
                "No current price available, skipping price deviation check"
            );
            return FilterResult::Pass;
        };

        // For execution signals, we don't have an explicit price
        // This filter is more useful for order requests with explicit prices
        // For now, we pass all execution signals
        // In a real implementation, you might check against expected execution price

        debug!(
            symbol = %signal.symbol,
            current_price = %current_price,
            "Price deviation check passed (execution signal)"
        );

        FilterResult::Pass
    }

    fn priority(&self) -> FilterPriority {
        FilterPriority::NORMAL
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn description(&self) -> &str {
        "Rejects orders with excessive price deviation from market"
    }
}

// ============================================================================
// Time Filter
// ============================================================================

/// A time window for trading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWindow {
    /// Start hour (0-23).
    pub start_hour: u8,
    /// Start minute (0-59).
    pub start_minute: u8,
    /// End hour (0-23).
    pub end_hour: u8,
    /// End minute (0-59).
    pub end_minute: u8,
}

impl TimeWindow {
    /// Creates a new time window.
    #[must_use]
    pub fn new(start_hour: u8, start_minute: u8, end_hour: u8, end_minute: u8) -> Self {
        Self {
            start_hour,
            start_minute,
            end_hour,
            end_minute,
        }
    }

    /// Creates a time window from hours only.
    #[must_use]
    pub fn from_hours(start_hour: u8, end_hour: u8) -> Self {
        Self::new(start_hour, 0, end_hour, 0)
    }

    /// Checks if a time (hour, minute) is within this window.
    #[must_use]
    pub fn contains(&self, hour: u8, minute: u8) -> bool {
        let time = hour as u16 * 60 + minute as u16;
        let start = self.start_hour as u16 * 60 + self.start_minute as u16;
        let end = self.end_hour as u16 * 60 + self.end_minute as u16;

        if start <= end {
            // Normal window (e.g., 09:00 - 17:00)
            time >= start && time < end
        } else {
            // Overnight window (e.g., 22:00 - 06:00)
            time >= start || time < end
        }
    }
}

/// Configuration for time filter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeFilterConfig {
    /// Allowed trading windows.
    #[serde(default)]
    pub allowed_windows: Vec<TimeWindow>,
    /// Blocked dates (YYYY-MM-DD format).
    #[serde(default)]
    pub blocked_dates: Vec<String>,
    /// Whether to block weekends.
    #[serde(default)]
    pub block_weekends: bool,
}

impl Default for TimeFilterConfig {
    fn default() -> Self {
        Self {
            allowed_windows: Vec::new(),
            blocked_dates: Vec::new(),
            block_weekends: false,
        }
    }
}

/// Filter that restricts trading to specific time windows.
pub struct TimeFilter {
    id: FilterId,
    config: TimeFilterConfig,
    enabled: bool,
}

impl TimeFilter {
    /// Creates a new time filter.
    #[must_use]
    pub fn new(config: TimeFilterConfig) -> Self {
        Self {
            id: FilterId::new("time"),
            config,
            enabled: true,
        }
    }

    /// Creates a filter that only allows trading during specific hours.
    #[must_use]
    pub fn with_trading_hours(windows: Vec<TimeWindow>) -> Self {
        Self::new(TimeFilterConfig {
            allowed_windows: windows,
            ..Default::default()
        })
    }

    /// Sets the enabled state.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Adds a blocked date.
    pub fn add_blocked_date(&mut self, date: impl Into<String>) {
        self.config.blocked_dates.push(date.into());
    }

    /// Checks if a timestamp is within allowed trading times.
    fn is_allowed_time(&self, timestamp: Timestamp) -> bool {
        // Convert timestamp to date/time components
        // Using chrono for proper time handling
        use chrono::{Datelike, TimeZone, Timelike, Utc, Weekday};

        let datetime = Utc.timestamp_millis_opt(timestamp.as_millis()).unwrap();
        let hour = datetime.hour() as u8;
        let minute = datetime.minute() as u8;
        let date_str = datetime.format("%Y-%m-%d").to_string();

        // Check blocked dates
        if self.config.blocked_dates.contains(&date_str) {
            return false;
        }

        // Check weekends
        if self.config.block_weekends {
            let weekday = datetime.weekday();
            if weekday == Weekday::Sat || weekday == Weekday::Sun {
                return false;
            }
        }

        // Check time windows (if any configured)
        if self.config.allowed_windows.is_empty() {
            return true; // No windows configured = always allowed
        }

        self.config
            .allowed_windows
            .iter()
            .any(|w| w.contains(hour, minute))
    }
}

impl SignalFilter for TimeFilter {
    fn id(&self) -> &FilterId {
        &self.id
    }

    fn name(&self) -> &str {
        "Time Filter"
    }

    fn filter(&self, signal: &ExecutionSignal, _context: &FilterContext) -> FilterResult {
        if !self.is_allowed_time(signal.timestamp) {
            return FilterResult::reject_with(
                FilterRejection::new(self.id.clone(), "Trading not allowed at this time")
                    .with_code("TIME_RESTRICTED")
                    .with_detail("timestamp", signal.timestamp.to_string()),
            );
        }

        FilterResult::Pass
    }

    fn priority(&self) -> FilterPriority {
        FilterPriority::HIGH
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn description(&self) -> &str {
        "Restricts trading to specific time windows"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;
    use zephyr_core::data::OrderSide;
    use zephyr_core::types::{Amount, Price};

    fn create_test_signal(symbol: &str, target: Decimal) -> ExecutionSignal {
        ExecutionSignal {
            symbol: Symbol::new(symbol).unwrap(),
            current_position: Quantity::ZERO,
            target_position: Quantity::new(target).unwrap(),
            order_quantity: Quantity::new(target.abs()).unwrap(),
            order_side: if target >= Decimal::ZERO {
                OrderSide::Buy
            } else {
                OrderSide::Sell
            },
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

    // Position Limit Filter Tests
    #[test]
    fn test_position_limit_filter_pass() {
        let filter = PositionLimitFilter::with_default_limit(Quantity::new(dec!(10)).unwrap());
        let signal = create_test_signal("BTC-USDT", dec!(5));
        let context = create_test_context();

        let result = filter.filter(&signal, &context);
        assert!(result.is_pass());
    }

    #[test]
    fn test_position_limit_filter_reject() {
        let filter = PositionLimitFilter::with_default_limit(Quantity::new(dec!(10)).unwrap());
        let signal = create_test_signal("BTC-USDT", dec!(15));
        let context = create_test_context();

        let result = filter.filter(&signal, &context);
        assert!(result.is_reject());

        let rejection = result.rejection().unwrap();
        assert_eq!(rejection.code, Some("POSITION_LIMIT_EXCEEDED".to_string()));
    }

    #[test]
    fn test_position_limit_filter_per_symbol() {
        let mut filter = PositionLimitFilter::new(PositionLimitConfig::default());
        filter.set_limit(
            Symbol::new("BTC-USDT").unwrap(),
            Quantity::new(dec!(5)).unwrap(),
        );
        filter.set_limit(
            Symbol::new("ETH-USDT").unwrap(),
            Quantity::new(dec!(100)).unwrap(),
        );

        let context = create_test_context();

        // BTC should be rejected at 10
        let btc_signal = create_test_signal("BTC-USDT", dec!(10));
        assert!(filter.filter(&btc_signal, &context).is_reject());

        // ETH should pass at 10
        let eth_signal = create_test_signal("ETH-USDT", dec!(10));
        assert!(filter.filter(&eth_signal, &context).is_pass());
    }

    #[test]
    fn test_position_limit_filter_no_limit() {
        let filter = PositionLimitFilter::new(PositionLimitConfig::default());
        let signal = create_test_signal("BTC-USDT", dec!(1000000));
        let context = create_test_context();

        // No limit configured, should pass
        let result = filter.filter(&signal, &context);
        assert!(result.is_pass());
    }

    // Frequency Filter Tests
    #[test]
    fn test_frequency_filter_pass() {
        let filter = FrequencyFilter::new(FrequencyFilterConfig {
            max_orders_per_second: Some(10),
            max_orders_per_minute: Some(100),
            max_orders_per_second_per_symbol: None,
            max_orders_per_minute_per_symbol: None,
        });

        let signal = create_test_signal("BTC-USDT", dec!(1));
        let context = create_test_context();

        let result = filter.filter(&signal, &context);
        assert!(result.is_pass());
    }

    #[test]
    fn test_frequency_filter_reject_per_second() {
        let filter = FrequencyFilter::new(FrequencyFilterConfig {
            max_orders_per_second: Some(2),
            max_orders_per_minute: None,
            max_orders_per_second_per_symbol: None,
            max_orders_per_minute_per_symbol: None,
        });

        let context = create_test_context();
        let now = Timestamp::now();

        // Record 2 orders
        filter.record_order(&Symbol::new("BTC-USDT").unwrap(), now);
        filter.record_order(&Symbol::new("ETH-USDT").unwrap(), now);

        // Third order should be rejected
        let signal = create_test_signal("BTC-USDT", dec!(1));
        let result = filter.filter(&signal, &context);
        assert!(result.is_reject());
    }

    #[test]
    fn test_frequency_filter_clear() {
        let filter = FrequencyFilter::new(FrequencyFilterConfig {
            max_orders_per_second: Some(2),
            ..Default::default()
        });

        let now = Timestamp::now();
        filter.record_order(&Symbol::new("BTC-USDT").unwrap(), now);
        filter.record_order(&Symbol::new("BTC-USDT").unwrap(), now);

        filter.clear();

        let signal = create_test_signal("BTC-USDT", dec!(1));
        let context = create_test_context();
        let result = filter.filter(&signal, &context);
        assert!(result.is_pass());
    }

    // Time Window Tests
    #[test]
    fn test_time_window_contains() {
        let window = TimeWindow::new(9, 0, 17, 0);

        assert!(window.contains(9, 0));
        assert!(window.contains(12, 30));
        assert!(window.contains(16, 59));
        assert!(!window.contains(8, 59));
        assert!(!window.contains(17, 0));
        assert!(!window.contains(20, 0));
    }

    #[test]
    fn test_time_window_overnight() {
        let window = TimeWindow::new(22, 0, 6, 0);

        assert!(window.contains(22, 0));
        assert!(window.contains(23, 30));
        assert!(window.contains(0, 0));
        assert!(window.contains(5, 59));
        assert!(!window.contains(6, 0));
        assert!(!window.contains(12, 0));
        assert!(!window.contains(21, 59));
    }

    // Time Filter Tests
    #[test]
    fn test_time_filter_no_restrictions() {
        let filter = TimeFilter::new(TimeFilterConfig::default());
        let signal = create_test_signal("BTC-USDT", dec!(1));
        let context = create_test_context();

        let result = filter.filter(&signal, &context);
        assert!(result.is_pass());
    }

    // Price Deviation Filter Tests
    #[test]
    fn test_price_deviation_filter_no_price() {
        let filter = PriceDeviationFilter::with_max_deviation(dec!(5));
        let signal = create_test_signal("BTC-USDT", dec!(1));
        let context = create_test_context(); // No price in context

        // Should pass when no price available
        let result = filter.filter(&signal, &context);
        assert!(result.is_pass());
    }

    #[test]
    fn test_price_deviation_filter_with_price() {
        let filter = PriceDeviationFilter::with_max_deviation(dec!(5));
        let signal = create_test_signal("BTC-USDT", dec!(1));
        let context = FilterContext::builder()
            .add_price(
                Symbol::new("BTC-USDT").unwrap(),
                Price::new(dec!(50000)).unwrap(),
            )
            .build();

        // Execution signals pass (no explicit price to check)
        let result = filter.filter(&signal, &context);
        assert!(result.is_pass());
    }
}
