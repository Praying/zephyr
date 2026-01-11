//! Tenant identification mechanisms.

#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::option_if_let_else)]

use super::tenant::TenantId;
use crate::error::{Result, SecurityError};
use serde::{Deserialize, Serialize};

/// Methods for identifying a tenant from a request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TenantIdentifier {
    /// Identify tenant by subdomain (e.g., acme.example.com).
    Subdomain(String),
    /// Identify tenant by API key header.
    ApiKey(String),
    /// Identify tenant by JWT claim.
    JwtClaim(String),
    /// Identify tenant by tenant ID header.
    TenantIdHeader(TenantId),
    /// Identify tenant by path prefix (e.g., /tenants/{id}/...).
    PathPrefix(String),
}

impl TenantIdentifier {
    /// Creates a subdomain identifier.
    #[must_use]
    pub fn subdomain(subdomain: impl Into<String>) -> Self {
        Self::Subdomain(subdomain.into())
    }

    /// Creates an API key identifier.
    #[must_use]
    pub fn api_key(key: impl Into<String>) -> Self {
        Self::ApiKey(key.into())
    }

    /// Creates a JWT claim identifier.
    #[must_use]
    pub fn jwt_claim(tenant_id: impl Into<String>) -> Self {
        Self::JwtClaim(tenant_id.into())
    }

    /// Creates a tenant ID header identifier.
    #[must_use]
    pub fn tenant_id_header(id: TenantId) -> Self {
        Self::TenantIdHeader(id)
    }

    /// Creates a path prefix identifier.
    #[must_use]
    pub fn path_prefix(prefix: impl Into<String>) -> Self {
        Self::PathPrefix(prefix.into())
    }

    /// Returns the identifier value as a string.
    #[must_use]
    pub fn value(&self) -> String {
        match self {
            Self::Subdomain(s) | Self::ApiKey(s) | Self::JwtClaim(s) | Self::PathPrefix(s) => {
                s.clone()
            }
            Self::TenantIdHeader(id) => id.to_string(),
        }
    }
}

/// Configuration for tenant identification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantIdentifierConfig {
    /// Enable subdomain-based identification.
    pub enable_subdomain: bool,
    /// Enable API key-based identification.
    pub enable_api_key: bool,
    /// Enable JWT claim-based identification.
    pub enable_jwt_claim: bool,
    /// Enable tenant ID header-based identification.
    pub enable_tenant_id_header: bool,
    /// Enable path prefix-based identification.
    pub enable_path_prefix: bool,
    /// Name of the API key header.
    pub api_key_header: String,
    /// Name of the tenant ID header.
    pub tenant_id_header: String,
    /// Name of the JWT claim containing tenant ID.
    pub jwt_claim_name: String,
    /// Base domain for subdomain extraction.
    pub base_domain: Option<String>,
}

impl Default for TenantIdentifierConfig {
    fn default() -> Self {
        Self {
            enable_subdomain: true,
            enable_api_key: true,
            enable_jwt_claim: true,
            enable_tenant_id_header: true,
            enable_path_prefix: false,
            api_key_header: "X-API-Key".to_string(),
            tenant_id_header: "X-Tenant-ID".to_string(),
            jwt_claim_name: "tenant_id".to_string(),
            base_domain: None,
        }
    }
}

/// Extracts tenant identifier from various sources.
pub struct TenantIdentifierExtractor {
    config: TenantIdentifierConfig,
}

impl TenantIdentifierExtractor {
    /// Creates a new extractor with the given configuration.
    #[must_use]
    pub fn new(config: TenantIdentifierConfig) -> Self {
        Self { config }
    }

