//! Security audit logging module.
//!
//! This module provides:
//! - Comprehensive audit logging of security-relevant events
//! - Tamper-evident signatures for audit records
//! - Audit log storage and retrieval
//! - Event filtering and querying
//!
//! # Example
//!
//! ```no_run
//! use zephyr_security::audit::{AuditLogger, AuditEvent, AuditAction};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let logger = AuditLogger::new();
//!
//! // Log an event
//! logger.log_event(AuditEvent::new(
//!     "alice",
//!     AuditAction::TradeExecute,
//!     "Executed buy order for BTC",
//! )).await?;
//!
//! // Query events
//! let events = logger.query_events(Some("alice"), None, None).await?;
//! # Ok(())
//! # }
//! ```

#![allow(clippy::unused_async)]
#![allow(clippy::unnecessary_sort_by)]
#![allow(clippy::must_use_candidate)]

mod event;
mod logger;
mod signature;
mod storage;

pub use event::{AuditAction, AuditEvent, AuditEventFilter};
pub use logger::AuditLogger;
pub use signature::AuditSignature;
pub use storage::AuditStorage;

use crate::error::Result;
use dashmap::DashMap;
use std::sync::Arc;

/// In-memory audit log storage.
pub struct InMemoryAuditStorage {
    events: DashMap<String, Arc<AuditEvent>>,
}

impl InMemoryAuditStorage {
    /// Creates a new in-memory audit storage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            events: DashMap::new(),
        }
    }

    /// Stores an event.
    pub async fn store(&self, event: AuditEvent) -> Result<()> {
        let id = event.id().to_string();
        self.events.insert(id, Arc::new(event));
        Ok(())
    }

    /// Retrieves an event by ID.
    pub async fn get(&self, id: &str) -> Result<Option<Arc<AuditEvent>>> {
        Ok(self.events.get(id).map(|entry| Arc::clone(entry.value())))
    }

    /// Lists all events.
    pub async fn list_all(&self) -> Result<Vec<Arc<AuditEvent>>> {
        Ok(self
            .events
            .iter()
            .map(|entry| Arc::clone(entry.value()))
            .collect())
    }

    /// Queries events by filter.
    pub async fn query(&self, filter: &AuditEventFilter) -> Result<Vec<Arc<AuditEvent>>> {
        let mut results: Vec<_> = self
            .events
            .iter()
            .map(|entry| Arc::clone(entry.value()))
            .collect();

        // Filter by username
        if let Some(username) = &filter.username {
            results.retain(|e| e.username() == username);
        }

        // Filter by action
        if let Some(action) = filter.action {
            results.retain(|e| e.action() == action);
        }

        // Filter by timestamp range
        if let Some(start) = filter.start_time {
            results.retain(|e| e.timestamp() >= start);
        }

        if let Some(end) = filter.end_time {
            results.retain(|e| e.timestamp() <= end);
        }

        // Sort by timestamp descending
        results.sort_by(|a, b| b.timestamp().cmp(&a.timestamp()));

        Ok(results)
    }

    /// Deletes an event.
    pub async fn delete(&self, id: &str) -> Result<bool> {
        Ok(self.events.remove(id).is_some())
    }

    /// Clears all events.
    pub async fn clear(&self) -> Result<()> {
        self.events.clear();
        Ok(())
    }

    /// Returns the number of stored events.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}

