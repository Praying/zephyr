//! Security error types.
//!
//! This module defines error types for security operations including
//! key management, access control, and audit logging.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Security-related errors.
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityError {
    /// Encryption or decryption failed.
    #[error("Encryption error: {reason}")]
    Encryption {
        /// Reason for the encryption failure.
        reason: String,
    },

    /// Key derivation failed.
    #[error("Key derivation error: {reason}")]
    KeyDerivation {
        /// Reason for the key derivation failure.
        reason: String,
    },

    /// Secret not found.
    #[error("Secret not found: {name}")]
    SecretNotFound {
        /// Name of the secret that was not found.
        name: String,
    },

    /// Invalid secret format.
    #[error("Invalid secret format: {reason}")]
    InvalidSecretFormat {
        /// Reason for the invalid format.
        reason: String,
    },

    /// Authentication failed.
    #[error("Authentication failed: {reason}")]
    AuthenticationFailed {
        /// Reason for the authentication failure.
        reason: String,
    },

    /// Authorization failed.
    #[error("Authorization failed: user '{user}' lacks permission '{permission}'")]
    AuthorizationFailed {
        /// User who attempted the action.
        user: String,
        /// Permission that was required.
        permission: String,
    },

    /// Role not found.
    #[error("Role not found: {role}")]
    RoleNotFound {
        /// Name of the role that was not found.
        role: String,
    },

    /// Permission not found.
    #[error("Permission not found: {permission}")]
    PermissionNotFound {
        /// Name of the permission that was not found.
        permission: String,
    },

    /// IP address not in whitelist.
    #[error("IP address not allowed: {ip}")]
    IpNotAllowed {
        /// IP address that was rejected.
        ip: String,
    },

    /// Session expired.
    #[error("Session expired")]
    SessionExpired,

    /// Invalid token.
    #[error("Invalid token: {reason}")]
    InvalidToken {
        /// Reason for the invalid token.
        reason: String,
    },

    /// Audit log error.
    #[error("Audit log error: {reason}")]
    AuditLogError {
        /// Reason for the audit log error.
        reason: String,
    },

    /// Signature verification failed.
    #[error("Signature verification failed")]
    SignatureVerificationFailed,

    /// Storage error.
    #[error("Storage error: {reason}")]
    StorageError {
        /// Reason for the storage error.
        reason: String,
    },

    /// Configuration error.
    #[error("Configuration error: {reason}")]
    ConfigurationError {
        /// Reason for the configuration error.
        reason: String,
    },

    /// Tenant not found.
    #[error("Tenant not found: {tenant_id}")]
    TenantNotFound {
        /// ID of the tenant that was not found.
        tenant_id: String,
    },

    /// Tenant suspended.
    #[error("Tenant suspended: {tenant_id}")]
    TenantSuspended {
        /// ID of the suspended tenant.
        tenant_id: String,
    },

    /// Quota exceeded.
    #[error("Quota exceeded for tenant '{tenant_id}': {resource}")]
    QuotaExceeded {
        /// ID of the tenant.
        tenant_id: String,
        /// Resource that exceeded quota.
        resource: String,
    },

    /// Tenant isolation violation.
    #[error("Tenant isolation violation: {reason}")]
    TenantIsolationViolation {
        /// Reason for the violation.
        reason: String,
    },
}

impl SecurityError {
    /// Creates a new encryption error.
    #[must_use]
    pub fn encryption(reason: impl Into<String>) -> Self {
        Self::Encryption {
            reason: reason.into(),
        }
    }

    /// Creates a new key derivation error.
    #[must_use]
    pub fn key_derivation(reason: impl Into<String>) -> Self {
        Self::KeyDerivation {
            reason: reason.into(),
        }
    }

    /// Creates a new secret not found error.
    #[must_use]
    pub fn secret_not_found(name: impl Into<String>) -> Self {
        Self::SecretNotFound { name: name.into() }
    }

    /// Creates a new invalid secret format error.
    #[must_use]
    pub fn invalid_secret_format(reason: impl Into<String>) -> Self {
        Self::InvalidSecretFormat {
            reason: reason.into(),
        }
    }

    /// Creates a new authentication failed error.
    #[must_use]
    pub fn authentication_failed(reason: impl Into<String>) -> Self {
        Self::AuthenticationFailed {
            reason: reason.into(),
        }
    }

    /// Creates a new authorization failed error.
    #[must_use]
    pub fn authorization_failed(user: impl Into<String>, permission: impl Into<String>) -> Self {
        Self::AuthorizationFailed {
            user: user.into(),
            permission: permission.into(),
        }
    }

    /// Creates a new role not found error.
    #[must_use]
    pub fn role_not_found(role: impl Into<String>) -> Self {
        Self::RoleNotFound { role: role.into() }
    }

