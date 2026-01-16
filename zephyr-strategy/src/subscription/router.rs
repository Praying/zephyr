//! Subscription router for filtering and routing market data.
//!
//! The `SubscriptionRouter` filters market data based on subscriptions
//! and routes it to the appropriate strategy runners.

use crate::runner::RunnerCommand;
use crate::subscription::SubscriptionManager;
use crate::r#trait::DataType;
use crate::types::{Bar, Tick};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing::{debug, warn};
use zephyr_core::types::Symbol;

/// Routes market data to subscribed strategies.
///
/// The router uses the `SubscriptionManager` to determine which strategies
/// should receive each piece of market data, then forwards the data to
/// the appropriate runner channels.
///
/// # Warning Logging
///
/// If a strategy receives data for a symbol it did not subscribe to,
/// the router logs a warning. This indicates a routing bug that should
/// be investigated.
///
/// # Example
///
/// ```ignore
/// use zephyr_strategy::subscription::{SubscriptionManager, SubscriptionRouter};
///
/// let manager = SubscriptionManager::new();
/// let router = SubscriptionRouter::new(runners);
///
/// // Route a tick to subscribed strategies
/// let count = router.route_tick(&tick, &manager).await;
/// println!("Routed tick to {} strategies", count);
/// ```
pub struct SubscriptionRouter {
    /// Command senders for each runner (by strategy name)
    runners: HashMap<String, mpsc::Sender<RunnerCommand>>,
}

impl SubscriptionRouter {
    /// Creates a new subscription router.
    ///
    /// # Arguments
    ///
    /// * `runners` - Map of strategy names to their command channel senders
    #[must_use]
    pub fn new(runners: HashMap<String, mpsc::Sender<RunnerCommand>>) -> Self {
        Self { runners }
    }

