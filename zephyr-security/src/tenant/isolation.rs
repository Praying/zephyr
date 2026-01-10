//! Tenant isolation mechanisms.

#![allow(clippy::uninlined_format_args)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::double_must_use)]

use super::tenant::TenantId;
use crate::error::{Result, SecurityError};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{debug, warn};

/// Level of isolation between tenants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolationLevel {
    /// Row-level security - all tenants share the same tables.
    RowLevel,
    /// Schema-level isolation - each tenant has its own schema.
    Schema,
    /// Database-level isolation - each tenant has its own database.
    Database,
}

impl Default for IsolationLevel {
    fn default() -> Self {
        Self::RowLevel
    }
}

impl std::fmt::Display for IsolationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RowLevel => write!(f, "row_level"),
            Self::Schema => write!(f, "schema"),
            Self::Database => write!(f, "database"),
        }
    }
}

/// Configuration for tenant isolation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsolationConfig {
    /// The isolation level to use.
    pub level: IsolationLevel,
    /// Whether to enforce strict isolation checks.
    pub strict_mode: bool,
    /// Whether to log isolation violations.
    pub log_violations: bool,
    /// Schema prefix for schema-level isolation.
    pub schema_prefix: String,
    /// Database prefix for database-level isolation.
    pub database_prefix: String,
}

impl Default for IsolationConfig {
    fn default() -> Self {
        Self {
            level: IsolationLevel::RowLevel,
            strict_mode: true,
            log_violations: true,
            schema_prefix: "tenant_".to_string(),
            database_prefix: "zephyr_".to_string(),
        }
    }
}

/// Manages tenant isolation and access control.
///
/// Ensures that tenant A cannot access tenant B's data.
#[derive(Debug)]
pub struct TenantIsolation {
    config: IsolationConfig,
    /// Current tenant context (thread-local would be better in production).
    current_tenant: RwLock<Option<TenantId>>,
    /// Set of active tenant IDs for validation.
    active_tenants: RwLock<HashSet<TenantId>>,
}

impl TenantIsolation {
    /// Creates a new tenant isolation manager.
    #[must_use]
    pub fn new(config: IsolationConfig) -> Self {
        Self {
            config,
            current_tenant: RwLock::new(None),
            active_tenants: RwLock::new(HashSet::new()),
        }
    }

    /// Creates a new tenant isolation manager with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(IsolationConfig::default())
    }

    /// Returns the isolation level.
    #[must_use]
    pub fn isolation_level(&self) -> IsolationLevel {
        self.config.level
    }

    /// Registers a tenant as active.
    pub fn register_tenant(&self, tenant_id: TenantId) {
        let id_str = tenant_id.to_string();
        self.active_tenants.write().insert(tenant_id);
        debug!("Registered tenant: {}", id_str);
    }

    /// Unregisters a tenant.
    pub fn unregister_tenant(&self, tenant_id: &TenantId) {
        self.active_tenants.write().remove(tenant_id);
        debug!("Unregistered tenant: {}", tenant_id);
    }

    /// Checks if a tenant is registered.
    #[must_use]
    pub fn is_tenant_registered(&self, tenant_id: &TenantId) -> bool {
        self.active_tenants.read().contains(tenant_id)
    }

    /// Sets the current tenant context.
    ///
    /// # Errors
    ///
    /// Returns an error if the tenant is not registered.
    pub fn set_current_tenant(&self, tenant_id: TenantId) -> Result<()> {
        if self.config.strict_mode && !self.is_tenant_registered(&tenant_id) {
            return Err(SecurityError::authentication_failed(format!(
                "Tenant {} is not registered",
                tenant_id
            )));
        }

        *self.current_tenant.write() = Some(tenant_id);
        Ok(())
    }

    /// Clears the current tenant context.
    pub fn clear_current_tenant(&self) {
        *self.current_tenant.write() = None;
    }

    /// Returns the current tenant ID.
    #[must_use]
    pub fn current_tenant(&self) -> Option<TenantId> {
        self.current_tenant.read().clone()
    }

    /// Validates that an operation is allowed for the current tenant.
    ///
    /// # Arguments
    ///
    /// * `resource_tenant_id` - The tenant ID that owns the resource
    ///
    /// # Errors
    ///
    /// Returns an error if the current tenant doesn't match the resource tenant.
    pub fn validate_access(&self, resource_tenant_id: &TenantId) -> Result<()> {
        let current = self.current_tenant.read();

        match &*current {
            Some(current_id) if current_id == resource_tenant_id => {
                debug!(
                    "Access validated for tenant {} to resource owned by {}",
                    current_id, resource_tenant_id
                );
                Ok(())
            }
            Some(current_id) => {
                if self.config.log_violations {
                    warn!(
                        "Tenant isolation violation: tenant {} attempted to access resource owned by {}",
                        current_id, resource_tenant_id
                    );
                }
                Err(SecurityError::authorization_failed(
                    current_id.to_string(),
                    format!("access to tenant {}", resource_tenant_id),
                ))
            }
            None => {
                if self.config.log_violations {
                    warn!(
                        "Tenant isolation violation: no tenant context set when accessing resource owned by {}",
                        resource_tenant_id
                    );
                }
                Err(SecurityError::authentication_failed(
                    "No tenant context set",
                ))
            }
        }
    }

    /// Returns the schema name for a tenant (for schema-level isolation).
    #[must_use]
    pub fn schema_name(&self, tenant_id: &TenantId) -> String {
        format!("{}{}", self.config.schema_prefix, tenant_id)
    }

    /// Returns the database name for a tenant (for database-level isolation).
    #[must_use]
    pub fn database_name(&self, tenant_id: &TenantId) -> String {
        format!("{}{}", self.config.database_prefix, tenant_id)
    }

    /// Returns the tenant ID column filter for row-level security.
    #[must_use]
    pub fn row_filter(&self, tenant_id: &TenantId) -> String {
        format!("tenant_id = '{}'", tenant_id)
    }

    /// Creates a scoped tenant context that automatically clears on drop.
    #[must_use]
    pub fn scoped_context(self: &Arc<Self>, tenant_id: TenantId) -> Result<TenantScope> {
        self.set_current_tenant(tenant_id.clone())?;
        Ok(TenantScope {
            isolation: Arc::clone(self),
            tenant_id,
        })
    }
}