    /// Creates a new permission not found error.
    #[must_use]
    pub fn permission_not_found(permission: impl Into<String>) -> Self {
        Self::PermissionNotFound {
            permission: permission.into(),
        }
    }

    /// Creates a new IP not allowed error.
    #[must_use]
    pub fn ip_not_allowed(ip: impl Into<String>) -> Self {
        Self::IpNotAllowed { ip: ip.into() }
    }

    /// Creates a new audit log error.
    #[must_use]
    pub fn audit_log_error(reason: impl Into<String>) -> Self {
        Self::AuditLogError {
            reason: reason.into(),
        }
    }

    /// Creates a new storage error.
    #[must_use]
    pub fn storage_error(reason: impl Into<String>) -> Self {
        Self::StorageError {
            reason: reason.into(),
        }
    }

    /// Creates a new configuration error.
    #[must_use]
    pub fn configuration_error(reason: impl Into<String>) -> Self {
        Self::ConfigurationError {
            reason: reason.into(),
        }
    }

    /// Creates a new invalid token error.
    #[must_use]
    pub fn invalid_token(reason: impl Into<String>) -> Self {
        Self::InvalidToken {
            reason: reason.into(),
        }
    }

    /// Creates a new tenant not found error.
    #[must_use]
    pub fn tenant_not_found(tenant_id: impl Into<String>) -> Self {
        Self::TenantNotFound {
            tenant_id: tenant_id.into(),
        }
    }

    /// Creates a new tenant suspended error.
    #[must_use]
    pub fn tenant_suspended(tenant_id: impl Into<String>) -> Self {
        Self::TenantSuspended {
            tenant_id: tenant_id.into(),
        }
    }

    /// Creates a new quota exceeded error.
    #[must_use]
    pub fn quota_exceeded(tenant_id: impl Into<String>, resource: impl Into<String>) -> Self {
        Self::QuotaExceeded {
            tenant_id: tenant_id.into(),
            resource: resource.into(),
        }
    }

    /// Creates a new tenant isolation violation error.
    #[must_use]
    pub fn tenant_isolation_violation(reason: impl Into<String>) -> Self {
        Self::TenantIsolationViolation {
            reason: reason.into(),
        }
    }

    /// Returns true if this error is related to authentication.
    #[must_use]
    pub const fn is_authentication_error(&self) -> bool {
        matches!(
            self,
            Self::AuthenticationFailed { .. } | Self::InvalidToken { .. } | Self::SessionExpired
        )
    }

    /// Returns true if this error is related to authorization.
    #[must_use]
    pub const fn is_authorization_error(&self) -> bool {
        matches!(
            self,
            Self::AuthorizationFailed { .. }
                | Self::RoleNotFound { .. }
                | Self::PermissionNotFound { .. }
                | Self::IpNotAllowed { .. }
        )
    }

    /// Returns true if this error is related to cryptography.
    #[must_use]
    pub const fn is_crypto_error(&self) -> bool {
        matches!(
            self,
            Self::Encryption { .. }
                | Self::KeyDerivation { .. }
                | Self::SignatureVerificationFailed
        )
    }

    /// Returns true if this error is related to tenants.
    #[must_use]
    pub const fn is_tenant_error(&self) -> bool {
        matches!(
            self,
            Self::TenantNotFound { .. }
                | Self::TenantSuspended { .. }
                | Self::QuotaExceeded { .. }
                | Self::TenantIsolationViolation { .. }
        )
    }
}

/// A specialized Result type for security operations.
pub type Result<T> = std::result::Result<T, SecurityError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = SecurityError::encryption("invalid key length");
        assert!(err.to_string().contains("invalid key length"));

        let err = SecurityError::secret_not_found("api_key");
        assert!(err.to_string().contains("api_key"));

        let err = SecurityError::authorization_failed("user1", "trade:execute");
        assert!(err.to_string().contains("user1"));
        assert!(err.to_string().contains("trade:execute"));
    }

    #[test]
    fn test_error_classification() {
        assert!(
            SecurityError::AuthenticationFailed {
                reason: "test".to_string()
            }
            .is_authentication_error()
        );
        assert!(SecurityError::SessionExpired.is_authentication_error());

        assert!(
            SecurityError::AuthorizationFailed {
                user: "u".to_string(),
                permission: "p".to_string()
            }
            .is_authorization_error()
        );
        assert!(
            SecurityError::IpNotAllowed {
                ip: "1.2.3.4".to_string()
            }
            .is_authorization_error()
        );

        assert!(
            SecurityError::Encryption {
                reason: "test".to_string()
            }
            .is_crypto_error()
        );
        assert!(SecurityError::SignatureVerificationFailed.is_crypto_error());
    }

    #[test]
    fn test_serde_roundtrip() {
        let err = SecurityError::encryption("test error");
        let json = serde_json::to_string(&err).unwrap();
        let parsed: SecurityError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, parsed);
    }
}
