//! Tenant management.

#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::unused_async)]

use super::config::TenantConfig;
use super::identifier::TenantIdentifier;
use super::isolation::TenantIsolation;
use super::quota::{QuotaUsage, ResourceQuota};
use super::tenant::{Tenant, TenantId, TenantMetadata, TenantStatus};
use crate::error::{Result, SecurityError};
use dashmap::DashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Manages tenants in the multi-tenant system.
///
/// Provides CRUD operations for tenants and integrates with
/// the isolation layer for access control.
#[derive(Debug)]
pub struct TenantManager {
    /// All tenants indexed by ID.
    tenants_by_id: DashMap<TenantId, Arc<Tenant>>,
    /// Tenants indexed by slug for fast lookup.
    tenants_by_slug: DashMap<String, TenantId>,
    /// Tenants indexed by API key for fast lookup.
    tenants_by_api_key: DashMap<String, TenantId>,
    /// Tenant isolation manager.
    isolation: Arc<TenantIsolation>,
}

impl TenantManager {
    /// Creates a new tenant manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tenants_by_id: DashMap::new(),
            tenants_by_slug: DashMap::new(),
            tenants_by_api_key: DashMap::new(),
            isolation: Arc::new(TenantIsolation::with_defaults()),
        }
    }

    /// Creates a new tenant manager with a custom isolation configuration.
    #[must_use]
    pub fn with_isolation(isolation: TenantIsolation) -> Self {
        Self {
            tenants_by_id: DashMap::new(),
            tenants_by_slug: DashMap::new(),
            tenants_by_api_key: DashMap::new(),
            isolation: Arc::new(isolation),
        }
    }

    /// Returns the isolation manager.
    #[must_use]
    pub fn isolation(&self) -> &Arc<TenantIsolation> {
        &self.isolation
    }

    /// Creates a new tenant.
    ///
    /// # Arguments
    ///
    /// * `config` - The tenant configuration
    ///
    /// # Errors
    ///
    /// Returns an error if the tenant slug already exists or validation fails.
    pub async fn create_tenant(&self, config: TenantConfig) -> Result<Arc<Tenant>> {
        // Validate configuration
        config
            .validate()
            .map_err(|e| SecurityError::configuration_error(e.to_string()))?;

        // Check if slug already exists
        if self.tenants_by_slug.contains_key(&config.slug) {
            return Err(SecurityError::configuration_error(format!(
                "Tenant with slug '{}' already exists",
                config.slug
            )));
        }

        // Create tenant
        let mut tenant = Tenant::new(&config.name, &config.slug, config.quota);

        // Set metadata
        let mut metadata = TenantMetadata::default();
        metadata.contact_email = config.contact_email;
        metadata.company_name = config.company_name;
        if let Some(branding) = config.branding {
            metadata.branding = Some(super::tenant::TenantBranding {
                logo_url: branding.logo_url,
                primary_color: branding.primary_color,
                secondary_color: branding.secondary_color,
                custom_domain: branding.custom_domain,
            });
        }
        metadata.custom_fields = config.metadata;
        tenant.set_metadata(metadata);

        let tenant_id = tenant.id().clone();
        let tenant_arc = Arc::new(tenant);

        // Store tenant
        self.tenants_by_id
            .insert(tenant_id.clone(), Arc::clone(&tenant_arc));
        self.tenants_by_slug
            .insert(config.slug.clone(), tenant_id.clone());

        // Register with isolation manager
        self.isolation.register_tenant(tenant_id.clone());

        info!("Created tenant: {} ({})", config.name, tenant_id);
        Ok(tenant_arc)
    }

    /// Gets a tenant by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the tenant is not found.
    pub async fn get_tenant(&self, tenant_id: &TenantId) -> Result<Arc<Tenant>> {
        self.tenants_by_id
            .get(tenant_id)
            .map(|entry| Arc::clone(entry.value()))
            .ok_or_else(|| {
                SecurityError::authentication_failed(format!("Tenant {} not found", tenant_id))
            })
    }

    /// Gets a tenant by slug.
    ///
    /// # Errors
    ///
    /// Returns an error if the tenant is not found.
    pub async fn get_tenant_by_slug(&self, slug: &str) -> Result<Arc<Tenant>> {
        let tenant_id = self
            .tenants_by_slug
            .get(slug)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| {
                SecurityError::authentication_failed(format!(
                    "Tenant with slug '{}' not found",
                    slug
                ))
            })?;

        self.get_tenant(&tenant_id).await
    }

    /// Gets a tenant by identifier.
    ///
    /// # Errors
    ///
    /// Returns an error if the tenant is not found.
    pub async fn get_tenant_by_identifier(
        &self,
        identifier: &TenantIdentifier,
    ) -> Result<Arc<Tenant>> {
        match identifier {
            TenantIdentifier::Subdomain(subdomain) | TenantIdentifier::PathPrefix(subdomain) => {
                self.get_tenant_by_slug(subdomain).await
            }
            TenantIdentifier::ApiKey(api_key) => {
                let tenant_id = self
                    .tenants_by_api_key
                    .get(api_key)
                    .map(|entry| entry.value().clone())
                    .ok_or_else(|| SecurityError::authentication_failed("Invalid API key"))?;
                self.get_tenant(&tenant_id).await
            }
            TenantIdentifier::JwtClaim(tenant_id_str) => {
                let tenant_id: TenantId = tenant_id_str.parse().map_err(|_| {
                    SecurityError::authentication_failed("Invalid tenant ID in JWT")
                })?;
                self.get_tenant(&tenant_id).await
            }
            TenantIdentifier::TenantIdHeader(tenant_id) => self.get_tenant(tenant_id).await,
        }
    }

    /// Lists all tenants.
    #[must_use]
    pub fn list_tenants(&self) -> Vec<Arc<Tenant>> {
        self.tenants_by_id
            .iter()
            .map(|entry| Arc::clone(entry.value()))
            .collect()
    }

    /// Lists tenants with a specific status.
    #[must_use]
    pub fn list_tenants_by_status(&self, status: TenantStatus) -> Vec<Arc<Tenant>> {
        self.tenants_by_id
            .iter()
            .filter(|entry| entry.value().status() == status)
            .map(|entry| Arc::clone(entry.value()))
            .collect()
    }

    /// Updates a tenant.
    ///
    /// # Arguments
    ///
    /// * `tenant_id` - The tenant ID
    /// * `update_fn` - A function that modifies the tenant
    ///
    /// # Errors
    ///
    /// Returns an error if the tenant is not found.
    pub async fn update_tenant<F>(&self, tenant_id: &TenantId, update_fn: F) -> Result<Arc<Tenant>>
    where
        F: FnOnce(&mut Tenant),
    {
        let existing = self
            .tenants_by_id
            .get(tenant_id)
            .map(|entry| Arc::clone(entry.value()))
            .ok_or_else(|| {
                SecurityError::authentication_failed(format!("Tenant {} not found", tenant_id))
            })?;

        // Clone the inner Tenant to modify it
        let mut tenant = (*existing).clone();
        update_fn(&mut tenant);

        let tenant_arc = Arc::new(tenant);
        self.tenants_by_id
            .insert(tenant_id.clone(), Arc::clone(&tenant_arc));

        debug!("Updated tenant: {}", tenant_id);
        Ok(tenant_arc)
    }

    /// Activates a tenant.
    ///
    /// # Errors
    ///
    /// Returns an error if the tenant is not found.
    pub async fn activate_tenant(&self, tenant_id: &TenantId) -> Result<Arc<Tenant>> {
        let tenant = self.update_tenant(tenant_id, |t| t.activate()).await?;
        info!("Activated tenant: {}", tenant_id);
        Ok(tenant)
    }

    /// Suspends a tenant.
    ///
    /// # Errors
    ///
    /// Returns an error if the tenant is not found.
    pub async fn suspend_tenant(&self, tenant_id: &TenantId) -> Result<Arc<Tenant>> {
        let tenant = self.update_tenant(tenant_id, |t| t.suspend()).await?;
        warn!("Suspended tenant: {}", tenant_id);
        Ok(tenant)
    }

    /// Deletes a tenant.
    ///
    /// # Errors
    ///
    /// Returns an error if the tenant is not found.
    pub async fn delete_tenant(&self, tenant_id: &TenantId) -> Result<()> {
        let tenant = self
            .tenants_by_id
            .remove(tenant_id)
            .map(|(_, t)| t)
            .ok_or_else(|| {
                SecurityError::authentication_failed(format!("Tenant {} not found", tenant_id))
            })?;

        // Remove from slug index
        self.tenants_by_slug.remove(tenant.slug());

        // Unregister from isolation manager
        self.isolation.unregister_tenant(tenant_id);

        info!("Deleted tenant: {} ({})", tenant.name(), tenant_id);
        Ok(())
    }

    /// Updates the resource quota for a tenant.
    ///
    /// # Errors
    ///
    /// Returns an error if the tenant is not found.
    pub async fn update_quota(
        &self,
        tenant_id: &TenantId,
        quota: ResourceQuota,
    ) -> Result<Arc<Tenant>> {
        self.update_tenant(tenant_id, |t| t.set_quota(quota)).await
    }

    /// Updates the resource usage for a tenant.
    ///
    /// # Errors
    ///
    /// Returns an error if the tenant is not found.
    pub async fn update_usage(
        &self,
        tenant_id: &TenantId,
        usage: QuotaUsage,
    ) -> Result<Arc<Tenant>> {
        self.update_tenant(tenant_id, |t| t.set_usage(usage)).await
    }

    /// Registers an API key for a tenant.
    ///
    /// # Errors
    ///
    /// Returns an error if the tenant is not found or API key already exists.
    pub async fn register_api_key(
        &self,
        tenant_id: &TenantId,
        api_key: impl Into<String>,
    ) -> Result<()> {
        // Verify tenant exists
        let _ = self.get_tenant(tenant_id).await?;

        let api_key = api_key.into();
        if self.tenants_by_api_key.contains_key(&api_key) {
            return Err(SecurityError::configuration_error(
                "API key already registered",
            ));
        }

        self.tenants_by_api_key.insert(api_key, tenant_id.clone());
        debug!("Registered API key for tenant: {}", tenant_id);
        Ok(())
    }

    /// Unregisters an API key.
    pub fn unregister_api_key(&self, api_key: &str) {
        self.tenants_by_api_key.remove(api_key);
    }

    /// Checks if a tenant can perform an operation based on quotas.
    ///
    /// # Errors
    ///
    /// Returns an error if the tenant is not found or quota would be exceeded.
    pub async fn check_quota(
        &self,
        tenant_id: &TenantId,
        resource: &str,
        additional: u64,
    ) -> Result<()> {
        let tenant = self.get_tenant(tenant_id).await?;

        if tenant.would_exceed_quota(resource, additional) {
            return Err(SecurityError::authorization_failed(
                tenant_id.to_string(),
                format!("quota exceeded for resource '{}'", resource),
            ));
        }

        Ok(())
    }

    /// Returns the number of tenants.
    #[must_use]
    pub fn tenant_count(&self) -> usize {
        self.tenants_by_id.len()
    }

    /// Returns the number of active tenants.
    #[must_use]
    pub fn active_tenant_count(&self) -> usize {
        self.tenants_by_id
            .iter()
            .filter(|entry| entry.value().status().is_active())
            .count()
    }
}

