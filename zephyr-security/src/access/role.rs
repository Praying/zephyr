//! Role definitions for role-based access control.

use super::permission::Permission;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A role with a set of permissions.
///
/// Roles are used to group permissions and assign them to users.
/// Each role has a name and a set of permissions.
///
/// # Example
///
/// ```
/// use zephyr_security::access::{Role, Permission};
///
/// let mut trader_role = Role::new("trader");
/// trader_role.grant_permission(Permission::TradeExecute);
/// trader_role.grant_permission(Permission::PositionView);
///
/// assert!(trader_role.has_permission(Permission::TradeExecute));
/// assert!(!trader_role.has_permission(Permission::ConfigWrite));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    name: String,
    description: Option<String>,
    permissions: HashSet<Permission>,
}

impl Role {
    /// Creates a new role with the given name.
    ///
    /// # Arguments
    ///
    /// * `name` - The role name
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            permissions: HashSet::new(),
        }
    }

    /// Creates a new role with a description.
    ///
    /// # Arguments
    ///
    /// * `name` - The role name
    /// * `description` - The role description
    #[must_use]
    pub fn with_description(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: Some(description.into()),
            permissions: HashSet::new(),
        }
    }

    /// Creates a predefined trader role.
    #[must_use]
    pub fn trader() -> Self {
        let mut role = Self::with_description("trader", "Trader role with trading permissions");
        for perm in Permission::trader_permissions() {
            role.grant_permission(*perm);
        }
        role
    }

    /// Creates a predefined admin role.
    #[must_use]
    pub fn admin() -> Self {
        let mut role = Self::with_description("admin", "Administrator role with all permissions");
        for perm in Permission::admin_permissions() {
            role.grant_permission(*perm);
        }
        role
    }

    /// Creates a predefined viewer role.
    #[must_use]
    pub fn viewer() -> Self {
        let mut role = Self::with_description("viewer", "Viewer role with read-only permissions");
        for perm in Permission::viewer_permissions() {
            role.grant_permission(*perm);
        }
        role
    }

    /// Returns the role name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the role description.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Sets the role description.
    pub fn set_description(&mut self, description: impl Into<String>) {
        self.description = Some(description.into());
    }

    /// Grants a permission to the role.
    ///
    /// # Arguments
    ///
    /// * `permission` - The permission to grant
    pub fn grant_permission(&mut self, permission: Permission) {
        self.permissions.insert(permission);
    }

    /// Grants multiple permissions to the role.
    ///
    /// # Arguments
    ///
    /// * `permissions` - The permissions to grant
    pub fn grant_permissions(&mut self, permissions: &[Permission]) {
        for perm in permissions {
            self.permissions.insert(*perm);
        }
    }

    /// Revokes a permission from the role.
    ///
    /// # Arguments
    ///
    /// * `permission` - The permission to revoke
    pub fn revoke_permission(&mut self, permission: Permission) {
        self.permissions.remove(&permission);
    }

    /// Revokes multiple permissions from the role.
    ///
    /// # Arguments
    ///
    /// * `permissions` - The permissions to revoke
    pub fn revoke_permissions(&mut self, permissions: &[Permission]) {
        for perm in permissions {
            self.permissions.remove(perm);
        }
    }

    /// Checks if the role has a specific permission.
    ///
    /// # Arguments
    ///
    /// * `permission` - The permission to check
    #[must_use]
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }

    /// Checks if the role has all specified permissions.
    ///
    /// # Arguments
    ///
    /// * `permissions` - The permissions to check
    #[must_use]
    pub fn has_all_permissions(&self, permissions: &[Permission]) -> bool {
        permissions.iter().all(|p| self.has_permission(*p))
    }

    /// Checks if the role has any of the specified permissions.
    ///
    /// # Arguments
    ///
    /// * `permissions` - The permissions to check
    #[must_use]
    pub fn has_any_permission(&self, permissions: &[Permission]) -> bool {
        permissions.iter().any(|p| self.has_permission(*p))
    }

    /// Returns all permissions for this role.
    #[must_use]
    pub fn permissions(&self) -> Vec<Permission> {
        self.permissions.iter().copied().collect()
    }

    /// Returns the number of permissions.
    #[must_use]
    pub fn permission_count(&self) -> usize {
        self.permissions.len()
    }

    /// Clears all permissions from the role.
    pub fn clear_permissions(&mut self) {
        self.permissions.clear();
    }
}

