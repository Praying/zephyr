//! Audit event definitions.

#![allow(clippy::return_self_not_must_use)]
#![allow(missing_docs)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Types of audit events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuditAction {
    // Trade actions
    TradeExecute,
    TradeCancel,
    TradeModify,

    // Position actions
    PositionView,
    PositionClose,
    PositionLimitModify,

    // Configuration actions
    ConfigRead,
    ConfigWrite,
    ConfigApiKeysManage,

    // User management actions
    UserCreate,
    UserDelete,
    UserModifyRole,
    UserLogin,
    UserLogout,
    UserPasswordChange,

    // Access control actions
    PermissionGrant,
    PermissionRevoke,
    RoleCreate,
    RoleDelete,
    RoleModify,

    // Risk control actions
    RiskLimitModify,
    RiskPause,
    RiskResume,

    // System actions
    SystemStart,
    SystemStop,
    SystemHealthCheck,

    // Security actions
    SecurityKeyRotation,
    SecurityAuditExport,
    SecurityIncident,
}

impl AuditAction {
    /// Returns a human-readable description of the action.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::TradeExecute => "Trade executed",
            Self::TradeCancel => "Trade canceled",
            Self::TradeModify => "Trade modified",
            Self::PositionView => "Position viewed",
            Self::PositionClose => "Position closed",
            Self::PositionLimitModify => "Position limit modified",
            Self::ConfigRead => "Configuration read",
            Self::ConfigWrite => "Configuration written",
            Self::ConfigApiKeysManage => "API keys managed",
            Self::UserCreate => "User created",
            Self::UserDelete => "User deleted",
            Self::UserModifyRole => "User role modified",
            Self::UserLogin => "User logged in",
            Self::UserLogout => "User logged out",
            Self::UserPasswordChange => "User password changed",
            Self::PermissionGrant => "Permission granted",
            Self::PermissionRevoke => "Permission revoked",
            Self::RoleCreate => "Role created",
            Self::RoleDelete => "Role deleted",
            Self::RoleModify => "Role modified",
            Self::RiskLimitModify => "Risk limit modified",
            Self::RiskPause => "Trading paused",
            Self::RiskResume => "Trading resumed",
            Self::SystemStart => "System started",
            Self::SystemStop => "System stopped",
            Self::SystemHealthCheck => "System health checked",
            Self::SecurityKeyRotation => "Security key rotated",
            Self::SecurityAuditExport => "Audit log exported",
            Self::SecurityIncident => "Security incident detected",
        }
    }

    /// Returns the category of the action.
    #[must_use]
    pub const fn category(&self) -> &'static str {
        match self {
            Self::TradeExecute | Self::TradeCancel | Self::TradeModify => "Trade",
            Self::PositionView | Self::PositionClose | Self::PositionLimitModify => "Position",
            Self::ConfigRead | Self::ConfigWrite | Self::ConfigApiKeysManage => "Configuration",
            Self::UserCreate
            | Self::UserDelete
            | Self::UserModifyRole
            | Self::UserLogin
            | Self::UserLogout
            | Self::UserPasswordChange => "User Management",
            Self::PermissionGrant
            | Self::PermissionRevoke
            | Self::RoleCreate
            | Self::RoleDelete
            | Self::RoleModify => "Access Control",
            Self::RiskLimitModify | Self::RiskPause | Self::RiskResume => "Risk Control",
            Self::SystemStart | Self::SystemStop | Self::SystemHealthCheck => "System",
            Self::SecurityKeyRotation | Self::SecurityAuditExport | Self::SecurityIncident => {
                "Security"
            }
        }
    }
}

impl fmt::Display for AuditAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// An audit event record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Unique event ID.
    id: String,
    /// Username of the actor.
    username: String,
    /// Action performed.
    action: AuditAction,
    /// Description of the event.
    description: String,
    /// When the event occurred.
    timestamp: DateTime<Utc>,
    /// Optional additional details.
    details: Option<String>,
    /// Optional IP address of the actor.
    ip_address: Option<String>,
    /// Optional result status.
    status: Option<String>,
}

