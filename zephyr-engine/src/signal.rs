//! Signal Aggregation Module.
//!
//! This module provides signal aggregation functionality for merging
//! position signals from multiple strategies before execution.

#![allow(clippy::disallowed_types)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::unused_async)]

use std::collections::HashMap;

use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use zephyr_core::types::{Quantity, Symbol, Timestamp};

/// A signal from a strategy indicating a target position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategySignal {
    /// Name of the strategy that generated this signal.
    pub strategy_name: String,
    /// Symbol for the position.
    pub symbol: Symbol,
    /// Target position quantity.
    pub target_position: Quantity,
    /// Tag for identifying this signal.
    pub tag: String,
    /// Timestamp when the signal was generated.
    pub timestamp: Timestamp,
}

/// Aggregated position from multiple strategies.
#[derive(Debug, Clone, Default)]
pub struct AggregatedPosition {
    /// Total target position (sum of all strategy positions).
    pub total_position: Quantity,
    /// Individual strategy positions.
    pub strategy_positions: HashMap<String, Quantity>,
    /// Last update timestamp.
    pub last_update: Option<Timestamp>,
}

impl AggregatedPosition {
    /// Creates a new empty aggregated position.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Updates the position for a strategy.
    pub fn update_strategy(
        &mut self,
        strategy_name: &str,
        position: Quantity,
        timestamp: Timestamp,
    ) {
        self.strategy_positions
            .insert(strategy_name.to_string(), position);
        self.recalculate_total();
        self.last_update = Some(timestamp);
    }

    /// Removes a strategy's position.
    pub fn remove_strategy(&mut self, strategy_name: &str) {
        self.strategy_positions.remove(strategy_name);
        self.recalculate_total();
    }

    /// Recalculates the total position from all strategies.
    fn recalculate_total(&mut self) {
        self.total_position = self.strategy_positions.values().copied().sum();
    }

    /// Returns the number of strategies with positions.
    #[must_use]
    pub fn strategy_count(&self) -> usize {
        self.strategy_positions.len()
    }
}

/// Signal Aggregator.
///
/// Collects signals from multiple strategies and aggregates them
/// into combined positions per symbol.
///
/// # Thread Safety
///
/// This implementation is thread-safe and can be shared across threads.
pub struct SignalAggregator {
    /// Aggregated positions per symbol.
    positions: DashMap<Symbol, AggregatedPosition>,
    /// Recent signals per symbol (for debugging/monitoring).
    recent_signals: DashMap<Symbol, Vec<StrategySignal>>,
    /// Maximum recent signals to keep per symbol.
    max_recent_signals: usize,
    /// Listeners for position changes.
    listeners: RwLock<Vec<Box<dyn PositionChangeListener>>>,
}

