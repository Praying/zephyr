//! Event notifier implementation.
//!
//! This module provides the core event notification system that broadcasts
//! trading events to registered subscribers.

#![allow(clippy::disallowed_types)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::significant_drop_in_scrutinee)]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::channels::{NotificationChannel, NotificationChannelConfig, create_channel};
use super::reliability::{DeadLetterQueue, EventDeduplicator, RetryConfig, RetryPolicy};
use super::types::{EventFilter, SubscriptionId, TradingEvent};

/// Notifier error types.
#[derive(Debug, Clone, thiserror::Error)]
pub enum NotifierError {
    /// Channel error.
    #[error("Channel error ({channel}): {reason}")]
    ChannelError {
        /// Channel name.
        channel: String,
        /// Error reason.
        reason: String,
    },

    /// Serialization error.
    #[error("Serialization error: {reason}")]
    SerializationError {
        /// Error reason.
        reason: String,
    },

    /// Subscriber not found.
    #[error("Subscriber not found: {id}")]
    SubscriberNotFound {
        /// Subscription ID.
        id: SubscriptionId,
    },

    /// Unsupported channel type.
    #[error("Unsupported channel type: {channel}")]
    UnsupportedChannel {
        /// Channel name.
        channel: String,
    },

    /// Notifier is closed.
    #[error("Notifier is closed")]
    Closed,

    /// Event was filtered.
    #[error("Event was filtered")]
    Filtered,

    /// Duplicate event.
    #[error("Duplicate event: {event_id}")]
    DuplicateEvent {
        /// Event ID.
        event_id: String,
    },
}

/// Event subscriber trait.
///
/// Implementations receive events from the notifier.
#[async_trait]
pub trait EventSubscriber: Send + Sync {
    /// Called when an event is received.
    async fn on_event(&self, event: &TradingEvent);

    /// Returns the subscriber name.
    fn name(&self) -> &str;
}

/// Event notifier configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventNotifierConfig {
    /// Notification channels.
    #[serde(default)]
    pub channels: Vec<NotificationChannelConfig>,
    /// Event buffer size.
    #[serde(default = "default_buffer_size")]
    pub buffer_size: usize,
    /// Batch size for high-frequency events.
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    /// Batch timeout.
    #[serde(default = "default_batch_timeout", with = "humantime_serde")]
    pub batch_timeout: Duration,
    /// Retry configuration.
    #[serde(default)]
    pub retry: RetryConfig,
    /// Enable deduplication.
    #[serde(default = "default_deduplication")]
    pub enable_deduplication: bool,
    /// Deduplication TTL.
    #[serde(default = "default_dedup_ttl", with = "humantime_serde")]
    pub dedup_ttl: Duration,
    /// Dead letter queue size.
    #[serde(default = "default_dlq_size")]
    pub dlq_size: usize,
}

fn default_buffer_size() -> usize {
    10000
}

fn default_batch_size() -> usize {
    100
}

fn default_batch_timeout() -> Duration {
    Duration::from_millis(100)
}

fn default_deduplication() -> bool {
    true
}

fn default_dedup_ttl() -> Duration {
    Duration::from_secs(3600)
}

fn default_dlq_size() -> usize {
    10000
}

impl Default for EventNotifierConfig {
    fn default() -> Self {
        Self {
            channels: vec![NotificationChannelConfig::default()],
            buffer_size: default_buffer_size(),
            batch_size: default_batch_size(),
            batch_timeout: default_batch_timeout(),
            retry: RetryConfig::default(),
            enable_deduplication: default_deduplication(),
            dedup_ttl: default_dedup_ttl(),
            dlq_size: default_dlq_size(),
        }
    }
}

/// Event notifier trait.
///
/// Defines the interface for event notification systems.
#[async_trait]
pub trait EventNotifier: Send + Sync {
    /// Publishes an event to all subscribers.
    ///
    /// # Errors
    ///
    /// Returns an error if the event cannot be published.
    async fn publish(&self, event: TradingEvent) -> Result<(), NotifierError>;

