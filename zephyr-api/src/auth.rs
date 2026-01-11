//! JWT authentication module.
//!
//! This module provides JWT token generation and validation for API authentication.

use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, TokenData, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::config::JwtConfig;
use crate::error::ApiError;

/// JWT claims structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    /// Issuer
    pub iss: String,
    /// Audience
    pub aud: String,
    /// Expiration time (Unix timestamp)
    pub exp: i64,
    /// Issued at (Unix timestamp)
    pub iat: i64,
    /// Not before (Unix timestamp)
    pub nbf: i64,
    /// JWT ID (unique identifier)
    pub jti: String,
    /// User role
    pub role: UserRole,
    /// Tenant ID (for multi-tenant support)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
}

/// User role for authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    /// Administrator with full access
    Admin,
    /// Trader with trading permissions
    Trader,
    /// Viewer with read-only access
    Viewer,
    /// Auditor with audit log access
    Auditor,
}

impl UserRole {
    /// Returns true if this role is an administrator.
    #[must_use]
    pub const fn is_admin(&self) -> bool {
        matches!(self, Self::Admin)
    }

    /// Returns true if this role can manage strategies.
    #[must_use]
    pub const fn can_manage_strategies(&self) -> bool {
        matches!(self, Self::Admin | Self::Trader)
    }

    /// Returns true if this role can submit orders.
    #[must_use]
    pub const fn can_submit_orders(&self) -> bool {
        matches!(self, Self::Admin | Self::Trader)
    }

    /// Returns true if this role can view positions.
    #[must_use]
    pub const fn can_view_positions(&self) -> bool {
        // All roles can view positions
        true
    }

    /// Returns true if this role can modify configuration.
    #[must_use]
    pub const fn can_modify_config(&self) -> bool {
        matches!(self, Self::Admin)
    }

    /// Returns true if this role can view audit logs.
    #[must_use]
    pub const fn can_view_audit_logs(&self) -> bool {
        matches!(self, Self::Admin | Self::Auditor)
    }
}

/// JWT token manager.
#[derive(Clone)]
pub struct JwtManager {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    issuer: String,
    audience: String,
    expiration_secs: i64,
}

impl std::fmt::Debug for JwtManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtManager")
            .field("issuer", &self.issuer)
            .field("audience", &self.audience)
            .field("expiration_secs", &self.expiration_secs)
            .finish_non_exhaustive()
    }
}

impl JwtManager {
    /// Creates a new JWT manager from configuration.
    #[must_use]
    pub fn new(config: &JwtConfig) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(config.secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(config.secret.as_bytes()),
            issuer: config.issuer.clone(),
            audience: config.audience.clone(),
            expiration_secs: config.expiration_secs.cast_signed(),
        }
    }

    /// Generates a new JWT token for a user.
    ///
    /// # Errors
    ///
    /// Returns an error if token encoding fails.
    pub fn generate_token(
        &self,
        user_id: &str,
        role: UserRole,
        tenant_id: Option<String>,
    ) -> Result<String, ApiError> {
        let now = Utc::now();
        let exp = now + Duration::seconds(self.expiration_secs);

        let claims = Claims {
            sub: user_id.to_string(),
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            nbf: now.timestamp(),
            jti: uuid::Uuid::new_v4().to_string(),
            role,
            tenant_id,
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| ApiError::Internal(format!("Failed to generate token: {e}")))
    }

    /// Validates a JWT token and returns the claims.
    ///
    /// # Errors
    ///
    /// Returns an error if the token is invalid or expired.
    pub fn validate_token(&self, token: &str) -> Result<Claims, ApiError> {
        let mut validation = Validation::default();
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);

        let token_data: TokenData<Claims> = decode(token, &self.decoding_key, &validation)
            .map_err(|e| ApiError::Unauthorized(format!("Invalid token: {e}")))?;

        Ok(token_data.claims)
    }

    /// Refreshes a token if it's still valid.
    ///
    /// # Errors
    ///
    /// Returns an error if the token is invalid or cannot be refreshed.
    pub fn refresh_token(&self, token: &str) -> Result<String, ApiError> {
        let claims = self.validate_token(token)?;
        self.generate_token(&claims.sub, claims.role, claims.tenant_id)
    }
}

/// Extracts the bearer token from an Authorization header value.
#[must_use]
pub fn extract_bearer_token(auth_header: &str) -> Option<&str> {
    auth_header
        .strip_prefix("Bearer ")
        .or_else(|| auth_header.strip_prefix("bearer "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::JwtConfig;

    fn test_config() -> JwtConfig {
        JwtConfig {
            secret: "test-secret-key-for-testing".to_string(),
            expiration_secs: 3600,
            issuer: "test-issuer".to_string(),
            audience: "test-audience".to_string(),
        }
    }

    #[test]
    fn test_generate_and_validate_token() {
        let manager = JwtManager::new(&test_config());

        let token = manager
            .generate_token("user123", UserRole::Trader, None)
            .unwrap();

        let claims = manager.validate_token(&token).unwrap();
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.role, UserRole::Trader);
        assert!(claims.tenant_id.is_none());
    }

    #[test]
    fn test_generate_token_with_tenant() {
        let manager = JwtManager::new(&test_config());

        let token = manager
            .generate_token("user123", UserRole::Admin, Some("tenant-abc".to_string()))
            .unwrap();

        let claims = manager.validate_token(&token).unwrap();
        assert_eq!(claims.tenant_id, Some("tenant-abc".to_string()));
    }

    #[test]
    fn test_invalid_token() {
        let manager = JwtManager::new(&test_config());

        let result = manager.validate_token("invalid-token");
        assert!(result.is_err());
    }

    #[test]
    fn test_refresh_token() {
        let manager = JwtManager::new(&test_config());

        let token = manager
            .generate_token("user123", UserRole::Viewer, None)
            .unwrap();

        let new_token = manager.refresh_token(&token).unwrap();
        assert_ne!(token, new_token);

        let claims = manager.validate_token(&new_token).unwrap();
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.role, UserRole::Viewer);
    }

    #[test]
    fn test_extract_bearer_token() {
        assert_eq!(extract_bearer_token("Bearer abc123"), Some("abc123"));
        assert_eq!(extract_bearer_token("bearer xyz789"), Some("xyz789"));
        assert_eq!(extract_bearer_token("Basic abc123"), None);
        assert_eq!(extract_bearer_token("abc123"), None);
    }

    #[test]
    fn test_user_role_permissions() {
        assert!(UserRole::Admin.can_manage_strategies());
        assert!(UserRole::Trader.can_manage_strategies());
        assert!(!UserRole::Viewer.can_manage_strategies());
        assert!(!UserRole::Auditor.can_manage_strategies());

        assert!(UserRole::Admin.can_submit_orders());
        assert!(UserRole::Trader.can_submit_orders());
        assert!(!UserRole::Viewer.can_submit_orders());

        assert!(UserRole::Admin.can_modify_config());
        assert!(!UserRole::Trader.can_modify_config());

        assert!(UserRole::Admin.can_view_audit_logs());
        assert!(UserRole::Auditor.can_view_audit_logs());
        assert!(!UserRole::Trader.can_view_audit_logs());
    }
}