impl SignalAggregator {
    /// Creates a new signal aggregator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            positions: DashMap::new(),
            recent_signals: DashMap::new(),
            max_recent_signals: 100,
            listeners: RwLock::new(Vec::new()),
        }
    }

    /// Creates a new signal aggregator with custom settings.
    #[must_use]
    pub fn with_max_signals(max_recent_signals: usize) -> Self {
        Self {
            positions: DashMap::new(),
            recent_signals: DashMap::new(),
            max_recent_signals,
            listeners: RwLock::new(Vec::new()),
        }
    }

    /// Adds a signal from a strategy.
    pub async fn add_signal(&self, signal: StrategySignal) {
        let symbol = signal.symbol.clone();
        let strategy_name = signal.strategy_name.clone();
        let target_position = signal.target_position;
        let timestamp = signal.timestamp;

        debug!(
            strategy = %strategy_name,
            symbol = %symbol,
            position = %target_position,
            tag = %signal.tag,
            "Received signal"
        );

        // Store recent signal
        self.recent_signals
            .entry(symbol.clone())
            .or_default()
            .push(signal);

        // Trim recent signals if needed
        if let Some(mut signals) = self.recent_signals.get_mut(&symbol) {
            let len = signals.len();
            if len > self.max_recent_signals {
                signals.drain(0..len - self.max_recent_signals);
            }
        }

        // Update aggregated position
        let old_total = self.get_aggregated_position(&symbol).total_position;

        self.positions
            .entry(symbol.clone())
            .or_default()
            .update_strategy(&strategy_name, target_position, timestamp);

        let new_total = self.get_aggregated_position(&symbol).total_position;

        // Notify listeners if position changed
        if old_total != new_total {
            info!(
                symbol = %symbol,
                old = %old_total,
                new = %new_total,
                "Aggregated position changed"
            );

            let listeners = self.listeners.read();
            for listener in listeners.iter() {
                listener.on_position_change(&symbol, old_total, new_total);
            }
        }
    }

    /// Gets the aggregated position for a symbol.
    #[must_use]
    pub fn get_aggregated_position(&self, symbol: &Symbol) -> AggregatedPosition {
        self.positions
            .get(symbol)
            .map(|p| p.clone())
            .unwrap_or_default()
    }

    /// Gets the total position for a symbol.
    #[must_use]
    pub fn get_total_position(&self, symbol: &Symbol) -> Quantity {
        self.positions
            .get(symbol)
            .map(|p| p.total_position)
            .unwrap_or(Quantity::ZERO)
    }

    /// Gets recent signals for a symbol.
    #[must_use]
    pub fn get_signals(&self, symbol: &Symbol) -> Vec<StrategySignal> {
        self.recent_signals
            .get(symbol)
            .map(|s| s.clone())
            .unwrap_or_default()
    }

    /// Gets all symbols with positions.
    #[must_use]
    pub fn get_symbols(&self) -> Vec<Symbol> {
        self.positions.iter().map(|e| e.key().clone()).collect()
    }

    /// Removes a strategy from all positions.
    pub fn remove_strategy(&self, strategy_name: &str) {
        for mut entry in self.positions.iter_mut() {
            entry.remove_strategy(strategy_name);
        }
    }

    /// Clears all positions and signals.
    pub fn clear(&self) {
        self.positions.clear();
        self.recent_signals.clear();
    }

    /// Adds a position change listener.
    pub fn add_listener(&self, listener: Box<dyn PositionChangeListener>) {
        self.listeners.write().push(listener);
    }
}

impl Default for SignalAggregator {
    fn default() -> Self {
        Self::new()
    }
}

/// Listener for position changes.
pub trait PositionChangeListener: Send + Sync {
    /// Called when an aggregated position changes.
    fn on_position_change(&self, symbol: &Symbol, old_position: Quantity, new_position: Quantity);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_symbol() -> Symbol {
        Symbol::new("BTC-USDT").unwrap()
    }

    fn create_signal(strategy: &str, symbol: &Symbol, qty: Quantity) -> StrategySignal {
        StrategySignal {
            strategy_name: strategy.to_string(),
            symbol: symbol.clone(),
            target_position: qty,
            tag: "test".to_string(),
            timestamp: Timestamp::now(),
        }
    }

    #[test]
    fn test_aggregated_position() {
        let mut pos = AggregatedPosition::new();
        let ts = Timestamp::now();

        pos.update_strategy("strategy1", Quantity::new(dec!(1.0)).unwrap(), ts);
        assert_eq!(pos.total_position.as_decimal(), dec!(1.0));
        assert_eq!(pos.strategy_count(), 1);

        pos.update_strategy("strategy2", Quantity::new(dec!(2.0)).unwrap(), ts);
        assert_eq!(pos.total_position.as_decimal(), dec!(3.0));
        assert_eq!(pos.strategy_count(), 2);

        pos.remove_strategy("strategy1");
        assert_eq!(pos.total_position.as_decimal(), dec!(2.0));
        assert_eq!(pos.strategy_count(), 1);
    }

    #[test]
    fn test_aggregated_position_negative() {
        let mut pos = AggregatedPosition::new();
        let ts = Timestamp::now();

        pos.update_strategy("long", Quantity::new(dec!(5.0)).unwrap(), ts);
        pos.update_strategy("short", Quantity::new(dec!(-3.0)).unwrap(), ts);

        // Net position: 5 - 3 = 2
        assert_eq!(pos.total_position.as_decimal(), dec!(2.0));
    }

