//! Risk controller implementation.

// Allow clippy warnings that are acceptable in this module
#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::significant_drop_in_scrutinee)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::collapsible_if)]

use parking_lot::RwLock;
#[allow(clippy::disallowed_types)]
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};
use zephyr_core::types::{Amount, Quantity, Symbol, Timestamp};

use crate::error::RiskError;
use crate::limits::RiskConfig;

/// Result of a risk check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskCheckResult {
    /// Order passed all risk checks.
    Passed,
    /// Order was rejected with a reason.
    Rejected(RiskError),
    /// Order passed but with warnings.
    PassedWithWarning(String),
}

impl RiskCheckResult {
    /// Returns true if the check passed (with or without warnings).
    #[must_use]
    pub const fn is_passed(&self) -> bool {
        matches!(self, Self::Passed | Self::PassedWithWarning(_))
    }

    /// Returns true if the check was rejected.
    #[must_use]
    pub const fn is_rejected(&self) -> bool {
        matches!(self, Self::Rejected(_))
    }

    /// Returns the rejection error if rejected.
    #[must_use]
    pub fn rejection_error(&self) -> Option<&RiskError> {
        match self {
            Self::Rejected(e) => Some(e),
            _ => None,
        }
    }
}

/// Internal state for tracking positions.
#[derive(Debug, Default)]
#[allow(clippy::disallowed_types)]
struct PositionState {
    /// Current position per symbol.
    positions: HashMap<Symbol, Quantity>,
    /// Current notional value per symbol.
    notional_values: HashMap<Symbol, Amount>,
}

/// Internal state for tracking order frequency.
#[derive(Debug)]
struct FrequencyState {
    /// Timestamps of recent orders (for rate limiting).
    order_timestamps: VecDeque<Instant>,
    /// Orders in the current second.
    orders_this_second: u32,
    /// Start of the current second window.
    second_window_start: Instant,
    /// Orders in the current minute.
    orders_this_minute: u32,
    /// Start of the current minute window.
    minute_window_start: Instant,
    /// Orders in the current hour.
    orders_this_hour: u32,
    /// Start of the current hour window.
    hour_window_start: Instant,
}

impl Default for FrequencyState {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            order_timestamps: VecDeque::new(),
            orders_this_second: 0,
            second_window_start: now,
            orders_this_minute: 0,
            minute_window_start: now,
            orders_this_hour: 0,
            hour_window_start: now,
        }
    }
}

/// Internal state for tracking daily P&L.
#[derive(Debug, Default)]
struct PnlState {
    /// Realized P&L for the current day.
    daily_realized_pnl: Amount,
    /// Unrealized P&L for the current day.
    daily_unrealized_pnl: Amount,
    /// Start of the current trading day (UTC midnight).
    day_start: Option<Timestamp>,
}

/// Risk controller for enforcing trading limits.
///
/// The `RiskController` provides:
/// - Position limit checks per symbol and account-wide
/// - Order frequency limiting (per second, minute, hour)
/// - Daily loss limit enforcement
/// - Manual trading pause/resume
///
/// # Thread Safety
///
/// The controller is thread-safe and can be shared across multiple tasks.
///
/// # Example
///
/// ```
/// use zephyr_risk::{RiskController, RiskConfig};
/// use zephyr_core::types::{Symbol, Quantity};
/// use rust_decimal_macros::dec;
///
/// let config = RiskConfig::default();
/// let controller = RiskController::new(config);
///
/// // Update position
/// let symbol = Symbol::new("BTC-USDT").unwrap();
/// controller.update_position(&symbol, Quantity::new(dec!(0.5)).unwrap());
///
/// // Check if we can add more
/// let result = controller.check_position_limit(&symbol, Quantity::new(dec!(0.3)).unwrap());
/// assert!(result.is_passed());
/// ```
pub struct RiskController {
    /// Risk configuration.
    config: RwLock<RiskConfig>,
    /// Position tracking state.
    position_state: RwLock<PositionState>,
    /// Order frequency tracking state.
    frequency_state: RwLock<FrequencyState>,
    /// P&L tracking state.
    pnl_state: RwLock<PnlState>,
    /// Whether trading is paused.
    trading_paused: RwLock<Option<String>>,
}