impl AuditEvent {
    /// Creates a new audit event.
    ///
    /// # Arguments
    ///
    /// * `username` - The username of the actor
    /// * `action` - The action performed
    /// * `description` - Description of the event
    #[must_use]
    pub fn new(
        username: impl Into<String>,
        action: AuditAction,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            username: username.into(),
            action,
            description: description.into(),
            timestamp: Utc::now(),
            details: None,
            ip_address: None,
            status: None,
        }
    }

    /// Returns the event ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the username.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Returns the action.
    #[must_use]
    pub const fn action(&self) -> AuditAction {
        self.action
    }

    /// Returns the description.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Returns the timestamp.
    #[must_use]
    pub const fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    /// Returns the details.
    #[must_use]
    pub fn details(&self) -> Option<&str> {
        self.details.as_deref()
    }

    /// Sets the details.
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Returns the IP address.
    #[must_use]
    pub fn ip_address(&self) -> Option<&str> {
        self.ip_address.as_deref()
    }

    /// Sets the IP address.
    pub fn with_ip_address(mut self, ip: impl Into<String>) -> Self {
        self.ip_address = Some(ip.into());
        self
    }

    /// Returns the status.
    #[must_use]
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// Sets the status.
    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }
}

/// Filter for querying audit events.
#[derive(Debug, Clone, Default)]
pub struct AuditEventFilter {
    /// Filter by username.
    pub username: Option<String>,
    /// Filter by action.
    pub action: Option<AuditAction>,
    /// Filter by start time.
    pub start_time: Option<DateTime<Utc>>,
    /// Filter by end time.
    pub end_time: Option<DateTime<Utc>>,
}

impl AuditEventFilter {
    /// Creates a new empty filter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the username filter.
    #[must_use]
    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// Sets the action filter.
    #[must_use]
    pub const fn with_action(mut self, action: AuditAction) -> Self {
        self.action = Some(action);
        self
    }

    /// Sets the start time filter.
    #[must_use]
    pub const fn with_start_time(mut self, time: DateTime<Utc>) -> Self {
        self.start_time = Some(time);
        self
    }

    /// Sets the end time filter.
    #[must_use]
    pub const fn with_end_time(mut self, time: DateTime<Utc>) -> Self {
        self.end_time = Some(time);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_event() {
        let event = AuditEvent::new("alice", AuditAction::TradeExecute, "Executed buy order");

        assert_eq!(event.username(), "alice");
        assert_eq!(event.action(), AuditAction::TradeExecute);
        assert_eq!(event.description(), "Executed buy order");
    }

    #[test]
    fn test_event_with_details() {
        let event = AuditEvent::new("alice", AuditAction::TradeExecute, "Executed buy order")
            .with_details("BTC/USDT, qty=1.5")
            .with_ip_address("192.168.1.1")
            .with_status("success");

        assert_eq!(event.details(), Some("BTC/USDT, qty=1.5"));
        assert_eq!(event.ip_address(), Some("192.168.1.1"));
        assert_eq!(event.status(), Some("success"));
    }

    #[test]
    fn test_action_description() {
        assert!(!AuditAction::TradeExecute.description().is_empty());
        assert!(!AuditAction::UserLogin.description().is_empty());
    }

    #[test]
    fn test_action_category() {
        assert_eq!(AuditAction::TradeExecute.category(), "Trade");
        assert_eq!(AuditAction::UserLogin.category(), "User Management");
        assert_eq!(AuditAction::SecurityIncident.category(), "Security");
    }

    #[test]
    fn test_filter_builder() {
        let filter = AuditEventFilter::new()
            .with_username("alice")
            .with_action(AuditAction::TradeExecute);

        assert_eq!(filter.username, Some("alice".to_string()));
        assert_eq!(filter.action, Some(AuditAction::TradeExecute));
    }

    #[test]
    fn test_event_serde() {
        let event = AuditEvent::new("alice", AuditAction::TradeExecute, "Test event")
            .with_details("details");

        let json = serde_json::to_string(&event).unwrap();
        let parsed: AuditEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(event.username(), parsed.username());
        assert_eq!(event.action(), parsed.action());
        assert_eq!(event.details(), parsed.details());
    }
}
