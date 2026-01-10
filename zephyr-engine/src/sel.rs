//! SEL Strategy Context Implementation.
//!
//! This module provides the implementation of the SEL strategy context,
//! which manages batch position operations, portfolio management, and
//! scheduled execution for SEL (Selection/Scheduled) strategies.

#![allow(clippy::disallowed_types)]
#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::explicit_iter_loop)]
#![allow(clippy::doc_markdown)]

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use parking_lot::RwLock;
use rust_decimal::Decimal;
use tracing::{debug, info, instrument};

use zephyr_core::data::{KlineData, KlinePeriod};
use zephyr_core::error::StrategyError;
use zephyr_core::traits::{LogLevel, SelStrategy, SelStrategyContext};
use zephyr_core::types::{Amount, Price, Quantity, Symbol, Timestamp};

use crate::signal::{SignalAggregator, StrategySignal};

/// SEL Strategy Context Implementation.
///
/// Provides batch position management and portfolio-level operations for SEL strategies.
/// Implements the `SelStrategyContext` trait.
///
/// # Thread Safety
///
/// This implementation is thread-safe and can be shared across threads.
/// Uses `DashMap` for concurrent position access and `RwLock` for data caches.
pub struct SelStrategyContextImpl {
    /// Strategy name
    strategy_name: String,
    /// Theoretical positions per symbol (strategy's view)
    positions: DashMap<Symbol, Quantity>,
    /// Current prices per symbol
    prices: DashMap<Symbol, Price>,
    /// Investment universe (tradeable symbols)
    universe: RwLock<Vec<Symbol>>,
    /// K-line data cache per symbol and period
    kline_cache: RwLock<HashMap<(Symbol, KlinePeriod), Vec<KlineData>>>,
    /// Signal aggregator for position changes
    signal_aggregator: Arc<SignalAggregator>,
    /// Current timestamp (for backtesting support)
    current_time: RwLock<Timestamp>,
    /// Portfolio cash balance
    cash_balance: RwLock<Amount>,
    /// Maximum K-line cache size per symbol/period
    max_kline_cache: usize,
}

impl SelStrategyContextImpl {
    /// Creates a new SEL strategy context.
    ///
    /// # Arguments
    ///
    /// * `strategy_name` - Name of the strategy
    /// * `signal_aggregator` - Shared signal aggregator
    #[must_use]
    pub fn new(strategy_name: impl Into<String>, signal_aggregator: Arc<SignalAggregator>) -> Self {
        Self {
            strategy_name: strategy_name.into(),
            positions: DashMap::new(),
            prices: DashMap::new(),
            universe: RwLock::new(Vec::new()),
            kline_cache: RwLock::new(HashMap::new()),
            signal_aggregator,
            current_time: RwLock::new(Timestamp::now()),
            cash_balance: RwLock::new(Amount::ZERO),
            max_kline_cache: 1000,
        }
    }

    /// Creates a builder for `SelStrategyContextImpl`.
    #[must_use]
    pub fn builder() -> SelStrategyContextBuilder {
        SelStrategyContextBuilder::default()
    }

    /// Updates the current price for a symbol.
    pub fn update_price(&self, symbol: &Symbol, price: Price) {
        self.prices.insert(symbol.clone(), price);
    }

    /// Updates prices for multiple symbols.
    pub fn update_prices(&self, prices: HashMap<Symbol, Price>) {
        for (symbol, price) in prices {
            self.prices.insert(symbol, price);
        }
    }

    /// Sets the current timestamp.
    pub fn set_current_time(&self, timestamp: Timestamp) {
        *self.current_time.write() = timestamp;
    }

    /// Sets the cash balance.
    pub fn set_cash_balance(&self, balance: Amount) {
        *self.cash_balance.write() = balance;
    }

    /// Adds K-line data to the cache.
    pub fn add_kline(&self, kline: KlineData) {
        let key = (kline.symbol.clone(), kline.period);
        let mut cache = self.kline_cache.write();
        let klines = cache.entry(key).or_default();

        // Maintain cache size limit
        if klines.len() >= self.max_kline_cache {
            klines.remove(0);
        }
        klines.push(kline);
    }