impl RiskController {
    /// Creates a new `RiskController` with the given configuration.
    #[must_use]
    pub fn new(config: RiskConfig) -> Self {
        Self {
            config: RwLock::new(config),
            position_state: RwLock::new(PositionState::default()),
            frequency_state: RwLock::new(FrequencyState::default()),
            pnl_state: RwLock::new(PnlState::default()),
            trading_paused: RwLock::new(None),
        }
    }

    /// Creates a new `RiskController` wrapped in an `Arc`.
    #[must_use]
    pub fn new_shared(config: RiskConfig) -> Arc<Self> {
        Arc::new(Self::new(config))
    }

    /// Updates the risk configuration.
    #[allow(clippy::significant_drop_tightening)]
    pub fn update_config(&self, config: RiskConfig) {
        let mut cfg = self.config.write();
        *cfg = config;
        info!("Risk configuration updated");
    }

    /// Returns a clone of the current configuration.
    #[must_use]
    pub fn config(&self) -> RiskConfig {
        self.config.read().clone()
    }

    /// Checks if risk controls are enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.read().enabled
    }

    // ========== Position Limit Checks ==========

    /// Updates the current position for a symbol.
    #[allow(clippy::significant_drop_tightening)]
    pub fn update_position(&self, symbol: &Symbol, quantity: Quantity) {
        let mut state = self.position_state.write();
        state.positions.insert(symbol.clone(), quantity);
        debug!(symbol = %symbol, quantity = %quantity, "Position updated");
    }

    /// Updates the notional value for a symbol.
    #[allow(clippy::significant_drop_tightening)]
    pub fn update_notional(&self, symbol: &Symbol, notional: Amount) {
        let mut state = self.position_state.write();
        state.notional_values.insert(symbol.clone(), notional);
    }

    /// Gets the current position for a symbol.
    #[must_use]
    pub fn get_position(&self, symbol: &Symbol) -> Quantity {
        self.position_state
            .read()
            .positions
            .get(symbol)
            .copied()
            .unwrap_or(Quantity::ZERO)
    }

    /// Checks if an order would exceed position limits.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `order_qty` - The order quantity (positive for buy, negative for sell)
    ///
    /// # Returns
    ///
    /// `RiskCheckResult::Passed` if within limits, `RiskCheckResult::Rejected` otherwise.
    #[must_use]
    #[allow(clippy::significant_drop_tightening)]
    pub fn check_position_limit(&self, symbol: &Symbol, order_qty: Quantity) -> RiskCheckResult {
        let config = self.config.read();
        if !config.enabled {
            return RiskCheckResult::Passed;
        }

        let current = self.get_position(symbol);
        let new_position = current + order_qty;
        let new_position_abs = new_position.abs();

        // Check symbol-specific limit first, then global
        if let Some(limit) = config.get_position_limit(symbol)
            && new_position_abs > limit
        {
            warn!(
                symbol = %symbol,
                current = %current,
                order = %order_qty,
                new_position = %new_position,
                limit = %limit,
                "Position limit exceeded"
            );
            return RiskCheckResult::Rejected(RiskError::PositionLimitExceeded {
                symbol: symbol.clone(),
                current: new_position_abs,
                limit,
            });
        }

        RiskCheckResult::Passed
    }

    // ========== Order Frequency Checks ==========

    /// Records an order for frequency tracking.
    #[allow(clippy::significant_drop_tightening)]
    pub fn record_order(&self) {
        let mut state = self.frequency_state.write();
        let now = Instant::now();

        // Update second window
        if now.duration_since(state.second_window_start) >= Duration::from_secs(1) {
            state.orders_this_second = 0;
            state.second_window_start = now;
        }
        state.orders_this_second += 1;

        // Update minute window
        if now.duration_since(state.minute_window_start) >= Duration::from_secs(60) {
            state.orders_this_minute = 0;
            state.minute_window_start = now;
        }
        state.orders_this_minute += 1;

        // Update hour window
        if now.duration_since(state.hour_window_start) >= Duration::from_secs(3600) {
            state.orders_this_hour = 0;
            state.hour_window_start = now;
        }
        state.orders_this_hour += 1;

        // Keep timestamp for detailed tracking
        state.order_timestamps.push_back(now);

        // Clean up old timestamps (keep last hour)
        while let Some(front) = state.order_timestamps.front() {
            if now.duration_since(*front) > Duration::from_secs(3600) {
                state.order_timestamps.pop_front();
            } else {
                break;
            }
        }
    }

    /// Checks if an order would exceed frequency limits.
    #[must_use]
    pub fn check_frequency_limit(&self) -> RiskCheckResult {
        let config = self.config.read();
        if !config.enabled {
            return RiskCheckResult::Passed;
        }

        let state = self.frequency_state.read();
        let now = Instant::now();
        let freq = &config.frequency_limit;

        // Check per-second limit
        let effective_second_limit = freq.max_orders_per_second + freq.burst_allowance;
        if now.duration_since(state.second_window_start) < Duration::from_secs(1)
            && state.orders_this_second >= effective_second_limit
        {
            warn!(
                count = state.orders_this_second,
                limit = freq.max_orders_per_second,
                "Per-second order limit exceeded"
            );
            return RiskCheckResult::Rejected(RiskError::FrequencyLimitExceeded {
                count: state.orders_this_second,
                window_secs: 1,
                limit: freq.max_orders_per_second,
            });
        }

        // Check per-minute limit
        if now.duration_since(state.minute_window_start) < Duration::from_secs(60)
            && state.orders_this_minute >= freq.max_orders_per_minute
        {
            warn!(
                count = state.orders_this_minute,
                limit = freq.max_orders_per_minute,
                "Per-minute order limit exceeded"
            );
            return RiskCheckResult::Rejected(RiskError::FrequencyLimitExceeded {
                count: state.orders_this_minute,
                window_secs: 60,
                limit: freq.max_orders_per_minute,
            });
        }

        // Check per-hour limit if configured
        if let Some(hourly_limit) = freq.max_orders_per_hour {
            if now.duration_since(state.hour_window_start) < Duration::from_secs(3600)
                && state.orders_this_hour >= hourly_limit
            {
                warn!(
                    count = state.orders_this_hour,
                    limit = hourly_limit,
                    "Per-hour order limit exceeded"
                );
                return RiskCheckResult::Rejected(RiskError::FrequencyLimitExceeded {
                    count: state.orders_this_hour,
                    window_secs: 3600,
                    limit: hourly_limit,
                });
            }
        }

        RiskCheckResult::Passed
    }

    // ========== Daily Loss Limit Checks ==========

    /// Updates the daily P&L.
    pub fn update_daily_pnl(&self, realized: Amount, unrealized: Amount) {
        let mut state = self.pnl_state.write();
        state.daily_realized_pnl = realized;
        state.daily_unrealized_pnl = unrealized;
    }

    /// Resets the daily P&L (call at start of new trading day).
    pub fn reset_daily_pnl(&self) {
        let mut state = self.pnl_state.write();
        state.daily_realized_pnl = Amount::ZERO;
        state.daily_unrealized_pnl = Amount::ZERO;
        state.day_start = Some(Timestamp::now());
        info!("Daily P&L reset");
    }

    /// Gets the current daily loss.
    #[must_use]
    pub fn get_daily_loss(&self) -> Amount {
        let state = self.pnl_state.read();
        let total_pnl = state.daily_realized_pnl + state.daily_unrealized_pnl;
        if total_pnl.is_negative() {
            total_pnl.abs()
        } else {
            Amount::ZERO
        }
    }

    /// Checks if the daily loss limit has been exceeded.
    #[must_use]
    pub fn check_daily_loss_limit(&self) -> RiskCheckResult {
        let config = self.config.read();
        if !config.enabled {
            return RiskCheckResult::Passed;
        }

        if let Some(limit) = config.global_limits.max_daily_loss {
            let loss = self.get_daily_loss();
            if loss > limit {
                warn!(
                    loss = %loss,
                    limit = %limit,
                    "Daily loss limit exceeded"
                );
                return RiskCheckResult::Rejected(RiskError::DailyLossLimitExceeded {
                    loss,
                    limit,
                });
            }
        }

        RiskCheckResult::Passed
    }

    // ========== Trading Pause/Resume ==========

    /// Pauses trading with a reason.
    pub fn pause_trading(&self, reason: impl Into<String>) {
        let reason = reason.into();
        warn!(reason = %reason, "Trading paused");
        *self.trading_paused.write() = Some(reason);
    }

    /// Resumes trading.
    pub fn resume_trading(&self) {
        info!("Trading resumed");
        *self.trading_paused.write() = None;
    }

    /// Checks if trading is paused.
    #[must_use]
    pub fn is_trading_paused(&self) -> bool {
        self.trading_paused.read().is_some()
    }

    /// Gets the pause reason if trading is paused.
    #[must_use]
    pub fn pause_reason(&self) -> Option<String> {
        self.trading_paused.read().clone()
    }

    /// Checks if trading is allowed (not paused).
    #[must_use]
    pub fn check_trading_allowed(&self) -> RiskCheckResult {
        if let Some(reason) = self.trading_paused.read().clone() {
            return RiskCheckResult::Rejected(RiskError::TradingPaused { reason });
        }
        RiskCheckResult::Passed
    }

    // ========== Combined Checks ==========

    /// Performs all risk checks for an order.
    ///
    /// This is the main entry point for risk checking. It performs:
    /// 1. Trading pause check
    /// 2. Daily loss limit check
    /// 3. Order frequency check
    /// 4. Position limit check
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `order_qty` - The order quantity
    ///
    /// # Returns
    ///
    /// `RiskCheckResult::Passed` if all checks pass, otherwise the first rejection.
    #[must_use]
    pub fn check_order(&self, symbol: &Symbol, order_qty: Quantity) -> RiskCheckResult {
        // Check if trading is paused
        let pause_check = self.check_trading_allowed();
        if pause_check.is_rejected() {
            return pause_check;
        }

        // Check daily loss limit
        let loss_check = self.check_daily_loss_limit();
        if loss_check.is_rejected() {
            return loss_check;
        }

        // Check order frequency
        let freq_check = self.check_frequency_limit();
        if freq_check.is_rejected() {
            return freq_check;
        }

        // Check position limit
        let pos_check = self.check_position_limit(symbol, order_qty);
        if pos_check.is_rejected() {
            return pos_check;
        }

        RiskCheckResult::Passed
    }

    /// Performs risk checks and records the order if passed.
    ///
    /// This combines `check_order` with `record_order` for convenience.
    pub fn check_and_record_order(&self, symbol: &Symbol, order_qty: Quantity) -> RiskCheckResult {
        let result = self.check_order(symbol, order_qty);
        if result.is_passed() {
            self.record_order();
        }
        result
    }
}

