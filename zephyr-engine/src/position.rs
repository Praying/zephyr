//! Position Calculation Module.
//!
//! This module provides position calculation functionality for converting
//! target positions to orders and tracking theoretical positions.

#![allow(clippy::disallowed_types)]

use std::collections::HashMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use zephyr_core::data::OrderSide;
use zephyr_core::types::{Quantity, Symbol, Timestamp};

/// Represents a change in position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionChange {
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
    /// Timestamp of the change.
    pub timestamp: Timestamp,
}

impl PositionChange {
    /// Returns true if no order is needed (positions are equal).
    #[must_use]
    pub fn is_no_change(&self) -> bool {
        self.order_quantity.is_zero()
    }

    /// Returns the signed quantity change (positive for buy, negative for sell).
    #[must_use]
    pub fn signed_quantity(&self) -> Quantity {
        match self.order_side {
            OrderSide::Buy => self.order_quantity,
            OrderSide::Sell => -self.order_quantity,
        }
    }
}

/// Target position for a symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetPosition {
    /// Symbol.
    pub symbol: Symbol,
    /// Target quantity (positive for long, negative for short).
    pub quantity: Quantity,
    /// Source strategy name.
    pub strategy: String,
    /// Tag for this position change.
    pub tag: String,
    /// Timestamp.
    pub timestamp: Timestamp,
}

/// Position Calculator.
///
/// Calculates the orders needed to move from current positions to target positions.
/// Supports multi-strategy position merging and theoretical position tracking.
pub struct PositionCalculator {
    /// Current actual positions per symbol.
    actual_positions: HashMap<Symbol, Quantity>,
    /// Theoretical positions per strategy per symbol.
    theoretical_positions: HashMap<String, HashMap<Symbol, Quantity>>,
    /// Position multiplier (for scaling positions).
    multiplier: Decimal,
}

