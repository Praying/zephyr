//! Subscription manager for tracking per-strategy subscriptions.
//!
//! The `SubscriptionManager` maintains a mapping of which strategies are
//! subscribed to which market data feeds.

use crate::context::SubscriptionCommand;
use crate::r#trait::{DataType, Subscription};
use std::collections::{HashMap, HashSet};
use tracing::{debug, info};
use zephyr_core::types::Symbol;

/// Key for subscription lookup: (symbol, data_type).
pub type SubscriptionKey = (Symbol, DataType);

/// Manages market data subscriptions per strategy.
///
/// The manager tracks:
/// - Which strategies are subscribed to which (symbol, data_type) pairs
/// - Reverse mapping from strategies to their subscriptions
///
/// # Thread Safety
///
/// The manager is not thread-safe by itself. Use appropriate synchronization
/// (e.g., `Arc<RwLock<SubscriptionManager>>`) if accessed from multiple threads.
///
/// # Example
///
/// ```ignore
/// use zephyr_strategy::subscription::SubscriptionManager;
/// use zephyr_strategy::r#trait::{DataType, Subscription};
/// use zephyr_core::types::Symbol;
///
/// let mut manager = SubscriptionManager::new();
///
/// // Register a strategy with initial subscriptions
/// let subs = vec![Subscription::tick(Symbol::new_unchecked("BTC-USDT"))];
/// manager.register_strategy("my_strategy", subs);
///
/// // Check if a strategy is subscribed
/// assert!(manager.is_subscribed(
///     "my_strategy",
///     &Symbol::new_unchecked("BTC-USDT"),
///     DataType::Tick
/// ));
///
/// // Get all strategies subscribed to a key
/// let strategies = manager.get_subscribers(
///     &Symbol::new_unchecked("BTC-USDT"),
///     DataType::Tick
/// );
/// assert!(strategies.contains(&"my_strategy".to_string()));
/// ```
#[derive(Debug, Default)]
pub struct SubscriptionManager {
    /// Forward mapping: (symbol, data_type) -> set of strategy names
    subscriptions: HashMap<SubscriptionKey, HashSet<String>>,

    /// Reverse mapping: strategy name -> set of (symbol, data_type)
    strategy_subscriptions: HashMap<String, HashSet<SubscriptionKey>>,
}

impl SubscriptionManager {
    /// Creates a new empty subscription manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a strategy with its initial subscriptions.
    ///
    /// This replaces any existing subscriptions for the strategy.
    ///
    /// # Arguments
    ///
    /// * `strategy_name` - The unique name of the strategy
    /// * `subscriptions` - The initial subscriptions declared by the strategy
    pub fn register_strategy(&mut self, strategy_name: &str, subscriptions: Vec<Subscription>) {
        // Remove any existing subscriptions for this strategy
        self.unregister_strategy(strategy_name);

        // Add new subscriptions
        let mut strategy_subs = HashSet::new();
        for sub in subscriptions {
            let key = (sub.symbol.clone(), sub.data_type);

            self.subscriptions
                .entry(key.clone())
                .or_default()
                .insert(strategy_name.to_string());

            strategy_subs.insert(key);
        }

        if !strategy_subs.is_empty() {
            self.strategy_subscriptions
                .insert(strategy_name.to_string(), strategy_subs);
        }

        info!(
            strategy = %strategy_name,
            subscription_count = self.strategy_subscriptions
                .get(strategy_name)
                .map_or(0, HashSet::len),
            "Registered strategy subscriptions"
        );
    }

    /// Unregisters a strategy and removes all its subscriptions.
    ///
    /// # Arguments
    ///
    /// * `strategy_name` - The name of the strategy to unregister
    pub fn unregister_strategy(&mut self, strategy_name: &str) {
        if let Some(keys) = self.strategy_subscriptions.remove(strategy_name) {
            for key in keys {
                if let Some(strategies) = self.subscriptions.get_mut(&key) {
                    strategies.remove(strategy_name);
                    if strategies.is_empty() {
                        self.subscriptions.remove(&key);
                    }
                }
            }
            debug!(strategy = %strategy_name, "Unregistered strategy subscriptions");
        }
    }

