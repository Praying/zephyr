//! Tenant configuration.

#![allow(clippy::disallowed_types)]
#![allow(clippy::collapsible_if)]

use super::quota::ResourceQuota;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for creating or updating a tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantConfig {
    /// Tenant name (display name).
    pub name: String,
    /// Tenant slug (URL-safe identifier).
    pub slug: String,
    /// Resource quota for the tenant.
    pub quota: ResourceQuota,
    /// Contact email.
    pub contact_email: Option<String>,
    /// Company name.
    pub company_name: Option<String>,
    /// Custom branding settings.
    pub branding: Option<BrandingConfig>,
    /// Exchange credentials (encrypted).
    pub exchange_credentials: HashMap<String, ExchangeCredentialConfig>,
    /// Risk limits.
    pub risk_limits: Option<RiskLimitConfig>,
    /// Custom metadata.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl TenantConfig {
    /// Creates a new tenant configuration.
    #[must_use]
    pub fn new(name: impl Into<String>, slug: impl Into<String>, quota: ResourceQuota) -> Self {
        Self {
            name: name.into(),
            slug: slug.into(),
            quota,
            contact_email: None,
            company_name: None,
            branding: None,
            exchange_credentials: HashMap::new(),
            risk_limits: None,
            metadata: HashMap::new(),
        }
    }

    /// Creates a minimal tenant configuration with default quota.
    #[must_use]
    pub fn minimal(name: impl Into<String>, slug: impl Into<String>) -> Self {
        Self::new(name, slug, ResourceQuota::default())
    }

    /// Sets the contact email.
    #[must_use]
    pub fn with_contact_email(mut self, email: impl Into<String>) -> Self {
        self.contact_email = Some(email.into());
        self
    }

    /// Sets the company name.
    #[must_use]
    pub fn with_company_name(mut self, company: impl Into<String>) -> Self {
        self.company_name = Some(company.into());
        self
    }

    /// Sets the branding configuration.
    #[must_use]
    pub fn with_branding(mut self, branding: BrandingConfig) -> Self {
        self.branding = Some(branding);
        self
    }

    /// Adds exchange credentials.
    #[must_use]
    pub fn with_exchange_credentials(
        mut self,
        exchange: impl Into<String>,
        credentials: ExchangeCredentialConfig,
    ) -> Self {
        self.exchange_credentials
            .insert(exchange.into(), credentials);
        self
    }

    /// Sets the risk limits.
    #[must_use]
    pub fn with_risk_limits(mut self, limits: RiskLimitConfig) -> Self {
        self.risk_limits = Some(limits);
        self
    }

    /// Adds custom metadata.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Validates the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        if self.name.is_empty() {
            return Err(ConfigValidationError::EmptyField("name".to_string()));
        }

        if self.slug.is_empty() {
            return Err(ConfigValidationError::EmptyField("slug".to_string()));
        }

        // Validate slug format (alphanumeric and hyphens only)
        if !self
            .slug
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(ConfigValidationError::InvalidSlug(self.slug.clone()));
        }

        // Validate email format if provided
        if let Some(email) = &self.contact_email {
            if !email.contains('@') {
                return Err(ConfigValidationError::InvalidEmail(email.clone()));
            }
        }

        Ok(())
    }
}

/// Branding configuration for white-label deployments.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BrandingConfig {
    /// Logo URL.
    pub logo_url: Option<String>,
    /// Primary color (hex format, e.g., "#007bff").
    pub primary_color: Option<String>,
    /// Secondary color (hex format).
    pub secondary_color: Option<String>,
    /// Custom domain for the tenant.
    pub custom_domain: Option<String>,
    /// Favicon URL.
    pub favicon_url: Option<String>,
    /// Custom CSS URL.
    pub custom_css_url: Option<String>,
}

impl BrandingConfig {
    /// Creates a new branding configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the logo URL.
    #[must_use]
    pub fn with_logo(mut self, url: impl Into<String>) -> Self {
        self.logo_url = Some(url.into());
        self
    }

    /// Sets the primary color.
    #[must_use]
    pub fn with_primary_color(mut self, color: impl Into<String>) -> Self {
        self.primary_color = Some(color.into());
        self
    }

    /// Sets the custom domain.
    #[must_use]
    pub fn with_custom_domain(mut self, domain: impl Into<String>) -> Self {
        self.custom_domain = Some(domain.into());
        self
    }
}

/// Exchange credential configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeCredentialConfig {
    /// Exchange name (e.g., "binance", "okx").
    pub exchange: String,
    /// API key (will be encrypted).
    pub api_key: String,
    /// API secret (will be encrypted).
    pub api_secret: String,
    /// Passphrase (for exchanges that require it).
    pub passphrase: Option<String>,
    /// Whether this is a testnet credential.
    pub testnet: bool,
    /// Sub-account name (if applicable).
    pub sub_account: Option<String>,
}

impl ExchangeCredentialConfig {
    /// Creates a new exchange credential configuration.
    #[must_use]
    pub fn new(
        exchange: impl Into<String>,
        api_key: impl Into<String>,
        api_secret: impl Into<String>,
    ) -> Self {
        Self {
            exchange: exchange.into(),
            api_key: api_key.into(),
            api_secret: api_secret.into(),
            passphrase: None,
            testnet: false,
            sub_account: None,
        }
    }

