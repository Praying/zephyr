//! Notification channel implementations.
//!
//! This module provides different notification channels for delivering
//! events to subscribers.

#![allow(clippy::disallowed_types)]
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, info};

use super::NotifierError;
use super::types::TradingEvent;

/// Notification channel configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NotificationChannelConfig {
    /// WebSocket channel configuration.
    WebSocket {
        /// Endpoint URL.
        endpoint: String,
        /// Buffer size for messages.
        #[serde(default = "default_buffer_size")]
        buffer_size: usize,
    },
    /// Webhook channel configuration.
    Webhook {
        /// Webhook URL.
        url: String,
        /// HTTP headers.
        #[serde(default)]
        headers: HashMap<String, String>,
        /// Request timeout.
        #[serde(default = "default_timeout", with = "humantime_serde")]
        timeout: Duration,
        /// Maximum retries.
        #[serde(default = "default_max_retries")]
        max_retries: u32,
    },
    /// Kafka channel configuration.
    Kafka {
        /// Broker addresses.
        brokers: Vec<String>,
        /// Topic name.
        topic: String,
        /// Client ID.
        #[serde(default)]
        client_id: Option<String>,
    },
    /// Redis channel configuration.
    Redis {
        /// Redis URL.
        url: String,
        /// Channel name.
        channel: String,
    },
    /// In-memory channel for testing.
    InMemory {
        /// Buffer size.
        #[serde(default = "default_buffer_size")]
        buffer_size: usize,
    },
}

fn default_buffer_size() -> usize {
    1000
}

fn default_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_max_retries() -> u32 {
    3
}

impl Default for NotificationChannelConfig {
    fn default() -> Self {
        Self::InMemory {
            buffer_size: default_buffer_size(),
        }
    }
}

/// Notification channel trait.
///
/// Defines the interface for delivering events to external systems.
#[async_trait]
pub trait NotificationChannel: Send + Sync {
    /// Returns the channel name.
    fn name(&self) -> &str;

    /// Sends an event through the channel.
    ///
    /// # Errors
    ///
    /// Returns an error if the event cannot be delivered.
    async fn send(&self, event: &TradingEvent) -> Result<(), NotifierError>;

    /// Sends a batch of events through the channel.
    ///
    /// # Errors
    ///
    /// Returns an error if any event cannot be delivered.
    async fn send_batch(&self, events: &[TradingEvent]) -> Result<(), NotifierError> {
        for event in events {
            self.send(event).await?;
        }
        Ok(())
    }

    /// Checks if the channel is connected/healthy.
    fn is_healthy(&self) -> bool;

    /// Closes the channel.
    async fn close(&self) -> Result<(), NotifierError>;
}

/// WebSocket notification channel.
///
/// Delivers events via WebSocket connections.
pub struct WebSocketChannel {
    name: String,
    endpoint: String,
    sender: mpsc::Sender<TradingEvent>,
    healthy: std::sync::atomic::AtomicBool,
}

impl WebSocketChannel {
    /// Creates a new WebSocket channel.
    #[must_use]
    pub fn new(endpoint: impl Into<String>, buffer_size: usize) -> Self {
        let (sender, _receiver) = mpsc::channel(buffer_size);
        Self {
            name: "websocket".to_string(),
            endpoint: endpoint.into(),
            sender,
            healthy: std::sync::atomic::AtomicBool::new(true),
        }
    }

    /// Creates a WebSocket channel from configuration.
    #[must_use]
    pub fn from_config(config: &NotificationChannelConfig) -> Option<Self> {
        match config {
            NotificationChannelConfig::WebSocket {
                endpoint,
                buffer_size,
            } => Some(Self::new(endpoint, *buffer_size)),
            _ => None,
        }
    }

    /// Returns the endpoint URL.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

#[async_trait]
impl NotificationChannel for WebSocketChannel {
    fn name(&self) -> &str {
        &self.name
    }

    async fn send(&self, event: &TradingEvent) -> Result<(), NotifierError> {
        self.sender
            .send(event.clone())
            .await
            .map_err(|e| NotifierError::ChannelError {
                channel: self.name.clone(),
                reason: e.to_string(),
            })?;
        debug!(channel = %self.name, event_id = %event.id(), "Event sent via WebSocket");
        Ok(())
    }

    fn is_healthy(&self) -> bool {
        self.healthy.load(std::sync::atomic::Ordering::Relaxed)
    }

    async fn close(&self) -> Result<(), NotifierError> {
        self.healthy
            .store(false, std::sync::atomic::Ordering::Relaxed);
        info!(channel = %self.name, "WebSocket channel closed");
        Ok(())
    }
}

/// Webhook notification channel.
///
/// Delivers events via HTTP POST requests.
pub struct WebhookChannel {
    name: String,
    url: String,
    headers: HashMap<String, String>,
    #[allow(dead_code)]
    timeout: Duration,
    max_retries: u32,
    client: reqwest::Client,
    healthy: std::sync::atomic::AtomicBool,
}

impl WebhookChannel {
    /// Creates a new Webhook channel.
    pub fn new(
        url: impl Into<String>,
        headers: HashMap<String, String>,
        timeout: Duration,
        max_retries: u32,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();

        Self {
            name: "webhook".to_string(),
            url: url.into(),
            headers,
            timeout,
            max_retries,
            client,
            healthy: std::sync::atomic::AtomicBool::new(true),
        }
    }