    /// Adds a subscription for a strategy.
    ///
    /// # Arguments
    ///
    /// * `strategy_name` - The name of the strategy
    /// * `symbol` - The symbol to subscribe to
    /// * `data_type` - The type of data to receive
    ///
    /// # Returns
    ///
    /// `true` if the subscription was added, `false` if it already existed.
    pub fn subscribe(&mut self, strategy_name: &str, symbol: Symbol, data_type: DataType) -> bool {
        let key = (symbol.clone(), data_type);

        // Check if already subscribed
        if let Some(strategy_subs) = self.strategy_subscriptions.get(strategy_name) {
            if strategy_subs.contains(&key) {
                debug!(
                    strategy = %strategy_name,
                    symbol = %symbol,
                    data_type = %data_type,
                    "Subscription already exists"
                );
                return false;
            }
        }

        // Add to forward mapping
        self.subscriptions
            .entry(key.clone())
            .or_default()
            .insert(strategy_name.to_string());

        // Add to reverse mapping
        self.strategy_subscriptions
            .entry(strategy_name.to_string())
            .or_default()
            .insert(key);

        info!(
            strategy = %strategy_name,
            symbol = %symbol,
            data_type = %data_type,
            "Added subscription"
        );

        true
    }

    /// Removes a subscription for a strategy.
    ///
    /// # Arguments
    ///
    /// * `strategy_name` - The name of the strategy
    /// * `symbol` - The symbol to unsubscribe from
    /// * `data_type` - The type of data to stop receiving
    ///
    /// # Returns
    ///
    /// `true` if the subscription was removed, `false` if it didn't exist.
    pub fn unsubscribe(
        &mut self,
        strategy_name: &str,
        symbol: &Symbol,
        data_type: DataType,
    ) -> bool {
        let key = (symbol.clone(), data_type);

        // Remove from reverse mapping
        let removed =
            if let Some(strategy_subs) = self.strategy_subscriptions.get_mut(strategy_name) {
                let removed = strategy_subs.remove(&key);
                if strategy_subs.is_empty() {
                    self.strategy_subscriptions.remove(strategy_name);
                }
                removed
            } else {
                false
            };

        if !removed {
            debug!(
                strategy = %strategy_name,
                symbol = %symbol,
                data_type = %data_type,
                "Subscription not found"
            );
            return false;
        }

        // Remove from forward mapping
        if let Some(strategies) = self.subscriptions.get_mut(&key) {
            strategies.remove(strategy_name);
            if strategies.is_empty() {
                self.subscriptions.remove(&key);
            }
        }

        info!(
            strategy = %strategy_name,
            symbol = %symbol,
            data_type = %data_type,
            "Removed subscription"
        );

        true
    }

    /// Handles a subscription command from a strategy.
    ///
    /// # Arguments
    ///
    /// * `strategy_name` - The name of the strategy that sent the command
    /// * `command` - The subscription command to process
    pub fn handle_command(&mut self, strategy_name: &str, command: SubscriptionCommand) {
        match command {
            SubscriptionCommand::Add { symbol, data_type } => {
                self.subscribe(strategy_name, symbol, data_type);
            }
            SubscriptionCommand::Remove { symbol, data_type } => {
                self.unsubscribe(strategy_name, &symbol, data_type);
            }
        }
    }

