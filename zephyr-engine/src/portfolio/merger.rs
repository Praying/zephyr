//! Signal merging and self-trading prevention.
//!
//! This module provides signal merging functionality for combining
//! signals from multiple strategies while preventing self-trading.

#![allow(clippy::disallowed_types)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::unused_self)]

use std::collections::HashMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use zephyr_core::data::OrderSide;
use zephyr_core::types::{Quantity, Symbol, Timestamp};

use super::{Portfolio, StrategyId};
use crate::StrategySignal;

/// Configuration for signal merging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeConfig {
    /// Minimum position change to generate an execution signal.
    /// Prevents tiny position adjustments.
    pub min_position_change: Decimal,
    /// Whether to apply position multipliers from strategy allocations.
    #[serde(default = "default_true")]
    pub apply_multipliers: bool,
    /// Whether to enable self-trading prevention.
    #[serde(default = "default_true")]
    pub prevent_self_trading: bool,
    /// Maximum time window for signal aggregation (milliseconds).
    #[serde(default = "default_signal_window")]
    pub signal_window_ms: u64,
}

fn default_true() -> bool {
    true
}

fn default_signal_window() -> u64 {
    1000 // 1 second
}

impl Default for MergeConfig {
    fn default() -> Self {
        Self {
            min_position_change: Decimal::new(1, 8), // 0.00000001
            apply_multipliers: true,
            prevent_self_trading: true,
            signal_window_ms: 1000,
        }
    }
}

/// Execution signal - the result of merging strategy signals.
///
/// Represents a position change that should be executed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSignal {
    /// Symbol for the position change.
    pub symbol: Symbol,
    /// Current position before the change.
    pub current_position: Quantity,
    /// Target position after the change.
    pub target_position: Quantity,
    /// Order quantity needed (absolute value).
    pub order_quantity: Quantity,
    /// Order side (Buy to increase, Sell to decrease).
    pub order_side: OrderSide,
    /// Contributing strategies and their individual targets.
    pub strategy_contributions: HashMap<StrategyId, Quantity>,
    /// Timestamp when the signal was generated.
    pub timestamp: Timestamp,
    /// Tag for tracking.
    pub tag: String,
}

impl ExecutionSignal {
    /// Returns true if no order is needed.
    #[must_use]
    pub fn is_no_change(&self) -> bool {
        self.order_quantity.is_zero()
    }

    /// Returns the signed quantity change.
    #[must_use]
    pub fn signed_quantity(&self) -> Quantity {
        match self.order_side {
            OrderSide::Buy => self.order_quantity,
            OrderSide::Sell => -self.order_quantity,
        }
    }
}

/// Signal merger for combining strategy signals.
///
/// Implements the "1" in the M+1+N architecture - merging signals
/// from M strategies before sending to N execution channels.
pub struct SignalMerger {
    /// Merge configuration.
    config: MergeConfig,
    /// Current positions per symbol (actual positions).
    current_positions: HashMap<Symbol, Quantity>,
    /// Theoretical positions per strategy per symbol.
    theoretical_positions: HashMap<StrategyId, HashMap<Symbol, Quantity>>,
    /// Pending signals within the aggregation window.
    pending_signals: Vec<StrategySignal>,
    /// Last merge timestamp.
    last_merge: Option<Timestamp>,
}