    /// Creates a new extractor with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(TenantIdentifierConfig::default())
    }

    /// Extracts tenant identifier from a subdomain.
    ///
    /// # Arguments
    ///
    /// * `host` - The full host header value (e.g., "acme.example.com")
    ///
    /// # Errors
    ///
    /// Returns an error if subdomain extraction is disabled or fails.
    pub fn from_subdomain(&self, host: &str) -> Result<TenantIdentifier> {
        if !self.config.enable_subdomain {
            return Err(SecurityError::configuration_error(
                "Subdomain identification is disabled",
            ));
        }

        let subdomain = if let Some(base_domain) = &self.config.base_domain {
            // Extract subdomain by removing base domain
            host.strip_suffix(base_domain)
                .and_then(|s| s.strip_suffix('.'))
                .map(String::from)
        } else {
            // Extract first part of the host
            host.split('.').next().map(String::from)
        };

        subdomain
            .filter(|s| !s.is_empty())
            .map(TenantIdentifier::subdomain)
            .ok_or_else(|| SecurityError::authentication_failed("Invalid subdomain"))
    }

    /// Extracts tenant identifier from an API key.
    ///
    /// # Arguments
    ///
    /// * `api_key` - The API key value
    ///
    /// # Errors
    ///
    /// Returns an error if API key identification is disabled.
    pub fn from_api_key(&self, api_key: &str) -> Result<TenantIdentifier> {
        if !self.config.enable_api_key {
            return Err(SecurityError::configuration_error(
                "API key identification is disabled",
            ));
        }

        if api_key.is_empty() {
            return Err(SecurityError::authentication_failed("Empty API key"));
        }

        Ok(TenantIdentifier::api_key(api_key))
    }

    /// Extracts tenant identifier from a JWT claim.
    ///
    /// # Arguments
    ///
    /// * `tenant_id` - The tenant ID from the JWT claim
    ///
    /// # Errors
    ///
    /// Returns an error if JWT claim identification is disabled.
    pub fn from_jwt_claim(&self, tenant_id: &str) -> Result<TenantIdentifier> {
        if !self.config.enable_jwt_claim {
            return Err(SecurityError::configuration_error(
                "JWT claim identification is disabled",
            ));
        }

        if tenant_id.is_empty() {
            return Err(SecurityError::authentication_failed(
                "Empty tenant ID in JWT",
            ));
        }

        Ok(TenantIdentifier::jwt_claim(tenant_id))
    }

    /// Extracts tenant identifier from a tenant ID header.
    ///
    /// # Arguments
    ///
    /// * `tenant_id` - The tenant ID header value
    ///
    /// # Errors
    ///
    /// Returns an error if tenant ID header identification is disabled or parsing fails.
    pub fn from_tenant_id_header(&self, tenant_id: &str) -> Result<TenantIdentifier> {
        if !self.config.enable_tenant_id_header {
            return Err(SecurityError::configuration_error(
                "Tenant ID header identification is disabled",
            ));
        }

        let id: TenantId = tenant_id
            .parse()
            .map_err(|_| SecurityError::authentication_failed("Invalid tenant ID format"))?;

        Ok(TenantIdentifier::tenant_id_header(id))
    }

    /// Extracts tenant identifier from a path prefix.
    ///
    /// # Arguments
    ///
    /// * `path` - The request path (e.g., "/tenants/acme/orders")
    ///
    /// # Errors
    ///
    /// Returns an error if path prefix identification is disabled or extraction fails.
    pub fn from_path_prefix(&self, path: &str) -> Result<TenantIdentifier> {
        if !self.config.enable_path_prefix {
            return Err(SecurityError::configuration_error(
                "Path prefix identification is disabled",
            ));
        }

        // Expected format: /tenants/{tenant_slug}/...
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() >= 3 && parts[1] == "tenants" {
            let tenant_slug = parts[2];
            if !tenant_slug.is_empty() {
                return Ok(TenantIdentifier::path_prefix(tenant_slug));
            }
        }

        Err(SecurityError::authentication_failed(
            "Could not extract tenant from path",
        ))
    }

    /// Returns the configured API key header name.
    #[must_use]
    pub fn api_key_header(&self) -> &str {
        &self.config.api_key_header
    }

    /// Returns the configured tenant ID header name.
    #[must_use]
    pub fn tenant_id_header(&self) -> &str {
        &self.config.tenant_id_header
    }

    /// Returns the configured JWT claim name.
    #[must_use]
    pub fn jwt_claim_name(&self) -> &str {
        &self.config.jwt_claim_name
    }
}

impl Default for TenantIdentifierExtractor {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subdomain_extraction() {
        let extractor = TenantIdentifierExtractor::with_defaults();

        let id = extractor.from_subdomain("acme.example.com").unwrap();
        assert_eq!(id.value(), "acme");
    }

    #[test]
    fn test_subdomain_with_base_domain() {
        let config = TenantIdentifierConfig {
            base_domain: Some("example.com".to_string()),
            ..Default::default()
        };
        let extractor = TenantIdentifierExtractor::new(config);

        let id = extractor.from_subdomain("acme.example.com").unwrap();
        assert_eq!(id.value(), "acme");
    }

    #[test]
    fn test_api_key_extraction() {
        let extractor = TenantIdentifierExtractor::with_defaults();

        let id = extractor.from_api_key("sk_test_123").unwrap();
        assert_eq!(id.value(), "sk_test_123");
    }

    #[test]
    fn test_empty_api_key_fails() {
        let extractor = TenantIdentifierExtractor::with_defaults();
        assert!(extractor.from_api_key("").is_err());
    }

    #[test]
    fn test_jwt_claim_extraction() {
        let extractor = TenantIdentifierExtractor::with_defaults();

        let id = extractor.from_jwt_claim("tenant-123").unwrap();
        assert_eq!(id.value(), "tenant-123");
    }

    #[test]
    fn test_tenant_id_header_extraction() {
        let extractor = TenantIdentifierExtractor::with_defaults();
        let tenant_id = TenantId::new();

        let id = extractor
            .from_tenant_id_header(&tenant_id.to_string())
            .unwrap();
        if let TenantIdentifier::TenantIdHeader(parsed_id) = id {
            assert_eq!(parsed_id, tenant_id);
        } else {
            panic!("Expected TenantIdHeader variant");
        }
    }

    #[test]
    fn test_path_prefix_extraction() {
        let config = TenantIdentifierConfig {
            enable_path_prefix: true,
            ..Default::default()
        };
        let extractor = TenantIdentifierExtractor::new(config);

        let id = extractor.from_path_prefix("/tenants/acme/orders").unwrap();
        assert_eq!(id.value(), "acme");
    }

    #[test]
    fn test_disabled_method_fails() {
        let config = TenantIdentifierConfig {
            enable_subdomain: false,
            ..Default::default()
        };
        let extractor = TenantIdentifierExtractor::new(config);

        assert!(extractor.from_subdomain("acme.example.com").is_err());
    }

    #[test]
    fn test_identifier_serde() {
        let id = TenantIdentifier::subdomain("acme");
        let json = serde_json::to_string(&id).unwrap();
        let parsed: TenantIdentifier = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }
}