    #[tokio::test]
    async fn test_signal_aggregator_single_strategy() {
        let aggregator = SignalAggregator::new();
        let symbol = create_test_symbol();

        let signal = create_signal("strategy1", &symbol, Quantity::new(dec!(1.5)).unwrap());
        aggregator.add_signal(signal).await;

        let total = aggregator.get_total_position(&symbol);
        assert_eq!(total.as_decimal(), dec!(1.5));
    }

    #[tokio::test]
    async fn test_signal_aggregator_multiple_strategies() {
        let aggregator = SignalAggregator::new();
        let symbol = create_test_symbol();

        aggregator
            .add_signal(create_signal(
                "s1",
                &symbol,
                Quantity::new(dec!(1.0)).unwrap(),
            ))
            .await;
        aggregator
            .add_signal(create_signal(
                "s2",
                &symbol,
                Quantity::new(dec!(2.0)).unwrap(),
            ))
            .await;
        aggregator
            .add_signal(create_signal(
                "s3",
                &symbol,
                Quantity::new(dec!(-0.5)).unwrap(),
            ))
            .await;

        let total = aggregator.get_total_position(&symbol);
        assert_eq!(total.as_decimal(), dec!(2.5)); // 1 + 2 - 0.5
    }

    #[tokio::test]
    async fn test_signal_aggregator_update_strategy() {
        let aggregator = SignalAggregator::new();
        let symbol = create_test_symbol();

        // Initial signal
        aggregator
            .add_signal(create_signal(
                "s1",
                &symbol,
                Quantity::new(dec!(1.0)).unwrap(),
            ))
            .await;
        assert_eq!(
            aggregator.get_total_position(&symbol).as_decimal(),
            dec!(1.0)
        );

        // Update same strategy
        aggregator
            .add_signal(create_signal(
                "s1",
                &symbol,
                Quantity::new(dec!(3.0)).unwrap(),
            ))
            .await;
        assert_eq!(
            aggregator.get_total_position(&symbol).as_decimal(),
            dec!(3.0)
        );
    }

    #[tokio::test]
    async fn test_signal_aggregator_remove_strategy() {
        let aggregator = SignalAggregator::new();
        let symbol = create_test_symbol();

        aggregator
            .add_signal(create_signal(
                "s1",
                &symbol,
                Quantity::new(dec!(1.0)).unwrap(),
            ))
            .await;
        aggregator
            .add_signal(create_signal(
                "s2",
                &symbol,
                Quantity::new(dec!(2.0)).unwrap(),
            ))
            .await;

        aggregator.remove_strategy("s1");
        assert_eq!(
            aggregator.get_total_position(&symbol).as_decimal(),
            dec!(2.0)
        );
    }

    #[tokio::test]
    async fn test_signal_aggregator_get_signals() {
        let aggregator = SignalAggregator::new();
        let symbol = create_test_symbol();

        aggregator
            .add_signal(create_signal(
                "s1",
                &symbol,
                Quantity::new(dec!(1.0)).unwrap(),
            ))
            .await;
        aggregator
            .add_signal(create_signal(
                "s2",
                &symbol,
                Quantity::new(dec!(2.0)).unwrap(),
            ))
            .await;

        let signals = aggregator.get_signals(&symbol);
        assert_eq!(signals.len(), 2);
    }

    #[tokio::test]
    async fn test_signal_aggregator_multiple_symbols() {
        let aggregator = SignalAggregator::new();
        let btc = Symbol::new("BTC-USDT").unwrap();
        let eth = Symbol::new("ETH-USDT").unwrap();

        aggregator
            .add_signal(create_signal("s1", &btc, Quantity::new(dec!(1.0)).unwrap()))
            .await;
        aggregator
            .add_signal(create_signal("s1", &eth, Quantity::new(dec!(5.0)).unwrap()))
            .await;

        assert_eq!(aggregator.get_total_position(&btc).as_decimal(), dec!(1.0));
        assert_eq!(aggregator.get_total_position(&eth).as_decimal(), dec!(5.0));

        let symbols = aggregator.get_symbols();
        assert_eq!(symbols.len(), 2);
    }
}