    /// Adds multiple K-lines to the cache.
    pub fn add_klines(&self, klines: Vec<KlineData>) {
        for kline in klines {
            self.add_kline(kline);
        }
    }

    /// Clears all cached data.
    pub fn clear_cache(&self) {
        self.kline_cache.write().clear();
    }

    /// Returns the theoretical position for a symbol.
    #[must_use]
    pub fn get_theoretical_position(&self, symbol: &Symbol) -> Quantity {
        self.positions
            .get(symbol)
            .map(|p| *p)
            .unwrap_or(Quantity::ZERO)
    }

    /// Sets the theoretical position directly (for initialization/recovery).
    pub fn set_theoretical_position(&self, symbol: &Symbol, qty: Quantity) {
        self.positions.insert(symbol.clone(), qty);
    }

    /// Sets multiple theoretical positions.
    pub fn set_theoretical_positions(&self, positions: HashMap<Symbol, Quantity>) {
        for (symbol, qty) in positions {
            self.positions.insert(symbol, qty);
        }
    }

    /// Calculates the total position value.
    #[must_use]
    pub fn calculate_position_value(&self) -> Amount {
        let mut total = Amount::ZERO;
        for entry in self.positions.iter() {
            let symbol = entry.key();
            let qty = *entry.value();
            if let Some(price) = self.prices.get(symbol) {
                let value = Amount::from_price_qty(*price, qty);
                total = total + value;
            }
        }
        total
    }
}

#[async_trait]
impl SelStrategyContext for SelStrategyContextImpl {
    fn get_all_positions(&self) -> HashMap<Symbol, Quantity> {
        self.positions
            .iter()
            .map(|entry| (entry.key().clone(), *entry.value()))
            .collect()
    }

    #[instrument(skip(self, positions), fields(strategy = %self.strategy_name, count = positions.len()))]
    async fn set_positions_batch(
        &self,
        positions: HashMap<Symbol, Quantity>,
        tag: &str,
    ) -> Result<(), StrategyError> {
        info!(
            tag = %tag,
            positions_count = positions.len(),
            "Setting batch positions"
        );

        let timestamp = self.current_time();

        // Process each position change
        for (symbol, target_qty) in positions {
            let current = self.get_theoretical_position(&symbol);

            if current == target_qty {
                debug!(
                    symbol = %symbol,
                    position = %target_qty,
                    "Position unchanged"
                );
                continue;
            }

            debug!(
                symbol = %symbol,
                from = %current,
                to = %target_qty,
                tag = %tag,
                "Updating position"
            );

            // Update local theoretical position
            self.positions.insert(symbol.clone(), target_qty);

            // Send signal to aggregator
            let signal = StrategySignal {
                strategy_name: self.strategy_name.clone(),
                symbol: symbol.clone(),
                target_position: target_qty,
                tag: tag.to_string(),
                timestamp,
            };

            self.signal_aggregator.add_signal(signal).await;
        }

        Ok(())
    }

    fn get_universe(&self) -> &[Symbol] {
        // Note: This returns an empty slice because we can't return a reference
        // to data inside a RwLock. In practice, callers should use get_universe_owned().
        static EMPTY: &[Symbol] = &[];
        EMPTY
    }

    fn set_universe(&self, symbols: Vec<Symbol>) {
        *self.universe.write() = symbols;
    }

    fn get_bars(&self, _symbol: &Symbol, _period: KlinePeriod, _count: usize) -> &[KlineData] {
        // Note: Returns empty slice due to RwLock limitation
        static EMPTY: &[KlineData] = &[];
        EMPTY
    }

    fn get_bars_batch(
        &self,
        symbols: &[Symbol],
        period: KlinePeriod,
        count: usize,
    ) -> HashMap<Symbol, Vec<KlineData>> {
        let cache = self.kline_cache.read();
        let mut result = HashMap::new();

        for symbol in symbols {
            let key = (symbol.clone(), period);
            if let Some(klines) = cache.get(&key) {
                let start = klines.len().saturating_sub(count);
                result.insert(symbol.clone(), klines[start..].to_vec());
            } else {
                result.insert(symbol.clone(), Vec::new());
            }
        }

        result
    }

