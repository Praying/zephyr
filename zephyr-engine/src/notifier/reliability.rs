//! Reliability features for event notification.
//!
//! This module provides reliability mechanisms including:
//! - Retry policies with exponential backoff
//! - Dead letter queue for failed deliveries
//! - Event deduplication

#![allow(clippy::disallowed_types)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::suboptimal_flops)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use super::types::{EventId, TradingEvent};

/// Retry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Initial delay between retries.
    #[serde(default = "default_initial_delay", with = "humantime_serde")]
    pub initial_delay: Duration,
    /// Maximum delay between retries.
    #[serde(default = "default_max_delay", with = "humantime_serde")]
    pub max_delay: Duration,
    /// Backoff multiplier.
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,
    /// Whether to add jitter to delays.
    #[serde(default = "default_jitter")]
    pub jitter: bool,
}

fn default_max_retries() -> u32 {
    3
}

fn default_initial_delay() -> Duration {
    Duration::from_millis(100)
}

fn default_max_delay() -> Duration {
    Duration::from_secs(30)
}

fn default_backoff_multiplier() -> f64 {
    2.0
}

fn default_jitter() -> bool {
    true
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            initial_delay: default_initial_delay(),
            max_delay: default_max_delay(),
            backoff_multiplier: default_backoff_multiplier(),
            jitter: default_jitter(),
        }
    }
}

/// Retry policy for event delivery.
///
/// Implements exponential backoff with optional jitter.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    config: RetryConfig,
}

impl RetryPolicy {
    /// Creates a new retry policy with the given configuration.
    #[must_use]
    pub fn new(config: RetryConfig) -> Self {
        Self { config }
    }

    /// Creates a retry policy with default configuration.
    #[must_use]
    pub fn default_policy() -> Self {
        Self::new(RetryConfig::default())
    }

    /// Returns the maximum number of retries.
    #[must_use]
    pub fn max_retries(&self) -> u32 {
        self.config.max_retries
    }

    /// Calculates the delay for the given attempt number.
    ///
    /// Uses exponential backoff: delay = initial_delay * (multiplier ^ attempt)
    #[must_use]
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }

        let base_delay = self.config.initial_delay.as_millis() as f64
            * self.config.backoff_multiplier.powi(attempt as i32 - 1);

        let delay_ms = base_delay.min(self.config.max_delay.as_millis() as f64);

        let final_delay = if self.config.jitter {
            // Add up to 25% jitter
            let jitter_factor = 1.0 + (rand_jitter() * 0.25);
            delay_ms * jitter_factor
        } else {
            delay_ms
        };

        Duration::from_millis(final_delay as u64)
    }

    /// Returns true if another retry should be attempted.
    #[must_use]
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.config.max_retries
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::default_policy()
    }
}

/// Simple pseudo-random jitter (0.0 to 1.0).
fn rand_jitter() -> f64 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos % 1000) as f64 / 1000.0
}

/// Delivery status for tracking event delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    /// Event is pending delivery.
    Pending,
    /// Event was delivered successfully.
    Delivered,
    /// Event delivery failed after all retries.
    Failed,
    /// Event was moved to dead letter queue.
    DeadLettered,
}

/// Dead letter queue entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterEntry {
    /// The event that failed delivery.
    pub event: TradingEvent,
    /// Number of delivery attempts.
    pub attempts: u32,
    /// Last error message.
    pub last_error: String,
    /// Timestamp when the event was dead-lettered (not serialized).
    #[serde(skip, default = "Instant::now")]
    pub dead_lettered_at: Instant,
    /// Target channel that failed.
    pub channel: String,
}

/// Dead letter queue for failed event deliveries.
///
/// Stores events that could not be delivered after all retry attempts.
pub struct DeadLetterQueue {
    entries: RwLock<VecDeque<DeadLetterEntry>>,
    max_size: usize,
    total_dead_lettered: AtomicU64,
}

