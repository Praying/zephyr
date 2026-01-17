//! CTA Strategy Context Implementation.
//!
//! This module provides the implementation of the CTA strategy context,
//! which manages position state, data access, and signal generation for
//! CTA (Commodity Trading Advisor) strategies.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use parking_lot::RwLock;
use tracing::{debug, info, instrument};

use zephyr_core::data::{KlineData, KlinePeriod, TickData};
use zephyr_core::error::StrategyError;
use zephyr_core::traits::{CtaStrategy, CtaStrategyContext, LogLevel};
use zephyr_core::types::{Price, Quantity, Symbol, Timestamp};

use crate::signal::{SignalAggregator, StrategySignal};

/// CTA Strategy Context Implementation.
///
/// Provides data access and position management for CTA strategies.
/// Implements the `CtaStrategyContext` trait.
///
/// # Thread Safety
///
/// This implementation is thread-safe and can be shared across threads.
/// Uses `DashMap` for concurrent position access and `RwLock` for data caches.
pub struct CtaStrategyContextImpl {
    /// Strategy name
    strategy_name: String,
    /// Theoretical positions per symbol (strategy's view)
    positions: DashMap<Symbol, Quantity>,
    /// Current prices per symbol
    prices: DashMap<Symbol, Price>,
    /// K-line data cache per symbol and period
    kline_cache: RwLock<HashMap<(Symbol, KlinePeriod), Vec<KlineData>>>,
    /// Tick data cache per symbol
    tick_cache: RwLock<HashMap<Symbol, Vec<TickData>>>,
    /// Subscribed tick symbols
    tick_subscriptions: RwLock<HashSet<Symbol>>,
    /// Trading symbols for this strategy
    symbols: Vec<Symbol>,
    /// Signal aggregator for position changes
    signal_aggregator: Arc<SignalAggregator>,
    /// Current timestamp (for backtesting support)
    current_time: RwLock<Timestamp>,
    /// Maximum K-line cache size per symbol/period
    max_kline_cache: usize,
    /// Maximum tick cache size per symbol
    max_tick_cache: usize,
}

impl CtaStrategyContextImpl {
    /// Creates a new CTA strategy context.
    ///
    /// # Arguments
    ///
    /// * `strategy_name` - Name of the strategy
    /// * `symbols` - Trading symbols for this strategy
    /// * `signal_aggregator` - Shared signal aggregator
    #[must_use]
    pub fn new(
        strategy_name: impl Into<String>,
        symbols: Vec<Symbol>,
        signal_aggregator: Arc<SignalAggregator>,
    ) -> Self {
        Self {
            strategy_name: strategy_name.into(),
            positions: DashMap::new(),
            prices: DashMap::new(),
            kline_cache: RwLock::new(HashMap::new()),
            tick_cache: RwLock::new(HashMap::new()),
            tick_subscriptions: RwLock::new(HashSet::new()),
            symbols,
            signal_aggregator,
            current_time: RwLock::new(Timestamp::now()),
            max_kline_cache: 1000,
            max_tick_cache: 10000,
        }
    }

    /// Creates a builder for `CtaStrategyContextImpl`.
    #[must_use]
    pub fn builder() -> CtaStrategyContextBuilder {
        CtaStrategyContextBuilder::default()
    }

    /// Updates the current price for a symbol.
    pub fn update_price(&self, symbol: &Symbol, price: Price) {
        self.prices.insert(symbol.clone(), price);
    }