    fn get_price(&self, symbol: &Symbol) -> Option<Price> {
        self.prices.get(symbol).map(|p| *p)
    }

    fn current_time(&self) -> Timestamp {
        *self.current_time.read()
    }

    fn get_portfolio_value(&self) -> Amount {
        let position_value = self.calculate_position_value();
        let cash = *self.cash_balance.read();
        position_value + cash
    }

    fn get_available_cash(&self) -> Amount {
        *self.cash_balance.read()
    }

    fn log(&self, level: LogLevel, message: &str) {
        match level {
            LogLevel::Trace => tracing::trace!(strategy = %self.strategy_name, "{}", message),
            LogLevel::Debug => tracing::debug!(strategy = %self.strategy_name, "{}", message),
            LogLevel::Info => tracing::info!(strategy = %self.strategy_name, "{}", message),
            LogLevel::Warn => tracing::warn!(strategy = %self.strategy_name, "{}", message),
            LogLevel::Error => tracing::error!(strategy = %self.strategy_name, "{}", message),
        }
    }

    fn strategy_name(&self) -> &str {
        &self.strategy_name
    }

    fn get_position_weights(&self) -> HashMap<Symbol, Decimal> {
        let portfolio_value = self.get_portfolio_value();
        if portfolio_value.is_zero() {
            return HashMap::new();
        }

        let mut weights = HashMap::new();
        for entry in self.positions.iter() {
            let symbol = entry.key();
            let qty = *entry.value();
            if let Some(price) = self.prices.get(symbol) {
                let position_value = Amount::from_price_qty(*price, qty);
                let weight = position_value.as_decimal() / portfolio_value.as_decimal();
                weights.insert(symbol.clone(), weight);
            }
        }
        weights
    }

    fn calculate_weight_deviation(
        &self,
        target_weights: &HashMap<Symbol, Decimal>,
    ) -> HashMap<Symbol, Decimal> {
        let current_weights = self.get_position_weights();
        let mut deviations = HashMap::new();

        // Calculate deviation for symbols in target
        for (symbol, target_weight) in target_weights {
            let current_weight = current_weights
                .get(symbol)
                .copied()
                .unwrap_or(Decimal::ZERO);
            deviations.insert(symbol.clone(), current_weight - *target_weight);
        }

        // Add symbols that are in current but not in target (should be zero)
        for (symbol, current_weight) in &current_weights {
            if !target_weights.contains_key(symbol) {
                deviations.insert(symbol.clone(), *current_weight);
            }
        }

        deviations
    }
}

/// Builder for `SelStrategyContextImpl`.
#[derive(Default)]
pub struct SelStrategyContextBuilder {
    strategy_name: Option<String>,
    signal_aggregator: Option<Arc<SignalAggregator>>,
    universe: Vec<Symbol>,
    initial_cash: Option<Amount>,
    max_kline_cache: Option<usize>,
}

impl SelStrategyContextBuilder {
    /// Sets the strategy name.
    #[must_use]
    pub fn strategy_name(mut self, name: impl Into<String>) -> Self {
        self.strategy_name = Some(name.into());
        self
    }

    /// Sets the signal aggregator.
    #[must_use]
    pub fn signal_aggregator(mut self, aggregator: Arc<SignalAggregator>) -> Self {
        self.signal_aggregator = Some(aggregator);
        self
    }

    /// Sets the initial universe.
    #[must_use]
    pub fn universe(mut self, symbols: Vec<Symbol>) -> Self {
        self.universe = symbols;
        self
    }

    /// Adds a symbol to the universe.
    #[must_use]
    pub fn add_symbol(mut self, symbol: Symbol) -> Self {
        self.universe.push(symbol);
        self
    }

    /// Sets the initial cash balance.
    #[must_use]
    pub fn initial_cash(mut self, cash: Amount) -> Self {
        self.initial_cash = Some(cash);
        self
    }

    /// Sets the maximum K-line cache size.
    #[must_use]
    pub fn max_kline_cache(mut self, size: usize) -> Self {
        self.max_kline_cache = Some(size);
        self
    }