impl PositionCalculator {
    /// Creates a new position calculator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            actual_positions: HashMap::new(),
            theoretical_positions: HashMap::new(),
            multiplier: Decimal::ONE,
        }
    }

    /// Creates a new position calculator with a multiplier.
    #[must_use]
    pub fn with_multiplier(multiplier: Decimal) -> Self {
        Self {
            actual_positions: HashMap::new(),
            theoretical_positions: HashMap::new(),
            multiplier,
        }
    }

    /// Sets the position multiplier.
    pub fn set_multiplier(&mut self, multiplier: Decimal) {
        self.multiplier = multiplier;
    }

    /// Gets the position multiplier.
    #[must_use]
    pub fn multiplier(&self) -> Decimal {
        self.multiplier
    }

    /// Updates the actual position for a symbol.
    pub fn update_actual_position(&mut self, symbol: &Symbol, quantity: Quantity) {
        debug!(symbol = %symbol, quantity = %quantity, "Updating actual position");
        self.actual_positions.insert(symbol.clone(), quantity);
    }

    /// Gets the actual position for a symbol.
    #[must_use]
    pub fn get_actual_position(&self, symbol: &Symbol) -> Quantity {
        self.actual_positions
            .get(symbol)
            .copied()
            .unwrap_or(Quantity::ZERO)
    }

    /// Updates the theoretical position for a strategy and symbol.
    pub fn update_theoretical_position(
        &mut self,
        strategy: &str,
        symbol: &Symbol,
        quantity: Quantity,
    ) {
        debug!(
            strategy = %strategy,
            symbol = %symbol,
            quantity = %quantity,
            "Updating theoretical position"
        );
        self.theoretical_positions
            .entry(strategy.to_string())
            .or_default()
            .insert(symbol.clone(), quantity);
    }

    /// Gets the theoretical position for a strategy and symbol.
    #[must_use]
    pub fn get_theoretical_position(&self, strategy: &str, symbol: &Symbol) -> Quantity {
        self.theoretical_positions
            .get(strategy)
            .and_then(|m| m.get(symbol))
            .copied()
            .unwrap_or(Quantity::ZERO)
    }

    /// Gets the merged theoretical position for a symbol (sum of all strategies).
    #[must_use]
    pub fn get_merged_position(&self, symbol: &Symbol) -> Quantity {
        self.theoretical_positions
            .values()
            .filter_map(|m| m.get(symbol))
            .copied()
            .sum()
    }

    /// Calculates the position change needed to reach a target position.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The trading symbol
    /// * `target` - Target position quantity
    ///
    /// # Returns
    ///
    /// A `PositionChange` describing the order needed.
    #[must_use]
    pub fn calculate_change(&self, symbol: &Symbol, target: Quantity) -> PositionChange {
        let current = self.get_actual_position(symbol);
        Self::calculate_position_change(symbol.clone(), current, target)
    }

    /// Calculates the position change from current to target.
    ///
    /// This is a pure function that can be used without a calculator instance.
    #[must_use]
    pub fn calculate_position_change(
        symbol: Symbol,
        current: Quantity,
        target: Quantity,
    ) -> PositionChange {
        let diff = target - current;
        let (order_quantity, order_side) = if diff.is_positive() {
            (diff, OrderSide::Buy)
        } else if diff.is_negative() {
            (-diff, OrderSide::Sell)
        } else {
            (Quantity::ZERO, OrderSide::Buy) // No change, side doesn't matter
        };

        PositionChange {
            symbol,
            current_position: current,
            target_position: target,
            order_quantity,
            order_side,
            timestamp: Timestamp::now(),
        }
    }

    /// Calculates all position changes needed to reach merged target positions.
    ///
    /// This merges all strategy positions and calculates the orders needed
    /// to move actual positions to the merged targets.
    #[must_use]
    pub fn calculate_all_changes(&self) -> Vec<PositionChange> {
        let mut changes = Vec::new();

        // Get all symbols from theoretical positions
        let symbols: std::collections::HashSet<_> = self
            .theoretical_positions
            .values()
            .flat_map(|m| m.keys())
            .cloned()
            .collect();

        for symbol in symbols {
            let merged = self.get_merged_position(&symbol);
            // Apply multiplier
            let scaled = Quantity::new_unchecked(merged.as_decimal() * self.multiplier);
            let change = self.calculate_change(&symbol, scaled);

            if !change.is_no_change() {
                info!(
                    symbol = %symbol,
                    current = %change.current_position,
                    target = %change.target_position,
                    order_qty = %change.order_quantity,
                    side = %change.order_side,
                    "Position change calculated"
                );
                changes.push(change);
            }
        }

        changes
    }

    /// Removes a strategy from the calculator.
    pub fn remove_strategy(&mut self, strategy: &str) {
        self.theoretical_positions.remove(strategy);
    }

    /// Clears all positions.
    pub fn clear(&mut self) {
        self.actual_positions.clear();
        self.theoretical_positions.clear();
    }

    /// Gets all strategies.
    #[must_use]
    pub fn strategies(&self) -> Vec<&str> {
        self.theoretical_positions
            .keys()
            .map(String::as_str)
            .collect()
    }

    /// Gets all symbols with positions.
    #[must_use]
    pub fn symbols(&self) -> Vec<Symbol> {
        let mut symbols: std::collections::HashSet<_> =
            self.actual_positions.keys().cloned().collect();
        for positions in self.theoretical_positions.values() {
            symbols.extend(positions.keys().cloned());
        }
        symbols.into_iter().collect()
    }
}

