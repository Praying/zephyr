//! Role-based access control (RBAC) module.
//!
//! This module provides:
//! - Role definitions with granular permissions
//! - Permission checking and enforcement
//! - IP whitelist management
//! - Session management with expiration
//!
//! # Example
//!
//! ```no_run
//! use zephyr_security::access::{AccessControl, Role, Permission};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let mut access = AccessControl::new();
//!
//! // Create a role with permissions
//! let mut trader_role = Role::new("trader");
//! trader_role.grant_permission(Permission::TradeExecute);
//! trader_role.grant_permission(Permission::PositionView);

#![allow(clippy::uninlined_format_args)]
#![allow(clippy::unnecessary_sort_by)]
#![allow(clippy::must_use_candidate)]
//! access.register_role(trader_role)?;
//!
//! // Create a user with the role
//! access.create_user("alice", "trader")?;
//!
//! // Check permissions
//! if access.has_permission("alice", Permission::TradeExecute)? {
//!     println!("Alice can execute trades");
//! }
//! # Ok(())
//! # }
//! ```

mod config;
mod permission;
mod role;
mod session;
mod user;

pub use config::AccessControlConfig;
pub use permission::Permission;
pub use role::Role;
pub use session::{Session, SessionManager};
pub use user::User;

use crate::error::{Result, SecurityError};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Role-based access control manager.
///
/// Manages users, roles, permissions, and IP whitelisting.
pub struct AccessControl {
    config: AccessControlConfig,
    roles: DashMap<String, Arc<Role>>,
    users: DashMap<String, Arc<User>>,
    sessions: Arc<SessionManager>,
    ip_whitelist: RwLock<HashSet<IpAddr>>,
}