impl DeadLetterQueue {
    /// Creates a new dead letter queue with the given maximum size.
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: RwLock::new(VecDeque::with_capacity(max_size)),
            max_size,
            total_dead_lettered: AtomicU64::new(0),
        }
    }

    /// Adds an event to the dead letter queue.
    pub fn add(
        &self,
        event: TradingEvent,
        attempts: u32,
        last_error: impl Into<String>,
        channel: impl Into<String>,
    ) {
        let entry = DeadLetterEntry {
            event,
            attempts,
            last_error: last_error.into(),
            dead_lettered_at: Instant::now(),
            channel: channel.into(),
        };

        let mut entries = self.entries.write();
        if entries.len() >= self.max_size {
            // Remove oldest entry
            if let Some(removed) = entries.pop_front() {
                warn!(
                    event_id = %removed.event.id(),
                    "Dead letter queue full, removing oldest entry"
                );
            }
        }
        entries.push_back(entry);
        self.total_dead_lettered.fetch_add(1, Ordering::Relaxed);

        debug!(
            queue_size = entries.len(),
            "Event added to dead letter queue"
        );
    }

    /// Returns the number of entries in the queue.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Returns true if the queue is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }

    /// Returns the total number of events ever dead-lettered.
    #[must_use]
    pub fn total_dead_lettered(&self) -> u64 {
        self.total_dead_lettered.load(Ordering::Relaxed)
    }

    /// Returns all entries in the queue.
    #[must_use]
    pub fn entries(&self) -> Vec<DeadLetterEntry> {
        self.entries.read().iter().cloned().collect()
    }

    /// Removes and returns the oldest entry.
    pub fn pop(&self) -> Option<DeadLetterEntry> {
        self.entries.write().pop_front()
    }

    /// Clears all entries from the queue.
    pub fn clear(&self) {
        self.entries.write().clear();
    }

    /// Removes entries older than the given duration.
    pub fn remove_older_than(&self, max_age: Duration) -> usize {
        let mut entries = self.entries.write();
        let now = Instant::now();
        let initial_len = entries.len();

        entries.retain(|entry| now.duration_since(entry.dead_lettered_at) < max_age);

        initial_len - entries.len()
    }
}

impl Default for DeadLetterQueue {
    fn default() -> Self {
        Self::new(10000)
    }
}

/// Event deduplicator.
///
/// Tracks recently seen event IDs to prevent duplicate processing.
pub struct EventDeduplicator {
    seen_events: RwLock<HashMap<EventId, Instant>>,
    max_entries: usize,
    ttl: Duration,
    duplicates_detected: AtomicU64,
}

impl EventDeduplicator {
    /// Creates a new event deduplicator.
    ///
    /// # Arguments
    ///
    /// * `max_entries` - Maximum number of event IDs to track
    /// * `ttl` - Time-to-live for tracked event IDs
    #[must_use]
    pub fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            seen_events: RwLock::new(HashMap::with_capacity(max_entries)),
            max_entries,
            ttl,
            duplicates_detected: AtomicU64::new(0),
        }
    }

    /// Creates a deduplicator with default settings.
    #[must_use]
    pub fn default_deduplicator() -> Self {
        Self::new(100_000, Duration::from_secs(3600))
    }

    /// Checks if an event is a duplicate.
    ///
    /// Returns `true` if the event has been seen before (duplicate),
    /// `false` if it's a new event.
    ///
    /// If the event is new, it will be recorded for future deduplication.
    pub fn is_duplicate(&self, event: &TradingEvent) -> bool {
        let event_id = event.id().clone();
        let now = Instant::now();

        // First check with read lock
        {
            let seen = self.seen_events.read();
            if let Some(&timestamp) = seen.get(&event_id) {
                if now.duration_since(timestamp) < self.ttl {
                    self.duplicates_detected.fetch_add(1, Ordering::Relaxed);
                    debug!(event_id = %event_id, "Duplicate event detected");
                    return true;
                }
            }
        }

        // Not a duplicate, record it with write lock
        let mut seen = self.seen_events.write();

        // Double-check after acquiring write lock
        if let Some(&timestamp) = seen.get(&event_id) {
            if now.duration_since(timestamp) < self.ttl {
                self.duplicates_detected.fetch_add(1, Ordering::Relaxed);
                return true;
            }
        }

        // Clean up old entries if at capacity
        if seen.len() >= self.max_entries {
            self.cleanup_expired(&mut seen, now);
        }

        // If still at capacity after cleanup, remove oldest
        if seen.len() >= self.max_entries {
            if let Some(oldest_key) = seen
                .iter()
                .min_by_key(|&(_, ts)| *ts)
                .map(|(k, _)| k.clone())
            {
                seen.remove(&oldest_key);
            }
        }

        seen.insert(event_id, now);
        false
    }

    /// Checks if an event has been seen without recording it.
    #[must_use]
    pub fn has_seen(&self, event: &TradingEvent) -> bool {
        let event_id = event.id();
        let now = Instant::now();

        let seen = self.seen_events.read();
        if let Some(&timestamp) = seen.get(event_id) {
            now.duration_since(timestamp) < self.ttl
        } else {
            false
        }
    }

    /// Returns the number of tracked event IDs.
    #[must_use]
    pub fn tracked_count(&self) -> usize {
        self.seen_events.read().len()
    }

    /// Returns the total number of duplicates detected.
    #[must_use]
    pub fn duplicates_detected(&self) -> u64 {
        self.duplicates_detected.load(Ordering::Relaxed)
    }

    /// Clears all tracked events.
    pub fn clear(&self) {
        self.seen_events.write().clear();
    }

    /// Removes expired entries.
    pub fn cleanup(&self) -> usize {
        let mut seen = self.seen_events.write();
        let now = Instant::now();
        self.cleanup_expired(&mut seen, now)
    }

    fn cleanup_expired(&self, seen: &mut HashMap<EventId, Instant>, now: Instant) -> usize {
        let initial_len = seen.len();
        seen.retain(|_, &mut timestamp| now.duration_since(timestamp) < self.ttl);
        initial_len - seen.len()
    }
}