impl std::fmt::Debug for RiskController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RiskController")
            .field("enabled", &self.is_enabled())
            .field("trading_paused", &self.is_trading_paused())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::FrequencyLimit;
    use rust_decimal_macros::dec;

    fn test_symbol() -> Symbol {
        Symbol::new("BTC-USDT").unwrap()
    }

    #[test]
    fn test_risk_controller_new() {
        let config = RiskConfig::default();
        let controller = RiskController::new(config);
        assert!(controller.is_enabled());
        assert!(!controller.is_trading_paused());
    }

    #[test]
    fn test_position_limit_check_passes() {
        let config = RiskConfig::new().with_max_position(Quantity::new(dec!(10)).unwrap());
        let controller = RiskController::new(config);

        let symbol = test_symbol();
        controller.update_position(&symbol, Quantity::new(dec!(5)).unwrap());

        let result = controller.check_position_limit(&symbol, Quantity::new(dec!(3)).unwrap());
        assert!(result.is_passed());
    }

    #[test]
    fn test_position_limit_check_fails() {
        let config = RiskConfig::new().with_max_position(Quantity::new(dec!(10)).unwrap());
        let controller = RiskController::new(config);

        let symbol = test_symbol();
        controller.update_position(&symbol, Quantity::new(dec!(8)).unwrap());

        let result = controller.check_position_limit(&symbol, Quantity::new(dec!(5)).unwrap());
        assert!(result.is_rejected());
        assert!(matches!(
            result.rejection_error(),
            Some(RiskError::PositionLimitExceeded { .. })
        ));
    }

    #[test]
    fn test_position_limit_with_short() {
        let config = RiskConfig::new().with_max_position(Quantity::new(dec!(10)).unwrap());
        let controller = RiskController::new(config);

        let symbol = test_symbol();
        // Short position of -5
        controller.update_position(&symbol, Quantity::new(dec!(-5)).unwrap());

        // Selling more should fail if it exceeds limit
        let result = controller.check_position_limit(&symbol, Quantity::new(dec!(-6)).unwrap());
        assert!(result.is_rejected());
    }

    #[test]
    fn test_frequency_limit_check() {
        // Set burst_allowance to 0 for predictable testing
        let mut freq_limit = FrequencyLimit::new(2, 100);
        freq_limit.burst_allowance = 0;
        let config = RiskConfig::new().with_frequency_limit(freq_limit);
        let controller = RiskController::new(config);

        // First order should pass
        assert!(controller.check_frequency_limit().is_passed());
        controller.record_order();

        // Second order should pass
        assert!(controller.check_frequency_limit().is_passed());
        controller.record_order();

        // Third order should fail (exceeds per-second limit of 2)
        let result = controller.check_frequency_limit();
        assert!(result.is_rejected());
    }

    #[test]
    fn test_daily_loss_limit() {
        let config = RiskConfig::new().with_daily_loss_limit(Amount::new(dec!(1000)).unwrap());
        let controller = RiskController::new(config);

        // No loss yet
        assert!(controller.check_daily_loss_limit().is_passed());

        // Update with a loss
        controller.update_daily_pnl(Amount::new(dec!(-500)).unwrap(), Amount::ZERO);
        assert!(controller.check_daily_loss_limit().is_passed());

        // Exceed the limit
        controller.update_daily_pnl(Amount::new(dec!(-1200)).unwrap(), Amount::ZERO);
        let result = controller.check_daily_loss_limit();
        assert!(result.is_rejected());
        assert!(matches!(
            result.rejection_error(),
            Some(RiskError::DailyLossLimitExceeded { .. })
        ));
    }

    #[test]
    fn test_trading_pause() {
        let config = RiskConfig::default();
        let controller = RiskController::new(config);

        assert!(!controller.is_trading_paused());
        assert!(controller.check_trading_allowed().is_passed());

        controller.pause_trading("Manual intervention");
        assert!(controller.is_trading_paused());
        assert_eq!(
            controller.pause_reason(),
            Some("Manual intervention".to_string())
        );

        let result = controller.check_trading_allowed();
        assert!(result.is_rejected());
        assert!(matches!(
            result.rejection_error(),
            Some(RiskError::TradingPaused { .. })
        ));

        controller.resume_trading();
        assert!(!controller.is_trading_paused());
        assert!(controller.check_trading_allowed().is_passed());
    }

    #[test]
    fn test_combined_check_order() {
        let config = RiskConfig::new()
            .with_max_position(Quantity::new(dec!(10)).unwrap())
            .with_daily_loss_limit(Amount::new(dec!(1000)).unwrap());
        let controller = RiskController::new(config);

        let symbol = test_symbol();

        // Should pass with no position
        let result = controller.check_order(&symbol, Quantity::new(dec!(5)).unwrap());
        assert!(result.is_passed());

        // Pause trading
        controller.pause_trading("Test");
        let result = controller.check_order(&symbol, Quantity::new(dec!(1)).unwrap());
        assert!(result.is_rejected());
        assert!(matches!(
            result.rejection_error(),
            Some(RiskError::TradingPaused { .. })
        ));
    }

    #[test]
    fn test_check_and_record_order() {
        let config = RiskConfig::new().with_frequency_limit(FrequencyLimit::new(5, 100));
        let controller = RiskController::new(config);

        let symbol = test_symbol();

        // Record several orders
        for _ in 0..5 {
            let result =
                controller.check_and_record_order(&symbol, Quantity::new(dec!(1)).unwrap());
            assert!(result.is_passed());
        }

        // Next should fail
        let result = controller.check_and_record_order(&symbol, Quantity::new(dec!(1)).unwrap());
        assert!(result.is_rejected());
    }

    #[test]
    fn test_disabled_controller() {
        let mut config = RiskConfig::new().with_max_position(Quantity::new(dec!(1)).unwrap());
        config.enabled = false;
        let controller = RiskController::new(config);

        let symbol = test_symbol();
        controller.update_position(&symbol, Quantity::new(dec!(100)).unwrap());

        // Should pass even though position exceeds limit
        let result = controller.check_position_limit(&symbol, Quantity::new(dec!(100)).unwrap());
        assert!(result.is_passed());
    }

    #[test]
    fn test_reset_daily_pnl() {
        let config = RiskConfig::new().with_daily_loss_limit(Amount::new(dec!(1000)).unwrap());
        let controller = RiskController::new(config);

        controller.update_daily_pnl(Amount::new(dec!(-1500)).unwrap(), Amount::ZERO);
        assert!(controller.check_daily_loss_limit().is_rejected());

        controller.reset_daily_pnl();
        assert!(controller.check_daily_loss_limit().is_passed());
    }
}