    /// Returns all strategy names subscribed to a specific (symbol, data_type) pair.
    ///
    /// # Arguments
    ///
    /// * `symbol` - The symbol to check
    /// * `data_type` - The data type to check
    ///
    /// # Returns
    ///
    /// A vector of strategy names subscribed to the given key.
    #[must_use]
    pub fn get_subscribers(&self, symbol: &Symbol, data_type: DataType) -> Vec<String> {
        let key = (symbol.clone(), data_type);
        self.subscriptions
            .get(&key)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Returns all strategy names subscribed to a specific key.
    ///
    /// # Arguments
    ///
    /// * `key` - The (symbol, data_type) key to check
    ///
    /// # Returns
    ///
    /// A reference to the set of strategy names, or None if no subscriptions exist.
    #[must_use]
    pub fn get_subscribers_by_key(&self, key: &SubscriptionKey) -> Option<&HashSet<String>> {
        self.subscriptions.get(key)
    }

    /// Checks if a strategy is subscribed to a specific (symbol, data_type) pair.
    ///
    /// # Arguments
    ///
    /// * `strategy_name` - The name of the strategy
    /// * `symbol` - The symbol to check
    /// * `data_type` - The data type to check
    ///
    /// # Returns
    ///
    /// `true` if the strategy is subscribed, `false` otherwise.
    #[must_use]
    pub fn is_subscribed(&self, strategy_name: &str, symbol: &Symbol, data_type: DataType) -> bool {
        let key = (symbol.clone(), data_type);
        self.strategy_subscriptions
            .get(strategy_name)
            .is_some_and(|subs| subs.contains(&key))
    }

    /// Returns all subscriptions for a specific strategy.
    ///
    /// # Arguments
    ///
    /// * `strategy_name` - The name of the strategy
    ///
    /// # Returns
    ///
    /// A vector of (symbol, data_type) pairs the strategy is subscribed to.
    #[must_use]
    pub fn get_strategy_subscriptions(&self, strategy_name: &str) -> Vec<SubscriptionKey> {
        self.strategy_subscriptions
            .get(strategy_name)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Returns the total number of unique subscriptions across all strategies.
    #[must_use]
    pub fn subscription_count(&self) -> usize {
        self.subscriptions.len()
    }

    /// Returns the number of registered strategies.
    #[must_use]
    pub fn strategy_count(&self) -> usize {
        self.strategy_subscriptions.len()
    }

    /// Returns all registered strategy names.
    #[must_use]
    pub fn strategy_names(&self) -> Vec<&str> {
        self.strategy_subscriptions
            .keys()
            .map(String::as_str)
            .collect()
    }

    /// Checks if any strategy is subscribed to the given key.
    #[must_use]
    pub fn has_subscribers(&self, symbol: &Symbol, data_type: DataType) -> bool {
        let key = (symbol.clone(), data_type);
        self.subscriptions.get(&key).is_some_and(|s| !s.is_empty())
    }

    /// Clears all subscriptions.
    pub fn clear(&mut self) {
        self.subscriptions.clear();
        self.strategy_subscriptions.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::r#trait::Timeframe;

    fn btc_symbol() -> Symbol {
        Symbol::new_unchecked("BTC-USDT")
    }

    fn eth_symbol() -> Symbol {
        Symbol::new_unchecked("ETH-USDT")
    }

    #[test]
    fn test_new_manager() {
        let manager = SubscriptionManager::new();
        assert_eq!(manager.subscription_count(), 0);
        assert_eq!(manager.strategy_count(), 0);
    }

    #[test]
    fn test_register_strategy() {
        let mut manager = SubscriptionManager::new();

        let subs = vec![
            Subscription::tick(btc_symbol()),
            Subscription::bar(eth_symbol(), Timeframe::H1),
        ];

        manager.register_strategy("strategy1", subs);

        assert_eq!(manager.strategy_count(), 1);
        assert_eq!(manager.subscription_count(), 2);
        assert!(manager.is_subscribed("strategy1", &btc_symbol(), DataType::Tick));
        assert!(manager.is_subscribed("strategy1", &eth_symbol(), DataType::Bar(Timeframe::H1)));
    }

    #[test]
    fn test_register_strategy_replaces_existing() {
        let mut manager = SubscriptionManager::new();

        // Register with BTC
        manager.register_strategy("strategy1", vec![Subscription::tick(btc_symbol())]);
        assert!(manager.is_subscribed("strategy1", &btc_symbol(), DataType::Tick));

        // Re-register with ETH only
        manager.register_strategy("strategy1", vec![Subscription::tick(eth_symbol())]);
        assert!(!manager.is_subscribed("strategy1", &btc_symbol(), DataType::Tick));
        assert!(manager.is_subscribed("strategy1", &eth_symbol(), DataType::Tick));
    }

    #[test]
    fn test_unregister_strategy() {
        let mut manager = SubscriptionManager::new();

        manager.register_strategy("strategy1", vec![Subscription::tick(btc_symbol())]);
        assert_eq!(manager.strategy_count(), 1);

        manager.unregister_strategy("strategy1");
        assert_eq!(manager.strategy_count(), 0);
        assert_eq!(manager.subscription_count(), 0);
    }

    #[test]
    fn test_subscribe() {
        let mut manager = SubscriptionManager::new();

        // Subscribe to new data
        assert!(manager.subscribe("strategy1", btc_symbol(), DataType::Tick));
        assert!(manager.is_subscribed("strategy1", &btc_symbol(), DataType::Tick));

        // Duplicate subscription returns false
        assert!(!manager.subscribe("strategy1", btc_symbol(), DataType::Tick));
    }

    #[test]
    fn test_unsubscribe() {
        let mut manager = SubscriptionManager::new();

        manager.subscribe("strategy1", btc_symbol(), DataType::Tick);

        // Unsubscribe existing
        assert!(manager.unsubscribe("strategy1", &btc_symbol(), DataType::Tick));
        assert!(!manager.is_subscribed("strategy1", &btc_symbol(), DataType::Tick));

        // Unsubscribe non-existing returns false
        assert!(!manager.unsubscribe("strategy1", &btc_symbol(), DataType::Tick));
    }

    #[test]
    fn test_handle_command_add() {
        let mut manager = SubscriptionManager::new();

        let cmd = SubscriptionCommand::Add {
            symbol: btc_symbol(),
            data_type: DataType::Tick,
        };

        manager.handle_command("strategy1", cmd);
        assert!(manager.is_subscribed("strategy1", &btc_symbol(), DataType::Tick));
    }

    #[test]
    fn test_handle_command_remove() {
        let mut manager = SubscriptionManager::new();

        manager.subscribe("strategy1", btc_symbol(), DataType::Tick);

        let cmd = SubscriptionCommand::Remove {
            symbol: btc_symbol(),
            data_type: DataType::Tick,
        };

        manager.handle_command("strategy1", cmd);
        assert!(!manager.is_subscribed("strategy1", &btc_symbol(), DataType::Tick));
    }

    #[test]
    fn test_get_subscribers() {
        let mut manager = SubscriptionManager::new();

        manager.subscribe("strategy1", btc_symbol(), DataType::Tick);
        manager.subscribe("strategy2", btc_symbol(), DataType::Tick);
        manager.subscribe("strategy3", eth_symbol(), DataType::Tick);

        let btc_subscribers = manager.get_subscribers(&btc_symbol(), DataType::Tick);
        assert_eq!(btc_subscribers.len(), 2);
        assert!(btc_subscribers.contains(&"strategy1".to_string()));
        assert!(btc_subscribers.contains(&"strategy2".to_string()));

        let eth_subscribers = manager.get_subscribers(&eth_symbol(), DataType::Tick);
        assert_eq!(eth_subscribers.len(), 1);
        assert!(eth_subscribers.contains(&"strategy3".to_string()));
    }

    #[test]
    fn test_get_strategy_subscriptions() {
        let mut manager = SubscriptionManager::new();

        manager.subscribe("strategy1", btc_symbol(), DataType::Tick);
        manager.subscribe("strategy1", eth_symbol(), DataType::Bar(Timeframe::H1));

        let subs = manager.get_strategy_subscriptions("strategy1");
        assert_eq!(subs.len(), 2);
        assert!(subs.contains(&(btc_symbol(), DataType::Tick)));
        assert!(subs.contains(&(eth_symbol(), DataType::Bar(Timeframe::H1))));
    }

    #[test]
    fn test_has_subscribers() {
        let mut manager = SubscriptionManager::new();

        assert!(!manager.has_subscribers(&btc_symbol(), DataType::Tick));

        manager.subscribe("strategy1", btc_symbol(), DataType::Tick);
        assert!(manager.has_subscribers(&btc_symbol(), DataType::Tick));

        manager.unsubscribe("strategy1", &btc_symbol(), DataType::Tick);
        assert!(!manager.has_subscribers(&btc_symbol(), DataType::Tick));
    }

    #[test]
    fn test_multiple_strategies_same_subscription() {
        let mut manager = SubscriptionManager::new();

        manager.subscribe("strategy1", btc_symbol(), DataType::Tick);
        manager.subscribe("strategy2", btc_symbol(), DataType::Tick);

        // Both should be subscribed
        assert!(manager.is_subscribed("strategy1", &btc_symbol(), DataType::Tick));
        assert!(manager.is_subscribed("strategy2", &btc_symbol(), DataType::Tick));

        // Unsubscribe one
        manager.unsubscribe("strategy1", &btc_symbol(), DataType::Tick);

        // Other should still be subscribed
        assert!(!manager.is_subscribed("strategy1", &btc_symbol(), DataType::Tick));
        assert!(manager.is_subscribed("strategy2", &btc_symbol(), DataType::Tick));

        // Subscription key should still exist
        assert!(manager.has_subscribers(&btc_symbol(), DataType::Tick));
    }

    #[test]
    fn test_clear() {
        let mut manager = SubscriptionManager::new();

        manager.subscribe("strategy1", btc_symbol(), DataType::Tick);
        manager.subscribe("strategy2", eth_symbol(), DataType::Tick);

        manager.clear();

        assert_eq!(manager.subscription_count(), 0);
        assert_eq!(manager.strategy_count(), 0);
    }

    #[test]
    fn test_strategy_names() {
        let mut manager = SubscriptionManager::new();

        manager.subscribe("strategy1", btc_symbol(), DataType::Tick);
        manager.subscribe("strategy2", eth_symbol(), DataType::Tick);

        let names = manager.strategy_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"strategy1"));
        assert!(names.contains(&"strategy2"));
    }
}