impl Default for PositionCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_symbol() -> Symbol {
        Symbol::new("BTC-USDT").unwrap()
    }

    #[test]
    fn test_position_change_calculation_buy() {
        let symbol = create_test_symbol();
        let current = Quantity::new(dec!(1.0)).unwrap();
        let target = Quantity::new(dec!(3.0)).unwrap();

        let change = PositionCalculator::calculate_position_change(symbol, current, target);

        assert_eq!(change.order_quantity.as_decimal(), dec!(2.0));
        assert_eq!(change.order_side, OrderSide::Buy);
        assert!(!change.is_no_change());
    }

    #[test]
    fn test_position_change_calculation_sell() {
        let symbol = create_test_symbol();
        let current = Quantity::new(dec!(5.0)).unwrap();
        let target = Quantity::new(dec!(2.0)).unwrap();

        let change = PositionCalculator::calculate_position_change(symbol, current, target);

        assert_eq!(change.order_quantity.as_decimal(), dec!(3.0));
        assert_eq!(change.order_side, OrderSide::Sell);
    }

    #[test]
    fn test_position_change_calculation_no_change() {
        let symbol = create_test_symbol();
        let current = Quantity::new(dec!(2.0)).unwrap();
        let target = Quantity::new(dec!(2.0)).unwrap();

        let change = PositionCalculator::calculate_position_change(symbol, current, target);

        assert!(change.is_no_change());
        assert_eq!(change.order_quantity, Quantity::ZERO);
    }

    #[test]
    fn test_position_change_calculation_close_long() {
        let symbol = create_test_symbol();
        let current = Quantity::new(dec!(5.0)).unwrap();
        let target = Quantity::ZERO;

        let change = PositionCalculator::calculate_position_change(symbol, current, target);

        assert_eq!(change.order_quantity.as_decimal(), dec!(5.0));
        assert_eq!(change.order_side, OrderSide::Sell);
    }

    #[test]
    fn test_position_change_calculation_open_short() {
        let symbol = create_test_symbol();
        let current = Quantity::ZERO;
        let target = Quantity::new(dec!(-3.0)).unwrap();

        let change = PositionCalculator::calculate_position_change(symbol, current, target);

        assert_eq!(change.order_quantity.as_decimal(), dec!(3.0));
        assert_eq!(change.order_side, OrderSide::Sell);
    }

    #[test]
    fn test_position_change_calculation_flip_position() {
        let symbol = create_test_symbol();
        let current = Quantity::new(dec!(2.0)).unwrap(); // Long 2
        let target = Quantity::new(dec!(-1.0)).unwrap(); // Short 1

        let change = PositionCalculator::calculate_position_change(symbol, current, target);

        // Need to sell 3 (2 to close long + 1 to open short)
        assert_eq!(change.order_quantity.as_decimal(), dec!(3.0));
        assert_eq!(change.order_side, OrderSide::Sell);
    }

    #[test]
    fn test_calculator_actual_positions() {
        let mut calc = PositionCalculator::new();
        let symbol = create_test_symbol();

        assert_eq!(calc.get_actual_position(&symbol), Quantity::ZERO);

        calc.update_actual_position(&symbol, Quantity::new(dec!(5.0)).unwrap());
        assert_eq!(calc.get_actual_position(&symbol).as_decimal(), dec!(5.0));
    }

    #[test]
    fn test_calculator_theoretical_positions() {
        let mut calc = PositionCalculator::new();
        let symbol = create_test_symbol();

        calc.update_theoretical_position("s1", &symbol, Quantity::new(dec!(2.0)).unwrap());
        calc.update_theoretical_position("s2", &symbol, Quantity::new(dec!(3.0)).unwrap());

        assert_eq!(
            calc.get_theoretical_position("s1", &symbol).as_decimal(),
            dec!(2.0)
        );
        assert_eq!(
            calc.get_theoretical_position("s2", &symbol).as_decimal(),
            dec!(3.0)
        );
    }

    #[test]
    fn test_calculator_merged_position() {
        let mut calc = PositionCalculator::new();
        let symbol = create_test_symbol();

        calc.update_theoretical_position("s1", &symbol, Quantity::new(dec!(2.0)).unwrap());
        calc.update_theoretical_position("s2", &symbol, Quantity::new(dec!(3.0)).unwrap());
        calc.update_theoretical_position("s3", &symbol, Quantity::new(dec!(-1.0)).unwrap());

        // Merged: 2 + 3 - 1 = 4
        assert_eq!(calc.get_merged_position(&symbol).as_decimal(), dec!(4.0));
    }

    #[test]
    fn test_calculator_calculate_change() {
        let mut calc = PositionCalculator::new();
        let symbol = create_test_symbol();

        calc.update_actual_position(&symbol, Quantity::new(dec!(1.0)).unwrap());

        let change = calc.calculate_change(&symbol, Quantity::new(dec!(5.0)).unwrap());

        assert_eq!(change.current_position.as_decimal(), dec!(1.0));
        assert_eq!(change.target_position.as_decimal(), dec!(5.0));
        assert_eq!(change.order_quantity.as_decimal(), dec!(4.0));
        assert_eq!(change.order_side, OrderSide::Buy);
    }

    #[test]
    fn test_calculator_calculate_all_changes() {
        let mut calc = PositionCalculator::new();
        let btc = Symbol::new("BTC-USDT").unwrap();
        let eth = Symbol::new("ETH-USDT").unwrap();

        // Set actual positions
        calc.update_actual_position(&btc, Quantity::new(dec!(1.0)).unwrap());
        calc.update_actual_position(&eth, Quantity::new(dec!(10.0)).unwrap());

        // Set theoretical positions from strategies
        calc.update_theoretical_position("s1", &btc, Quantity::new(dec!(2.0)).unwrap());
        calc.update_theoretical_position("s2", &btc, Quantity::new(dec!(1.0)).unwrap());
        calc.update_theoretical_position("s1", &eth, Quantity::new(dec!(5.0)).unwrap());

        let changes = calc.calculate_all_changes();

        // BTC: actual=1, merged=3, need to buy 2
        // ETH: actual=10, merged=5, need to sell 5
        assert_eq!(changes.len(), 2);

        let btc_change = changes.iter().find(|c| c.symbol == btc).unwrap();
        assert_eq!(btc_change.order_quantity.as_decimal(), dec!(2.0));
        assert_eq!(btc_change.order_side, OrderSide::Buy);

        let eth_change = changes.iter().find(|c| c.symbol == eth).unwrap();
        assert_eq!(eth_change.order_quantity.as_decimal(), dec!(5.0));
        assert_eq!(eth_change.order_side, OrderSide::Sell);
    }

    #[test]
    fn test_calculator_with_multiplier() {
        let mut calc = PositionCalculator::with_multiplier(dec!(2.0));
        let symbol = create_test_symbol();

        calc.update_actual_position(&symbol, Quantity::ZERO);
        calc.update_theoretical_position("s1", &symbol, Quantity::new(dec!(1.0)).unwrap());

        let changes = calc.calculate_all_changes();

        // Merged=1, scaled by 2x = 2, actual=0, need to buy 2
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].order_quantity.as_decimal(), dec!(2.0));
    }

    #[test]
    fn test_calculator_remove_strategy() {
        let mut calc = PositionCalculator::new();
        let symbol = create_test_symbol();

        calc.update_theoretical_position("s1", &symbol, Quantity::new(dec!(2.0)).unwrap());
        calc.update_theoretical_position("s2", &symbol, Quantity::new(dec!(3.0)).unwrap());

        assert_eq!(calc.get_merged_position(&symbol).as_decimal(), dec!(5.0));

        calc.remove_strategy("s1");
        assert_eq!(calc.get_merged_position(&symbol).as_decimal(), dec!(3.0));
    }

    #[test]
    fn test_position_change_signed_quantity() {
        let symbol = create_test_symbol();

        let buy_change = PositionChange {
            symbol: symbol.clone(),
            current_position: Quantity::ZERO,
            target_position: Quantity::new(dec!(5.0)).unwrap(),
            order_quantity: Quantity::new(dec!(5.0)).unwrap(),
            order_side: OrderSide::Buy,
            timestamp: Timestamp::now(),
        };
        assert_eq!(buy_change.signed_quantity().as_decimal(), dec!(5.0));

        let sell_change = PositionChange {
            symbol,
            current_position: Quantity::new(dec!(5.0)).unwrap(),
            target_position: Quantity::ZERO,
            order_quantity: Quantity::new(dec!(5.0)).unwrap(),
            order_side: OrderSide::Sell,
            timestamp: Timestamp::now(),
        };
        assert_eq!(sell_change.signed_quantity().as_decimal(), dec!(-5.0));
    }
}