impl Default for TenantIsolation {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// A scoped tenant context that clears the tenant on drop.
pub struct TenantScope {
    isolation: Arc<TenantIsolation>,
    tenant_id: TenantId,
}

impl TenantScope {
    /// Returns the tenant ID for this scope.
    #[must_use]
    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }
}

impl Drop for TenantScope {
    fn drop(&mut self) {
        self.isolation.clear_current_tenant();
        debug!("Cleared tenant context for {}", self.tenant_id);
    }
}

/// Trait for resources that belong to a tenant.
#[allow(dead_code)]
pub trait TenantOwned {
    /// Returns the tenant ID that owns this resource.
    fn tenant_id(&self) -> &TenantId;
}

/// Extension trait for validating tenant access to owned resources.
#[allow(dead_code)]
pub trait TenantAccessValidator {
    /// Validates that the current tenant can access this resource.
    ///
    /// # Errors
    ///
    /// Returns an error if access is denied.
    fn validate_tenant_access(&self, isolation: &TenantIsolation) -> Result<()>;
}

impl<T: TenantOwned> TenantAccessValidator for T {
    fn validate_tenant_access(&self, isolation: &TenantIsolation) -> Result<()> {
        isolation.validate_access(self.tenant_id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolation_level_display() {
        assert_eq!(IsolationLevel::RowLevel.to_string(), "row_level");
        assert_eq!(IsolationLevel::Schema.to_string(), "schema");
        assert_eq!(IsolationLevel::Database.to_string(), "database");
    }

    #[test]
    fn test_tenant_registration() {
        let isolation = TenantIsolation::with_defaults();
        let tenant_id = TenantId::new();

        assert!(!isolation.is_tenant_registered(&tenant_id));

        isolation.register_tenant(tenant_id.clone());
        assert!(isolation.is_tenant_registered(&tenant_id));

        isolation.unregister_tenant(&tenant_id);
        assert!(!isolation.is_tenant_registered(&tenant_id));
    }

    #[test]
    fn test_current_tenant_context() {
        let isolation = TenantIsolation::with_defaults();
        let tenant_id = TenantId::new();

        isolation.register_tenant(tenant_id.clone());
        isolation.set_current_tenant(tenant_id.clone()).unwrap();

        assert_eq!(isolation.current_tenant(), Some(tenant_id.clone()));

        isolation.clear_current_tenant();
        assert_eq!(isolation.current_tenant(), None);
    }

    #[test]
    fn test_access_validation_success() {
        let isolation = TenantIsolation::with_defaults();
        let tenant_id = TenantId::new();

        isolation.register_tenant(tenant_id.clone());
        isolation.set_current_tenant(tenant_id.clone()).unwrap();

        assert!(isolation.validate_access(&tenant_id).is_ok());
    }

    #[test]
    fn test_access_validation_failure() {
        let isolation = TenantIsolation::with_defaults();
        let tenant_a = TenantId::new();
        let tenant_b = TenantId::new();

        isolation.register_tenant(tenant_a.clone());
        isolation.register_tenant(tenant_b.clone());
        isolation.set_current_tenant(tenant_a).unwrap();

        assert!(isolation.validate_access(&tenant_b).is_err());
    }

    #[test]
    fn test_access_validation_no_context() {
        let isolation = TenantIsolation::with_defaults();
        let tenant_id = TenantId::new();

        isolation.register_tenant(tenant_id.clone());

        assert!(isolation.validate_access(&tenant_id).is_err());
    }

    #[test]
    fn test_schema_name() {
        let isolation = TenantIsolation::with_defaults();
        let tenant_id = TenantId::new();

        let schema = isolation.schema_name(&tenant_id);
        assert!(schema.starts_with("tenant_"));
    }

    #[test]
    fn test_database_name() {
        let isolation = TenantIsolation::with_defaults();
        let tenant_id = TenantId::new();

        let db = isolation.database_name(&tenant_id);
        assert!(db.starts_with("zephyr_"));
    }

    #[test]
    fn test_scoped_context() {
        let isolation = Arc::new(TenantIsolation::with_defaults());
        let tenant_id = TenantId::new();

        isolation.register_tenant(tenant_id.clone());

        {
            let _scope = isolation.scoped_context(tenant_id.clone()).unwrap();
            assert_eq!(isolation.current_tenant(), Some(tenant_id.clone()));
        }

        // Context should be cleared after scope drops
        assert_eq!(isolation.current_tenant(), None);
    }

    #[test]
    fn test_strict_mode_unregistered_tenant() {
        let config = IsolationConfig {
            strict_mode: true,
            ..Default::default()
        };
        let isolation = TenantIsolation::new(config);
        let tenant_id = TenantId::new();

        // Should fail because tenant is not registered
        assert!(isolation.set_current_tenant(tenant_id).is_err());
    }

    #[test]
    fn test_non_strict_mode_unregistered_tenant() {
        let config = IsolationConfig {
            strict_mode: false,
            ..Default::default()
        };
        let isolation = TenantIsolation::new(config);
        let tenant_id = TenantId::new();

        // Should succeed even though tenant is not registered
        assert!(isolation.set_current_tenant(tenant_id).is_ok());
    }
}
