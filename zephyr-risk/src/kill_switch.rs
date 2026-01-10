//! Kill Switch module for emergency trading controls.
//!
//! This module provides emergency trading controls:
//! - Emergency position liquidation
//! - Trading pause/halt functionality
//! - Automatic triggers based on risk thresholds
//!
//! # Example
//!
//! ```
//! use zephyr_risk::kill_switch::{KillSwitch, KillSwitchConfig, KillSwitchTrigger};
//! use rust_decimal_macros::dec;
//!
//! let config = KillSwitchConfig::default();
//! let kill_switch = KillSwitch::new(config);
//!
//! // Check if kill switch is active
//! assert!(!kill_switch.is_active());
//!
//! // Activate kill switch
//! kill_switch.activate("Emergency: Market crash detected");
//! assert!(kill_switch.is_active());
//! ```

#![allow(clippy::module_name_repetitions)]

use parking_lot::RwLock;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{error, info, warn};
use zephyr_core::types::{Amount, Symbol, Timestamp};

/// Kill switch state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KillSwitchState {
    /// Kill switch is inactive, trading is allowed
    Inactive,
    /// Kill switch is active, trading is halted
    Active,
    /// Kill switch is in liquidation mode
    Liquidating,
    /// Kill switch has completed liquidation
    Liquidated,
}

impl KillSwitchState {
    /// Returns true if trading is allowed.
    #[must_use]
    pub const fn allows_trading(&self) -> bool {
        matches!(self, Self::Inactive)
    }

    /// Returns true if the kill switch is engaged.
    #[must_use]
    pub const fn is_engaged(&self) -> bool {
        !matches!(self, Self::Inactive)
    }
}

/// Trigger type for automatic kill switch activation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KillSwitchTrigger {
    /// Manual activation
    Manual {
        /// Reason for manual activation
        reason: String,
        /// User who activated
        activated_by: Option<String>,
    },
    /// Daily loss limit exceeded
    DailyLossLimit {
        /// Current loss
        loss: Amount,
        /// Configured limit
        limit: Amount,
    },
    /// Drawdown limit exceeded
    DrawdownLimit {
        /// Current drawdown percentage
        drawdown: Decimal,
        /// Configured limit
        limit: Decimal,
    },
    /// Position limit exceeded
    PositionLimit {
        /// Symbol that exceeded limit
        symbol: Symbol,
        /// Current position value
        position: Amount,
        /// Configured limit
        limit: Amount,
    },
    /// Connectivity issues
    ConnectivityLoss {
        /// Exchange that lost connectivity
        exchange: String,
        /// Duration of outage in seconds
        duration_secs: u64,
    },
    /// Market volatility spike
    VolatilitySpike {
        /// Symbol with volatility spike
        symbol: Symbol,
        /// Current volatility
        volatility: Decimal,
        /// Normal volatility
        normal_volatility: Decimal,
    },
    /// System error
    SystemError {
        /// Error description
        error: String,
    },
}

/// Kill switch activation record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillSwitchActivation {
    /// Unique activation ID
    pub id: String,
    /// Trigger that caused activation
    pub trigger: KillSwitchTrigger,
    /// Timestamp of activation
    pub activated_at: Timestamp,
    /// Timestamp of deactivation (if deactivated)
    pub deactivated_at: Option<Timestamp>,
    /// State at activation
    pub state: KillSwitchState,
    /// Notes or comments
    pub notes: Option<String>,
}

/// Liquidation order for emergency position closure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationOrder {
    /// Symbol to liquidate
    pub symbol: Symbol,
    /// Quantity to liquidate (positive for long, negative for short)
    pub quantity: Decimal,
    /// Target price (None for market order)
    pub target_price: Option<Decimal>,
    /// Priority (lower = higher priority)
    pub priority: u32,
    /// Status of the liquidation
    pub status: LiquidationStatus,
    /// Created timestamp
    pub created_at: Timestamp,
    /// Completed timestamp
    pub completed_at: Option<Timestamp>,
}

/// Status of a liquidation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiquidationStatus {
    /// Pending execution
    Pending,
    /// Currently executing
    Executing,
    /// Partially filled
    PartiallyFilled,
    /// Fully completed
    Completed,
    /// Failed to execute
    Failed,
    /// Cancelled
    Cancelled,
}