    /// Sets the passphrase.
    #[must_use]
    pub fn with_passphrase(mut self, passphrase: impl Into<String>) -> Self {
        self.passphrase = Some(passphrase.into());
        self
    }

    /// Sets whether this is a testnet credential.
    #[must_use]
    pub fn with_testnet(mut self, testnet: bool) -> Self {
        self.testnet = testnet;
        self
    }

    /// Sets the sub-account name.
    #[must_use]
    pub fn with_sub_account(mut self, sub_account: impl Into<String>) -> Self {
        self.sub_account = Some(sub_account.into());
        self
    }
}

/// Risk limit configuration for a tenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimitConfig {
    /// Maximum position value per symbol (in quote currency).
    pub max_position_value: Option<f64>,
    /// Maximum total position value across all symbols.
    pub max_total_position_value: Option<f64>,
    /// Maximum daily loss (in quote currency).
    pub max_daily_loss: Option<f64>,
    /// Maximum orders per second.
    pub max_orders_per_second: Option<u32>,
    /// Maximum leverage.
    pub max_leverage: Option<u8>,
    /// Symbols that are allowed to trade.
    pub allowed_symbols: Option<Vec<String>>,
    /// Symbols that are blocked from trading.
    pub blocked_symbols: Option<Vec<String>>,
}

impl Default for RiskLimitConfig {
    fn default() -> Self {
        Self {
            max_position_value: Some(100_000.0),
            max_total_position_value: Some(500_000.0),
            max_daily_loss: Some(10_000.0),
            max_orders_per_second: Some(10),
            max_leverage: Some(20),
            allowed_symbols: None,
            blocked_symbols: None,
        }
    }
}

/// Configuration validation errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigValidationError {
    /// A required field is empty.
    #[error("Field '{0}' cannot be empty")]
    EmptyField(String),

    /// Invalid slug format.
    #[error("Invalid slug format: '{0}' (must be alphanumeric with hyphens)")]
    InvalidSlug(String),

    /// Invalid email format.
    #[error("Invalid email format: '{0}'")]
    InvalidEmail(String),

    /// Invalid color format.
    #[error("Invalid color format: '{0}' (must be hex format like #007bff)")]
    InvalidColor(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_config_creation() {
        let config = TenantConfig::new("Acme Corp", "acme-corp", ResourceQuota::default());

        assert_eq!(config.name, "Acme Corp");
        assert_eq!(config.slug, "acme-corp");
    }

    #[test]
    fn test_tenant_config_builder() {
        let config = TenantConfig::minimal("Acme Corp", "acme-corp")
            .with_contact_email("admin@acme.com")
            .with_company_name("Acme Corporation")
            .with_metadata("tier", "enterprise");

        assert_eq!(config.contact_email, Some("admin@acme.com".to_string()));
        assert_eq!(config.company_name, Some("Acme Corporation".to_string()));
        assert_eq!(config.metadata.get("tier"), Some(&"enterprise".to_string()));
    }

    #[test]
    fn test_config_validation_success() {
        let config =
            TenantConfig::minimal("Acme Corp", "acme-corp").with_contact_email("admin@acme.com");

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_empty_name() {
        let config = TenantConfig::minimal("", "acme-corp");
        assert!(matches!(
            config.validate(),
            Err(ConfigValidationError::EmptyField(_))
        ));
    }

    #[test]
    fn test_config_validation_invalid_slug() {
        let config = TenantConfig::minimal("Acme Corp", "acme corp");
        assert!(matches!(
            config.validate(),
            Err(ConfigValidationError::InvalidSlug(_))
        ));
    }

    #[test]
    fn test_config_validation_invalid_email() {
        let config =
            TenantConfig::minimal("Acme Corp", "acme-corp").with_contact_email("invalid-email");
        assert!(matches!(
            config.validate(),
            Err(ConfigValidationError::InvalidEmail(_))
        ));
    }

    #[test]
    fn test_branding_config() {
        let branding = BrandingConfig::new()
            .with_logo("https://example.com/logo.png")
            .with_primary_color("#007bff")
            .with_custom_domain("trading.acme.com");

        assert_eq!(
            branding.logo_url,
            Some("https://example.com/logo.png".to_string())
        );
        assert_eq!(branding.primary_color, Some("#007bff".to_string()));
        assert_eq!(branding.custom_domain, Some("trading.acme.com".to_string()));
    }

    #[test]
    fn test_exchange_credential_config() {
        let creds = ExchangeCredentialConfig::new("binance", "api_key", "api_secret")
            .with_passphrase("passphrase")
            .with_testnet(true);

        assert_eq!(creds.exchange, "binance");
        assert_eq!(creds.passphrase, Some("passphrase".to_string()));
        assert!(creds.testnet);
    }

    #[test]
    fn test_config_serde() {
        let config =
            TenantConfig::minimal("Acme Corp", "acme-corp").with_contact_email("admin@acme.com");

        let json = serde_json::to_string(&config).unwrap();
        let parsed: TenantConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.name, parsed.name);
        assert_eq!(config.slug, parsed.slug);
    }
}