impl AccessControl {
    /// Creates a new access control manager.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(AccessControlConfig::default())
    }

    /// Creates a new access control manager with the given configuration.
    #[must_use]
    pub fn with_config(config: AccessControlConfig) -> Self {
        let session_timeout = config.session_timeout;
        Self {
            config,
            roles: DashMap::new(),
            users: DashMap::new(),
            sessions: Arc::new(SessionManager::new(session_timeout)),
            ip_whitelist: RwLock::new(HashSet::new()),
        }
    }

    /// Registers a new role.
    ///
    /// # Arguments
    ///
    /// * `role` - The role to register
    ///
    /// # Errors
    ///
    /// Returns an error if a role with the same name already exists.
    pub fn register_role(&self, role: Role) -> Result<()> {
        let name = role.name().to_string();

        if self.roles.contains_key(&name) {
            return Err(SecurityError::role_not_found(format!(
                "Role '{}' already exists",
                name
            )));
        }

        self.roles.insert(name.clone(), Arc::new(role));
        info!("Registered role: {}", name);
        Ok(())
    }

    /// Gets a role by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The role name
    ///
    /// # Errors
    ///
    /// Returns an error if the role is not found.
    pub fn get_role(&self, name: &str) -> Result<Arc<Role>> {
        self.roles
            .get(name)
            .map(|entry| Arc::clone(entry.value()))
            .ok_or_else(|| SecurityError::role_not_found(name))
    }

    /// Lists all registered roles.
    #[must_use]
    pub fn list_roles(&self) -> Vec<String> {
        self.roles.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Creates a new user with the given role.
    ///
    /// # Arguments
    ///
    /// * `username` - The username
    /// * `role_name` - The role to assign
    ///
    /// # Errors
    ///
    /// Returns an error if the user already exists or the role is not found.
    pub fn create_user(&self, username: &str, role_name: &str) -> Result<()> {
        if self.users.contains_key(username) {
            return Err(SecurityError::authentication_failed(format!(
                "User '{}' already exists",
                username
            )));
        }

        let role = self.get_role(role_name)?;
        let user = User::new(username, role);

        self.users.insert(username.to_string(), Arc::new(user));
        info!("Created user: {}", username);
        Ok(())
    }

    /// Gets a user by username.
    ///
    /// # Arguments
    ///
    /// * `username` - The username
    ///
    /// # Errors
    ///
    /// Returns an error if the user is not found.
    pub fn get_user(&self, username: &str) -> Result<Arc<User>> {
        self.users
            .get(username)
            .map(|entry| Arc::clone(entry.value()))
            .ok_or_else(|| {
                SecurityError::authentication_failed(format!("User '{}' not found", username))
            })
    }

    /// Lists all users.
    #[must_use]
    pub fn list_users(&self) -> Vec<String> {
        self.users.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Deletes a user.
    ///
    /// # Arguments
    ///
    /// * `username` - The username
    ///
    /// # Errors
    ///
    /// Returns an error if the user is not found.
    pub fn delete_user(&self, username: &str) -> Result<()> {
        if self.users.remove(username).is_none() {
            return Err(SecurityError::authentication_failed(format!(
                "User '{}' not found",
                username
            )));
        }

        // Invalidate all sessions for this user
        self.sessions.invalidate_user_sessions(username);

        info!("Deleted user: {}", username);
        Ok(())
    }

    /// Checks if a user has a specific permission.
    ///
    /// # Arguments
    ///
    /// * `username` - The username
    /// * `permission` - The permission to check
    ///
    /// # Errors
    ///
    /// Returns an error if the user is not found or lacks the permission.
    pub fn has_permission(&self, username: &str, permission: Permission) -> Result<bool> {
        let user = self.get_user(username)?;
        Ok(user.has_permission(permission))
    }

    /// Checks if a user has all specified permissions.
    ///
    /// # Arguments
    ///
    /// * `username` - The username
    /// * `permissions` - The permissions to check
    ///
    /// # Errors
    ///
    /// Returns an error if the user is not found.
    pub fn has_all_permissions(&self, username: &str, permissions: &[Permission]) -> Result<bool> {
        let user = self.get_user(username)?;
        Ok(permissions.iter().all(|p| user.has_permission(*p)))
    }

    /// Checks if a user has any of the specified permissions.
    ///
    /// # Arguments
    ///
    /// * `username` - The username
    /// * `permissions` - The permissions to check
    ///
    /// # Errors
    ///
    /// Returns an error if the user is not found.
    pub fn has_any_permission(&self, username: &str, permissions: &[Permission]) -> Result<bool> {
        let user = self.get_user(username)?;
        Ok(permissions.iter().any(|p| user.has_permission(*p)))
    }

    /// Enforces a permission check, returning an error if the user lacks the permission.
    ///
    /// # Arguments
    ///
    /// * `username` - The username
    /// * `permission` - The permission to check
    ///
    /// # Errors
    ///
    /// Returns an error if the user is not found or lacks the permission.
    pub fn enforce_permission(&self, username: &str, permission: Permission) -> Result<()> {
        if !self.has_permission(username, permission)? {
            warn!("User '{}' denied permission: {:?}", username, permission);
            return Err(SecurityError::authorization_failed(
                username,
                format!("{:?}", permission),
            ));
        }
        Ok(())
    }

    /// Creates a new session for a user.
    ///
    /// # Arguments
    ///
    /// * `username` - The username
    /// * `ip_address` - The IP address of the client
    ///
    /// # Errors
    ///
    /// Returns an error if the user is not found or IP is not whitelisted.
    pub async fn create_session(&self, username: &str, ip_address: IpAddr) -> Result<String> {
        // Check if user exists
        let _user = self.get_user(username)?;

        // Check IP whitelist if enabled
        if self.config.enforce_ip_whitelist {
            self.check_ip_allowed(ip_address)?;
        }

        let session_id = self.sessions.create_session(username, ip_address).await;
        info!(
            "Created session for user '{}' from {}",
            username, ip_address
        );
        Ok(session_id)
    }

    /// Validates a session.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID
    ///
    /// # Errors
    ///
    /// Returns an error if the session is invalid or expired.
    pub async fn validate_session(&self, session_id: &str) -> Result<Arc<Session>> {
        self.sessions.validate_session(session_id).await
    }

    /// Invalidates a session.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID
    pub async fn invalidate_session(&self, session_id: &str) {
        self.sessions.invalidate_session(session_id).await;
    }

    /// Adds an IP address to the whitelist.
    ///
    /// # Arguments
    ///
    /// * `ip` - The IP address to whitelist
    pub fn add_ip_whitelist(&self, ip: IpAddr) {
        self.ip_whitelist.write().insert(ip);
        info!("Added IP to whitelist: {}", ip);
    }

    /// Removes an IP address from the whitelist.
    ///
    /// # Arguments
    ///
    /// * `ip` - The IP address to remove
    pub fn remove_ip_whitelist(&self, ip: IpAddr) {
        self.ip_whitelist.write().remove(&ip);
        info!("Removed IP from whitelist: {}", ip);
    }

    /// Checks if an IP address is whitelisted.
    ///
    /// # Arguments
    ///
    /// * `ip` - The IP address to check
    ///
    /// # Errors
    ///
    /// Returns an error if the IP is not whitelisted.
    pub fn check_ip_allowed(&self, ip: IpAddr) -> Result<()> {
        if !self.config.enforce_ip_whitelist {
            return Ok(());
        }

        if self.ip_whitelist.read().contains(&ip) {
            debug!("IP allowed: {}", ip);
            Ok(())
        } else {
            warn!("IP not allowed: {}", ip);
            Err(SecurityError::ip_not_allowed(ip.to_string()))
        }
    }

    /// Lists all whitelisted IPs.
    #[must_use]
    pub fn list_whitelisted_ips(&self) -> Vec<IpAddr> {
        self.ip_whitelist.read().iter().copied().collect()
    }

    /// Clears the IP whitelist.
    pub fn clear_ip_whitelist(&self) {
        self.ip_whitelist.write().clear();
        info!("Cleared IP whitelist");
    }
}

impl Default for AccessControl {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_access_control() -> AccessControl {
        let ac = AccessControl::new();

        // Create a trader role
        let mut trader_role = Role::new("trader");
        trader_role.grant_permission(Permission::TradeExecute);
        trader_role.grant_permission(Permission::PositionView);
        ac.register_role(trader_role).unwrap();

        // Create an admin role
        let mut admin_role = Role::new("admin");
        admin_role.grant_permission(Permission::TradeExecute);
        admin_role.grant_permission(Permission::PositionView);
        admin_role.grant_permission(Permission::ConfigWrite);
        ac.register_role(admin_role).unwrap();

        ac
    }

    #[test]
    fn test_register_role() {
        let ac = AccessControl::new();
        let role = Role::new("test_role");
        ac.register_role(role).unwrap();

        assert!(ac.get_role("test_role").is_ok());
    }

    #[test]
    fn test_duplicate_role_fails() {
        let ac = AccessControl::new();
        let role1 = Role::new("test_role");
        let role2 = Role::new("test_role");

        ac.register_role(role1).unwrap();
        assert!(ac.register_role(role2).is_err());
    }

    #[test]
    fn test_create_user() {
        let ac = create_test_access_control();
        ac.create_user("alice", "trader").unwrap();

        let user = ac.get_user("alice").unwrap();
        assert_eq!(user.username(), "alice");
    }

    #[test]
    fn test_user_permissions() {
        let ac = create_test_access_control();
        ac.create_user("alice", "trader").unwrap();

        assert!(
            ac.has_permission("alice", Permission::TradeExecute)
                .unwrap()
        );
        assert!(
            ac.has_permission("alice", Permission::PositionView)
                .unwrap()
        );
        assert!(!ac.has_permission("alice", Permission::ConfigWrite).unwrap());
    }

    #[test]
    fn test_enforce_permission() {
        let ac = create_test_access_control();
        ac.create_user("alice", "trader").unwrap();

        assert!(
            ac.enforce_permission("alice", Permission::TradeExecute)
                .is_ok()
        );
        assert!(
            ac.enforce_permission("alice", Permission::ConfigWrite)
                .is_err()
        );
    }

    #[test]
    fn test_delete_user() {
        let ac = create_test_access_control();
        ac.create_user("alice", "trader").unwrap();
        assert!(ac.get_user("alice").is_ok());

        ac.delete_user("alice").unwrap();
        assert!(ac.get_user("alice").is_err());
    }

    #[test]
    fn test_ip_whitelist() {
        let ac = AccessControl::new();
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        ac.add_ip_whitelist(ip);
        assert!(ac.check_ip_allowed(ip).is_ok());

        let other_ip: IpAddr = "10.0.0.1".parse().unwrap();
        let config = AccessControlConfig {
            enforce_ip_whitelist: true,
            ..Default::default()
        };
        let ac_strict = AccessControl::with_config(config);
        ac_strict.add_ip_whitelist(ip);
        assert!(ac_strict.check_ip_allowed(other_ip).is_err());
    }

    #[test]
    fn test_list_roles() {
        let ac = create_test_access_control();
        let roles = ac.list_roles();
        assert_eq!(roles.len(), 2);
        assert!(roles.contains(&"trader".to_string()));
        assert!(roles.contains(&"admin".to_string()));
    }

    #[test]
    fn test_list_users() {
        let ac = create_test_access_control();
        ac.create_user("alice", "trader").unwrap();
        ac.create_user("bob", "admin").unwrap();

        let users = ac.list_users();
        assert_eq!(users.len(), 2);
        assert!(users.contains(&"alice".to_string()));
        assert!(users.contains(&"bob".to_string()));
    }

    #[tokio::test]
    async fn test_session_creation() {
        let ac = create_test_access_control();
        ac.create_user("alice", "trader").unwrap();

        let ip: IpAddr = "192.168.1.1".parse().unwrap();
        let session_id = ac.create_session("alice", ip).await.unwrap();
        assert!(!session_id.is_empty());
    }

    #[tokio::test]
    async fn test_session_validation() {
        let ac = create_test_access_control();
        ac.create_user("alice", "trader").unwrap();

        let ip: IpAddr = "192.168.1.1".parse().unwrap();
        let session_id = ac.create_session("alice", ip).await.unwrap();

        let session = ac.validate_session(&session_id).await.unwrap();
        assert_eq!(session.username(), "alice");
    }

    #[test]
    fn test_has_all_permissions() {
        let ac = create_test_access_control();
        ac.create_user("alice", "trader").unwrap();

        let perms = vec![Permission::TradeExecute, Permission::PositionView];
        assert!(ac.has_all_permissions("alice", &perms).unwrap());

        let perms = vec![Permission::TradeExecute, Permission::ConfigWrite];
        assert!(!ac.has_all_permissions("alice", &perms).unwrap());
    }

    #[test]
    fn test_has_any_permission() {
        let ac = create_test_access_control();
        ac.create_user("alice", "trader").unwrap();

        let perms = vec![Permission::ConfigWrite, Permission::TradeExecute];
        assert!(ac.has_any_permission("alice", &perms).unwrap());

        let perms = vec![Permission::ConfigWrite, Permission::AuditRead];
        assert!(!ac.has_any_permission("alice", &perms).unwrap());
    }
}
