//! # Zephyr Security
//!
//! Security module for the Zephyr cryptocurrency trading system.
//!
//! This crate provides:
//! - Key management with AES-256-GCM encryption and Argon2id key derivation
//! - Role-based access control (RBAC) with granular permissions
//! - Security audit logging with tamper-evident signatures
//! - Multi-tenant support with data isolation and resource quotas
//!
//! # Features
//!
//! - **Key Management**: Secure storage and retrieval of API keys and secrets
//! - **Access Control**: RBAC with predefined and custom roles
//! - **Audit Logging**: Comprehensive logging of security-relevant events
//! - **Multi-Tenancy**: Complete tenant isolation with resource quotas
//!
//! # Example
//!
//! ```no_run
//! use zephyr_security::keys::{KeyManager, KeyManagerConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = KeyManagerConfig::default();
//! let manager = KeyManager::new(config)?;
//!
//! // Store a secret
//! manager.store_secret("binance_api_key", b"my-secret-key").await?;
//!
//! // Retrieve a secret
//! let secret = manager.get_secret("binance_api_key").await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Multi-Tenant Example
//!
//! ```no_run
//! use zephyr_security::tenant::{TenantManager, TenantConfig, ResourceQuota};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let manager = TenantManager::new();
//!
//! // Create a tenant with resource quotas
//! let quota = ResourceQuota::professional_tier();
//! let config = TenantConfig::new("Acme Corp", "acme-corp", quota);
//! let tenant = manager.create_tenant(config).await?;
//!
//! // Activate the tenant
//! manager.activate_tenant(tenant.id()).await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Audit Logging Example
//!
//! ```no_run
//! use zephyr_security::audit::{AuditLogger, AuditEvent, AuditAction};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let logger = AuditLogger::new();
//!
//! // Log a trade execution
//! logger.log_trade_execute("alice", "Executed buy order for BTC").await?;
//!
//! // Query events for a user
//! let events = logger.query_events(Some("alice"), None, None).await?;
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::module_name_repetitions)]

/// Error types for the security module
pub mod error;

/// Key management with encryption and secure storage
pub mod keys;

/// Role-based access control
pub mod access;

/// Security audit logging
pub mod audit;

/// Multi-tenant support
pub mod tenant;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::access::*;
    pub use crate::audit::*;
    pub use crate::error::*;
    pub use crate::keys::*;
    pub use crate::tenant::*;
}