    /// Creates a Webhook channel from configuration.
    #[must_use]
    pub fn from_config(config: &NotificationChannelConfig) -> Option<Self> {
        match config {
            NotificationChannelConfig::Webhook {
                url,
                headers,
                timeout,
                max_retries,
            } => Some(Self::new(url, headers.clone(), *timeout, *max_retries)),
            _ => None,
        }
    }

    /// Returns the webhook URL.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    async fn send_with_retry(&self, event: &TradingEvent) -> Result<(), NotifierError> {
        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                let delay = Duration::from_millis(100 * 2u64.pow(attempt - 1));
                tokio::time::sleep(delay).await;
                debug!(
                    channel = %self.name,
                    attempt = attempt,
                    "Retrying webhook delivery"
                );
            }

            let mut request = self.client.post(&self.url);
            for (key, value) in &self.headers {
                request = request.header(key, value);
            }
            request = request.header("Content-Type", "application/json");

            let body =
                serde_json::to_string(event).map_err(|e| NotifierError::SerializationError {
                    reason: e.to_string(),
                })?;

            match request.body(body).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        debug!(
                            channel = %self.name,
                            event_id = %event.id(),
                            status = %response.status(),
                            "Webhook delivery successful"
                        );
                        return Ok(());
                    }
                    last_error = Some(NotifierError::ChannelError {
                        channel: self.name.clone(),
                        reason: format!("HTTP {}", response.status()),
                    });
                }
                Err(e) => {
                    last_error = Some(NotifierError::ChannelError {
                        channel: self.name.clone(),
                        reason: e.to_string(),
                    });
                }
            }
        }

        Err(last_error.unwrap_or_else(|| NotifierError::ChannelError {
            channel: self.name.clone(),
            reason: "Unknown error".to_string(),
        }))
    }
}

#[async_trait]
impl NotificationChannel for WebhookChannel {
    fn name(&self) -> &str {
        &self.name
    }

    async fn send(&self, event: &TradingEvent) -> Result<(), NotifierError> {
        self.send_with_retry(event).await
    }

    fn is_healthy(&self) -> bool {
        self.healthy.load(std::sync::atomic::Ordering::Relaxed)
    }

    async fn close(&self) -> Result<(), NotifierError> {
        self.healthy
            .store(false, std::sync::atomic::Ordering::Relaxed);
        info!(channel = %self.name, "Webhook channel closed");
        Ok(())
    }
}

/// In-memory notification channel for testing.
///
/// Stores events in memory for verification in tests.
pub struct InMemoryChannel {
    name: String,
    events: Arc<parking_lot::RwLock<Vec<TradingEvent>>>,
    buffer_size: usize,
    healthy: std::sync::atomic::AtomicBool,
}

impl InMemoryChannel {
    /// Creates a new in-memory channel.
    #[must_use]
    pub fn new(buffer_size: usize) -> Self {
        Self {
            name: "in_memory".to_string(),
            events: Arc::new(parking_lot::RwLock::new(Vec::with_capacity(buffer_size))),
            buffer_size,
            healthy: std::sync::atomic::AtomicBool::new(true),
        }
    }

    /// Returns the number of events stored in the channel.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.events.read().len()
    }

    /// Returns a copy of all events in the channel.
    #[must_use]
    pub fn events(&self) -> Vec<TradingEvent> {
        self.events.read().clone()
    }

    /// Clears all events from the channel.
    pub fn clear(&self) {
        self.events.write().clear();
    }
}

impl Clone for InMemoryChannel {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            events: Arc::clone(&self.events),
            buffer_size: self.buffer_size,
            healthy: std::sync::atomic::AtomicBool::new(
                self.healthy.load(std::sync::atomic::Ordering::Relaxed),
            ),
        }
    }
}

#[async_trait]
impl NotificationChannel for InMemoryChannel {
    fn name(&self) -> &str {
        &self.name
    }

    async fn send(&self, event: &TradingEvent) -> Result<(), NotifierError> {
        let mut events = self.events.write();
        if events.len() >= self.buffer_size {
            // Remove oldest event to make room
            events.remove(0);
        }
        events.push(event.clone());
        debug!(
            channel = %self.name,
            event_id = %event.id(),
            total_events = events.len(),
            "Event stored in memory"
        );
        Ok(())
    }

    fn is_healthy(&self) -> bool {
        self.healthy.load(std::sync::atomic::Ordering::Relaxed)
    }