/// Kill switch configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KillSwitchConfig {
    /// Whether automatic triggers are enabled
    pub auto_triggers_enabled: bool,
    /// Daily loss limit for automatic trigger
    pub daily_loss_trigger: Option<Amount>,
    /// Drawdown limit for automatic trigger (as decimal, e.g., 0.20 for 20%)
    pub drawdown_trigger: Option<Decimal>,
    /// Connectivity loss duration trigger (seconds)
    pub connectivity_loss_trigger_secs: Option<u64>,
    /// Volatility spike multiplier trigger
    pub volatility_spike_trigger: Option<Decimal>,
    /// Whether to auto-liquidate on activation
    pub auto_liquidate: bool,
    /// Liquidation strategy
    pub liquidation_strategy: LiquidationStrategy,
    /// Cooldown period before allowing reactivation (seconds)
    pub cooldown_secs: u64,
    /// Whether to require manual confirmation for deactivation
    pub require_manual_deactivation: bool,
}

impl Default for KillSwitchConfig {
    fn default() -> Self {
        Self {
            auto_triggers_enabled: true,
            daily_loss_trigger: None,
            drawdown_trigger: Some(Decimal::new(20, 2)), // 20%
            connectivity_loss_trigger_secs: Some(60),
            volatility_spike_trigger: Some(Decimal::new(300, 2)), // 3x normal
            auto_liquidate: false,
            liquidation_strategy: LiquidationStrategy::default(),
            cooldown_secs: 300, // 5 minutes
            require_manual_deactivation: true,
        }
    }
}

/// Liquidation strategy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationStrategy {
    /// Order type for liquidation
    pub order_type: LiquidationOrderType,
    /// Maximum slippage allowed (as decimal)
    pub max_slippage: Decimal,
    /// Time limit for liquidation (seconds)
    pub time_limit_secs: u64,
    /// Whether to liquidate in batches
    pub use_batching: bool,
    /// Batch size as percentage of position
    pub batch_size_percent: Decimal,
    /// Delay between batches (milliseconds)
    pub batch_delay_ms: u64,
}

impl Default for LiquidationStrategy {
    fn default() -> Self {
        Self {
            order_type: LiquidationOrderType::Market,
            max_slippage: Decimal::new(2, 2), // 2%
            time_limit_secs: 300,             // 5 minutes
            use_batching: true,
            batch_size_percent: Decimal::new(25, 2), // 25% per batch
            batch_delay_ms: 1000,                    // 1 second
        }
    }
}

/// Order type for liquidation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LiquidationOrderType {
    /// Market order for immediate execution
    #[default]
    Market,
    /// Limit order with slippage tolerance
    LimitWithSlippage,
    /// TWAP (Time-Weighted Average Price)
    Twap,
}

/// Position to be liquidated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionToLiquidate {
    /// Symbol
    pub symbol: Symbol,
    /// Current quantity (positive for long, negative for short)
    pub quantity: Decimal,
    /// Current market value
    pub market_value: Amount,
    /// Current price
    pub current_price: Decimal,
}

/// Kill switch for emergency trading controls.
pub struct KillSwitch {
    /// Configuration
    config: RwLock<KillSwitchConfig>,
    /// Current state
    state: RwLock<KillSwitchState>,
    /// Whether the kill switch is active (atomic for fast checks)
    active: AtomicBool,
    /// Current activation record
    current_activation: RwLock<Option<KillSwitchActivation>>,
    /// Activation history
    activation_history: RwLock<Vec<KillSwitchActivation>>,
    /// Pending liquidation orders
    liquidation_orders: RwLock<Vec<LiquidationOrder>>,
    /// Last deactivation timestamp (for cooldown)
    last_deactivation: RwLock<Option<Timestamp>>,
}

impl KillSwitch {
    /// Creates a new kill switch with the given configuration.
    #[must_use]
    pub fn new(config: KillSwitchConfig) -> Self {
        Self {
            config: RwLock::new(config),
            state: RwLock::new(KillSwitchState::Inactive),
            active: AtomicBool::new(false),
            current_activation: RwLock::new(None),
            activation_history: RwLock::new(Vec::new()),
            liquidation_orders: RwLock::new(Vec::new()),
            last_deactivation: RwLock::new(None),
        }
    }

    /// Creates a new kill switch wrapped in an Arc.
    #[must_use]
    pub fn new_shared(config: KillSwitchConfig) -> Arc<Self> {
        Arc::new(Self::new(config))
    }

