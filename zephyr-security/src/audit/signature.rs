//! Tamper-evident signatures for audit records.

use crate::error::{Result, SecurityError};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::fmt;

type HmacSha256 = Hmac<Sha256>;

/// A tamper-evident signature for an audit record.
///
/// Uses HMAC-SHA256 to create a signature that can detect tampering
/// with audit records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditSignature {
    /// The signature value (hex-encoded).
    signature: String,
    /// The algorithm used.
    algorithm: String,
}

impl AuditSignature {
    /// Creates a new signature for the given data using a secret key.
    ///
    /// # Arguments
    ///
    /// * `data` - The data to sign
    /// * `secret_key` - The secret key for HMAC
    ///
    /// # Errors
    ///
    /// Returns an error if signature creation fails.
    pub fn create(data: &[u8], secret_key: &[u8]) -> Result<Self> {
        let mut mac = HmacSha256::new_from_slice(secret_key)
            .map_err(|_| SecurityError::encryption("Invalid HMAC key length"))?;

        mac.update(data);
        let result = mac.finalize();
        let signature = hex::encode(result.into_bytes());

        Ok(Self {
            signature,
            algorithm: "HMAC-SHA256".to_string(),
        })
    }

    /// Verifies the signature against the given data.
    ///
    /// # Arguments
    ///
    /// * `data` - The data to verify
    /// * `secret_key` - The secret key for HMAC
    ///
    /// # Errors
    ///
    /// Returns an error if verification fails.
    pub fn verify(&self, data: &[u8], secret_key: &[u8]) -> Result<()> {
        let expected = Self::create(data, secret_key)?;

        if self.signature == expected.signature {
            Ok(())
        } else {
            Err(SecurityError::SignatureVerificationFailed)
        }
    }

    /// Returns the signature value.
    #[must_use]
    pub fn signature(&self) -> &str {
        &self.signature
    }

    /// Returns the algorithm.
    #[must_use]
    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }
}

impl fmt::Display for AuditSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.algorithm, self.signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_signature() {
        let data = b"audit event data";
        let key = b"secret-key";

        let sig = AuditSignature::create(data, key).unwrap();
        assert!(!sig.signature().is_empty());
        assert_eq!(sig.algorithm(), "HMAC-SHA256");
    }

    #[test]
    fn test_verify_signature() {
        let data = b"audit event data";
        let key = b"secret-key";

        let sig = AuditSignature::create(data, key).unwrap();
        assert!(sig.verify(data, key).is_ok());
    }

    #[test]
    fn test_verify_fails_with_wrong_key() {
        let data = b"audit event data";
        let key1 = b"secret-key-1";
        let key2 = b"secret-key-2";

        let sig = AuditSignature::create(data, key1).unwrap();
        assert!(sig.verify(data, key2).is_err());
    }

    #[test]
    fn test_verify_fails_with_tampered_data() {
        let data1 = b"audit event data";
        let data2 = b"tampered data";
        let key = b"secret-key";

        let sig = AuditSignature::create(data1, key).unwrap();
        assert!(sig.verify(data2, key).is_err());
    }

    #[test]
    fn test_signature_deterministic() {
        let data = b"audit event data";
        let key = b"secret-key";

        let sig1 = AuditSignature::create(data, key).unwrap();
        let sig2 = AuditSignature::create(data, key).unwrap();

        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_signature_display() {
        let data = b"audit event data";
        let key = b"secret-key";

        let sig = AuditSignature::create(data, key).unwrap();
        let display = format!("{}", sig);

        assert!(display.contains("HMAC-SHA256"));
        assert!(display.contains(":"));
    }

    #[test]
    fn test_signature_serde() {
        let data = b"audit event data";
        let key = b"secret-key";

        let sig = AuditSignature::create(data, key).unwrap();
        let json = serde_json::to_string(&sig).unwrap();
        let parsed: AuditSignature = serde_json::from_str(&json).unwrap();

        assert_eq!(sig, parsed);
    }
}