    async fn close(&self) -> Result<(), NotifierError> {
        self.healthy
            .store(false, std::sync::atomic::Ordering::Relaxed);
        info!(channel = %self.name, "In-memory channel closed");
        Ok(())
    }
}

/// Creates a notification channel from configuration.
///
/// # Errors
///
/// Returns an error if the channel type is not supported.
pub fn create_channel(
    config: &NotificationChannelConfig,
) -> Result<Arc<dyn NotificationChannel>, NotifierError> {
    match config {
        NotificationChannelConfig::WebSocket {
            endpoint,
            buffer_size,
        } => Ok(Arc::new(WebSocketChannel::new(endpoint, *buffer_size))),
        NotificationChannelConfig::Webhook {
            url,
            headers,
            timeout,
            max_retries,
        } => Ok(Arc::new(WebhookChannel::new(
            url,
            headers.clone(),
            *timeout,
            *max_retries,
        ))),
        NotificationChannelConfig::InMemory { buffer_size } => {
            Ok(Arc::new(InMemoryChannel::new(*buffer_size)))
        }
        NotificationChannelConfig::Kafka { .. } => Err(NotifierError::UnsupportedChannel {
            channel: "kafka".to_string(),
        }),
        NotificationChannelConfig::Redis { .. } => Err(NotifierError::UnsupportedChannel {
            channel: "redis".to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifier::types::{SystemEvent, SystemEventType};

    fn create_test_event() -> TradingEvent {
        TradingEvent::system_event(SystemEvent::new(SystemEventType::Started, "Test event"))
    }

    #[tokio::test]
    async fn test_in_memory_channel_send() {
        let channel = InMemoryChannel::new(100);
        let event = create_test_event();

        channel.send(&event).await.unwrap();

        assert_eq!(channel.event_count(), 1);
        let events = channel.events();
        assert_eq!(events[0].id(), event.id());
    }

    #[tokio::test]
    async fn test_in_memory_channel_buffer_overflow() {
        let channel = InMemoryChannel::new(2);

        for i in 0..5 {
            let event = TradingEvent::system_event(SystemEvent::new(
                SystemEventType::Started,
                format!("Event {i}"),
            ));
            channel.send(&event).await.unwrap();
        }

        // Should only keep the last 2 events
        assert_eq!(channel.event_count(), 2);
    }

    #[tokio::test]
    async fn test_in_memory_channel_clear() {
        let channel = InMemoryChannel::new(100);
        let event = create_test_event();

        channel.send(&event).await.unwrap();
        assert_eq!(channel.event_count(), 1);

        channel.clear();
        assert_eq!(channel.event_count(), 0);
    }

    #[tokio::test]
    async fn test_in_memory_channel_close() {
        let channel = InMemoryChannel::new(100);
        assert!(channel.is_healthy());

        channel.close().await.unwrap();
        assert!(!channel.is_healthy());
    }

    #[test]
    fn test_websocket_channel_from_config() {
        let config = NotificationChannelConfig::WebSocket {
            endpoint: "ws://localhost:8080".to_string(),
            buffer_size: 500,
        };

        let channel = WebSocketChannel::from_config(&config).unwrap();
        assert_eq!(channel.endpoint(), "ws://localhost:8080");
    }

    #[test]
    fn test_webhook_channel_from_config() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer token".to_string());

        let config = NotificationChannelConfig::Webhook {
            url: "https://example.com/webhook".to_string(),
            headers,
            timeout: Duration::from_secs(10),
            max_retries: 5,
        };

        let channel = WebhookChannel::from_config(&config).unwrap();
        assert_eq!(channel.url(), "https://example.com/webhook");
    }

    #[test]
    fn test_create_channel_in_memory() {
        let config = NotificationChannelConfig::InMemory { buffer_size: 100 };
        let channel = create_channel(&config).unwrap();
        assert_eq!(channel.name(), "in_memory");
    }

    #[test]
    fn test_create_channel_unsupported() {
        let config = NotificationChannelConfig::Kafka {
            brokers: vec!["localhost:9092".to_string()],
            topic: "events".to_string(),
            client_id: None,
        };

        let result = create_channel(&config);
        assert!(matches!(
            result,
            Err(NotifierError::UnsupportedChannel { .. })
        ));
    }

    #[test]
    fn test_notification_channel_config_default() {
        let config = NotificationChannelConfig::default();
        assert!(matches!(config, NotificationChannelConfig::InMemory { .. }));
    }

    #[test]
    fn test_notification_channel_config_serde() {
        let config = NotificationChannelConfig::Webhook {
            url: "https://example.com".to_string(),
            headers: HashMap::new(),
            timeout: Duration::from_secs(30),
            max_retries: 3,
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: NotificationChannelConfig = serde_json::from_str(&json).unwrap();

        match parsed {
            NotificationChannelConfig::Webhook { url, .. } => {
                assert_eq!(url, "https://example.com");
            }
            _ => panic!("Expected Webhook config"),
        }
    }
}