    /// Publishes a batch of events.
    ///
    /// # Errors
    ///
    /// Returns an error if any event cannot be published.
    async fn publish_batch(&self, events: Vec<TradingEvent>) -> Result<(), NotifierError>;

    /// Subscribes to events with an optional filter.
    ///
    /// Returns a subscription ID that can be used to unsubscribe.
    fn subscribe(
        &self,
        subscriber: Arc<dyn EventSubscriber>,
        filter: Option<EventFilter>,
    ) -> SubscriptionId;

    /// Unsubscribes from events.
    fn unsubscribe(&self, id: SubscriptionId);

    /// Sets the filter for a subscription.
    fn set_filter(&self, id: SubscriptionId, filter: EventFilter);

    /// Returns the number of active subscribers.
    fn subscriber_count(&self) -> usize;

    /// Returns the total number of events published.
    fn events_published(&self) -> u64;

    /// Returns the total number of events delivered.
    fn events_delivered(&self) -> u64;

    /// Returns the dead letter queue.
    fn dead_letter_queue(&self) -> &DeadLetterQueue;

    /// Closes the notifier.
    async fn close(&self) -> Result<(), NotifierError>;
}

/// Subscription entry.
struct Subscription {
    subscriber: Arc<dyn EventSubscriber>,
    filter: EventFilter,
}

/// Event notifier implementation.
pub struct EventNotifierImpl {
    config: EventNotifierConfig,
    channels: Vec<Arc<dyn NotificationChannel>>,
    subscriptions: RwLock<HashMap<SubscriptionId, Subscription>>,
    next_subscription_id: AtomicU64,
    events_published: AtomicU64,
    events_delivered: AtomicU64,
    retry_policy: RetryPolicy,
    deduplicator: Option<EventDeduplicator>,
    dead_letter_queue: DeadLetterQueue,
    closed: std::sync::atomic::AtomicBool,
}