impl SignalMerger {
    /// Creates a new signal merger with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(MergeConfig::default())
    }

    /// Creates a new signal merger with custom configuration.
    #[must_use]
    pub fn with_config(config: MergeConfig) -> Self {
        Self {
            config,
            current_positions: HashMap::new(),
            theoretical_positions: HashMap::new(),
            pending_signals: Vec::new(),
            last_merge: None,
        }
    }

    /// Updates the current actual position for a symbol.
    pub fn update_current_position(&mut self, symbol: &Symbol, quantity: Quantity) {
        debug!(symbol = %symbol, quantity = %quantity, "Updating current position");
        self.current_positions.insert(symbol.clone(), quantity);
    }

    /// Gets the current actual position for a symbol.
    #[must_use]
    pub fn get_current_position(&self, symbol: &Symbol) -> Quantity {
        self.current_positions
            .get(symbol)
            .copied()
            .unwrap_or(Quantity::ZERO)
    }

    /// Gets the theoretical position for a strategy and symbol.
    #[must_use]
    pub fn get_theoretical_position(&self, strategy_id: &StrategyId, symbol: &Symbol) -> Quantity {
        self.theoretical_positions
            .get(strategy_id)
            .and_then(|m| m.get(symbol))
            .copied()
            .unwrap_or(Quantity::ZERO)
    }

    /// Adds a signal to be merged.
    pub fn add_signal(&mut self, signal: StrategySignal) {
        debug!(
            strategy = %signal.strategy_name,
            symbol = %signal.symbol,
            target = %signal.target_position,
            "Adding signal for merge"
        );
        self.pending_signals.push(signal);
    }

    /// Merges pending signals and returns execution signals.
    ///
    /// This is the core merging logic that:
    /// 1. Groups signals by symbol
    /// 2. Applies position multipliers
    /// 3. Calculates merged target positions
    /// 4. Prevents self-trading by netting opposing positions
    /// 5. Generates execution signals for net position changes
    pub fn merge_signals(&mut self, portfolio: &Portfolio) -> Vec<ExecutionSignal> {
        if self.pending_signals.is_empty() {
            return Vec::new();
        }

        let now = Timestamp::now();
        self.last_merge = Some(now);

        // Group signals by symbol
        let mut signals_by_symbol: HashMap<Symbol, Vec<&StrategySignal>> = HashMap::new();
        for signal in &self.pending_signals {
            signals_by_symbol
                .entry(signal.symbol.clone())
                .or_default()
                .push(signal);
        }

        let mut execution_signals = Vec::new();

        for (symbol, signals) in signals_by_symbol {
            let exec_signal = self.merge_symbol_signals(portfolio, &symbol, &signals);

            if let Some(signal) = exec_signal {
                if !signal.is_no_change() {
                    info!(
                        symbol = %symbol,
                        current = %signal.current_position,
                        target = %signal.target_position,
                        order_qty = %signal.order_quantity,
                        side = %signal.order_side,
                        "Generated execution signal"
                    );
                    execution_signals.push(signal);
                }
            }
        }

        // Update theoretical positions from signals
        for signal in &self.pending_signals {
            let strategy_id = StrategyId::new(&signal.strategy_name);
            self.theoretical_positions
                .entry(strategy_id)
                .or_default()
                .insert(signal.symbol.clone(), signal.target_position);
        }

        // Clear pending signals
        self.pending_signals.clear();

        execution_signals
    }

    /// Merges signals for a single symbol.
    fn merge_symbol_signals(
        &self,
        portfolio: &Portfolio,
        symbol: &Symbol,
        signals: &[&StrategySignal],
    ) -> Option<ExecutionSignal> {
        let current_position = self.get_current_position(symbol);
        let mut strategy_contributions: HashMap<StrategyId, Quantity> = HashMap::new();
        let mut merged_target = Quantity::ZERO;

        for signal in signals {
            let strategy_id = StrategyId::new(&signal.strategy_name);

            // Get strategy allocation for multiplier
            let multiplier = if self.config.apply_multipliers {
                portfolio
                    .get_strategy(&strategy_id)
                    .map(|a| a.effective_multiplier())
                    .unwrap_or(Decimal::ONE)
            } else {
                Decimal::ONE
            };

            // Apply multiplier to target position
            let scaled_target =
                Quantity::new_unchecked(signal.target_position.as_decimal() * multiplier);

            strategy_contributions.insert(strategy_id, scaled_target);
            merged_target = merged_target + scaled_target;
        }

        // Self-trading prevention: check if we have opposing signals
        if self.config.prevent_self_trading {
            let (long_total, short_total) =
                self.calculate_opposing_positions(&strategy_contributions);

            if !long_total.is_zero() && !short_total.is_zero() {
                warn!(
                    symbol = %symbol,
                    long = %long_total,
                    short = %short_total,
                    "Detected opposing positions - netting for self-trading prevention"
                );
                // The merged_target already represents the net position
            }
        }

        // Calculate position change
        let diff = merged_target - current_position;
        let abs_diff = diff.abs();

        // Check minimum position change threshold
        if abs_diff.as_decimal() < self.config.min_position_change {
            debug!(
                symbol = %symbol,
                diff = %abs_diff,
                threshold = %self.config.min_position_change,
                "Position change below threshold, skipping"
            );
            return None;
        }

        let (order_quantity, order_side) = if diff.is_positive() {
            (diff, OrderSide::Buy)
        } else if diff.is_negative() {
            (-diff, OrderSide::Sell)
        } else {
            return None;
        };

        Some(ExecutionSignal {
            symbol: symbol.clone(),
            current_position,
            target_position: merged_target,
            order_quantity,
            order_side,
            strategy_contributions,
            timestamp: Timestamp::now(),
            tag: "merged".to_string(),
        })
    }

    /// Calculates opposing (long vs short) position totals.
    fn calculate_opposing_positions(
        &self,
        contributions: &HashMap<StrategyId, Quantity>,
    ) -> (Quantity, Quantity) {
        let mut long_total = Quantity::ZERO;
        let mut short_total = Quantity::ZERO;

        for qty in contributions.values() {
            if qty.is_positive() {
                long_total = long_total + *qty;
            } else if qty.is_negative() {
                short_total = short_total + qty.abs();
            }
        }

        (long_total, short_total)
    }

    /// Clears all state.
    pub fn clear(&mut self) {
        self.current_positions.clear();
        self.theoretical_positions.clear();
        self.pending_signals.clear();
        self.last_merge = None;
    }

    /// Removes a strategy from tracking.
    pub fn remove_strategy(&mut self, strategy_id: &StrategyId) {
        self.theoretical_positions.remove(strategy_id);
    }

    /// Returns the number of pending signals.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending_signals.len()
    }

    /// Returns all symbols with positions.
    #[must_use]
    pub fn symbols(&self) -> Vec<Symbol> {
        self.current_positions.keys().cloned().collect()
    }
}