    /// Returns true if the kill switch is active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Returns the current state.
    #[must_use]
    pub fn state(&self) -> KillSwitchState {
        *self.state.read()
    }

    /// Returns true if trading is allowed.
    #[must_use]
    pub fn allows_trading(&self) -> bool {
        !self.is_active()
    }

    /// Activates the kill switch with a manual trigger.
    pub fn activate(&self, reason: impl Into<String>) {
        let trigger = KillSwitchTrigger::Manual {
            reason: reason.into(),
            activated_by: None,
        };
        self.activate_with_trigger(trigger);
    }

    /// Activates the kill switch with a specific trigger.
    pub fn activate_with_trigger(&self, trigger: KillSwitchTrigger) {
        // Check cooldown
        if let Some(last) = *self.last_deactivation.read() {
            let config = self.config.read();
            let cooldown_ms = config.cooldown_secs * 1000;
            let elapsed = Timestamp::now().as_millis() - last.as_millis();
            if elapsed < cooldown_ms as i64 {
                warn!(
                    "Kill switch activation blocked by cooldown. {} seconds remaining.",
                    (cooldown_ms as i64 - elapsed) / 1000
                );
                return;
            }
        }

        let activation = KillSwitchActivation {
            id: generate_id(),
            trigger: trigger.clone(),
            activated_at: Timestamp::now(),
            deactivated_at: None,
            state: KillSwitchState::Active,
            notes: None,
        };

        error!(
            trigger = ?trigger,
            "KILL SWITCH ACTIVATED"
        );

        *self.state.write() = KillSwitchState::Active;
        self.active.store(true, Ordering::SeqCst);
        *self.current_activation.write() = Some(activation);
    }

    /// Deactivates the kill switch.
    ///
    /// Returns true if deactivation was successful.
    pub fn deactivate(&self) -> bool {
        let config = self.config.read();
        if config.require_manual_deactivation {
            // In a real system, this would require additional authentication
            info!("Kill switch deactivation requires manual confirmation");
        }

        let mut current = self.current_activation.write();
        if let Some(ref mut activation) = *current {
            activation.deactivated_at = Some(Timestamp::now());
            self.activation_history.write().push(activation.clone());
        }

        *self.state.write() = KillSwitchState::Inactive;
        self.active.store(false, Ordering::SeqCst);
        *current = None;
        *self.last_deactivation.write() = Some(Timestamp::now());

        info!("Kill switch deactivated");
        true
    }

    /// Checks automatic triggers and activates if necessary.
    ///
    /// Returns true if the kill switch was activated.
    pub fn check_triggers(
        &self,
        daily_loss: Amount,
        drawdown: Decimal,
        connectivity_loss_secs: Option<u64>,
    ) -> bool {
        if self.is_active() {
            return false;
        }

        let (auto_enabled, daily_limit, drawdown_limit, conn_limit) = {
            let config = self.config.read();
            if !config.auto_triggers_enabled {
                return false;
            }
            (
                config.auto_triggers_enabled,
                config.daily_loss_trigger,
                config.drawdown_trigger,
                config.connectivity_loss_trigger_secs,
            )
        };

        if !auto_enabled {
            return false;
        }

        // Check daily loss trigger
        if let Some(limit) = daily_limit {
            if daily_loss.as_decimal().abs() > limit.as_decimal() {
                self.activate_with_trigger(KillSwitchTrigger::DailyLossLimit {
                    loss: daily_loss,
                    limit,
                });
                return true;
            }
        }

        // Check drawdown trigger
        if let Some(limit) = drawdown_limit {
            if drawdown.abs() > limit {
                self.activate_with_trigger(KillSwitchTrigger::DrawdownLimit { drawdown, limit });
                return true;
            }
        }

        // Check connectivity loss trigger
        if let (Some(duration), Some(limit)) = (connectivity_loss_secs, conn_limit) {
            if duration >= limit {
                self.activate_with_trigger(KillSwitchTrigger::ConnectivityLoss {
                    exchange: "unknown".to_string(),
                    duration_secs: duration,
                });
                return true;
            }
        }

        false
    }

