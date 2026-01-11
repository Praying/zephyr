//! Secret types with secure memory handling.
//!
//! This module provides types for handling sensitive data with:
//! - Automatic memory zeroing on drop
//! - Masked display for logging
//! - Metadata tracking for audit purposes

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// A secret value with automatic memory zeroing.
///
/// The secret data is automatically zeroed from memory when dropped,
/// preventing sensitive data from lingering in memory.
///
/// # Example
///
/// ```
/// use zephyr_security::keys::Secret;
///
/// let secret = Secret::new(b"my-api-key".to_vec());
/// assert_eq!(secret.expose(), b"my-api-key");
/// // Secret is automatically zeroed when dropped
/// ```
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Secret {
    data: Vec<u8>,
}

impl Secret {
    /// Creates a new secret from the given data.
    #[must_use]
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Creates a new secret from a string.
    #[must_use]
    pub fn from_string(s: String) -> Self {
        Self::new(s.into_bytes())
    }

    /// Exposes the secret data.
    ///
    /// Use this method carefully - the returned reference should not be
    /// stored or logged.
    #[must_use]
    pub fn expose(&self) -> &[u8] {
        &self.data
    }

    /// Exposes the secret data as a string, if valid UTF-8.
    ///
    /// # Errors
    ///
    /// Returns `None` if the secret is not valid UTF-8.
    #[must_use]
    pub fn expose_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.data).ok()
    }

    /// Returns the length of the secret data.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if the secret is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Creates a masked version of this secret for display.
    #[must_use]
    pub fn masked(&self) -> MaskedSecret {
        MaskedSecret::from_secret(self)
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secret([REDACTED, {} bytes])", self.data.len())
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl PartialEq for Secret {
    fn eq(&self, other: &Self) -> bool {
        // Use constant-time comparison to prevent timing attacks
        if self.data.len() != other.data.len() {
            return false;
        }
        let mut result = 0u8;
        for (a, b) in self.data.iter().zip(other.data.iter()) {
            result |= a ^ b;
        }
        result == 0
    }
}

impl Eq for Secret {}

/// A masked representation of a secret for safe display.
///
/// Shows only the first and last few characters, with the middle masked.
/// For example: "abc***xyz"
///
/// # Example
///
/// ```
/// use zephyr_security::keys::{Secret, MaskedSecret};
///
/// let secret = Secret::new(b"my-secret-api-key".to_vec());
/// let masked = secret.masked();
/// println!("{}", masked); // Prints something like "my-***key"
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskedSecret {
    masked: String,
    length: usize,
}

impl MaskedSecret {
    /// Creates a masked secret from a secret.
    #[must_use]
    pub fn from_secret(secret: &Secret) -> Self {
        Self::from_bytes(secret.expose())
    }

    /// Creates a masked secret from raw bytes.
    #[must_use]
    pub fn from_bytes(data: &[u8]) -> Self {
        let length = data.len();
        let masked = if length <= 6 {
            // Too short to show any characters
            "***".to_string()
        } else {
            // Show first 3 and last 3 characters
            let s = String::from_utf8_lossy(data);
            let chars: Vec<char> = s.chars().collect();
            if chars.len() <= 6 {
                "***".to_string()
            } else {
                let prefix: String = chars[..3].iter().collect();
                let suffix: String = chars[chars.len() - 3..].iter().collect();
                format!("{prefix}***{suffix}")
            }
        };

        Self { masked, length }
    }

    /// Returns the masked string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.masked
    }

    /// Returns the original length of the secret.
    #[must_use]
    pub const fn original_length(&self) -> usize {
        self.length
    }
}

impl fmt::Display for MaskedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.masked)
    }
}