impl Default for SignalMerger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::portfolio::StrategyAllocation;
    use rust_decimal_macros::dec;

    fn create_test_portfolio() -> Portfolio {
        Portfolio::builder()
            .name("Test Portfolio")
            .add_strategy(StrategyAllocation::new(
                StrategyId::new("s1"),
                dec!(0.5),
                dec!(1.0),
            ))
            .add_strategy(StrategyAllocation::new(
                StrategyId::new("s2"),
                dec!(0.5),
                dec!(2.0), // 2x multiplier
            ))
            .build()
            .unwrap()
    }

    fn create_signal(strategy: &str, symbol: &str, qty: Decimal) -> StrategySignal {
        StrategySignal {
            strategy_name: strategy.to_string(),
            symbol: Symbol::new(symbol).unwrap(),
            target_position: Quantity::new(qty).unwrap(),
            tag: "test".to_string(),
            timestamp: Timestamp::now(),
        }
    }

    #[test]
    fn test_merge_config_default() {
        let config = MergeConfig::default();
        assert!(config.apply_multipliers);
        assert!(config.prevent_self_trading);
        assert_eq!(config.signal_window_ms, 1000);
    }

    #[test]
    fn test_signal_merger_new() {
        let merger = SignalMerger::new();
        assert_eq!(merger.pending_count(), 0);
    }

    #[test]
    fn test_signal_merger_add_signal() {
        let mut merger = SignalMerger::new();
        merger.add_signal(create_signal("s1", "BTC-USDT", dec!(1.0)));
        assert_eq!(merger.pending_count(), 1);
    }

    #[test]
    fn test_signal_merger_current_position() {
        let mut merger = SignalMerger::new();
        let symbol = Symbol::new("BTC-USDT").unwrap();

        assert_eq!(merger.get_current_position(&symbol), Quantity::ZERO);

        merger.update_current_position(&symbol, Quantity::new(dec!(5.0)).unwrap());
        assert_eq!(merger.get_current_position(&symbol).as_decimal(), dec!(5.0));
    }

    #[test]
    fn test_signal_merger_single_strategy() {
        let mut merger = SignalMerger::new();
        let portfolio = create_test_portfolio();
        let symbol = Symbol::new("BTC-USDT").unwrap();

        // Current position: 0
        merger.add_signal(create_signal("s1", "BTC-USDT", dec!(1.0)));

        let signals = merger.merge_signals(&portfolio);
        assert_eq!(signals.len(), 1);

        let signal = &signals[0];
        assert_eq!(signal.symbol, symbol);
        assert_eq!(signal.current_position, Quantity::ZERO);
        assert_eq!(signal.target_position.as_decimal(), dec!(1.0));
        assert_eq!(signal.order_quantity.as_decimal(), dec!(1.0));
        assert_eq!(signal.order_side, OrderSide::Buy);
    }

    #[test]
    fn test_signal_merger_multiple_strategies_same_direction() {
        let mut merger = SignalMerger::new();
        let portfolio = create_test_portfolio();

        // s1: target 1.0 (multiplier 1.0) = 1.0
        // s2: target 1.0 (multiplier 2.0) = 2.0
        // Total: 3.0
        merger.add_signal(create_signal("s1", "BTC-USDT", dec!(1.0)));
        merger.add_signal(create_signal("s2", "BTC-USDT", dec!(1.0)));

        let signals = merger.merge_signals(&portfolio);
        assert_eq!(signals.len(), 1);

        let signal = &signals[0];
        assert_eq!(signal.target_position.as_decimal(), dec!(3.0));
        assert_eq!(signal.order_quantity.as_decimal(), dec!(3.0));
        assert_eq!(signal.order_side, OrderSide::Buy);
    }

    #[test]
    fn test_signal_merger_opposing_positions_netting() {
        let mut merger = SignalMerger::new();
        let portfolio = create_test_portfolio();

        // s1: target 2.0 (multiplier 1.0) = 2.0 (long)
        // s2: target -1.0 (multiplier 2.0) = -2.0 (short)
        // Net: 0.0
        merger.add_signal(create_signal("s1", "BTC-USDT", dec!(2.0)));
        merger.add_signal(create_signal("s2", "BTC-USDT", dec!(-1.0)));

        let signals = merger.merge_signals(&portfolio);

        // Net position is 0, so no signal should be generated
        assert!(signals.is_empty() || signals[0].is_no_change());
    }

    #[test]
    fn test_signal_merger_partial_netting() {
        let mut merger = SignalMerger::new();
        let portfolio = create_test_portfolio();

        // s1: target 3.0 (multiplier 1.0) = 3.0 (long)
        // s2: target -1.0 (multiplier 2.0) = -2.0 (short)
        // Net: 1.0 (long)
        merger.add_signal(create_signal("s1", "BTC-USDT", dec!(3.0)));
        merger.add_signal(create_signal("s2", "BTC-USDT", dec!(-1.0)));

        let signals = merger.merge_signals(&portfolio);
        assert_eq!(signals.len(), 1);

        let signal = &signals[0];
        assert_eq!(signal.target_position.as_decimal(), dec!(1.0));
        assert_eq!(signal.order_side, OrderSide::Buy);
    }

    #[test]
    fn test_signal_merger_with_existing_position() {
        let mut merger = SignalMerger::new();
        let portfolio = create_test_portfolio();
        let symbol = Symbol::new("BTC-USDT").unwrap();

        // Current position: 2.0
        merger.update_current_position(&symbol, Quantity::new(dec!(2.0)).unwrap());

        // Target: 5.0
        merger.add_signal(create_signal("s1", "BTC-USDT", dec!(5.0)));

        let signals = merger.merge_signals(&portfolio);
        assert_eq!(signals.len(), 1);

        let signal = &signals[0];
        assert_eq!(signal.current_position.as_decimal(), dec!(2.0));
        assert_eq!(signal.target_position.as_decimal(), dec!(5.0));
        assert_eq!(signal.order_quantity.as_decimal(), dec!(3.0)); // 5 - 2 = 3
        assert_eq!(signal.order_side, OrderSide::Buy);
    }

    #[test]
    fn test_signal_merger_reduce_position() {
        let mut merger = SignalMerger::new();
        let portfolio = create_test_portfolio();
        let symbol = Symbol::new("BTC-USDT").unwrap();

        // Current position: 5.0
        merger.update_current_position(&symbol, Quantity::new(dec!(5.0)).unwrap());

        // Target: 2.0
        merger.add_signal(create_signal("s1", "BTC-USDT", dec!(2.0)));

        let signals = merger.merge_signals(&portfolio);
        assert_eq!(signals.len(), 1);

        let signal = &signals[0];
        assert_eq!(signal.order_quantity.as_decimal(), dec!(3.0)); // 5 - 2 = 3
        assert_eq!(signal.order_side, OrderSide::Sell);
    }

    #[test]
    fn test_signal_merger_below_threshold() {
        let config = MergeConfig {
            min_position_change: dec!(0.1),
            ..Default::default()
        };
        let mut merger = SignalMerger::with_config(config);
        let portfolio = create_test_portfolio();

        // Very small position change
        merger.add_signal(create_signal("s1", "BTC-USDT", dec!(0.01)));

        let signals = merger.merge_signals(&portfolio);
        assert!(signals.is_empty());
    }

    #[test]
    fn test_signal_merger_multiple_symbols() {
        let mut merger = SignalMerger::new();
        let portfolio = create_test_portfolio();

        merger.add_signal(create_signal("s1", "BTC-USDT", dec!(1.0)));
        merger.add_signal(create_signal("s1", "ETH-USDT", dec!(5.0)));

        let signals = merger.merge_signals(&portfolio);
        assert_eq!(signals.len(), 2);

        let btc_signal = signals
            .iter()
            .find(|s| s.symbol.as_str() == "BTC-USDT")
            .unwrap();
        let eth_signal = signals
            .iter()
            .find(|s| s.symbol.as_str() == "ETH-USDT")
            .unwrap();

        assert_eq!(btc_signal.target_position.as_decimal(), dec!(1.0));
        assert_eq!(eth_signal.target_position.as_decimal(), dec!(5.0));
    }

    #[test]
    fn test_signal_merger_without_multipliers() {
        let config = MergeConfig {
            apply_multipliers: false,
            ..Default::default()
        };
        let mut merger = SignalMerger::with_config(config);
        let portfolio = create_test_portfolio();

        // s2 has 2x multiplier but we disabled it
        merger.add_signal(create_signal("s2", "BTC-USDT", dec!(1.0)));

        let signals = merger.merge_signals(&portfolio);
        assert_eq!(signals.len(), 1);

        // Without multiplier, target should be 1.0 (not 2.0)
        assert_eq!(signals[0].target_position.as_decimal(), dec!(1.0));
    }

    #[test]
    fn test_signal_merger_clear() {
        let mut merger = SignalMerger::new();
        let symbol = Symbol::new("BTC-USDT").unwrap();

        merger.update_current_position(&symbol, Quantity::new(dec!(1.0)).unwrap());
        merger.add_signal(create_signal("s1", "BTC-USDT", dec!(2.0)));

        merger.clear();

        assert_eq!(merger.pending_count(), 0);
        assert_eq!(merger.get_current_position(&symbol), Quantity::ZERO);
    }

    #[test]
    fn test_execution_signal_signed_quantity() {
        let buy_signal = ExecutionSignal {
            symbol: Symbol::new("BTC-USDT").unwrap(),
            current_position: Quantity::ZERO,
            target_position: Quantity::new(dec!(5.0)).unwrap(),
            order_quantity: Quantity::new(dec!(5.0)).unwrap(),
            order_side: OrderSide::Buy,
            strategy_contributions: HashMap::new(),
            timestamp: Timestamp::now(),
            tag: "test".to_string(),
        };
        assert_eq!(buy_signal.signed_quantity().as_decimal(), dec!(5.0));

        let sell_signal = ExecutionSignal {
            symbol: Symbol::new("BTC-USDT").unwrap(),
            current_position: Quantity::new(dec!(5.0)).unwrap(),
            target_position: Quantity::ZERO,
            order_quantity: Quantity::new(dec!(5.0)).unwrap(),
            order_side: OrderSide::Sell,
            strategy_contributions: HashMap::new(),
            timestamp: Timestamp::now(),
            tag: "test".to_string(),
        };
        assert_eq!(sell_signal.signed_quantity().as_decimal(), dec!(-5.0));
    }

    #[test]
    fn test_execution_signal_serde_roundtrip() {
        let signal = ExecutionSignal {
            symbol: Symbol::new("BTC-USDT").unwrap(),
            current_position: Quantity::ZERO,
            target_position: Quantity::new(dec!(5.0)).unwrap(),
            order_quantity: Quantity::new(dec!(5.0)).unwrap(),
            order_side: OrderSide::Buy,
            strategy_contributions: HashMap::new(),
            timestamp: Timestamp::now(),
            tag: "test".to_string(),
        };

        let json = serde_json::to_string(&signal).unwrap();
        let parsed: ExecutionSignal = serde_json::from_str(&json).unwrap();

        assert_eq!(signal.symbol, parsed.symbol);
        assert_eq!(signal.target_position, parsed.target_position);
        assert_eq!(signal.order_side, parsed.order_side);
    }
}