    /// Initiates emergency liquidation of all positions.
    pub fn initiate_liquidation(&self, positions: Vec<PositionToLiquidate>) {
        if !self.is_active() {
            warn!("Cannot initiate liquidation when kill switch is not active");
            return;
        }

        *self.state.write() = KillSwitchState::Liquidating;

        let config = self.config.read();
        let strategy = &config.liquidation_strategy;

        let mut orders: Vec<LiquidationOrder> = positions
            .into_iter()
            .enumerate()
            .map(|(i, pos)| {
                let target_price = match strategy.order_type {
                    LiquidationOrderType::Market => None,
                    LiquidationOrderType::LimitWithSlippage => {
                        let slippage_factor = if pos.quantity > Decimal::ZERO {
                            Decimal::ONE - strategy.max_slippage
                        } else {
                            Decimal::ONE + strategy.max_slippage
                        };
                        Some(pos.current_price * slippage_factor)
                    }
                    LiquidationOrderType::Twap => None,
                };

                LiquidationOrder {
                    symbol: pos.symbol,
                    quantity: -pos.quantity, // Reverse the position
                    target_price,
                    priority: i as u32,
                    status: LiquidationStatus::Pending,
                    created_at: Timestamp::now(),
                    completed_at: None,
                }
            })
            .collect();

        // Sort by priority
        orders.sort_by_key(|o| o.priority);

        error!(count = orders.len(), "Initiating emergency liquidation");

        *self.liquidation_orders.write() = orders;
    }

    /// Gets the current liquidation orders.
    #[must_use]
    pub fn liquidation_orders(&self) -> Vec<LiquidationOrder> {
        self.liquidation_orders.read().clone()
    }

    /// Updates the status of a liquidation order.
    pub fn update_liquidation_status(&self, symbol: &Symbol, status: LiquidationStatus) {
        let mut orders = self.liquidation_orders.write();
        if let Some(order) = orders.iter_mut().find(|o| &o.symbol == symbol) {
            order.status = status;
            if matches!(
                status,
                LiquidationStatus::Completed | LiquidationStatus::Failed
            ) {
                order.completed_at = Some(Timestamp::now());
            }
        }

        // Check if all liquidations are complete
        let all_complete = orders.iter().all(|o| {
            matches!(
                o.status,
                LiquidationStatus::Completed
                    | LiquidationStatus::Failed
                    | LiquidationStatus::Cancelled
            )
        });

        if all_complete {
            drop(orders);
            *self.state.write() = KillSwitchState::Liquidated;
            info!("All liquidation orders completed");
        }
    }

    /// Gets the current activation record.
    #[must_use]
    pub fn current_activation(&self) -> Option<KillSwitchActivation> {
        self.current_activation.read().clone()
    }

    /// Gets the activation history.
    #[must_use]
    pub fn activation_history(&self) -> Vec<KillSwitchActivation> {
        self.activation_history.read().clone()
    }

    /// Updates the configuration.
    pub fn update_config(&self, config: KillSwitchConfig) {
        *self.config.write() = config;
    }

    /// Gets the current configuration.
    #[must_use]
    pub fn config(&self) -> KillSwitchConfig {
        self.config.read().clone()
    }
}

impl Default for KillSwitch {
    fn default() -> Self {
        Self::new(KillSwitchConfig::default())
    }
}

impl std::fmt::Debug for KillSwitch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KillSwitch")
            .field("active", &self.is_active())
            .field("state", &self.state())
            .finish()
    }
}