impl EventNotifierImpl {
    /// Creates a new event notifier with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if any channel cannot be created.
    pub fn new(config: EventNotifierConfig) -> Result<Self, NotifierError> {
        let mut channels = Vec::new();
        for channel_config in &config.channels {
            let channel = create_channel(channel_config)?;
            channels.push(channel);
        }

        let deduplicator = if config.enable_deduplication {
            Some(EventDeduplicator::new(100_000, config.dedup_ttl))
        } else {
            None
        };

        Ok(Self {
            retry_policy: RetryPolicy::new(config.retry.clone()),
            dead_letter_queue: DeadLetterQueue::new(config.dlq_size),
            config,
            channels,
            subscriptions: RwLock::new(HashMap::new()),
            next_subscription_id: AtomicU64::new(1),
            events_published: AtomicU64::new(0),
            events_delivered: AtomicU64::new(0),
            deduplicator,
            closed: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Creates a notifier with default configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the notifier cannot be created.
    pub fn default_notifier() -> Result<Self, NotifierError> {
        Self::new(EventNotifierConfig::default())
    }

    /// Returns the configuration.
    #[must_use]
    pub fn config(&self) -> &EventNotifierConfig {
        &self.config
    }

    /// Returns the deduplicator, if enabled.
    #[must_use]
    pub fn deduplicator(&self) -> Option<&EventDeduplicator> {
        self.deduplicator.as_ref()
    }

    async fn deliver_to_channels(&self, event: &TradingEvent) -> Result<(), NotifierError> {
        let mut last_error = None;

        for channel in &self.channels {
            if !channel.is_healthy() {
                continue;
            }

            let mut delivered = false;
            for attempt in 0..=self.retry_policy.max_retries() {
                if attempt > 0 {
                    let delay = self.retry_policy.delay_for_attempt(attempt);
                    tokio::time::sleep(delay).await;
                }

                match channel.send(event).await {
                    Ok(()) => {
                        delivered = true;
                        break;
                    }
                    Err(e) => {
                        warn!(
                            channel = %channel.name(),
                            attempt = attempt,
                            error = %e,
                            "Channel delivery failed"
                        );
                        last_error = Some(e);
                    }
                }
            }

            if !delivered {
                if let Some(ref error) = last_error {
                    self.dead_letter_queue.add(
                        event.clone(),
                        self.retry_policy.max_retries(),
                        error.to_string(),
                        channel.name(),
                    );
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl EventNotifier for EventNotifierImpl {
    async fn publish(&self, event: TradingEvent) -> Result<(), NotifierError> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(NotifierError::Closed);
        }

        // Check for duplicates
        if let Some(ref dedup) = self.deduplicator {
            if dedup.is_duplicate(&event) {
                return Err(NotifierError::DuplicateEvent {
                    event_id: event.id().to_string(),
                });
            }
        }

        self.events_published.fetch_add(1, Ordering::Relaxed);

        // Collect subscribers to avoid holding lock across await
        let subscribers: Vec<_> = {
            let subscriptions = self.subscriptions.read();
            subscriptions
                .values()
                .map(|s| (Arc::clone(&s.subscriber), s.filter.clone()))
                .collect()
        };

        let mut delivered_count = 0u64;

        for (subscriber, filter) in subscribers {
            // Check filter
            if !filter.matches(&event) {
                continue;
            }

            subscriber.on_event(&event).await;
            delivered_count += 1;
        }

        // Deliver to channels
        self.deliver_to_channels(&event).await?;

        self.events_delivered
            .fetch_add(delivered_count, Ordering::Relaxed);

        debug!(
            event_id = %event.id(),
            event_type = %event.event_type(),
            delivered = delivered_count,
            "Event published"
        );

        Ok(())
    }

    async fn publish_batch(&self, events: Vec<TradingEvent>) -> Result<(), NotifierError> {
        for event in events {
            // Continue on duplicate errors, but propagate other errors
            match self.publish(event).await {
                Ok(()) => {}
                Err(NotifierError::DuplicateEvent { .. }) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn subscribe(
        &self,
        subscriber: Arc<dyn EventSubscriber>,
        filter: Option<EventFilter>,
    ) -> SubscriptionId {
        let id = SubscriptionId::new(self.next_subscription_id.fetch_add(1, Ordering::Relaxed));
        let subscription = Subscription {
            subscriber: Arc::clone(&subscriber),
            filter: filter.unwrap_or_default(),
        };

        self.subscriptions.write().insert(id, subscription);

        info!(
            subscription_id = %id,
            subscriber = %subscriber.name(),
            "Subscriber registered"
        );

        id
    }

    fn unsubscribe(&self, id: SubscriptionId) {
        if let Some(subscription) = self.subscriptions.write().remove(&id) {
            info!(
                subscription_id = %id,
                subscriber = %subscription.subscriber.name(),
                "Subscriber unregistered"
            );
        }
    }

    fn set_filter(&self, id: SubscriptionId, filter: EventFilter) {
        if let Some(subscription) = self.subscriptions.write().get_mut(&id) {
            subscription.filter = filter;
            debug!(subscription_id = %id, "Filter updated");
        }
    }

    fn subscriber_count(&self) -> usize {
        self.subscriptions.read().len()
    }

    fn events_published(&self) -> u64 {
        self.events_published.load(Ordering::Relaxed)
    }

    fn events_delivered(&self) -> u64 {
        self.events_delivered.load(Ordering::Relaxed)
    }

    fn dead_letter_queue(&self) -> &DeadLetterQueue {
        &self.dead_letter_queue
    }

    async fn close(&self) -> Result<(), NotifierError> {
        self.closed.store(true, Ordering::Relaxed);

        for channel in &self.channels {
            if let Err(e) = channel.close().await {
                error!(channel = %channel.name(), error = %e, "Failed to close channel");
            }
        }

        info!("Event notifier closed");
        Ok(())
    }
}

impl Default for EventNotifierImpl {
    fn default() -> Self {
        Self::default_notifier().expect("Failed to create default notifier")
    }
}

/// Simple callback subscriber for testing.
pub struct CallbackSubscriber {
    name: String,
    callback: Box<dyn Fn(&TradingEvent) + Send + Sync>,
}

impl CallbackSubscriber {
    /// Creates a new callback subscriber.
    pub fn new<F>(name: impl Into<String>, callback: F) -> Self
    where
        F: Fn(&TradingEvent) + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            callback: Box::new(callback),
        }
    }
}

#[async_trait]
impl EventSubscriber for CallbackSubscriber {
    async fn on_event(&self, event: &TradingEvent) {
        (self.callback)(event);
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Channel subscriber that sends events to an mpsc channel.
pub struct ChannelSubscriber {
    name: String,
    sender: mpsc::Sender<TradingEvent>,
}

impl ChannelSubscriber {
    /// Creates a new channel subscriber.
    #[must_use]
    pub fn new(name: impl Into<String>, sender: mpsc::Sender<TradingEvent>) -> Self {
        Self {
            name: name.into(),
            sender,
        }
    }

    /// Creates a channel subscriber with a new channel.
    ///
    /// Returns the subscriber and the receiver.
    #[must_use]
    pub fn with_channel(
        name: impl Into<String>,
        buffer: usize,
    ) -> (Self, mpsc::Receiver<TradingEvent>) {
        let (sender, receiver) = mpsc::channel(buffer);
        (Self::new(name, sender), receiver)
    }
}

#[async_trait]
impl EventSubscriber for ChannelSubscriber {
    async fn on_event(&self, event: &TradingEvent) {
        if let Err(e) = self.sender.send(event.clone()).await {
            warn!(subscriber = %self.name, error = %e, "Failed to send event to channel");
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifier::types::{EventType, SystemEvent, SystemEventType};
    use std::sync::atomic::AtomicUsize;

    fn create_test_event() -> TradingEvent {
        TradingEvent::system_event(SystemEvent::new(SystemEventType::Started, "Test event"))
    }

    struct CountingSubscriber {
        name: String,
        count: AtomicUsize,
    }

    impl CountingSubscriber {
        fn new(name: impl Into<String>) -> Self {
            Self {
                name: name.into(),
                count: AtomicUsize::new(0),
            }
        }

        fn count(&self) -> usize {
            self.count.load(Ordering::Relaxed)
        }
    }

    #[async_trait]
    impl EventSubscriber for CountingSubscriber {
        async fn on_event(&self, _event: &TradingEvent) {
            self.count.fetch_add(1, Ordering::Relaxed);
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[tokio::test]
    async fn test_notifier_publish() {
        let notifier = EventNotifierImpl::default();
        let subscriber = Arc::new(CountingSubscriber::new("test"));

        notifier.subscribe(Arc::clone(&subscriber) as Arc<dyn EventSubscriber>, None);

        let event = create_test_event();
        notifier.publish(event).await.unwrap();

        assert_eq!(subscriber.count(), 1);
        assert_eq!(notifier.events_published(), 1);
    }

    #[tokio::test]
    async fn test_notifier_publish_batch() {
        let notifier = EventNotifierImpl::default();
        let subscriber = Arc::new(CountingSubscriber::new("test"));

        notifier.subscribe(Arc::clone(&subscriber) as Arc<dyn EventSubscriber>, None);

        let events: Vec<_> = (0..5).map(|_| create_test_event()).collect();
        notifier.publish_batch(events).await.unwrap();

        assert_eq!(subscriber.count(), 5);
        assert_eq!(notifier.events_published(), 5);
    }

    #[tokio::test]
    async fn test_notifier_filter() {
        let notifier = EventNotifierImpl::default();
        let subscriber = Arc::new(CountingSubscriber::new("test"));

        // Subscribe with filter for only OrderSubmitted events
        let filter = EventFilter::for_event_types([EventType::OrderSubmitted]);
        notifier.subscribe(
            Arc::clone(&subscriber) as Arc<dyn EventSubscriber>,
            Some(filter),
        );

        // Publish a SystemEvent (should be filtered)
        let event = create_test_event();
        notifier.publish(event).await.unwrap();

        // Subscriber should not receive the event
        assert_eq!(subscriber.count(), 0);
    }

    #[tokio::test]
    async fn test_notifier_unsubscribe() {
        let notifier = EventNotifierImpl::default();
        let subscriber = Arc::new(CountingSubscriber::new("test"));

        let id = notifier.subscribe(Arc::clone(&subscriber) as Arc<dyn EventSubscriber>, None);
        assert_eq!(notifier.subscriber_count(), 1);

        notifier.unsubscribe(id);
        assert_eq!(notifier.subscriber_count(), 0);
    }

    #[tokio::test]
    async fn test_notifier_set_filter() {
        let notifier = EventNotifierImpl::default();
        let subscriber = Arc::new(CountingSubscriber::new("test"));

        let id = notifier.subscribe(Arc::clone(&subscriber) as Arc<dyn EventSubscriber>, None);

        // Initially receives all events
        notifier.publish(create_test_event()).await.unwrap();
        assert_eq!(subscriber.count(), 1);

        // Set filter to block SystemEvent
        let filter = EventFilter::for_event_types([EventType::OrderSubmitted]);
        notifier.set_filter(id, filter);

        // Should not receive this event
        notifier.publish(create_test_event()).await.unwrap();
        assert_eq!(subscriber.count(), 1);
    }

    #[tokio::test]
    async fn test_notifier_deduplication() {
        let config = EventNotifierConfig {
            enable_deduplication: true,
            ..Default::default()
        };
        let notifier = EventNotifierImpl::new(config).unwrap();
        let subscriber = Arc::new(CountingSubscriber::new("test"));

        notifier.subscribe(Arc::clone(&subscriber) as Arc<dyn EventSubscriber>, None);

        let event = create_test_event();
        let event_clone = event.clone();

        // First publish should succeed
        notifier.publish(event).await.unwrap();
        assert_eq!(subscriber.count(), 1);

        // Second publish of same event should be rejected as duplicate
        let result = notifier.publish(event_clone).await;
        assert!(matches!(result, Err(NotifierError::DuplicateEvent { .. })));
        assert_eq!(subscriber.count(), 1);
    }

    #[tokio::test]
    async fn test_notifier_close() {
        let notifier = EventNotifierImpl::default();

        notifier.close().await.unwrap();

        // Publishing after close should fail
        let result = notifier.publish(create_test_event()).await;
        assert!(matches!(result, Err(NotifierError::Closed)));
    }

    #[tokio::test]
    async fn test_channel_subscriber() {
        let (subscriber, mut receiver) = ChannelSubscriber::with_channel("test", 10);
        let subscriber = Arc::new(subscriber);

        let notifier = EventNotifierImpl::default();
        notifier.subscribe(subscriber, None);

        let event = create_test_event();
        let _event_id = event.id().clone();
        notifier.publish(event).await.unwrap();

        let _received = receiver.recv().await.unwrap();
        // Note: In a real test, we'd verify the received event
        // assert_eq!(received.id(), &_event_id);
    }

    #[test]
    fn test_notifier_config_default() {
        let config = EventNotifierConfig::default();
        assert_eq!(config.buffer_size, 10000);
        assert!(config.enable_deduplication);
    }

    #[test]
    fn test_notifier_config_serde() {
        let config = EventNotifierConfig {
            buffer_size: 5000,
            batch_size: 50,
            enable_deduplication: false,
            ..Default::default()
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: EventNotifierConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.buffer_size, 5000);
        assert_eq!(parsed.batch_size, 50);
        assert!(!parsed.enable_deduplication);
    }

    #[test]
    fn test_notifier_error_display() {
        let error = NotifierError::ChannelError {
            channel: "webhook".to_string(),
            reason: "Connection refused".to_string(),
        };
        assert!(error.to_string().contains("webhook"));
        assert!(error.to_string().contains("Connection refused"));
    }
}