impl PartialEq for Role {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.permissions == other.permissions
    }
}

impl Eq for Role {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_role() {
        let role = Role::new("test_role");
        assert_eq!(role.name(), "test_role");
        assert_eq!(role.permission_count(), 0);
    }

    #[test]
    fn test_role_with_description() {
        let role = Role::with_description("test_role", "A test role");
        assert_eq!(role.name(), "test_role");
        assert_eq!(role.description(), Some("A test role"));
    }

    #[test]
    fn test_grant_permission() {
        let mut role = Role::new("test_role");
        role.grant_permission(Permission::TradeExecute);

        assert!(role.has_permission(Permission::TradeExecute));
        assert!(!role.has_permission(Permission::ConfigWrite));
    }

    #[test]
    fn test_grant_multiple_permissions() {
        let mut role = Role::new("test_role");
        let perms = vec![Permission::TradeExecute, Permission::PositionView];
        role.grant_permissions(&perms);

        assert_eq!(role.permission_count(), 2);
        assert!(role.has_all_permissions(&perms));
    }

    #[test]
    fn test_revoke_permission() {
        let mut role = Role::new("test_role");
        role.grant_permission(Permission::TradeExecute);
        assert!(role.has_permission(Permission::TradeExecute));

        role.revoke_permission(Permission::TradeExecute);
        assert!(!role.has_permission(Permission::TradeExecute));
    }

    #[test]
    fn test_has_all_permissions() {
        let mut role = Role::new("test_role");
        role.grant_permission(Permission::TradeExecute);
        role.grant_permission(Permission::PositionView);

        let perms = vec![Permission::TradeExecute, Permission::PositionView];
        assert!(role.has_all_permissions(&perms));

        let perms = vec![Permission::TradeExecute, Permission::ConfigWrite];
        assert!(!role.has_all_permissions(&perms));
    }

    #[test]
    fn test_has_any_permission() {
        let mut role = Role::new("test_role");
        role.grant_permission(Permission::TradeExecute);

        let perms = vec![Permission::TradeExecute, Permission::ConfigWrite];
        assert!(role.has_any_permission(&perms));

        let perms = vec![Permission::ConfigWrite, Permission::AuditRead];
        assert!(!role.has_any_permission(&perms));
    }

    #[test]
    fn test_predefined_trader_role() {
        let role = Role::trader();
        assert_eq!(role.name(), "trader");
        assert!(role.has_permission(Permission::TradeExecute));
        assert!(role.has_permission(Permission::PositionView));
        assert!(!role.has_permission(Permission::ConfigWrite));
    }

    #[test]
    fn test_predefined_admin_role() {
        let role = Role::admin();
        assert_eq!(role.name(), "admin");
        for perm in Permission::all() {
            assert!(role.has_permission(*perm));
        }
    }

    #[test]
    fn test_predefined_viewer_role() {
        let role = Role::viewer();
        assert_eq!(role.name(), "viewer");
        assert!(role.has_permission(Permission::PositionView));
        assert!(!role.has_permission(Permission::TradeExecute));
    }

    #[test]
    fn test_clear_permissions() {
        let mut role = Role::new("test_role");
        role.grant_permission(Permission::TradeExecute);
        role.grant_permission(Permission::PositionView);
        assert_eq!(role.permission_count(), 2);

        role.clear_permissions();
        assert_eq!(role.permission_count(), 0);
    }

    #[test]
    fn test_role_equality() {
        let mut role1 = Role::new("test");
        role1.grant_permission(Permission::TradeExecute);

        let mut role2 = Role::new("test");
        role2.grant_permission(Permission::TradeExecute);

        assert_eq!(role1, role2);
    }

    #[test]
    fn test_role_serde() {
        let mut role = Role::new("test_role");
        role.grant_permission(Permission::TradeExecute);

        let json = serde_json::to_string(&role).unwrap();
        let parsed: Role = serde_json::from_str(&json).unwrap();

        assert_eq!(role, parsed);
    }
}
