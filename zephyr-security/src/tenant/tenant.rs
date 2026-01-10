//! Tenant entity definition.

#![allow(clippy::derivable_impls)]
#![allow(clippy::disallowed_types)]

use super::quota::{QuotaUsage, ResourceQuota};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a tenant.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct TenantId(Uuid);

impl TenantId {
    /// Creates a new random tenant ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates a tenant ID from a UUID.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the underlying UUID.
    #[must_use]
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for TenantId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TenantId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// Status of a tenant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TenantStatus {
    /// Tenant is active and can use the system.
    Active,
    /// Tenant is suspended and cannot use the system.
    Suspended,
    /// Tenant is pending activation.
    Pending,
    /// Tenant is being deleted.
    Deleting,
    /// Tenant has been deleted.
    Deleted,
}

impl TenantStatus {
    /// Returns true if the tenant can use the system.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// Returns true if the tenant is suspended.
    #[must_use]
    pub const fn is_suspended(&self) -> bool {
        matches!(self, Self::Suspended)
    }

    /// Returns true if the tenant is being deleted or has been deleted.
    #[must_use]
    pub const fn is_deleted(&self) -> bool {
        matches!(self, Self::Deleting | Self::Deleted)
    }
}

impl Default for TenantStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl std::fmt::Display for TenantStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Suspended => write!(f, "suspended"),
            Self::Pending => write!(f, "pending"),
            Self::Deleting => write!(f, "deleting"),
            Self::Deleted => write!(f, "deleted"),
        }
    }
}

