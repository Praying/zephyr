//! REST client infrastructure.
//!
//! This module provides a robust REST client with:
//! - Request signing (HMAC-SHA256)
//! - Rate limiting with backoff
//! - Error response parsing
//! - Retry logic for transient failures
//!
//! # Example
//!
//! ```ignore
//! use zephyr_gateway::rest::{RestClient, RestConfig};
//!
//! let config = RestConfig::builder()
//!     .base_url("https://api.binance.com")
//!     .api_key("your_api_key")
//!     .api_secret("your_api_secret")
//!     .build();
//!
//! let client = RestClient::new(config)?;
//! let response = client.get("/api/v3/ticker/price")
//!     .query("symbol", "BTCUSDT")
//!     .send()
//!     .await?;
//! ```

mod client;
mod config;
mod rate_limiter;
mod signer;

pub use client::{RequestBuilder, RestClient};
pub use config::{RestConfig, RestConfigBuilder};
pub use rate_limiter::RateLimiter;
pub use signer::{RequestSigner, SignatureType};