    /// Builds the `SelStrategyContextImpl`.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing.
    pub fn build(self) -> Result<SelStrategyContextImpl, &'static str> {
        let strategy_name = self.strategy_name.ok_or("strategy_name is required")?;
        let signal_aggregator = self
            .signal_aggregator
            .ok_or("signal_aggregator is required")?;

        let mut ctx = SelStrategyContextImpl::new(strategy_name, signal_aggregator);

        if !self.universe.is_empty() {
            ctx.set_universe(self.universe);
        }

        if let Some(cash) = self.initial_cash {
            ctx.set_cash_balance(cash);
        }

        if let Some(size) = self.max_kline_cache {
            ctx.max_kline_cache = size;
        }

        Ok(ctx)
    }
}

/// SEL Strategy Runner.
///
/// Manages the lifecycle and execution of SEL strategies.
pub struct SelStrategyRunner {
    /// Strategy instance
    strategy: Box<dyn SelStrategy>,
    /// Strategy context
    context: Arc<SelStrategyContextImpl>,
    /// Whether the strategy is running
    running: RwLock<bool>,
}

impl SelStrategyRunner {
    /// Creates a new strategy runner.
    #[must_use]
    pub fn new(strategy: Box<dyn SelStrategy>, context: Arc<SelStrategyContextImpl>) -> Self {
        Self {
            strategy,
            context,
            running: RwLock::new(false),
        }
    }

    /// Initializes and starts the strategy.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if initialization fails.
    pub async fn start(&mut self) -> Result<(), StrategyError> {
        if *self.running.read() {
            return Err(StrategyError::AlreadyRunning {
                name: self.strategy.name().to_string(),
            });
        }

        info!(strategy = %self.strategy.name(), "Starting SEL strategy");

        self.strategy.on_init(self.context.as_ref()).await?;
        *self.running.write() = true;

        info!(strategy = %self.strategy.name(), "SEL strategy started");
        Ok(())
    }

    /// Stops the strategy.
    pub async fn stop(&mut self) {
        if !*self.running.read() {
            return;
        }

        info!(strategy = %self.strategy.name(), "Stopping SEL strategy");

        self.strategy.on_stop(self.context.as_ref()).await;
        *self.running.write() = false;

        info!(strategy = %self.strategy.name(), "SEL strategy stopped");
    }

    /// Returns whether the strategy is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        *self.running.read()
    }

    /// Triggers the scheduled callback.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if processing fails.
    pub async fn on_schedule(&mut self) -> Result<(), StrategyError> {
        if !*self.running.read() {
            return Err(StrategyError::NotRunning {
                name: self.strategy.name().to_string(),
            });
        }

        info!(strategy = %self.strategy.name(), "Triggering scheduled callback");

        self.strategy.on_schedule(self.context.as_ref()).await
    }

    /// Processes a bar (K-line) event.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if processing fails.
    pub async fn on_bar(&mut self, bar: &KlineData) -> Result<(), StrategyError> {
        if !*self.running.read() {
            return Err(StrategyError::NotRunning {
                name: self.strategy.name().to_string(),
            });
        }

        // Update context with bar data
        self.context.add_kline(bar.clone());

        // Update price from bar
        self.context.update_price(&bar.symbol, bar.close);

        // Call strategy callback
        self.strategy.on_bar(self.context.as_ref(), bar).await
    }

    /// Returns the strategy name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.strategy.name()
    }

    /// Returns a reference to the context.
    #[must_use]
    pub fn context(&self) -> &Arc<SelStrategyContextImpl> {
        &self.context
    }
}

