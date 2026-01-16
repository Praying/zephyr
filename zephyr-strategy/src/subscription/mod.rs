//! Subscription routing for market data.
//!
//! This module provides the `SubscriptionManager` for tracking and routing
//! market data subscriptions per strategy.
//!
//! # Architecture
//!
//! The subscription system works as follows:
//! 1. Strategies declare initial subscriptions via `required_subscriptions()`
//! 2. Strategies can dynamically add/remove subscriptions at runtime
//! 3. The `SubscriptionManager` tracks which strategies are subscribed to which data
//! 4. The `SubscriptionRouter` filters and routes market data to appropriate strategies
//!
//! # Example
//!
//! ```ignore
//! use zephyr_strategy::subscription::{SubscriptionManager, SubscriptionRouter};
//!
//! // Create manager and router
//! let mut manager = SubscriptionManager::new();
//! let router = SubscriptionRouter::new();
//!
//! // Register initial subscriptions
//! manager.register_strategy("my_strategy", subscriptions);
//!
//! // Handle dynamic subscription changes
//! manager.subscribe("my_strategy", symbol, DataType::Tick);
//! manager.unsubscribe("my_strategy", symbol, DataType::Tick);
//!
//! // Route market data
//! let targets = router.route_tick(&tick, &manager);
//! ```

mod manager;
mod router;

pub use manager::SubscriptionManager;
pub use router::SubscriptionRouter;