impl Default for TenantManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config(name: &str, slug: &str) -> TenantConfig {
        TenantConfig::minimal(name, slug)
    }

    #[tokio::test]
    async fn test_create_tenant() {
        let manager = TenantManager::new();
        let config = create_test_config("Acme Corp", "acme-corp");

        let tenant = manager.create_tenant(config).await.unwrap();

        assert_eq!(tenant.name(), "Acme Corp");
        assert_eq!(tenant.slug(), "acme-corp");
        assert_eq!(tenant.status(), TenantStatus::Pending);
    }

    #[tokio::test]
    async fn test_duplicate_slug_fails() {
        let manager = TenantManager::new();

        let config1 = create_test_config("Acme Corp", "acme-corp");
        let config2 = create_test_config("Another Corp", "acme-corp");

        manager.create_tenant(config1).await.unwrap();
        assert!(manager.create_tenant(config2).await.is_err());
    }

    #[tokio::test]
    async fn test_get_tenant_by_id() {
        let manager = TenantManager::new();
        let config = create_test_config("Acme Corp", "acme-corp");

        let created = manager.create_tenant(config).await.unwrap();
        let fetched = manager.get_tenant(created.id()).await.unwrap();

        assert_eq!(created.id(), fetched.id());
    }

    #[tokio::test]
    async fn test_get_tenant_by_slug() {
        let manager = TenantManager::new();
        let config = create_test_config("Acme Corp", "acme-corp");

        let created = manager.create_tenant(config).await.unwrap();
        let fetched = manager.get_tenant_by_slug("acme-corp").await.unwrap();

        assert_eq!(created.id(), fetched.id());
    }

    #[tokio::test]
    async fn test_activate_tenant() {
        let manager = TenantManager::new();
        let config = create_test_config("Acme Corp", "acme-corp");

        let tenant = manager.create_tenant(config).await.unwrap();
        assert!(!tenant.can_operate());

        let activated = manager.activate_tenant(tenant.id()).await.unwrap();
        assert!(activated.can_operate());
    }

    #[tokio::test]
    async fn test_suspend_tenant() {
        let manager = TenantManager::new();
        let config = create_test_config("Acme Corp", "acme-corp");

        let tenant = manager.create_tenant(config).await.unwrap();
        manager.activate_tenant(tenant.id()).await.unwrap();

        let suspended = manager.suspend_tenant(tenant.id()).await.unwrap();
        assert!(!suspended.can_operate());
        assert!(suspended.status().is_suspended());
    }

    #[tokio::test]
    async fn test_delete_tenant() {
        let manager = TenantManager::new();
        let config = create_test_config("Acme Corp", "acme-corp");

        let tenant = manager.create_tenant(config).await.unwrap();
        let tenant_id = tenant.id().clone();

        manager.delete_tenant(&tenant_id).await.unwrap();

        assert!(manager.get_tenant(&tenant_id).await.is_err());
        assert!(manager.get_tenant_by_slug("acme-corp").await.is_err());
    }

    #[tokio::test]
    async fn test_list_tenants() {
        let manager = TenantManager::new();

        manager
            .create_tenant(create_test_config("Acme Corp", "acme"))
            .await
            .unwrap();
        manager
            .create_tenant(create_test_config("Beta Corp", "beta"))
            .await
            .unwrap();

        let tenants = manager.list_tenants();
        assert_eq!(tenants.len(), 2);
    }

    #[tokio::test]
    async fn test_list_tenants_by_status() {
        let manager = TenantManager::new();

        let tenant1 = manager
            .create_tenant(create_test_config("Acme Corp", "acme"))
            .await
            .unwrap();
        let _tenant2 = manager
            .create_tenant(create_test_config("Beta Corp", "beta"))
            .await
            .unwrap();

        manager.activate_tenant(tenant1.id()).await.unwrap();

        let active = manager.list_tenants_by_status(TenantStatus::Active);
        let pending = manager.list_tenants_by_status(TenantStatus::Pending);

        assert_eq!(active.len(), 1);
        assert_eq!(pending.len(), 1);
        assert_eq!(active[0].slug(), "acme");
        assert_eq!(pending[0].slug(), "beta");
    }

    #[tokio::test]
    async fn test_update_quota() {
        let manager = TenantManager::new();
        let config = create_test_config("Acme Corp", "acme-corp");

        let tenant = manager.create_tenant(config).await.unwrap();

        let new_quota = ResourceQuota::professional_tier();
        let updated = manager.update_quota(tenant.id(), new_quota).await.unwrap();

        assert_eq!(
            updated.quota().max_strategies,
            ResourceQuota::professional_tier().max_strategies
        );
    }

    #[tokio::test]
    async fn test_api_key_registration() {
        let manager = TenantManager::new();
        let config = create_test_config("Acme Corp", "acme-corp");

        let tenant = manager.create_tenant(config).await.unwrap();

        manager
            .register_api_key(tenant.id(), "sk_test_123")
            .await
            .unwrap();

        let identifier = TenantIdentifier::api_key("sk_test_123");
        let fetched = manager.get_tenant_by_identifier(&identifier).await.unwrap();

        assert_eq!(tenant.id(), fetched.id());
    }

    #[tokio::test]
    async fn test_duplicate_api_key_fails() {
        let manager = TenantManager::new();

        let tenant1 = manager
            .create_tenant(create_test_config("Acme Corp", "acme"))
            .await
            .unwrap();
        let tenant2 = manager
            .create_tenant(create_test_config("Beta Corp", "beta"))
            .await
            .unwrap();

        manager
            .register_api_key(tenant1.id(), "sk_test_123")
            .await
            .unwrap();

        assert!(
            manager
                .register_api_key(tenant2.id(), "sk_test_123")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_check_quota() {
        let manager = TenantManager::new();
        let mut config = create_test_config("Acme Corp", "acme-corp");
        config.quota.max_strategies = 10;

        let tenant = manager.create_tenant(config).await.unwrap();

        // Should succeed - within quota
        assert!(
            manager
                .check_quota(tenant.id(), "strategies", 5)
                .await
                .is_ok()
        );

        // Should fail - exceeds quota
        assert!(
            manager
                .check_quota(tenant.id(), "strategies", 15)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_tenant_count() {
        let manager = TenantManager::new();

        assert_eq!(manager.tenant_count(), 0);
        assert_eq!(manager.active_tenant_count(), 0);

        let tenant = manager
            .create_tenant(create_test_config("Acme Corp", "acme"))
            .await
            .unwrap();

        assert_eq!(manager.tenant_count(), 1);
        assert_eq!(manager.active_tenant_count(), 0);

        manager.activate_tenant(tenant.id()).await.unwrap();

        assert_eq!(manager.tenant_count(), 1);
        assert_eq!(manager.active_tenant_count(), 1);
    }

    #[tokio::test]
    async fn test_isolation_integration() {
        let manager = TenantManager::new();
        let config = create_test_config("Acme Corp", "acme-corp");

        let tenant = manager.create_tenant(config).await.unwrap();

        // Tenant should be registered with isolation manager
        assert!(manager.isolation().is_tenant_registered(tenant.id()));

        // Delete tenant
        manager.delete_tenant(tenant.id()).await.unwrap();

        // Tenant should be unregistered
        assert!(!manager.isolation().is_tenant_registered(tenant.id()));
    }
}