/// Generates a simple unique ID.
fn generate_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("ks-{:016x}", timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn test_symbol() -> Symbol {
        Symbol::new("BTC-USDT").unwrap()
    }

    #[test]
    fn test_kill_switch_default_inactive() {
        let ks = KillSwitch::default();
        assert!(!ks.is_active());
        assert!(ks.allows_trading());
        assert_eq!(ks.state(), KillSwitchState::Inactive);
    }

    #[test]
    fn test_kill_switch_activate() {
        let ks = KillSwitch::default();

        ks.activate("Test activation");

        assert!(ks.is_active());
        assert!(!ks.allows_trading());
        assert_eq!(ks.state(), KillSwitchState::Active);

        let activation = ks.current_activation().unwrap();
        assert!(matches!(
            activation.trigger,
            KillSwitchTrigger::Manual { .. }
        ));
    }

    #[test]
    fn test_kill_switch_deactivate() {
        let mut config = KillSwitchConfig::default();
        config.require_manual_deactivation = false;
        config.cooldown_secs = 0;
        let ks = KillSwitch::new(config);

        ks.activate("Test");
        assert!(ks.is_active());

        let result = ks.deactivate();
        assert!(result);
        assert!(!ks.is_active());
        assert_eq!(ks.state(), KillSwitchState::Inactive);

        // Check history
        let history = ks.activation_history();
        assert_eq!(history.len(), 1);
        assert!(history[0].deactivated_at.is_some());
    }

    #[test]
    fn test_auto_trigger_daily_loss() {
        let config = KillSwitchConfig {
            auto_triggers_enabled: true,
            daily_loss_trigger: Some(Amount::new(dec!(1000)).unwrap()),
            cooldown_secs: 0,
            ..Default::default()
        };
        let ks = KillSwitch::new(config);

        // Loss below threshold
        let triggered = ks.check_triggers(Amount::new(dec!(-500)).unwrap(), dec!(0), None);
        assert!(!triggered);
        assert!(!ks.is_active());

        // Loss above threshold
        let triggered = ks.check_triggers(Amount::new(dec!(-1500)).unwrap(), dec!(0), None);
        assert!(triggered);
        assert!(ks.is_active());
    }

    #[test]
    fn test_auto_trigger_drawdown() {
        let config = KillSwitchConfig {
            auto_triggers_enabled: true,
            drawdown_trigger: Some(dec!(0.15)), // 15%
            cooldown_secs: 0,
            ..Default::default()
        };
        let ks = KillSwitch::new(config);

        // Drawdown below threshold
        let triggered = ks.check_triggers(Amount::ZERO, dec!(0.10), None);
        assert!(!triggered);

        // Drawdown above threshold
        let triggered = ks.check_triggers(Amount::ZERO, dec!(0.20), None);
        assert!(triggered);
        assert!(ks.is_active());
    }

    #[test]
    fn test_auto_trigger_connectivity() {
        let config = KillSwitchConfig {
            auto_triggers_enabled: true,
            connectivity_loss_trigger_secs: Some(30),
            cooldown_secs: 0,
            ..Default::default()
        };
        let ks = KillSwitch::new(config);

        // Short outage
        let triggered = ks.check_triggers(Amount::ZERO, dec!(0), Some(10));
        assert!(!triggered);

        // Long outage
        let triggered = ks.check_triggers(Amount::ZERO, dec!(0), Some(60));
        assert!(triggered);
        assert!(ks.is_active());
    }

    #[test]
    fn test_liquidation_initiation() {
        let ks = KillSwitch::default();
        ks.activate("Test");

        let positions = vec![PositionToLiquidate {
            symbol: test_symbol(),
            quantity: dec!(1.0),
            market_value: Amount::new(dec!(50000)).unwrap(),
            current_price: dec!(50000),
        }];

        ks.initiate_liquidation(positions);

        assert_eq!(ks.state(), KillSwitchState::Liquidating);
        let orders = ks.liquidation_orders();
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].quantity, dec!(-1.0)); // Reversed
    }

    #[test]
    fn test_liquidation_status_update() {
        let ks = KillSwitch::default();
        ks.activate("Test");

        let positions = vec![PositionToLiquidate {
            symbol: test_symbol(),
            quantity: dec!(1.0),
            market_value: Amount::new(dec!(50000)).unwrap(),
            current_price: dec!(50000),
        }];

        ks.initiate_liquidation(positions);

        ks.update_liquidation_status(&test_symbol(), LiquidationStatus::Completed);

        let orders = ks.liquidation_orders();
        assert_eq!(orders[0].status, LiquidationStatus::Completed);
        assert!(orders[0].completed_at.is_some());
        assert_eq!(ks.state(), KillSwitchState::Liquidated);
    }

    #[test]
    fn test_kill_switch_state_allows_trading() {
        assert!(KillSwitchState::Inactive.allows_trading());
        assert!(!KillSwitchState::Active.allows_trading());
        assert!(!KillSwitchState::Liquidating.allows_trading());
        assert!(!KillSwitchState::Liquidated.allows_trading());
    }

    #[test]
    fn test_liquidation_strategy_default() {
        let strategy = LiquidationStrategy::default();
        assert_eq!(strategy.order_type, LiquidationOrderType::Market);
        assert_eq!(strategy.max_slippage, dec!(0.02));
        assert!(strategy.use_batching);
    }

    #[test]
    fn test_kill_switch_config_serde() {
        let config = KillSwitchConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: KillSwitchConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.auto_triggers_enabled, parsed.auto_triggers_enabled);
        assert_eq!(config.cooldown_secs, parsed.cooldown_secs);
    }
}
