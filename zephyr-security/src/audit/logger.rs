//! Audit logger for recording security events.

#![allow(clippy::derivable_impls)]
#![allow(clippy::unnecessary_sort_by)]

use super::event::{AuditAction, AuditEvent, AuditEventFilter};
use super::signature::AuditSignature;
use super::storage::AuditStorage;
use crate::error::Result;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Configuration for the audit logger.
#[derive(Debug, Clone)]
pub struct AuditLoggerConfig {
    /// Secret key for signing audit records.
    pub signing_key: Vec<u8>,
    /// Whether to sign audit records.
    pub sign_records: bool,
}

impl Default for AuditLoggerConfig {
    fn default() -> Self {
        Self {
            signing_key: vec![],
            sign_records: false,
        }
    }
}

/// Audit logger for recording security events.
///
/// The audit logger records all security-relevant events and can
/// optionally sign them for tamper detection.
pub struct AuditLogger {
    storage: Arc<dyn AuditStorage>,
    config: AuditLoggerConfig,
}

impl AuditLogger {
    /// Creates a new audit logger with in-memory storage.
    #[must_use]
    pub fn new() -> Self {
        Self::with_storage(Arc::new(super::InMemoryAuditStorage::new()))
    }

    /// Creates a new audit logger with custom storage.
    ///
    /// # Arguments
    ///
    /// * `storage` - The storage backend
    #[must_use]
    pub fn with_storage(storage: Arc<dyn AuditStorage>) -> Self {
        Self {
            storage,
            config: AuditLoggerConfig::default(),
        }
    }

    /// Creates a new audit logger with custom storage and configuration.
    ///
    /// # Arguments
    ///
    /// * `storage` - The storage backend
    /// * `config` - The logger configuration
    #[must_use]
    pub fn with_config(storage: Arc<dyn AuditStorage>, config: AuditLoggerConfig) -> Self {
        Self { storage, config }
    }

    /// Logs an audit event.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to log
    ///
    /// # Errors
    ///
    /// Returns an error if logging fails.
    pub async fn log_event(&self, event: AuditEvent) -> Result<()> {
        info!(
            "Audit: {} - {} ({})",
            event.username(),
            event.action(),
            event.description()
        );

        self.storage.store(event).await?;
        Ok(())
    }

    /// Logs a trade execution event.
    ///
    /// # Arguments
    ///
    /// * `username` - The username
    /// * `description` - Event description
    ///
    /// # Errors
    ///
    /// Returns an error if logging fails.
    pub async fn log_trade_execute(&self, username: &str, description: &str) -> Result<()> {
        let event = AuditEvent::new(username, AuditAction::TradeExecute, description);
        self.log_event(event).await
    }

    /// Logs a position view event.
    ///
    /// # Arguments
    ///
    /// * `username` - The username
    /// * `description` - Event description
    ///
    /// # Errors
    ///
    /// Returns an error if logging fails.
    pub async fn log_position_view(&self, username: &str, description: &str) -> Result<()> {
        let event = AuditEvent::new(username, AuditAction::PositionView, description);
        self.log_event(event).await
    }

    /// Logs a user login event.
    ///
    /// # Arguments
    ///
    /// * `username` - The username
    /// * `ip_address` - The IP address
    ///
    /// # Errors
    ///
    /// Returns an error if logging fails.
    pub async fn log_user_login(&self, username: &str, ip_address: &str) -> Result<()> {
        let event = AuditEvent::new(username, AuditAction::UserLogin, "User logged in")
            .with_ip_address(ip_address);
        self.log_event(event).await
    }

    /// Logs a user logout event.
    ///
    /// # Arguments
    ///
    /// * `username` - The username
    ///
    /// # Errors
    ///
    /// Returns an error if logging fails.
    pub async fn log_user_logout(&self, username: &str) -> Result<()> {
        let event = AuditEvent::new(username, AuditAction::UserLogout, "User logged out");
        self.log_event(event).await
    }

    /// Logs a configuration change event.
    ///
    /// # Arguments
    ///
    /// * `username` - The username
    /// * `description` - Event description
    ///
    /// # Errors
    ///
    /// Returns an error if logging fails.
    pub async fn log_config_write(&self, username: &str, description: &str) -> Result<()> {
        let event = AuditEvent::new(username, AuditAction::ConfigWrite, description);
        self.log_event(event).await
    }

    /// Logs a security incident.
    ///
    /// # Arguments
    ///
    /// * `username` - The username (or "system" if not user-initiated)
    /// * `description` - Incident description
    ///
    /// # Errors
    ///
    /// Returns an error if logging fails.
    pub async fn log_security_incident(&self, username: &str, description: &str) -> Result<()> {
        warn!("Security incident: {}", description);
        let event = AuditEvent::new(username, AuditAction::SecurityIncident, description)
            .with_status("incident");
        self.log_event(event).await
    }

