//! Audit log storage interface.

use super::event::{AuditEvent, AuditEventFilter};
use crate::error::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// Trait for audit log storage backends.
///
/// Implementations can store audit logs in different backends
/// (in-memory, file, database, etc.).
#[async_trait]
pub trait AuditStorage: Send + Sync {
    /// Stores an audit event.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to store
    ///
    /// # Errors
    ///
    /// Returns an error if storage fails.
    async fn store(&self, event: AuditEvent) -> Result<()>;

    /// Retrieves an event by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The event ID
    ///
    /// # Errors
    ///
    /// Returns an error if retrieval fails.
    async fn get(&self, id: &str) -> Result<Option<Arc<AuditEvent>>>;

    /// Lists all events.
    ///
    /// # Errors
    ///
    /// Returns an error if listing fails.
    async fn list_all(&self) -> Result<Vec<Arc<AuditEvent>>>;

    /// Queries events by filter.
    ///
    /// # Arguments
    ///
    /// * `filter` - The filter to apply
    ///
    /// # Errors
    ///
    /// Returns an error if querying fails.
    async fn query(&self, filter: &AuditEventFilter) -> Result<Vec<Arc<AuditEvent>>>;

    /// Deletes an event.
    ///
    /// # Arguments
    ///
    /// * `id` - The event ID
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails.
    async fn delete(&self, id: &str) -> Result<bool>;

    /// Clears all events.
    ///
    /// # Errors
    ///
    /// Returns an error if clearing fails.
    async fn clear(&self) -> Result<()>;

    /// Returns the number of stored events.
    fn event_count(&self) -> usize;
}