impl Default for InMemoryAuditStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AuditStorage for InMemoryAuditStorage {
    async fn store(&self, event: AuditEvent) -> Result<()> {
        let id = event.id().to_string();
        self.events.insert(id, Arc::new(event));
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<Arc<AuditEvent>>> {
        Ok(self.events.get(id).map(|entry| Arc::clone(entry.value())))
    }

    async fn list_all(&self) -> Result<Vec<Arc<AuditEvent>>> {
        Ok(self
            .events
            .iter()
            .map(|entry| Arc::clone(entry.value()))
            .collect())
    }

    async fn query(&self, filter: &AuditEventFilter) -> Result<Vec<Arc<AuditEvent>>> {
        let mut results: Vec<_> = self
            .events
            .iter()
            .map(|entry| Arc::clone(entry.value()))
            .collect();

        // Filter by username
        if let Some(username) = &filter.username {
            results.retain(|e| e.username() == username);
        }

        // Filter by action
        if let Some(action) = filter.action {
            results.retain(|e| e.action() == action);
        }

        // Filter by timestamp range
        if let Some(start) = filter.start_time {
            results.retain(|e| e.timestamp() >= start);
        }

        if let Some(end) = filter.end_time {
            results.retain(|e| e.timestamp() <= end);
        }

        // Sort by timestamp descending
        results.sort_by(|a, b| b.timestamp().cmp(&a.timestamp()));

        Ok(results)
    }

    async fn delete(&self, id: &str) -> Result<bool> {
        Ok(self.events.remove(id).is_some())
    }

    async fn clear(&self) -> Result<()> {
        self.events.clear();
        Ok(())
    }

    fn event_count(&self) -> usize {
        self.events.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_and_retrieve() {
        let storage = InMemoryAuditStorage::new();
        let event = AuditEvent::new("alice", AuditAction::TradeExecute, "Test event");

        storage.store(event.clone()).await.unwrap();
        let retrieved = storage.get(event.id()).await.unwrap();

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().username(), "alice");
    }

    #[tokio::test]
    async fn test_list_all() {
        let storage = InMemoryAuditStorage::new();

        let event1 = AuditEvent::new("alice", AuditAction::TradeExecute, "Event 1");
        let event2 = AuditEvent::new("bob", AuditAction::PositionView, "Event 2");

        storage.store(event1).await.unwrap();
        storage.store(event2).await.unwrap();

        let all = storage.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_query_by_username() {
        let storage = InMemoryAuditStorage::new();

        storage
            .store(AuditEvent::new(
                "alice",
                AuditAction::TradeExecute,
                "Event 1",
            ))
            .await
            .unwrap();
        storage
            .store(AuditEvent::new(
                "alice",
                AuditAction::PositionView,
                "Event 2",
            ))
            .await
            .unwrap();
        storage
            .store(AuditEvent::new("bob", AuditAction::TradeExecute, "Event 3"))
            .await
            .unwrap();

        let filter = AuditEventFilter::new().with_username("alice");
        let results = storage.query(&filter).await.unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|e| e.username() == "alice"));
    }

    #[tokio::test]
    async fn test_query_by_action() {
        let storage = InMemoryAuditStorage::new();

        storage
            .store(AuditEvent::new(
                "alice",
                AuditAction::TradeExecute,
                "Event 1",
            ))
            .await
            .unwrap();
        storage
            .store(AuditEvent::new(
                "alice",
                AuditAction::PositionView,
                "Event 2",
            ))
            .await
            .unwrap();

        let filter = AuditEventFilter::new().with_action(AuditAction::TradeExecute);
        let results = storage.query(&filter).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action(), AuditAction::TradeExecute);
    }

    #[tokio::test]
    async fn test_delete() {
        let storage = InMemoryAuditStorage::new();
        let event = AuditEvent::new("alice", AuditAction::TradeExecute, "Test");

        storage.store(event.clone()).await.unwrap();
        assert_eq!(storage.event_count(), 1);

        let deleted = storage.delete(event.id()).await.unwrap();
        assert!(deleted);
        assert_eq!(storage.event_count(), 0);
    }

    #[tokio::test]
    async fn test_clear() {
        let storage = InMemoryAuditStorage::new();

        storage
            .store(AuditEvent::new(
                "alice",
                AuditAction::TradeExecute,
                "Event 1",
            ))
            .await
            .unwrap();
        storage
            .store(AuditEvent::new("bob", AuditAction::PositionView, "Event 2"))
            .await
            .unwrap();

        assert_eq!(storage.event_count(), 2);

        storage.clear().await.unwrap();
        assert_eq!(storage.event_count(), 0);
    }
}