/// Metadata about a stored secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretMetadata {
    /// Name/identifier of the secret.
    pub name: String,
    /// When the secret was created.
    pub created_at: DateTime<Utc>,
    /// When the secret was last updated.
    pub updated_at: DateTime<Utc>,
    /// When the secret was last accessed.
    pub last_accessed: Option<DateTime<Utc>>,
    /// Number of times the secret has been accessed.
    pub access_count: u64,
    /// Optional description of the secret.
    pub description: Option<String>,
    /// Optional tags for categorization.
    pub tags: Vec<String>,
    /// Whether the secret is currently active.
    pub active: bool,
}

impl SecretMetadata {
    /// Creates new metadata for a secret.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            name: name.into(),
            created_at: now,
            updated_at: now,
            last_accessed: None,
            access_count: 0,
            description: None,
            tags: Vec::new(),
            active: true,
        }
    }

    /// Sets the description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Adds a tag.
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Records an access to the secret.
    pub fn record_access(&mut self) {
        self.last_accessed = Some(Utc::now());
        self.access_count += 1;
    }

    /// Marks the secret as updated.
    pub fn mark_updated(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Deactivates the secret.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.mark_updated();
    }

    /// Activates the secret.
    pub fn activate(&mut self) {
        self.active = true;
        self.mark_updated();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_expose() {
        let data = b"my-secret-key".to_vec();
        let secret = Secret::new(data.clone());
        assert_eq!(secret.expose(), data.as_slice());
    }

    #[test]
    fn test_secret_expose_str() {
        let secret = Secret::from_string("hello".to_string());
        assert_eq!(secret.expose_str(), Some("hello"));

        // Invalid UTF-8
        let secret = Secret::new(vec![0xFF, 0xFE]);
        assert_eq!(secret.expose_str(), None);
    }

    #[test]
    fn test_secret_debug_redacted() {
        let secret = Secret::new(b"sensitive".to_vec());
        let debug = format!("{secret:?}");
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains("sensitive"));
    }

    #[test]
    fn test_secret_display_redacted() {
        let secret = Secret::new(b"sensitive".to_vec());
        let display = format!("{secret}");
        assert_eq!(display, "[REDACTED]");
    }

    #[test]
    fn test_secret_equality() {
        let secret1 = Secret::new(b"test".to_vec());
        let secret2 = Secret::new(b"test".to_vec());
        let secret3 = Secret::new(b"different".to_vec());

        assert_eq!(secret1, secret2);
        assert_ne!(secret1, secret3);
    }

    #[test]
    fn test_masked_secret_long() {
        let secret = Secret::new(b"my-secret-api-key".to_vec());
        let masked = secret.masked();
        assert_eq!(masked.as_str(), "my-***key");
        assert_eq!(masked.original_length(), 17);
    }

    #[test]
    fn test_masked_secret_short() {
        let secret = Secret::new(b"short".to_vec());
        let masked = secret.masked();
        assert_eq!(masked.as_str(), "***");
    }

    #[test]
    fn test_masked_secret_display() {
        let masked = MaskedSecret::from_bytes(b"abcdefghij");
        let display = format!("{masked}");
        assert_eq!(display, "abc***hij");
    }

    #[test]
    fn test_secret_metadata() {
        let mut metadata = SecretMetadata::new("api_key")
            .with_description("Binance API key")
            .with_tag("exchange")
            .with_tag("binance");

        assert_eq!(metadata.name, "api_key");
        assert_eq!(metadata.description, Some("Binance API key".to_string()));
        assert_eq!(metadata.tags, vec!["exchange", "binance"]);
        assert!(metadata.active);
        assert_eq!(metadata.access_count, 0);

        metadata.record_access();
        assert_eq!(metadata.access_count, 1);
        assert!(metadata.last_accessed.is_some());

        metadata.deactivate();
        assert!(!metadata.active);
    }

    #[test]
    fn test_secret_len() {
        let secret = Secret::new(b"12345".to_vec());
        assert_eq!(secret.len(), 5);
        assert!(!secret.is_empty());

        let empty = Secret::new(vec![]);
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());
    }
}
