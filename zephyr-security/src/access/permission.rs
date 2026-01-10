//! Permission definitions for role-based access control.

#![allow(clippy::use_self)]
#![allow(clippy::uninlined_format_args)]

use serde::{Deserialize, Serialize};
use std::fmt;

/// Granular permissions for the trading system.
///
/// Permissions are organized by functional area:
/// - Trade: Order execution and management
/// - Position: Position viewing and management
/// - Config: System configuration
/// - Audit: Audit log access
/// - User: User management
/// - Risk: Risk control settings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    // Trade permissions
    /// Execute trades (place orders)
    TradeExecute,
    /// Cancel orders
    TradeCancel,
    /// Modify orders
    TradeModify,

    // Position permissions
    /// View positions
    PositionView,
    /// Close positions
    PositionClose,
    /// Modify position limits
    PositionLimitModify,

    // Configuration permissions
    /// Read configuration
    ConfigRead,
    /// Write configuration
    ConfigWrite,
    /// Manage API keys
    ConfigApiKeys,

    // Audit permissions
    /// Read audit logs
    AuditRead,
    /// Export audit logs
    AuditExport,

    // User management permissions
    /// Create users
    UserCreate,
    /// Delete users
    UserDelete,
    /// Modify user roles
    UserModifyRole,
    /// View user information
    UserView,

    // Risk control permissions
    /// View risk limits
    RiskView,
    /// Modify risk limits
    RiskModify,
    /// Pause trading
    RiskPause,

    // System permissions
    /// Access system health information
    SystemHealth,
    /// Manage system settings
    SystemManage,
}

impl Permission {
    /// Returns a human-readable description of the permission.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::TradeExecute => "Execute trades (place orders)",
            Self::TradeCancel => "Cancel orders",
            Self::TradeModify => "Modify orders",
            Self::PositionView => "View positions",
            Self::PositionClose => "Close positions",
            Self::PositionLimitModify => "Modify position limits",
            Self::ConfigRead => "Read configuration",
            Self::ConfigWrite => "Write configuration",
            Self::ConfigApiKeys => "Manage API keys",
            Self::AuditRead => "Read audit logs",
            Self::AuditExport => "Export audit logs",
            Self::UserCreate => "Create users",
            Self::UserDelete => "Delete users",
            Self::UserModifyRole => "Modify user roles",
            Self::UserView => "View user information",
            Self::RiskView => "View risk limits",
            Self::RiskModify => "Modify risk limits",
            Self::RiskPause => "Pause trading",
            Self::SystemHealth => "Access system health information",
            Self::SystemManage => "Manage system settings",
        }
    }

    /// Returns the category of the permission.
    #[must_use]
    pub const fn category(&self) -> &'static str {
        match self {
            Self::TradeExecute | Self::TradeCancel | Self::TradeModify => "Trade",
            Self::PositionView | Self::PositionClose | Self::PositionLimitModify => "Position",
            Self::ConfigRead | Self::ConfigWrite | Self::ConfigApiKeys => "Configuration",
            Self::AuditRead | Self::AuditExport => "Audit",
            Self::UserCreate | Self::UserDelete | Self::UserModifyRole | Self::UserView => {
                "User Management"
            }
            Self::RiskView | Self::RiskModify | Self::RiskPause => "Risk Control",
            Self::SystemHealth | Self::SystemManage => "System",
        }
    }

    /// Returns all available permissions.
    #[must_use]
    pub const fn all() -> &'static [Permission] {
        &[
            Self::TradeExecute,
            Self::TradeCancel,
            Self::TradeModify,
            Self::PositionView,
            Self::PositionClose,
            Self::PositionLimitModify,
            Self::ConfigRead,
            Self::ConfigWrite,
            Self::ConfigApiKeys,
            Self::AuditRead,
            Self::AuditExport,
            Self::UserCreate,
            Self::UserDelete,
            Self::UserModifyRole,
            Self::UserView,
            Self::RiskView,
            Self::RiskModify,
            Self::RiskPause,
            Self::SystemHealth,
            Self::SystemManage,
        ]
    }

    /// Returns permissions for a trader role.
    #[must_use]
    pub const fn trader_permissions() -> &'static [Permission] {
        &[
            Self::TradeExecute,
            Self::TradeCancel,
            Self::PositionView,
            Self::ConfigRead,
            Self::AuditRead,
        ]
    }

    /// Returns permissions for an admin role.
    #[must_use]
    pub const fn admin_permissions() -> &'static [Permission] {
        Self::all()
    }

    /// Returns permissions for a viewer role.
    #[must_use]
    pub const fn viewer_permissions() -> &'static [Permission] {
        &[
            Self::PositionView,
            Self::ConfigRead,
            Self::AuditRead,
            Self::SystemHealth,
        ]
    }
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_description() {
        assert!(!Permission::TradeExecute.description().is_empty());
        assert!(!Permission::PositionView.description().is_empty());
    }

    #[test]
    fn test_permission_category() {
        assert_eq!(Permission::TradeExecute.category(), "Trade");
        assert_eq!(Permission::PositionView.category(), "Position");
        assert_eq!(Permission::ConfigRead.category(), "Configuration");
        assert_eq!(Permission::AuditRead.category(), "Audit");
    }

    #[test]
    fn test_all_permissions() {
        let all = Permission::all();
        assert!(all.len() > 0);
        assert!(all.contains(&Permission::TradeExecute));
    }

    #[test]
    fn test_trader_permissions() {
        let trader = Permission::trader_permissions();
        assert!(trader.contains(&Permission::TradeExecute));
        assert!(trader.contains(&Permission::PositionView));
        assert!(!trader.contains(&Permission::ConfigWrite));
    }

    #[test]
    fn test_admin_permissions() {
        let admin = Permission::admin_permissions();
        assert_eq!(admin.len(), Permission::all().len());
    }

    #[test]
    fn test_viewer_permissions() {
        let viewer = Permission::viewer_permissions();
        assert!(viewer.contains(&Permission::PositionView));
        assert!(!viewer.contains(&Permission::TradeExecute));
    }

    #[test]
    fn test_permission_serde() {
        let perm = Permission::TradeExecute;
        let json = serde_json::to_string(&perm).unwrap();
        let parsed: Permission = serde_json::from_str(&json).unwrap();
        assert_eq!(perm, parsed);
    }
}
