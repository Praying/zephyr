//! Multi-tenant support module.
//!
//! This module provides:
//! - Tenant isolation at database and cache level
//! - Tenant identification via subdomain, API key, or JWT claim
//! - Resource quotas per tenant
//! - Tenant management API
//!
//! # Example
//!
//! ```no_run
//! use zephyr_security::tenant::{TenantManager, Tenant, TenantConfig, ResourceQuota};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let manager = TenantManager::new();

#![allow(clippy::module_inception)]
//!
//! // Create a tenant with resource quotas
//! let quota = ResourceQuota::default();
//! let config = TenantConfig::new("Acme Corp", "acme-corp", quota);
//! let tenant = manager.create_tenant(config).await?;
//!
//! // Get tenant by ID
//! let tenant = manager.get_tenant(&tenant.id()).await?;
//! # Ok(())
//! # }
//! ```

mod config;
mod identifier;
mod isolation;
mod manager;
mod quota;
mod tenant;

pub use config::TenantConfig;
pub use identifier::{TenantIdentifier, TenantIdentifierExtractor};
pub use isolation::{IsolationLevel, TenantIsolation};
pub use manager::TenantManager;
pub use quota::{QuotaUsage, ResourceQuota};
pub use tenant::{Tenant, TenantMetadata, TenantStatus};
