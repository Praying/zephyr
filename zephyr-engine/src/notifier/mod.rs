//! Event notification system for the Zephyr trading platform.
//!
//! This module provides a comprehensive event notification system that broadcasts
//! trading events to registered subscribers through multiple channels.
//!
//! # Features
//!
//! - Multiple notification channels (WebSocket, Webhook, Kafka, Redis)
//! - Event filtering by type, symbol, and strategy
//! - At-least-once delivery guarantee with retry policies
//! - Event batching for high-frequency events
//! - Dead letter queue for failed deliveries
//! - Event deduplication
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────┐
//! │  Trading Engine │
//! └────────┬────────┘
//!          │ publish()
//!          ▼
//! ┌─────────────────┐
//! │ Event Notifier  │
//! │  ┌───────────┐  │
//! │  │  Filter   │  │
//! │  └─────┬─────┘  │
//! │        │        │
//! │  ┌─────▼─────┐  │
//! │  │  Batcher  │  │
//! │  └─────┬─────┘  │
//! │        │        │
//! │  ┌─────▼─────┐  │
//! │  │ Dispatcher│  │
//! │  └─────┬─────┘  │
//! └────────┼────────┘
//!          │
//!    ┌─────┼─────┬─────────┐
//!    ▼     ▼     ▼         ▼
//! ┌─────┐┌─────┐┌─────┐┌─────┐
//! │ WS  ││Webhk││Kafka││Redis│
//! └─────┘└─────┘└─────┘└─────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use zephyr_engine::notifier::{EventNotifier, EventNotifierImpl, EventFilter, TradingEvent};
//!
//! // Create notifier
//! let notifier = EventNotifierImpl::new(config);
//!
//! // Subscribe with filter
//! let filter = EventFilter::for_event_types([EventType::OrderFilled]);
//! let sub_id = notifier.subscribe(subscriber, Some(filter));
//!
//! // Publish event
//! notifier.publish(TradingEvent::order_filled(order, trade)).await?;
//! ```

#![allow(clippy::module_inception)]

mod channels;
mod notifier;
mod reliability;
mod types;

pub use channels::{
    NotificationChannel, NotificationChannelConfig, WebSocketChannel, WebhookChannel,
};
pub use notifier::{
    CallbackSubscriber, ChannelSubscriber, EventNotifier, EventNotifierConfig, EventNotifierImpl,
    EventSubscriber, NotifierError,
};
pub use reliability::{
    DeadLetterQueue, DeliveryStatus, EventDeduplicator, RetryConfig, RetryPolicy,
};
pub use types::{
    AlertSeverity, BalanceChange, EventFilter, EventId, EventType, RiskAlert, StrategySignalEvent,
    SubscriptionId, SystemEvent, SystemEventType, TradingEvent,
};