/// Gets the universe as an owned vector.
///
/// This is a helper function since `get_universe()` returns an empty slice
/// due to RwLock limitations.
impl SelStrategyContextImpl {
    /// Gets the universe as an owned vector.
    #[must_use]
    pub fn get_universe_owned(&self) -> Vec<Symbol> {
        self.universe.read().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_aggregator() -> Arc<SignalAggregator> {
        Arc::new(SignalAggregator::new())
    }

    fn create_test_symbol(name: &str) -> Symbol {
        Symbol::new(name).unwrap()
    }

    #[test]
    fn test_context_builder() {
        let aggregator = create_test_aggregator();
        let btc = create_test_symbol("BTC-USDT");
        let eth = create_test_symbol("ETH-USDT");

        let ctx = SelStrategyContextImpl::builder()
            .strategy_name("test_sel_strategy")
            .signal_aggregator(aggregator)
            .universe(vec![btc.clone(), eth.clone()])
            .initial_cash(Amount::new(dec!(100000)).unwrap())
            .max_kline_cache(500)
            .build()
            .unwrap();

        assert_eq!(ctx.strategy_name(), "test_sel_strategy");
        assert_eq!(ctx.get_universe_owned().len(), 2);
        assert_eq!(ctx.get_available_cash().as_decimal(), dec!(100000));
        assert_eq!(ctx.max_kline_cache, 500);
    }

    #[test]
    fn test_context_position_management() {
        let aggregator = create_test_aggregator();
        let btc = create_test_symbol("BTC-USDT");
        let eth = create_test_symbol("ETH-USDT");

        let ctx = SelStrategyContextImpl::new("test", aggregator);

        // Initial positions should be empty
        assert!(ctx.get_all_positions().is_empty());

        // Set theoretical positions
        ctx.set_theoretical_position(&btc, Quantity::new(dec!(1.5)).unwrap());
        ctx.set_theoretical_position(&eth, Quantity::new(dec!(10.0)).unwrap());

        let positions = ctx.get_all_positions();
        assert_eq!(positions.len(), 2);
        assert_eq!(positions.get(&btc).unwrap().as_decimal(), dec!(1.5));
        assert_eq!(positions.get(&eth).unwrap().as_decimal(), dec!(10.0));
    }

    #[test]
    fn test_context_price_update() {
        let aggregator = create_test_aggregator();
        let btc = create_test_symbol("BTC-USDT");

        let ctx = SelStrategyContextImpl::new("test", aggregator);

        // No price initially
        assert!(ctx.get_price(&btc).is_none());

        // Update price
        ctx.update_price(&btc, Price::new(dec!(50000)).unwrap());
        assert_eq!(ctx.get_price(&btc).unwrap().as_decimal(), dec!(50000));
    }

    #[test]
    fn test_context_portfolio_value() {
        let aggregator = create_test_aggregator();
        let btc = create_test_symbol("BTC-USDT");
        let eth = create_test_symbol("ETH-USDT");

        let ctx = SelStrategyContextImpl::new("test", aggregator);

        // Set cash
        ctx.set_cash_balance(Amount::new(dec!(10000)).unwrap());

        // Set positions and prices
        ctx.set_theoretical_position(&btc, Quantity::new(dec!(1.0)).unwrap());
        ctx.set_theoretical_position(&eth, Quantity::new(dec!(10.0)).unwrap());
        ctx.update_price(&btc, Price::new(dec!(50000)).unwrap());
        ctx.update_price(&eth, Price::new(dec!(3000)).unwrap());

        // Portfolio value = 10000 (cash) + 50000 (1 BTC) + 30000 (10 ETH) = 90000
        let portfolio_value = ctx.get_portfolio_value();
        assert_eq!(portfolio_value.as_decimal(), dec!(90000));
    }

    #[test]
    fn test_context_position_weights() {
        let aggregator = create_test_aggregator();
        let btc = create_test_symbol("BTC-USDT");
        let eth = create_test_symbol("ETH-USDT");

        let ctx = SelStrategyContextImpl::new("test", aggregator);

        // Set positions and prices (no cash for simplicity)
        ctx.set_theoretical_position(&btc, Quantity::new(dec!(1.0)).unwrap());
        ctx.set_theoretical_position(&eth, Quantity::new(dec!(10.0)).unwrap());
        ctx.update_price(&btc, Price::new(dec!(50000)).unwrap());
        ctx.update_price(&eth, Price::new(dec!(5000)).unwrap());

        // Total value = 50000 + 50000 = 100000
        // BTC weight = 50000 / 100000 = 0.5
        // ETH weight = 50000 / 100000 = 0.5
        let weights = ctx.get_position_weights();
        assert_eq!(weights.get(&btc).unwrap(), &dec!(0.5));
        assert_eq!(weights.get(&eth).unwrap(), &dec!(0.5));
    }

    #[test]
    fn test_context_weight_deviation() {
        let aggregator = create_test_aggregator();
        let btc = create_test_symbol("BTC-USDT");
        let eth = create_test_symbol("ETH-USDT");

        let ctx = SelStrategyContextImpl::new("test", aggregator);

        // Set positions and prices
        ctx.set_theoretical_position(&btc, Quantity::new(dec!(1.0)).unwrap());
        ctx.set_theoretical_position(&eth, Quantity::new(dec!(10.0)).unwrap());
        ctx.update_price(&btc, Price::new(dec!(50000)).unwrap());
        ctx.update_price(&eth, Price::new(dec!(5000)).unwrap());

        // Current weights: BTC=0.5, ETH=0.5
        // Target weights: BTC=0.6, ETH=0.4
        let mut target_weights = HashMap::new();
        target_weights.insert(btc.clone(), dec!(0.6));
        target_weights.insert(eth.clone(), dec!(0.4));

        let deviations = ctx.calculate_weight_deviation(&target_weights);
        // BTC deviation = 0.5 - 0.6 = -0.1
        // ETH deviation = 0.5 - 0.4 = 0.1
        assert_eq!(deviations.get(&btc).unwrap(), &dec!(-0.1));
        assert_eq!(deviations.get(&eth).unwrap(), &dec!(0.1));
    }

    #[test]
    fn test_context_universe_management() {
        let aggregator = create_test_aggregator();
        let btc = create_test_symbol("BTC-USDT");
        let eth = create_test_symbol("ETH-USDT");

        let ctx = SelStrategyContextImpl::new("test", aggregator);

        // Initially empty
        assert!(ctx.get_universe_owned().is_empty());

        // Set universe
        ctx.set_universe(vec![btc.clone(), eth.clone()]);
        let universe = ctx.get_universe_owned();
        assert_eq!(universe.len(), 2);
        assert!(universe.contains(&btc));
        assert!(universe.contains(&eth));
    }

    #[test]
    fn test_context_current_time() {
        let aggregator = create_test_aggregator();
        let ctx = SelStrategyContextImpl::new("test", aggregator);

        let ts = Timestamp::new(1704067200000).unwrap();
        ctx.set_current_time(ts);
        assert_eq!(ctx.current_time(), ts);
    }

    #[tokio::test]
    async fn test_context_set_positions_batch() {
        let aggregator = create_test_aggregator();
        let btc = create_test_symbol("BTC-USDT");
        let eth = create_test_symbol("ETH-USDT");

        let ctx = SelStrategyContextImpl::new("test", aggregator.clone());

        // Set batch positions
        let mut positions = HashMap::new();
        positions.insert(btc.clone(), Quantity::new(dec!(2.0)).unwrap());
        positions.insert(eth.clone(), Quantity::new(dec!(15.0)).unwrap());

        ctx.set_positions_batch(positions, "rebalance")
            .await
            .unwrap();

        // Positions should be updated
        let all_positions = ctx.get_all_positions();
        assert_eq!(all_positions.get(&btc).unwrap().as_decimal(), dec!(2.0));
        assert_eq!(all_positions.get(&eth).unwrap().as_decimal(), dec!(15.0));

        // Signals should be in aggregator
        let btc_signals = aggregator.get_signals(&btc);
        let eth_signals = aggregator.get_signals(&eth);
        assert_eq!(btc_signals.len(), 1);
        assert_eq!(eth_signals.len(), 1);
    }

    #[tokio::test]
    async fn test_context_batch_skips_unchanged() {
        let aggregator = create_test_aggregator();
        let btc = create_test_symbol("BTC-USDT");

        let ctx = SelStrategyContextImpl::new("test", aggregator.clone());

        // Set initial position
        ctx.set_theoretical_position(&btc, Quantity::new(dec!(1.0)).unwrap());

        // Set same position via batch
        let mut positions = HashMap::new();
        positions.insert(btc.clone(), Quantity::new(dec!(1.0)).unwrap());

        ctx.set_positions_batch(positions, "no_change")
            .await
            .unwrap();

        // No signal should be generated (position unchanged)
        let signals = aggregator.get_signals(&btc);
        assert!(signals.is_empty());
    }
}