    /// Updates the current timestamp.
    pub fn set_current_time(&self, timestamp: Timestamp) {
        *self.current_time.write() = timestamp;
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

    /// Adds tick data to the cache.
    pub fn add_tick(&self, tick: TickData) {
        // Update price from tick
        self.update_price(&tick.symbol, tick.price);

        let mut cache = self.tick_cache.write();
        let ticks = cache.entry(tick.symbol.clone()).or_default();

        // Maintain cache size limit
        if ticks.len() >= self.max_tick_cache {
            ticks.remove(0);
        }
        ticks.push(tick);
    }

    /// Clears all cached data.
    pub fn clear_cache(&self) {
        self.kline_cache.write().clear();
        self.tick_cache.write().clear();
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
}

#[async_trait]
impl CtaStrategyContext for CtaStrategyContextImpl {
    fn get_position(&self, symbol: &Symbol) -> Quantity {
        self.positions
            .get(symbol)
            .map(|p| *p)
            .unwrap_or(Quantity::ZERO)
    }

    #[instrument(skip(self), fields(strategy = %self.strategy_name))]
    async fn set_position(
        &self,
        symbol: &Symbol,
        qty: Quantity,
        tag: &str,
    ) -> Result<(), StrategyError> {
        let current = self.get_position(symbol);

        if current == qty {
            debug!(
                symbol = %symbol,
                position = %qty,
                tag = %tag,
                "Position unchanged"
            );
            return Ok(());
        }

        info!(
            symbol = %symbol,
            from = %current,
            to = %qty,
            tag = %tag,
            "Setting position"
        );

        // Update local theoretical position
        self.positions.insert(symbol.clone(), qty);

        // Send signal to aggregator
        let signal = StrategySignal {
            strategy_name: self.strategy_name.clone(),
            symbol: symbol.clone(),
            target_position: qty,
            tag: tag.to_string(),
            timestamp: self.current_time(),
        };

        self.signal_aggregator.add_signal(signal).await;

        Ok(())
    }

    fn get_price(&self, symbol: &Symbol) -> Option<Price> {
        self.prices.get(symbol).map(|p| *p)
    }

    fn get_bars(&self, symbol: &Symbol, period: KlinePeriod, count: usize) -> Vec<KlineData> {
        let cache = self.kline_cache.read();
        let key = (symbol.clone(), period);
        cache
            .get(&key)
            .map(|bars| {
                let start = bars.len().saturating_sub(count);
                bars[start..].to_vec()
            })
            .unwrap_or_default()
    }

    fn get_ticks(&self, symbol: &Symbol, count: usize) -> Vec<TickData> {
        let cache = self.tick_cache.read();
        cache
            .get(symbol)
            .map(|ticks| {
                let start = ticks.len().saturating_sub(count);
                ticks[start..].to_vec()
            })
            .unwrap_or_default()
    }

    fn subscribe_ticks(&self, symbol: &Symbol) {
        let mut subs = self.tick_subscriptions.write();
        if subs.insert(symbol.clone()) {
            debug!(symbol = %symbol, "Subscribed to ticks");
        }
    }

    fn unsubscribe_ticks(&self, symbol: &Symbol) {
        let mut subs = self.tick_subscriptions.write();
        if subs.remove(symbol) {
            debug!(symbol = %symbol, "Unsubscribed from ticks");
        }
    }

    fn current_time(&self) -> Timestamp {
        *self.current_time.read()
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

    fn symbols(&self) -> Vec<Symbol> {
        self.symbols.clone()
    }
}

/// Builder for `CtaStrategyContextImpl`.
#[derive(Default)]
pub struct CtaStrategyContextBuilder {
    strategy_name: Option<String>,
    symbols: Vec<Symbol>,
    signal_aggregator: Option<Arc<SignalAggregator>>,
    max_kline_cache: Option<usize>,
    max_tick_cache: Option<usize>,
}

impl CtaStrategyContextBuilder {
    /// Sets the strategy name.
    #[must_use]
    pub fn strategy_name(mut self, name: impl Into<String>) -> Self {
        self.strategy_name = Some(name.into());
        self
    }

    /// Sets the trading symbols.
    #[must_use]
    pub fn symbols(mut self, symbols: Vec<Symbol>) -> Self {
        self.symbols = symbols;
        self
    }

    /// Adds a trading symbol.
    #[must_use]
    pub fn add_symbol(mut self, symbol: Symbol) -> Self {
        self.symbols.push(symbol);
        self
    }

    /// Sets the signal aggregator.
    #[must_use]
    pub fn signal_aggregator(mut self, aggregator: Arc<SignalAggregator>) -> Self {
        self.signal_aggregator = Some(aggregator);
        self
    }

    /// Sets the maximum K-line cache size.
    #[must_use]
    pub fn max_kline_cache(mut self, size: usize) -> Self {
        self.max_kline_cache = Some(size);
        self
    }

    /// Sets the maximum tick cache size.
    #[must_use]
    pub fn max_tick_cache(mut self, size: usize) -> Self {
        self.max_tick_cache = Some(size);
        self
    }

    /// Builds the `CtaStrategyContextImpl`.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing.
    pub fn build(self) -> Result<CtaStrategyContextImpl, &'static str> {
        let strategy_name = self.strategy_name.ok_or("strategy_name is required")?;
        let signal_aggregator = self
            .signal_aggregator
            .ok_or("signal_aggregator is required")?;

        let mut ctx = CtaStrategyContextImpl::new(strategy_name, self.symbols, signal_aggregator);

        if let Some(size) = self.max_kline_cache {
            ctx.max_kline_cache = size;
        }
        if let Some(size) = self.max_tick_cache {
            ctx.max_tick_cache = size;
        }

        Ok(ctx)
    }
}

/// CTA Strategy Runner.
///
/// Manages the lifecycle and execution of CTA strategies.
pub struct CtaStrategyRunner {
    /// Strategy instance
    strategy: Box<dyn CtaStrategy>,
    /// Strategy context
    context: Arc<CtaStrategyContextImpl>,
    /// Whether the strategy is running
    running: RwLock<bool>,
}

impl CtaStrategyRunner {
    /// Creates a new strategy runner.
    #[must_use]
    pub fn new(strategy: Box<dyn CtaStrategy>, context: Arc<CtaStrategyContextImpl>) -> Self {
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

        info!(strategy = %self.strategy.name(), "Starting strategy");

        self.strategy.on_init(self.context.as_ref()).await?;
        *self.running.write() = true;

        info!(strategy = %self.strategy.name(), "Strategy started");
        Ok(())
    }

    /// Stops the strategy.
    pub async fn stop(&mut self) {
        if !*self.running.read() {
            return;
        }

        info!(strategy = %self.strategy.name(), "Stopping strategy");

        self.strategy.on_stop(self.context.as_ref()).await;
        *self.running.write() = false;

        info!(strategy = %self.strategy.name(), "Strategy stopped");
    }

    /// Returns whether the strategy is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        *self.running.read()
    }

    /// Processes a tick event.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if processing fails.
    pub async fn on_tick(&mut self, tick: &TickData) -> Result<(), StrategyError> {
        if !*self.running.read() {
            return Err(StrategyError::NotRunning {
                name: self.strategy.name().to_string(),
            });
        }

        // Update context with tick data
        self.context.add_tick(tick.clone());

        // Call strategy callback
        self.strategy.on_tick(self.context.as_ref(), tick).await
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
    pub fn context(&self) -> &Arc<CtaStrategyContextImpl> {
        &self.context
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_aggregator() -> Arc<SignalAggregator> {
        Arc::new(SignalAggregator::new())
    }

    fn create_test_symbol() -> Symbol {
        Symbol::new("BTC-USDT").unwrap()
    }

    #[test]
    fn test_context_builder() {
        let aggregator = create_test_aggregator();
        let symbol = create_test_symbol();

        let ctx = CtaStrategyContextImpl::builder()
            .strategy_name("test_strategy")
            .add_symbol(symbol.clone())
            .signal_aggregator(aggregator)
            .max_kline_cache(500)
            .build()
            .unwrap();

        assert_eq!(ctx.strategy_name(), "test_strategy");
        assert_eq!(ctx.symbols().len(), 1);
        assert_eq!(ctx.max_kline_cache, 500);
    }

    #[test]
    fn test_context_position_management() {
        let aggregator = create_test_aggregator();
        let symbol = create_test_symbol();

        let ctx = CtaStrategyContextImpl::new("test", vec![symbol.clone()], aggregator);

        // Initial position should be zero
        assert_eq!(ctx.get_position(&symbol), Quantity::ZERO);

        // Set theoretical position
        ctx.set_theoretical_position(&symbol, Quantity::new(dec!(1.5)).unwrap());
        assert_eq!(ctx.get_position(&symbol).as_decimal(), dec!(1.5));
    }

    #[test]
    fn test_context_price_update() {
        let aggregator = create_test_aggregator();
        let symbol = create_test_symbol();

        let ctx = CtaStrategyContextImpl::new("test", vec![symbol.clone()], aggregator);

        // No price initially
        assert!(ctx.get_price(&symbol).is_none());

        // Update price
        ctx.update_price(&symbol, Price::new(dec!(50000)).unwrap());
        assert_eq!(ctx.get_price(&symbol).unwrap().as_decimal(), dec!(50000));
    }

    #[test]
    fn test_context_tick_subscription() {
        let aggregator = create_test_aggregator();
        let symbol = create_test_symbol();

        let ctx = CtaStrategyContextImpl::new("test", vec![symbol.clone()], aggregator);

        // Subscribe
        ctx.subscribe_ticks(&symbol);
        assert!(ctx.tick_subscriptions.read().contains(&symbol));

        // Unsubscribe
        ctx.unsubscribe_ticks(&symbol);
        assert!(!ctx.tick_subscriptions.read().contains(&symbol));
    }

    #[test]
    fn test_context_current_time() {
        let aggregator = create_test_aggregator();
        let ctx = CtaStrategyContextImpl::new("test", vec![], aggregator);

        let ts = Timestamp::new(1704067200000).unwrap();
        ctx.set_current_time(ts);
        assert_eq!(ctx.current_time(), ts);
    }

    #[tokio::test]
    async fn test_context_set_position() {
        let aggregator = create_test_aggregator();
        let symbol = create_test_symbol();

        let ctx = CtaStrategyContextImpl::new("test", vec![symbol.clone()], aggregator.clone());

        // Set position
        ctx.set_position(&symbol, Quantity::new(dec!(2.0)).unwrap(), "test_signal")
            .await
            .unwrap();

        // Position should be updated
        assert_eq!(ctx.get_position(&symbol).as_decimal(), dec!(2.0));

        // Signal should be in aggregator
        let signals = aggregator.get_signals(&symbol);
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].strategy_name, "test");
    }
}
