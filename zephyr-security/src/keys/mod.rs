//! Key management module.
//!
//! This module provides secure key management functionality including:
//! - AES-256-GCM encryption for secret storage
//! - Argon2id key derivation from master password
//! - Secure memory handling with zeroize
//!
//! # Security Features
//!
//! - All secrets are encrypted at rest using AES-256-GCM
//! - Master keys are derived using Argon2id with configurable parameters
//! - Sensitive data is automatically zeroed from memory when dropped
//! - Secrets are masked in logs and display output
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
//! manager.store_secret("api_key", b"secret-value").await?;
//!
//! // Retrieve a secret
//! let secret = manager.get_secret("api_key").await?;
//! # Ok(())
//! # }
//! ```

mod config;
mod crypto;
mod manager;
mod secret;

pub use config::KeyManagerConfig;
pub use crypto::{Cipher, derive_key};
pub use manager::KeyManager;
pub use secret::{MaskedSecret, Secret, SecretMetadata};
