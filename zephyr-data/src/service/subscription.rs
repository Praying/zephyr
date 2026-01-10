//! Subscription management for data service.
//!
//! Manages subscriptions to real-time data feeds with support for:
//! - Multiple symbols and data types per subscription
//! - Async notification delivery
//! - Subscription lifecycle management

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use zephyr_core::data::{KlineData, TickData};
use zephyr_core::types::Symbol;

use super::types::{DataServiceError, DataType, SubscriptionId};

/// A data subscription.
#[derive(Debug, Clone)]
pub struct DataSubscription {
    /// Subscription ID
    pub id: SubscriptionId,
    /// Subscribed symbols
    pub symbols: Vec<Symbol>,
    /// Subscribed data types
    pub data_types: Vec<DataType>,
    /// Whether subscription is active
    pub active: bool,
}

/// Manages data subscriptions.
pub struct SubscriptionManager {
    subscriptions: Arc<RwLock<HashMap<SubscriptionId, DataSubscription>>>,
    next_id: Arc<RwLock<u64>>,
}

impl SubscriptionManager {
    /// Creates a new subscription manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(RwLock::new(1)),
        }
    }

    /// Subscribes to data for the specified symbols and types.
    pub async fn subscribe(
        &self,
        symbols: &[Symbol],
        data_types: &[DataType],
    ) -> Result<SubscriptionId, DataServiceError> {
        if symbols.is_empty() {
            return Err(DataServiceError::internal("No symbols provided"));
        }
        if data_types.is_empty() {
            return Err(DataServiceError::internal("No data types provided"));
        }

        let mut next_id = self.next_id.write().await;
        let id = SubscriptionId::new(*next_id);
        *next_id += 1;

        let subscription = DataSubscription {
            id,
            symbols: symbols.to_vec(),
            data_types: data_types.to_vec(),
            active: true,
        };

        let mut subs = self.subscriptions.write().await;
        subs.insert(id, subscription);

        Ok(id)
    }

    /// Unsubscribes from a subscription.
    pub async fn unsubscribe(&self, id: SubscriptionId) -> Result<(), DataServiceError> {
        let mut subs = self.subscriptions.write().await;
        subs.remove(&id)
            .ok_or(DataServiceError::SubscriptionNotFound(id))?;
        Ok(())
    }

    /// Gets all active subscriptions (async version).
    pub async fn get_all(&self) -> Vec<DataSubscription> {
        let subs = self.subscriptions.read().await;
        subs.values().cloned().collect()
    }

    /// Gets all active subscriptions (synchronous version).
    pub fn get_all_sync(&self) -> Vec<DataSubscription> {
        // This is a synchronous wrapper that uses blocking
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(handle) = rt {
            handle.block_on(async { self.get_all().await })
        } else {
            Vec::new()
        }
    }

    /// Gets a specific subscription.
    pub async fn get(&self, id: SubscriptionId) -> Option<DataSubscription> {
        let subs = self.subscriptions.read().await;
        subs.get(&id).cloned()
    }

    /// Notifies all subscribers of a tick.
    pub async fn notify_tick(&self, tick: &TickData) {
        let subs = self.subscriptions.read().await;
        for sub in subs.values() {
            if !sub.active {
                continue;
            }
            if !sub.symbols.contains(&tick.symbol) {
                continue;
            }
            if !sub.data_types.contains(&DataType::Tick) {
                continue;
            }
            // In a real implementation, this would send to subscribers
            // For now, we just log that we would notify
            tracing::debug!(
                subscription_id = %sub.id,
                symbol = %tick.symbol,
                "Notifying subscriber of tick"
            );
        }
    }

    /// Notifies all subscribers of a kline.
    pub async fn notify_kline(&self, kline: &KlineData) {
        let subs = self.subscriptions.read().await;
        for sub in subs.values() {
            if !sub.active {
                continue;
            }
            if !sub.symbols.contains(&kline.symbol) {
                continue;
            }
            let kline_type = DataType::Kline(kline.period);
            if !sub.data_types.contains(&kline_type) {
                continue;
            }
            // In a real implementation, this would send to subscribers
            tracing::debug!(
                subscription_id = %sub.id,
                symbol = %kline.symbol,
                period = ?kline.period,
                "Notifying subscriber of kline"
            );
        }
    }
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subscribe() {
        let manager = SubscriptionManager::new();
        let symbols = vec![Symbol::new("BTC-USDT").unwrap()];
        let data_types = vec![DataType::Tick];

        let id = manager.subscribe(&symbols, &data_types).await.unwrap();
        assert_eq!(id.as_u64(), 1);

        let sub = manager.get(id).await.unwrap();
        assert_eq!(sub.symbols, symbols);
        assert_eq!(sub.data_types, data_types);
        assert!(sub.active);
    }

    #[tokio::test]
    async fn test_unsubscribe() {
        let manager = SubscriptionManager::new();
        let symbols = vec![Symbol::new("BTC-USDT").unwrap()];
        let data_types = vec![DataType::Tick];

        let id = manager.subscribe(&symbols, &data_types).await.unwrap();
        assert!(manager.get(id).await.is_some());

        manager.unsubscribe(id).await.unwrap();
        assert!(manager.get(id).await.is_none());
    }

    #[tokio::test]
    async fn test_multiple_subscriptions() {
        let manager = SubscriptionManager::new();
        let symbols1 = vec![Symbol::new("BTC-USDT").unwrap()];
        let symbols2 = vec![Symbol::new("ETH-USDT").unwrap()];
        let data_types = vec![DataType::Tick];

        let id1 = manager.subscribe(&symbols1, &data_types).await.unwrap();
        let id2 = manager.subscribe(&symbols2, &data_types).await.unwrap();

        assert_ne!(id1, id2);
        assert_eq!(id1.as_u64(), 1);
        assert_eq!(id2.as_u64(), 2);

        let all = manager.get_all().await;
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_subscribe_empty_symbols() {
        let manager = SubscriptionManager::new();
        let symbols = vec![];
        let data_types = vec![DataType::Tick];

        let result = manager.subscribe(&symbols, &data_types).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_subscribe_empty_data_types() {
        let manager = SubscriptionManager::new();
        let symbols = vec![Symbol::new("BTC-USDT").unwrap()];
        let data_types = vec![];

        let result = manager.subscribe(&symbols, &data_types).await;
        assert!(result.is_err());
    }
}
