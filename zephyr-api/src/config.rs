//! API configuration types.
//!
//! This module provides configuration for the API server including:
//! - Server binding address and port
//! - JWT authentication settings
//! - Rate limiting configuration
//! - CORS settings
//! - WebSocket settings

use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::ws::WsConfig;

/// API server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Server host address
    #[serde(default = "default_host")]
    pub host: String,

    /// Server port
    #[serde(default = "default_port")]
    pub port: u16,

    /// JWT configuration
    #[serde(default)]
    pub jwt: JwtConfig,

    /// Rate limiting configuration
    #[serde(default)]
    pub rate_limit: RateLimitConfig,

    /// CORS configuration
    #[serde(default)]
    pub cors: CorsConfig,

    /// WebSocket configuration
    #[serde(default)]
    pub websocket: WsConfig,

    /// Request timeout in seconds
    #[serde(default = "default_request_timeout")]
    pub request_timeout_secs: u64,

    /// Enable request logging
    #[serde(default = "default_true")]
    pub enable_request_logging: bool,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            jwt: JwtConfig::default(),
            rate_limit: RateLimitConfig::default(),
            cors: CorsConfig::default(),
            websocket: WsConfig::default(),
            request_timeout_secs: default_request_timeout(),
            enable_request_logging: true,
        }
    }
}

impl ApiConfig {
    /// Returns the server bind address.
    #[must_use]
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Returns the request timeout duration.
    #[must_use]
    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.request_timeout_secs)
    }
}

/// JWT authentication configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    /// Secret key for signing tokens (should be loaded from environment in production)
    #[serde(default = "default_jwt_secret")]
    pub secret: String,

    /// Token expiration time in seconds
    #[serde(default = "default_token_expiration")]
    pub expiration_secs: u64,

    /// Issuer claim
    #[serde(default = "default_issuer")]
    pub issuer: String,

    /// Audience claim
    #[serde(default = "default_audience")]
    pub audience: String,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: default_jwt_secret(),
            expiration_secs: default_token_expiration(),
            issuer: default_issuer(),
            audience: default_audience(),
        }
    }
}

impl JwtConfig {
    /// Returns the token expiration duration.
    #[must_use]
    pub fn expiration(&self) -> Duration {
        Duration::from_secs(self.expiration_secs)
    }
}

/// Rate limiting configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Enable rate limiting
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Maximum requests per window
    #[serde(default = "default_max_requests")]
    pub max_requests: u32,

    /// Window duration in seconds
    #[serde(default = "default_window_secs")]
    pub window_secs: u64,

    /// Burst allowance (additional requests allowed in burst)
    #[serde(default = "default_burst")]
    pub burst: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_requests: default_max_requests(),
            window_secs: default_window_secs(),
            burst: default_burst(),
        }
    }
}

impl RateLimitConfig {
    /// Returns the window duration.
    #[must_use]
    pub fn window(&self) -> Duration {
        Duration::from_secs(self.window_secs)
    }
}

/// CORS configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    /// Enable CORS
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Allowed origins (empty means all origins)
    #[serde(default)]
    pub allowed_origins: Vec<String>,

    /// Allowed methods
    #[serde(default = "default_methods")]
    pub allowed_methods: Vec<String>,

    /// Allowed headers
    #[serde(default = "default_headers")]
    pub allowed_headers: Vec<String>,

    /// Allow credentials
    #[serde(default)]
    pub allow_credentials: bool,

    /// Max age for preflight cache in seconds
    #[serde(default = "default_max_age")]
    pub max_age_secs: u64,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            allowed_origins: vec![],
            allowed_methods: default_methods(),
            allowed_headers: default_headers(),
            allow_credentials: false,
            max_age_secs: default_max_age(),
        }
    }
}

// Default value functions
fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_jwt_secret() -> String {
    // In production, this should be loaded from environment
    "change-me-in-production".to_string()
}

fn default_token_expiration() -> u64 {
    3600 // 1 hour
}

fn default_issuer() -> String {
    "zephyr".to_string()
}

fn default_audience() -> String {
    "zephyr-api".to_string()
}

fn default_request_timeout() -> u64 {
    30
}

fn default_true() -> bool {
    true
}

fn default_max_requests() -> u32 {
    100
}

fn default_window_secs() -> u64 {
    60
}

fn default_burst() -> u32 {
    20
}

fn default_methods() -> Vec<String> {
    vec![
        "GET".to_string(),
        "POST".to_string(),
        "PUT".to_string(),
        "DELETE".to_string(),
        "OPTIONS".to_string(),
    ]
}

fn default_headers() -> Vec<String> {
    vec![
        "Content-Type".to_string(),
        "Authorization".to_string(),
        "X-Request-Id".to_string(),
    ]
}

fn default_max_age() -> u64 {
    3600
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_config_default() {
        let config = ApiConfig::default();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert!(config.jwt.expiration_secs > 0);
        assert!(config.rate_limit.enabled);
        assert!(config.cors.enabled);
    }

    #[test]
    fn test_bind_address() {
        let config = ApiConfig {
            host: "127.0.0.1".to_string(),
            port: 3000,
            ..Default::default()
        };
        assert_eq!(config.bind_address(), "127.0.0.1:3000");
    }

    #[test]
    fn test_jwt_config_expiration() {
        let config = JwtConfig {
            expiration_secs: 7200,
            ..Default::default()
        };
        assert_eq!(config.expiration(), Duration::from_secs(7200));
    }

    #[test]
    fn test_rate_limit_window() {
        let config = RateLimitConfig {
            window_secs: 120,
            ..Default::default()
        };
        assert_eq!(config.window(), Duration::from_secs(120));
    }
}
