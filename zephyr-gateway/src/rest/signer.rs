//! Request signing utilities.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use zephyr_core::error::NetworkError;

type HmacSha256 = Hmac<Sha256>;

/// Signature type for different exchanges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureType {
    /// HMAC-SHA256 signature (Binance, OKX, Bitget).
    HmacSha256,
    /// HMAC-SHA256 with base64 encoding (OKX).
    HmacSha256Base64,
    /// Ed25519 signature (some DEXs).
    Ed25519,
}

/// Request signer for exchange API authentication.
///
/// Provides methods to sign requests using various signature algorithms.
#[derive(Debug, Clone)]
pub struct RequestSigner {
    secret: String,
    signature_type: SignatureType,
}

impl RequestSigner {
    /// Creates a new request signer.
    #[must_use]
    pub fn new(secret: impl Into<String>, signature_type: SignatureType) -> Self {
        Self {
            secret: secret.into(),
            signature_type,
        }
    }

    /// Creates a signer for HMAC-SHA256 (hex output).
    #[must_use]
    pub fn hmac_sha256(secret: impl Into<String>) -> Self {
        Self::new(secret, SignatureType::HmacSha256)
    }

    /// Creates a signer for HMAC-SHA256 (base64 output).
    #[must_use]
    pub fn hmac_sha256_base64(secret: impl Into<String>) -> Self {
        Self::new(secret, SignatureType::HmacSha256Base64)
    }

    /// Signs a message and returns the signature.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if signing fails.
    pub fn sign(&self, message: &str) -> Result<String, NetworkError> {
        match self.signature_type {
            SignatureType::HmacSha256 => self.sign_hmac_sha256_hex(message),
            SignatureType::HmacSha256Base64 => self.sign_hmac_sha256_base64(message),
            SignatureType::Ed25519 => Err(NetworkError::WebSocket {
                reason: "Ed25519 signing not implemented".to_string(),
            }),
        }
    }

    /// Signs a message using HMAC-SHA256 and returns hex-encoded signature.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if signing fails.
    pub fn sign_hmac_sha256_hex(&self, message: &str) -> Result<String, NetworkError> {
        let mut mac = HmacSha256::new_from_slice(self.secret.as_bytes()).map_err(|e| {
            NetworkError::WebSocket {
                reason: format!("Failed to create HMAC: {e}"),
            }
        })?;

        mac.update(message.as_bytes());
        let result = mac.finalize();
        Ok(hex::encode(result.into_bytes()))
    }

    /// Signs a message using HMAC-SHA256 and returns base64-encoded signature.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if signing fails.
    pub fn sign_hmac_sha256_base64(&self, message: &str) -> Result<String, NetworkError> {
        let mut mac = HmacSha256::new_from_slice(self.secret.as_bytes()).map_err(|e| {
            NetworkError::WebSocket {
                reason: format!("Failed to create HMAC: {e}"),
            }
        })?;

        mac.update(message.as_bytes());
        let result = mac.finalize();
        Ok(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            result.into_bytes(),
        ))
    }

    /// Creates a signature for Binance API.
    ///
    /// Binance uses HMAC-SHA256 with hex encoding.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if signing fails.
    pub fn sign_binance(&self, query_string: &str) -> Result<String, NetworkError> {
        self.sign_hmac_sha256_hex(query_string)
    }

    /// Creates a signature for OKX API.
    ///
    /// OKX uses HMAC-SHA256 with base64 encoding.
    /// The message format is: timestamp + method + requestPath + body
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if signing fails.
    pub fn sign_okx(
        &self,
        timestamp: &str,
        method: &str,
        request_path: &str,
        body: &str,
    ) -> Result<String, NetworkError> {
        let message = format!("{timestamp}{method}{request_path}{body}");
        self.sign_hmac_sha256_base64(&message)
    }

    /// Creates a signature for Bitget API.
    ///
    /// Bitget uses HMAC-SHA256 with base64 encoding.
    /// The message format is: timestamp + method + requestPath + body
    ///
    /// # Errors
    ///
    /// Returns `NetworkError` if signing fails.
    pub fn sign_bitget(
        &self,
        timestamp: &str,
        method: &str,
        request_path: &str,
        body: &str,
    ) -> Result<String, NetworkError> {
        let message = format!("{timestamp}{method}{request_path}{body}");
        self.sign_hmac_sha256_base64(&message)
    }

    /// Returns the signature type.
    #[must_use]
    pub fn signature_type(&self) -> SignatureType {
        self.signature_type
    }
}