/// A tenant in the multi-tenant system.
///
/// Each tenant has isolated data, configuration, and resource quotas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    id: TenantId,
    name: String,
    slug: String,
    status: TenantStatus,
    quota: ResourceQuota,
    usage: QuotaUsage,
    metadata: TenantMetadata,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl Tenant {
    /// Creates a new tenant with the given name and quota.
    #[must_use]
    pub fn new(name: impl Into<String>, slug: impl Into<String>, quota: ResourceQuota) -> Self {
        let now = Utc::now();
        Self {
            id: TenantId::new(),
            name: name.into(),
            slug: slug.into(),
            status: TenantStatus::Pending,
            quota,
            usage: QuotaUsage::default(),
            metadata: TenantMetadata::default(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Returns the tenant ID.
    #[must_use]
    pub fn id(&self) -> &TenantId {
        &self.id
    }

    /// Returns the tenant name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the tenant slug (URL-safe identifier).
    #[must_use]
    pub fn slug(&self) -> &str {
        &self.slug
    }

    /// Returns the tenant status.
    #[must_use]
    pub fn status(&self) -> TenantStatus {
        self.status
    }

    /// Returns the resource quota.
    #[must_use]
    pub fn quota(&self) -> &ResourceQuota {
        &self.quota
    }

    /// Returns the current resource usage.
    #[must_use]
    pub fn usage(&self) -> &QuotaUsage {
        &self.usage
    }

    /// Returns the tenant metadata.
    #[must_use]
    pub fn metadata(&self) -> &TenantMetadata {
        &self.metadata
    }

    /// Returns the creation timestamp.
    #[must_use]
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Returns the last update timestamp.
    #[must_use]
    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    /// Sets the tenant name.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
        self.updated_at = Utc::now();
    }

    /// Sets the tenant status.
    pub fn set_status(&mut self, status: TenantStatus) {
        self.status = status;
        self.updated_at = Utc::now();
    }

    /// Activates the tenant.
    pub fn activate(&mut self) {
        self.set_status(TenantStatus::Active);
    }

    /// Suspends the tenant.
    pub fn suspend(&mut self) {
        self.set_status(TenantStatus::Suspended);
    }

    /// Marks the tenant for deletion.
    pub fn mark_for_deletion(&mut self) {
        self.set_status(TenantStatus::Deleting);
    }

    /// Updates the resource quota.
    pub fn set_quota(&mut self, quota: ResourceQuota) {
        self.quota = quota;
        self.updated_at = Utc::now();
    }

    /// Updates the resource usage.
    pub fn set_usage(&mut self, usage: QuotaUsage) {
        self.usage = usage;
        self.updated_at = Utc::now();
    }

    /// Updates the metadata.
    pub fn set_metadata(&mut self, metadata: TenantMetadata) {
        self.metadata = metadata;
        self.updated_at = Utc::now();
    }

    /// Checks if the tenant can use the system.
    #[must_use]
    pub fn can_operate(&self) -> bool {
        self.status.is_active()
    }

    /// Checks if a resource usage would exceed the quota.
    #[must_use]
    pub fn would_exceed_quota(&self, resource: &str, additional: u64) -> bool {
        match resource {
            "strategies" => self.usage.strategies + additional > self.quota.max_strategies,
            "positions" => self.usage.positions + additional > self.quota.max_positions,
            "orders_per_second" => {
                self.usage.orders_per_second + additional > self.quota.max_orders_per_second
            }
            "api_requests_per_minute" => {
                self.usage.api_requests_per_minute + additional
                    > self.quota.max_api_requests_per_minute
            }
            _ => false,
        }
    }
}

/// Metadata associated with a tenant.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TenantMetadata {
    /// Contact email for the tenant.
    pub contact_email: Option<String>,
    /// Company name.
    pub company_name: Option<String>,
    /// Custom branding settings for white-label deployments.
    pub branding: Option<TenantBranding>,
    /// Additional custom fields.
    #[serde(default)]
    pub custom_fields: std::collections::HashMap<String, String>,
}

/// Branding settings for white-label deployments.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TenantBranding {
    /// Logo URL.
    pub logo_url: Option<String>,
    /// Primary color (hex).
    pub primary_color: Option<String>,
    /// Secondary color (hex).
    pub secondary_color: Option<String>,
    /// Custom domain.
    pub custom_domain: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_id_generation() {
        let id1 = TenantId::new();
        let id2 = TenantId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_tenant_id_parsing() {
        let id = TenantId::new();
        let s = id.to_string();
        let parsed: TenantId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_tenant_status() {
        assert!(TenantStatus::Active.is_active());
        assert!(!TenantStatus::Suspended.is_active());
        assert!(TenantStatus::Suspended.is_suspended());
        assert!(TenantStatus::Deleted.is_deleted());
        assert!(TenantStatus::Deleting.is_deleted());
    }

    #[test]
    fn test_tenant_creation() {
        let quota = ResourceQuota::default();
        let tenant = Tenant::new("Acme Corp", "acme-corp", quota);

        assert_eq!(tenant.name(), "Acme Corp");
        assert_eq!(tenant.slug(), "acme-corp");
        assert_eq!(tenant.status(), TenantStatus::Pending);
        assert!(!tenant.can_operate());
    }

    #[test]
    fn test_tenant_activation() {
        let quota = ResourceQuota::default();
        let mut tenant = Tenant::new("Acme Corp", "acme-corp", quota);

        tenant.activate();
        assert!(tenant.can_operate());
        assert_eq!(tenant.status(), TenantStatus::Active);
    }

    #[test]
    fn test_tenant_suspension() {
        let quota = ResourceQuota::default();
        let mut tenant = Tenant::new("Acme Corp", "acme-corp", quota);

        tenant.activate();
        tenant.suspend();
        assert!(!tenant.can_operate());
        assert!(tenant.status().is_suspended());
    }

    #[test]
    fn test_quota_check() {
        let mut quota = ResourceQuota::default();
        quota.max_strategies = 10;

        let mut tenant = Tenant::new("Acme Corp", "acme-corp", quota);
        tenant.usage.strategies = 8;

        assert!(!tenant.would_exceed_quota("strategies", 2));
        assert!(tenant.would_exceed_quota("strategies", 3));
    }

    #[test]
    fn test_tenant_serde() {
        let quota = ResourceQuota::default();
        let tenant = Tenant::new("Acme Corp", "acme-corp", quota);

        let json = serde_json::to_string(&tenant).unwrap();
        let parsed: Tenant = serde_json::from_str(&json).unwrap();

        assert_eq!(tenant.id(), parsed.id());
        assert_eq!(tenant.name(), parsed.name());
    }
}
