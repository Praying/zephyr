//! User definitions for role-based access control.

use super::permission::Permission;
use super::role::Role;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::sync::Arc;

/// A user with an assigned role.
///
/// Users inherit permissions from their assigned role.
///
/// # Example
///
/// ```
/// use zephyr_security::access::{User, Role, Permission};
/// use std::sync::Arc;
///
/// let mut role = Role::new("trader");
/// role.grant_permission(Permission::TradeExecute);
///
/// let user = User::new("alice", Arc::new(role));
/// assert!(user.has_permission(Permission::TradeExecute));
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct User {
    username: String,
    #[serde(skip)]
    role: Arc<Role>,
    created_at: DateTime<Utc>,
    last_login: Option<DateTime<Utc>>,
    active: bool,
}

impl User {
    /// Creates a new user with the given role.
    ///
    /// # Arguments
    ///
    /// * `username` - The username
    /// * `role` - The role to assign
    #[must_use]
    pub fn new(username: impl Into<String>, role: Arc<Role>) -> Self {
        Self {
            username: username.into(),
            role,
            created_at: Utc::now(),
            last_login: None,
            active: true,
        }
    }

    /// Returns the username.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Returns the assigned role.
    #[must_use]
    pub fn role(&self) -> &Arc<Role> {
        &self.role
    }

    /// Returns the creation timestamp.
    #[must_use]
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Returns the last login timestamp.
    #[must_use]
    pub const fn last_login(&self) -> Option<DateTime<Utc>> {
        self.last_login
    }

    /// Records a login.
    pub fn record_login(&mut self) {
        self.last_login = Some(Utc::now());
    }

    /// Returns whether the user is active.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    /// Activates the user.
    pub fn activate(&mut self) {
        self.active = true;
    }

    /// Deactivates the user.
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    /// Checks if the user has a specific permission.
    ///
    /// # Arguments
    ///
    /// * `permission` - The permission to check
    #[must_use]
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.active && self.role.has_permission(permission)
    }

    /// Checks if the user has all specified permissions.
    ///
    /// # Arguments
    ///
    /// * `permissions` - The permissions to check
    #[must_use]
    pub fn has_all_permissions(&self, permissions: &[Permission]) -> bool {
        self.active && self.role.has_all_permissions(permissions)
    }

    /// Checks if the user has any of the specified permissions.
    ///
    /// # Arguments
    ///
    /// * `permissions` - The permissions to check
    #[must_use]
    pub fn has_any_permission(&self, permissions: &[Permission]) -> bool {
        self.active && self.role.has_any_permission(permissions)
    }

    /// Returns all permissions for this user.
    #[must_use]
    pub fn permissions(&self) -> Vec<Permission> {
        if self.active {
            self.role.permissions()
        } else {
            Vec::new()
        }
    }
}

impl PartialEq for User {
    fn eq(&self, other: &Self) -> bool {
        self.username == other.username && self.active == other.active
    }
}

impl Eq for User {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_user() {
        let mut role = Role::new("trader");
        role.grant_permission(Permission::TradeExecute);

        let user = User::new("alice", Arc::new(role));
        assert_eq!(user.username(), "alice");
        assert!(user.is_active());
    }

    #[test]
    fn test_user_permissions() {
        let mut role = Role::new("trader");
        role.grant_permission(Permission::TradeExecute);
        role.grant_permission(Permission::PositionView);

        let user = User::new("alice", Arc::new(role));
        assert!(user.has_permission(Permission::TradeExecute));
        assert!(user.has_permission(Permission::PositionView));
        assert!(!user.has_permission(Permission::ConfigWrite));
    }

    #[test]
    fn test_inactive_user_no_permissions() {
        let mut role = Role::new("trader");
        role.grant_permission(Permission::TradeExecute);

        let mut user = User::new("alice", Arc::new(role));
        assert!(user.has_permission(Permission::TradeExecute));

        user.deactivate();
        assert!(!user.has_permission(Permission::TradeExecute));
    }

    #[test]
    fn test_activate_deactivate() {
        let role = Role::new("trader");
        let mut user = User::new("alice", Arc::new(role));

        assert!(user.is_active());
        user.deactivate();
        assert!(!user.is_active());
        user.activate();
        assert!(user.is_active());
    }

    #[test]
    fn test_record_login() {
        let role = Role::new("trader");
        let mut user = User::new("alice", Arc::new(role));

        assert!(user.last_login().is_none());
        user.record_login();
        assert!(user.last_login().is_some());
    }

    #[test]
    fn test_has_all_permissions() {
        let mut role = Role::new("trader");
        role.grant_permission(Permission::TradeExecute);
        role.grant_permission(Permission::PositionView);

        let user = User::new("alice", Arc::new(role));

        let perms = vec![Permission::TradeExecute, Permission::PositionView];
        assert!(user.has_all_permissions(&perms));

        let perms = vec![Permission::TradeExecute, Permission::ConfigWrite];
        assert!(!user.has_all_permissions(&perms));
    }

    #[test]
    fn test_has_any_permission() {
        let mut role = Role::new("trader");
        role.grant_permission(Permission::TradeExecute);

        let user = User::new("alice", Arc::new(role));

        let perms = vec![Permission::TradeExecute, Permission::ConfigWrite];
        assert!(user.has_any_permission(&perms));

        let perms = vec![Permission::ConfigWrite, Permission::AuditRead];
        assert!(!user.has_any_permission(&perms));
    }

    #[test]
    fn test_user_equality() {
        let role = Role::new("trader");
        let user1 = User::new("alice", Arc::new(role.clone()));
        let user2 = User::new("alice", Arc::new(role));

        assert_eq!(user1, user2);
    }
}