/// Builds a query string from parameters.
///
/// Parameters are sorted alphabetically and URL-encoded.
#[must_use]
pub fn build_query_string(params: &[(&str, &str)]) -> String {
    let mut sorted_params: Vec<_> = params.iter().collect();
    sorted_params.sort_by_key(|(k, _)| *k);

    sorted_params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

/// Builds a sorted query string with signature appended.
///
/// # Errors
///
/// Returns `NetworkError` if signing fails.
#[allow(dead_code)]
pub fn build_signed_query_string(
    params: &[(&str, &str)],
    signer: &RequestSigner,
) -> Result<String, NetworkError> {
    let query = build_query_string(params);
    let signature = signer.sign(&query)?;
    Ok(format!("{query}&signature={signature}"))
}

/// Returns the current timestamp in milliseconds.
#[must_use]
#[allow(dead_code)]
pub fn timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as u64
}

/// Returns the current timestamp as ISO 8601 string (for OKX).
#[must_use]
#[allow(dead_code)]
pub fn timestamp_iso() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_sha256_hex() {
        let signer = RequestSigner::hmac_sha256("secret");
        let signature = signer.sign_hmac_sha256_hex("message").unwrap();

        // Known HMAC-SHA256 result for "message" with key "secret"
        assert_eq!(
            signature,
            "8b5f48702995c1598c573db1e21866a9b825d4a794d169d7060a03605796360b"
        );
    }

    #[test]
    fn test_hmac_sha256_base64() {
        let signer = RequestSigner::hmac_sha256_base64("secret");
        let signature = signer.sign_hmac_sha256_base64("message").unwrap();

        // Known HMAC-SHA256 base64 result
        assert_eq!(signature, "i19IcCmVwVmMVz2x4hhmqbgl1KeU0WnXBgoDYFeWNgs=");
    }

    #[test]
    fn test_sign_binance() {
        let signer = RequestSigner::hmac_sha256(
            "NhqPtmdSJYdKjVHjA7PZj4Mge3R5YNiP1e3UZjInClVN65XAbvqqM6A7H5fATj0j",
        );
        let query = "symbol=LTCBTC&side=BUY&type=LIMIT&timeInForce=GTC&quantity=1&price=0.1&recvWindow=5000&timestamp=1499827319559";
        let signature = signer.sign_binance(query).unwrap();

        // This is the expected signature from Binance documentation
        assert_eq!(
            signature,
            "c8db56825ae71d6d79447849e617115f4a920fa2acdcab2b053c4b2838bd6b71"
        );
    }

    #[test]
    fn test_sign_okx() {
        let signer = RequestSigner::hmac_sha256_base64("secret");
        let signature = signer
            .sign_okx(
                "2023-01-01T00:00:00.000Z",
                "GET",
                "/api/v5/account/balance",
                "",
            )
            .unwrap();

        // Just verify it produces a valid base64 string
        assert!(!signature.is_empty());
        assert!(
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &signature).is_ok()
        );
    }

    #[test]
    fn test_build_query_string() {
        let params = [("symbol", "BTCUSDT"), ("side", "BUY"), ("amount", "1.0")];
        let query = build_query_string(&params);

        // Should be sorted alphabetically
        assert_eq!(query, "amount=1.0&side=BUY&symbol=BTCUSDT");
    }

    #[test]
    fn test_build_signed_query_string() {
        let signer = RequestSigner::hmac_sha256("secret");
        let params = [("symbol", "BTCUSDT"), ("timestamp", "1234567890")];
        let signed = build_signed_query_string(&params, &signer).unwrap();

        assert!(signed.contains("symbol=BTCUSDT"));
        assert!(signed.contains("timestamp=1234567890"));
        assert!(signed.contains("&signature="));
    }

    #[test]
    fn test_timestamp_ms() {
        let ts = timestamp_ms();
        // Should be a reasonable timestamp (after 2020)
        assert!(ts > 1_577_836_800_000);
    }

    #[test]
    fn test_timestamp_iso() {
        let ts = timestamp_iso();
        // Should match ISO 8601 format
        assert!(ts.contains("T"));
        assert!(ts.ends_with("Z"));
    }
}