impl Default for EventDeduplicator {
    fn default() -> Self {
        Self::default_deduplicator()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifier::types::{SystemEvent, SystemEventType};

    fn create_test_event() -> TradingEvent {
        TradingEvent::system_event(SystemEvent::new(SystemEventType::Started, "Test event"))
    }

    // RetryPolicy tests
    #[test]
    fn test_retry_policy_default() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries(), 3);
    }

    #[test]
    fn test_retry_policy_delay_for_attempt() {
        let config = RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: false,
        };
        let policy = RetryPolicy::new(config);

        assert_eq!(policy.delay_for_attempt(0), Duration::ZERO);
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(100));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(200));
        assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(400));
    }

    #[test]
    fn test_retry_policy_max_delay() {
        let config = RetryConfig {
            max_retries: 10,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            jitter: false,
        };
        let policy = RetryPolicy::new(config);

        // After several attempts, should cap at max_delay
        let delay = policy.delay_for_attempt(10);
        assert!(delay <= Duration::from_secs(5));
    }

    #[test]
    fn test_retry_policy_should_retry() {
        let policy = RetryPolicy::default();

        assert!(policy.should_retry(0));
        assert!(policy.should_retry(1));
        assert!(policy.should_retry(2));
        assert!(!policy.should_retry(3));
        assert!(!policy.should_retry(4));
    }

    // DeadLetterQueue tests
    #[test]
    fn test_dead_letter_queue_add() {
        let dlq = DeadLetterQueue::new(100);
        let event = create_test_event();

        dlq.add(event, 3, "Connection refused", "webhook");

        assert_eq!(dlq.len(), 1);
        assert_eq!(dlq.total_dead_lettered(), 1);
    }

    #[test]
    fn test_dead_letter_queue_max_size() {
        let dlq = DeadLetterQueue::new(2);

        for i in 0..5 {
            let event = TradingEvent::system_event(SystemEvent::new(
                SystemEventType::Started,
                format!("Event {i}"),
            ));
            dlq.add(event, 3, "Error", "channel");
        }

        // Should only keep the last 2 entries
        assert_eq!(dlq.len(), 2);
        assert_eq!(dlq.total_dead_lettered(), 5);
    }

    #[test]
    fn test_dead_letter_queue_pop() {
        let dlq = DeadLetterQueue::new(100);
        let event = create_test_event();
        let event_id = event.id().clone();

        dlq.add(event, 3, "Error", "channel");

        let entry = dlq.pop().unwrap();
        assert_eq!(entry.event.id(), &event_id);
        assert!(dlq.is_empty());
    }

    #[test]
    fn test_dead_letter_queue_clear() {
        let dlq = DeadLetterQueue::new(100);
        dlq.add(create_test_event(), 3, "Error", "channel");
        dlq.add(create_test_event(), 3, "Error", "channel");

        assert_eq!(dlq.len(), 2);
        dlq.clear();
        assert!(dlq.is_empty());
    }

    // EventDeduplicator tests
    #[test]
    fn test_deduplicator_new_event() {
        let dedup = EventDeduplicator::new(100, Duration::from_secs(60));
        let event = create_test_event();

        assert!(!dedup.is_duplicate(&event));
        assert_eq!(dedup.tracked_count(), 1);
    }

    #[test]
    fn test_deduplicator_duplicate_event() {
        let dedup = EventDeduplicator::new(100, Duration::from_secs(60));
        let event = create_test_event();

        assert!(!dedup.is_duplicate(&event));
        assert!(dedup.is_duplicate(&event));
        assert_eq!(dedup.duplicates_detected(), 1);
    }

    #[test]
    fn test_deduplicator_has_seen() {
        let dedup = EventDeduplicator::new(100, Duration::from_secs(60));
        let event = create_test_event();

        assert!(!dedup.has_seen(&event));
        dedup.is_duplicate(&event); // Record it
        assert!(dedup.has_seen(&event));
    }

    #[test]
    fn test_deduplicator_max_entries() {
        let dedup = EventDeduplicator::new(2, Duration::from_secs(60));

        for _ in 0..5 {
            let event = create_test_event();
            dedup.is_duplicate(&event);
        }

        // Should not exceed max_entries
        assert!(dedup.tracked_count() <= 2);
    }

    #[test]
    fn test_deduplicator_clear() {
        let dedup = EventDeduplicator::new(100, Duration::from_secs(60));
        let event = create_test_event();

        dedup.is_duplicate(&event);
        assert_eq!(dedup.tracked_count(), 1);

        dedup.clear();
        assert_eq!(dedup.tracked_count(), 0);
    }

    #[test]
    fn test_retry_config_serde() {
        let config = RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 1.5,
            jitter: false,
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: RetryConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.max_retries, 5);
        assert_eq!(parsed.backoff_multiplier, 1.5);
        assert!(!parsed.jitter);
    }
}