    /// Queries audit events.
    ///
    /// # Arguments
    ///
    /// * `username` - Optional username filter
    /// * `action` - Optional action filter
    /// * `limit` - Optional result limit
    ///
    /// # Errors
    ///
    /// Returns an error if querying fails.
    pub async fn query_events(
        &self,
        username: Option<&str>,
        action: Option<AuditAction>,
        _limit: Option<usize>,
    ) -> Result<Vec<Arc<AuditEvent>>> {
        let mut filter = AuditEventFilter::new();

        if let Some(u) = username {
            filter = filter.with_username(u);
        }

        if let Some(a) = action {
            filter = filter.with_action(a);
        }

        let mut events = self.storage.query(&filter).await?;

        // Sort by timestamp descending
        events.sort_by(|a, b| b.timestamp().cmp(&a.timestamp()));

        Ok(events)
    }

    /// Exports audit logs.
    ///
    /// # Errors
    ///
    /// Returns an error if export fails.
    pub async fn export_logs(&self) -> Result<Vec<Arc<AuditEvent>>> {
        debug!("Exporting audit logs");
        self.storage.list_all().await
    }

    /// Creates a signature for an event.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to sign
    ///
    /// # Errors
    ///
    /// Returns an error if signing fails.
    pub fn sign_event(&self, event: &AuditEvent) -> Result<AuditSignature> {
        let json = serde_json::to_vec(event).map_err(|e| {
            crate::error::SecurityError::encryption(format!("Failed to serialize event: {e}"))
        })?;

        AuditSignature::create(&json, &self.config.signing_key)
    }

    /// Verifies an event signature.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to verify
    /// * `signature` - The signature to verify
    ///
    /// # Errors
    ///
    /// Returns an error if verification fails.
    pub fn verify_event_signature(
        &self,
        event: &AuditEvent,
        signature: &AuditSignature,
    ) -> Result<()> {
        let json = serde_json::to_vec(event).map_err(|e| {
            crate::error::SecurityError::encryption(format!("Failed to serialize event: {e}"))
        })?;

        signature.verify(&json, &self.config.signing_key)
    }

    /// Returns the number of stored events.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.storage.event_count()
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_log_event() {
        let logger = AuditLogger::new();
        let event = AuditEvent::new("alice", AuditAction::TradeExecute, "Test event");

        logger.log_event(event).await.unwrap();
        assert_eq!(logger.event_count(), 1);
    }

    #[tokio::test]
    async fn test_log_trade_execute() {
        let logger = AuditLogger::new();
        logger
            .log_trade_execute("alice", "Executed buy order")
            .await
            .unwrap();

        assert_eq!(logger.event_count(), 1);
    }

    #[tokio::test]
    async fn test_log_user_login() {
        let logger = AuditLogger::new();
        logger.log_user_login("alice", "192.168.1.1").await.unwrap();

        let events = logger
            .query_events(Some("alice"), None, None)
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action(), AuditAction::UserLogin);
    }

    #[tokio::test]
    async fn test_query_events() {
        let logger = AuditLogger::new();

        logger.log_trade_execute("alice", "Event 1").await.unwrap();
        logger.log_position_view("alice", "Event 2").await.unwrap();
        logger.log_trade_execute("bob", "Event 3").await.unwrap();

        let alice_events = logger
            .query_events(Some("alice"), None, None)
            .await
            .unwrap();
        assert_eq!(alice_events.len(), 2);

        let trade_events = logger
            .query_events(None, Some(AuditAction::TradeExecute), None)
            .await
            .unwrap();
        assert_eq!(trade_events.len(), 2);
    }

    #[tokio::test]
    async fn test_export_logs() {
        let logger = AuditLogger::new();

        logger.log_trade_execute("alice", "Event 1").await.unwrap();
        logger.log_position_view("bob", "Event 2").await.unwrap();

        let exported = logger.export_logs().await.unwrap();
        assert_eq!(exported.len(), 2);
    }

    #[test]
    fn test_sign_event() {
        let config = AuditLoggerConfig {
            signing_key: b"secret-key".to_vec(),
            sign_records: true,
        };
        let logger =
            AuditLogger::with_config(Arc::new(super::super::InMemoryAuditStorage::new()), config);

        let event = AuditEvent::new("alice", AuditAction::TradeExecute, "Test");
        let signature = logger.sign_event(&event).unwrap();

        assert!(!signature.signature().is_empty());
    }

    #[test]
    fn test_verify_event_signature() {
        let config = AuditLoggerConfig {
            signing_key: b"secret-key".to_vec(),
            sign_records: true,
        };
        let logger =
            AuditLogger::with_config(Arc::new(super::super::InMemoryAuditStorage::new()), config);

        let event = AuditEvent::new("alice", AuditAction::TradeExecute, "Test");
        let signature = logger.sign_event(&event).unwrap();

        assert!(logger.verify_event_signature(&event, &signature).is_ok());
    }

    #[test]
    fn test_verify_fails_with_tampered_event() {
        let config = AuditLoggerConfig {
            signing_key: b"secret-key".to_vec(),
            sign_records: true,
        };
        let logger =
            AuditLogger::with_config(Arc::new(super::super::InMemoryAuditStorage::new()), config);

        let event1 = AuditEvent::new("alice", AuditAction::TradeExecute, "Test");
        let signature = logger.sign_event(&event1).unwrap();

        let event2 = AuditEvent::new("bob", AuditAction::TradeExecute, "Test");
        assert!(logger.verify_event_signature(&event2, &signature).is_err());
    }
}