    /// Creates an empty router (for testing).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            runners: HashMap::new(),
        }
    }

    /// Adds a runner to the router.
    ///
    /// # Arguments
    ///
    /// * `name` - The strategy name
    /// * `sender` - The command channel sender for the runner
    pub fn add_runner(&mut self, name: String, sender: mpsc::Sender<RunnerCommand>) {
        self.runners.insert(name, sender);
    }

    /// Removes a runner from the router.
    ///
    /// # Arguments
    ///
    /// * `name` - The strategy name to remove
    ///
    /// # Returns
    ///
    /// The removed sender, if it existed.
    pub fn remove_runner(&mut self, name: &str) -> Option<mpsc::Sender<RunnerCommand>> {
        self.runners.remove(name)
    }

    /// Routes a tick to all subscribed strategies.
    ///
    /// # Arguments
    ///
    /// * `tick` - The tick data to route
    /// * `manager` - The subscription manager to check subscriptions
    ///
    /// # Returns
    ///
    /// The number of strategies that received the tick.
    pub async fn route_tick(&self, tick: &Tick, manager: &SubscriptionManager) -> usize {
        let subscribers = manager.get_subscribers(&tick.symbol, DataType::Tick);

        if subscribers.is_empty() {
            debug!(
                symbol = %tick.symbol,
                "No subscribers for tick data"
            );
            return 0;
        }

        let cmd = RunnerCommand::tick(tick.clone());
        self.send_to_strategies(&subscribers, cmd, &tick.symbol, DataType::Tick)
            .await
    }

    /// Routes a tick to all subscribed strategies (sync version using `try_send`).
    ///
    /// This is useful when you need to route data without awaiting.
    ///
    /// # Arguments
    ///
    /// * `tick` - The tick data to route
    /// * `manager` - The subscription manager to check subscriptions
    ///
    /// # Returns
    ///
    /// The number of strategies that received the tick.
    pub fn route_tick_sync(&self, tick: &Tick, manager: &SubscriptionManager) -> usize {
        let subscribers = manager.get_subscribers(&tick.symbol, DataType::Tick);

        if subscribers.is_empty() {
            debug!(
                symbol = %tick.symbol,
                "No subscribers for tick data"
            );
            return 0;
        }

        let cmd = RunnerCommand::tick(tick.clone());
        self.send_to_strategies_sync(&subscribers, cmd, &tick.symbol, DataType::Tick)
    }

    /// Routes a bar to all subscribed strategies.
    ///
    /// # Arguments
    ///
    /// * `bar` - The bar data to route
    /// * `data_type` - The data type (should be `DataType::Bar(timeframe)`)
    /// * `manager` - The subscription manager to check subscriptions
    ///
    /// # Returns
    ///
    /// The number of strategies that received the bar.
    pub async fn route_bar(
        &self,
        bar: &Bar,
        data_type: DataType,
        manager: &SubscriptionManager,
    ) -> usize {
        let subscribers = manager.get_subscribers(&bar.symbol, data_type);

        if subscribers.is_empty() {
            debug!(
                symbol = %bar.symbol,
                data_type = %data_type,
                "No subscribers for bar data"
            );
            return 0;
        }

        let cmd = RunnerCommand::bar(bar.clone());
        self.send_to_strategies(&subscribers, cmd, &bar.symbol, data_type)
            .await
    }

    /// Routes a bar to all subscribed strategies (sync version using `try_send`).
    ///
    /// # Arguments
    ///
    /// * `bar` - The bar data to route
    /// * `data_type` - The data type (should be `DataType::Bar(timeframe)`)
    /// * `manager` - The subscription manager to check subscriptions
    ///
    /// # Returns
    ///
    /// The number of strategies that received the bar.
    pub fn route_bar_sync(
        &self,
        bar: &Bar,
        data_type: DataType,
        manager: &SubscriptionManager,
    ) -> usize {
        let subscribers = manager.get_subscribers(&bar.symbol, data_type);

        if subscribers.is_empty() {
            debug!(
                symbol = %bar.symbol,
                data_type = %data_type,
                "No subscribers for bar data"
            );
            return 0;
        }

        let cmd = RunnerCommand::bar(bar.clone());
        self.send_to_strategies_sync(&subscribers, cmd, &bar.symbol, data_type)
    }

    /// Validates that a strategy should receive data for a given subscription.
    ///
    /// Logs a warning if the strategy is not subscribed to the data.
    /// This indicates a routing bug.
    ///
    /// # Arguments
    ///
    /// * `strategy_name` - The name of the strategy
    /// * `symbol` - The symbol of the data
    /// * `data_type` - The type of data
    /// * `manager` - The subscription manager to check
    ///
    /// # Returns
    ///
    /// `true` if the strategy is subscribed, `false` otherwise.
    pub fn validate_subscription(
        &self,
        strategy_name: &str,
        symbol: &Symbol,
        data_type: DataType,
        manager: &SubscriptionManager,
    ) -> bool {
        let is_subscribed = manager.is_subscribed(strategy_name, symbol, data_type);

        if !is_subscribed {
            warn!(
                strategy = %strategy_name,
                symbol = %symbol,
                data_type = %data_type,
                "Strategy received data for unsubscribed symbol/type - routing bug detected"
            );
        }

        is_subscribed
    }

    /// Sends a command to multiple strategies.
    async fn send_to_strategies(
        &self,
        strategy_names: &[String],
        cmd: RunnerCommand,
        symbol: &Symbol,
        data_type: DataType,
    ) -> usize {
        let mut sent_count = 0;

        for name in strategy_names {
            if let Some(tx) = self.runners.get(name) {
                match tx.send(cmd.clone()).await {
                    Ok(()) => {
                        sent_count += 1;
                    }
                    Err(e) => {
                        warn!(
                            strategy = %name,
                            symbol = %symbol,
                            data_type = %data_type,
                            error = %e,
                            "Failed to send data to runner"
                        );
                    }
                }
            } else {
                warn!(
                    strategy = %name,
                    symbol = %symbol,
                    data_type = %data_type,
                    "Runner not found for subscribed strategy"
                );
            }
        }

        sent_count
    }

    /// Sends a command to multiple strategies (sync version using `try_send`).
    #[allow(clippy::needless_pass_by_value)]
    fn send_to_strategies_sync(
        &self,
        strategy_names: &[String],
        cmd: RunnerCommand,
        symbol: &Symbol,
        data_type: DataType,
    ) -> usize {
        let mut sent_count = 0;

        for name in strategy_names {
            if let Some(tx) = self.runners.get(name) {
                match tx.try_send(cmd.clone()) {
                    Ok(()) => {
                        sent_count += 1;
                    }
                    Err(e) => {
                        warn!(
                            strategy = %name,
                            symbol = %symbol,
                            data_type = %data_type,
                            error = %e,
                            "Failed to send data to runner (sync)"
                        );
                    }
                }
            } else {
                warn!(
                    strategy = %name,
                    symbol = %symbol,
                    data_type = %data_type,
                    "Runner not found for subscribed strategy"
                );
            }
        }

        sent_count
    }

    /// Returns the number of registered runners.
    #[must_use]
    pub fn runner_count(&self) -> usize {
        self.runners.len()
    }

    /// Returns the names of all registered runners.
    #[must_use]
    pub fn runner_names(&self) -> Vec<&str> {
        self.runners.keys().map(String::as_str).collect()
    }

    /// Checks if a runner is registered.
    #[must_use]
    pub fn has_runner(&self, name: &str) -> bool {
        self.runners.contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::r#trait::Timeframe;
    use rust_decimal_macros::dec;
    use zephyr_core::data::{KlineData, KlinePeriod, TickData};
    use zephyr_core::types::{Amount, Price, Quantity, Timestamp};

    fn btc_symbol() -> Symbol {
        Symbol::new_unchecked("BTC-USDT")
    }

    fn eth_symbol() -> Symbol {
        Symbol::new_unchecked("ETH-USDT")
    }

    fn create_test_tick(symbol: Symbol) -> Tick {
        TickData::builder()
            .symbol(symbol)
            .timestamp(Timestamp::new_unchecked(1_704_067_200_000))
            .price(Price::new(dec!(42000)).unwrap())
            .volume(Quantity::new(dec!(0.5)).unwrap())
            .build()
            .unwrap()
    }

    fn create_test_bar(symbol: Symbol) -> Bar {
        KlineData::builder()
            .symbol(symbol)
            .timestamp(Timestamp::new_unchecked(1_704_067_200_000))
            .period(KlinePeriod::Hour1)
            .open(Price::new(dec!(42000)).unwrap())
            .high(Price::new(dec!(42500)).unwrap())
            .low(Price::new(dec!(41800)).unwrap())
            .close(Price::new(dec!(42300)).unwrap())
            .volume(Quantity::new(dec!(100)).unwrap())
            .turnover(Amount::new(dec!(4200000)).unwrap())
            .build()
            .unwrap()
    }

    #[test]
    fn test_new_router() {
        let router = SubscriptionRouter::empty();
        assert_eq!(router.runner_count(), 0);
    }

    #[test]
    fn test_add_remove_runner() {
        let mut router = SubscriptionRouter::empty();
        let (tx, _rx) = mpsc::channel(10);

        router.add_runner("strategy1".to_string(), tx);
        assert!(router.has_runner("strategy1"));
        assert_eq!(router.runner_count(), 1);

        let removed = router.remove_runner("strategy1");
        assert!(removed.is_some());
        assert!(!router.has_runner("strategy1"));
        assert_eq!(router.runner_count(), 0);
    }

    #[test]
    fn test_runner_names() {
        let mut router = SubscriptionRouter::empty();
        let (tx1, _rx1) = mpsc::channel(10);
        let (tx2, _rx2) = mpsc::channel(10);

        router.add_runner("strategy1".to_string(), tx1);
        router.add_runner("strategy2".to_string(), tx2);

        let names = router.runner_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"strategy1"));
        assert!(names.contains(&"strategy2"));
    }

    #[tokio::test]
    async fn test_route_tick_no_subscribers() {
        let router = SubscriptionRouter::empty();
        let manager = SubscriptionManager::new();
        let tick = create_test_tick(btc_symbol());

        let count = router.route_tick(&tick, &manager).await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_route_tick_with_subscribers() {
        let mut router = SubscriptionRouter::empty();
        let mut manager = SubscriptionManager::new();

        let (tx, mut rx) = mpsc::channel(10);
        router.add_runner("strategy1".to_string(), tx);
        manager.subscribe("strategy1", &btc_symbol(), DataType::Tick);

        let tick = create_test_tick(btc_symbol());
        let count = router.route_tick(&tick, &manager).await;

        assert_eq!(count, 1);

        // Verify the command was received
        let cmd = rx.recv().await.unwrap();
        assert!(matches!(cmd, RunnerCommand::OnTick(_)));
    }

    #[tokio::test]
    async fn test_route_tick_multiple_subscribers() {
        let mut router = SubscriptionRouter::empty();
        let mut manager = SubscriptionManager::new();

        let (tx1, mut rx1) = mpsc::channel(10);
        let (tx2, mut rx2) = mpsc::channel(10);

        router.add_runner("strategy1".to_string(), tx1);
        router.add_runner("strategy2".to_string(), tx2);

        manager.subscribe("strategy1", &btc_symbol(), DataType::Tick);
        manager.subscribe("strategy2", &btc_symbol(), DataType::Tick);

        let tick = create_test_tick(btc_symbol());
        let count = router.route_tick(&tick, &manager).await;

        assert_eq!(count, 2);

        // Both should receive the tick
        assert!(rx1.recv().await.is_some());
        assert!(rx2.recv().await.is_some());
    }

    #[tokio::test]
    async fn test_route_tick_filters_by_symbol() {
        let mut router = SubscriptionRouter::empty();
        let mut manager = SubscriptionManager::new();

        let (tx1, mut rx1) = mpsc::channel(10);
        let (tx2, mut rx2) = mpsc::channel(10);

        router.add_runner("strategy1".to_string(), tx1);
        router.add_runner("strategy2".to_string(), tx2);

        // strategy1 subscribes to BTC, strategy2 subscribes to ETH
        manager.subscribe("strategy1", &btc_symbol(), DataType::Tick);
        manager.subscribe("strategy2", &eth_symbol(), DataType::Tick);

        // Send BTC tick
        let btc_tick = create_test_tick(btc_symbol());
        let count = router.route_tick(&btc_tick, &manager).await;

        assert_eq!(count, 1);

        // Only strategy1 should receive it
        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_route_bar() {
        let mut router = SubscriptionRouter::empty();
        let mut manager = SubscriptionManager::new();

        let (tx, mut rx) = mpsc::channel(10);
        router.add_runner("strategy1".to_string(), tx);
        manager.subscribe("strategy1", &btc_symbol(), DataType::Bar(Timeframe::H1));

        let bar = create_test_bar(btc_symbol());
        let count = router
            .route_bar(&bar, DataType::Bar(Timeframe::H1), &manager)
            .await;

        assert_eq!(count, 1);

        let cmd = rx.recv().await.unwrap();
        assert!(matches!(cmd, RunnerCommand::OnBar(_)));
    }

    #[tokio::test]
    async fn test_route_bar_filters_by_timeframe() {
        let mut router = SubscriptionRouter::empty();
        let mut manager = SubscriptionManager::new();

        let (tx1, mut rx1) = mpsc::channel(10);
        let (tx2, mut rx2) = mpsc::channel(10);

        router.add_runner("strategy1".to_string(), tx1);
        router.add_runner("strategy2".to_string(), tx2);

        // strategy1 subscribes to H1, strategy2 subscribes to D1
        manager.subscribe("strategy1", &btc_symbol(), DataType::Bar(Timeframe::H1));
        manager.subscribe("strategy2", &btc_symbol(), DataType::Bar(Timeframe::D1));

        // Send H1 bar
        let bar = create_test_bar(btc_symbol());
        let count = router
            .route_bar(&bar, DataType::Bar(Timeframe::H1), &manager)
            .await;

        assert_eq!(count, 1);

        // Only strategy1 should receive it
        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_err());
    }

    #[test]
    fn test_route_tick_sync() {
        let mut router = SubscriptionRouter::empty();
        let mut manager = SubscriptionManager::new();

        let (tx, mut rx) = mpsc::channel(10);
        router.add_runner("strategy1".to_string(), tx);
        manager.subscribe("strategy1", &btc_symbol(), DataType::Tick);

        let tick = create_test_tick(btc_symbol());
        let count = router.route_tick_sync(&tick, &manager);

        assert_eq!(count, 1);
        assert!(rx.try_recv().is_ok());
    }

    #[test]
    fn test_validate_subscription_valid() {
        let router = SubscriptionRouter::empty();
        let mut manager = SubscriptionManager::new();

        manager.subscribe("strategy1", &btc_symbol(), DataType::Tick);

        let is_valid =
            router.validate_subscription("strategy1", &btc_symbol(), DataType::Tick, &manager);

        assert!(is_valid);
    }

    #[test]
    fn test_validate_subscription_invalid() {
        let router = SubscriptionRouter::empty();
        let manager = SubscriptionManager::new();

        // strategy1 is not subscribed to anything
        let is_valid =
            router.validate_subscription("strategy1", &btc_symbol(), DataType::Tick, &manager);

        assert!(!is_valid);
    }

    #[tokio::test]
    async fn test_route_to_missing_runner() {
        let router = SubscriptionRouter::empty();
        let mut manager = SubscriptionManager::new();

        // Subscribe a strategy but don't add its runner
        manager.subscribe("strategy1", &btc_symbol(), DataType::Tick);

        let tick = create_test_tick(btc_symbol());
        let count = router.route_tick(&tick, &manager).await;

        // Should return 0 since runner is missing
        assert_eq!(count, 0);
    }
}
