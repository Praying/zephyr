//! Cryptographic primitives for key management.
//!
//! This module provides:
//! - AES-256-GCM encryption/decryption
//! - Argon2id key derivation
//! - Secure random number generation

use crate::error::{Result, SecurityError};
use ring::aead::{self, Aad, BoundKey, Nonce, NonceSequence, OpeningKey, SealingKey, UnboundKey};
use ring::rand::{SecureRandom, SystemRandom};
use zeroize::Zeroize;

/// AES-256-GCM nonce size in bytes.
pub const NONCE_SIZE: usize = 12;

/// AES-256-GCM tag size in bytes.
pub const TAG_SIZE: usize = 16;

/// AES-256 key size in bytes.
pub const KEY_SIZE: usize = 32;

/// Salt size for key derivation.
pub const SALT_SIZE: usize = 32;

/// Argon2id parameters for key derivation.
#[derive(Debug, Clone)]
pub struct Argon2Params {
    /// Memory cost in KiB.
    pub memory_cost: u32,
    /// Time cost (iterations).
    pub time_cost: u32,
    /// Parallelism factor.
    pub parallelism: u32,
}

impl Default for Argon2Params {
    fn default() -> Self {
        Self {
            // 64 MiB memory cost
            memory_cost: 65536,
            // 3 iterations
            time_cost: 3,
            // 4 parallel threads
            parallelism: 4,
        }
    }
}

/// Derives a 256-bit key from a password using Argon2id.
///
/// # Arguments
///
/// * `password` - The password to derive the key from
/// * `salt` - A random salt (should be at least 16 bytes)
/// * `params` - Argon2id parameters
///
/// # Returns
///
/// A 32-byte derived key.
///
/// # Errors
///
/// Returns an error if key derivation fails.
pub fn derive_key(password: &[u8], salt: &[u8], params: &Argon2Params) -> Result<[u8; KEY_SIZE]> {
    use argon2::{Algorithm, Argon2, Params, Version};

    let argon2_params = Params::new(
        params.memory_cost,
        params.time_cost,
        params.parallelism,
        Some(KEY_SIZE),
    )
    .map_err(|e| SecurityError::key_derivation(format!("Invalid Argon2 params: {e}")))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2_params);

    let mut key = [0u8; KEY_SIZE];
    argon2
        .hash_password_into(password, salt, &mut key)
        .map_err(|e| SecurityError::key_derivation(format!("Argon2id failed: {e}")))?;

    Ok(key)
}

/// Generates a random salt for key derivation.
///
/// # Errors
///
/// Returns an error if random number generation fails.
pub fn generate_salt() -> Result<[u8; SALT_SIZE]> {
    let rng = SystemRandom::new();
    let mut salt = [0u8; SALT_SIZE];
    rng.fill(&mut salt)
        .map_err(|_| SecurityError::encryption("Failed to generate random salt"))?;
    Ok(salt)
}

/// Generates a random nonce for AES-GCM.
///
/// # Errors
///
/// Returns an error if random number generation fails.
pub fn generate_nonce() -> Result<[u8; NONCE_SIZE]> {
    let rng = SystemRandom::new();
    let mut nonce = [0u8; NONCE_SIZE];
    rng.fill(&mut nonce)
        .map_err(|_| SecurityError::encryption("Failed to generate random nonce"))?;
    Ok(nonce)
}

/// A nonce sequence that uses a single nonce.
struct SingleNonce {
    nonce: Option<[u8; NONCE_SIZE]>,
}

impl SingleNonce {
    fn new(nonce: [u8; NONCE_SIZE]) -> Self {
        Self { nonce: Some(nonce) }
    }
}

impl NonceSequence for SingleNonce {
    fn advance(&mut self) -> std::result::Result<Nonce, ring::error::Unspecified> {
        self.nonce
            .take()
            .map(Nonce::assume_unique_for_key)
            .ok_or(ring::error::Unspecified)
    }
}

/// AES-256-GCM cipher for encryption and decryption.
///
/// This cipher provides authenticated encryption with associated data (AEAD).
/// The key is automatically zeroed from memory when the cipher is dropped.
pub struct Cipher {
    key: [u8; KEY_SIZE],
}

impl Cipher {
    /// Creates a new cipher with the given key.
    ///
    /// # Arguments
    ///
    /// * `key` - A 32-byte AES-256 key
    #[must_use]
    pub fn new(key: [u8; KEY_SIZE]) -> Self {
        Self { key }
    }

    /// Creates a cipher from a password and salt using Argon2id key derivation.
    ///
    /// # Arguments
    ///
    /// * `password` - The password to derive the key from
    /// * `salt` - A random salt
    /// * `params` - Argon2id parameters
    ///
    /// # Errors
    ///
    /// Returns an error if key derivation fails.
    pub fn from_password(password: &[u8], salt: &[u8], params: &Argon2Params) -> Result<Self> {
        let key = derive_key(password, salt, params)?;
        Ok(Self::new(key))
    }

    /// Encrypts plaintext using AES-256-GCM.
    ///
    /// # Arguments
    ///
    /// * `plaintext` - The data to encrypt
    /// * `aad` - Additional authenticated data (not encrypted, but authenticated)
    ///
    /// # Returns
    ///
    /// A vector containing: nonce (12 bytes) || ciphertext || tag (16 bytes)
    ///
    /// # Errors
    ///
    /// Returns an error if encryption fails.
    pub fn encrypt(&self, plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
        let nonce = generate_nonce()?;
        self.encrypt_with_nonce(plaintext, aad, &nonce)
    }

    /// Encrypts plaintext using AES-256-GCM with a specific nonce.
    ///
    /// # Arguments
    ///
    /// * `plaintext` - The data to encrypt
    /// * `aad` - Additional authenticated data
    /// * `nonce` - The nonce to use (must be unique for each encryption)
    ///
    /// # Returns
    ///
    /// A vector containing: nonce (12 bytes) || ciphertext || tag (16 bytes)
    ///
    /// # Errors
    ///
    /// Returns an error if encryption fails.
    pub fn encrypt_with_nonce(
        &self,
        plaintext: &[u8],
        aad: &[u8],
        nonce: &[u8; NONCE_SIZE],
    ) -> Result<Vec<u8>> {
        let unbound_key = UnboundKey::new(&aead::AES_256_GCM, &self.key)
            .map_err(|_| SecurityError::encryption("Failed to create encryption key"))?;

        let nonce_seq = SingleNonce::new(*nonce);
        let mut sealing_key = SealingKey::new(unbound_key, nonce_seq);

        // Prepare output buffer: nonce || plaintext || space for tag
        let mut output = Vec::with_capacity(NONCE_SIZE + plaintext.len() + TAG_SIZE);
        output.extend_from_slice(nonce);
        output.extend_from_slice(plaintext);

        // Encrypt in place (after the nonce)
        // We need to work with a Vec to satisfy the Extend trait
        let mut ciphertext_and_tag = output.split_off(NONCE_SIZE);
        sealing_key
            .seal_in_place_append_tag(Aad::from(aad), &mut ciphertext_and_tag)
            .map_err(|_| SecurityError::encryption("Encryption failed"))?;

        output.append(&mut ciphertext_and_tag);
        Ok(output)
    }

    /// Decrypts ciphertext using AES-256-GCM.
    ///
    /// # Arguments
    ///
    /// * `ciphertext` - The encrypted data (nonce || ciphertext || tag)
    /// * `aad` - Additional authenticated data (must match what was used during encryption)
    ///
    /// # Returns
    ///
    /// The decrypted plaintext.
    ///
    /// # Errors
    ///
    /// Returns an error if decryption fails or authentication fails.
    pub fn decrypt(&self, ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
        if ciphertext.len() < NONCE_SIZE + TAG_SIZE {
            return Err(SecurityError::encryption("Ciphertext too short"));
        }

        let nonce: [u8; NONCE_SIZE] = ciphertext[..NONCE_SIZE]
            .try_into()
            .map_err(|_| SecurityError::encryption("Invalid nonce"))?;

        let unbound_key = UnboundKey::new(&aead::AES_256_GCM, &self.key)
            .map_err(|_| SecurityError::encryption("Failed to create decryption key"))?;

        let nonce_seq = SingleNonce::new(nonce);
        let mut opening_key = OpeningKey::new(unbound_key, nonce_seq);

        // Copy ciphertext (without nonce) for in-place decryption
        let mut buffer = ciphertext[NONCE_SIZE..].to_vec();

        let plaintext = opening_key
            .open_in_place(Aad::from(aad), &mut buffer)
            .map_err(|_| SecurityError::encryption("Decryption failed - authentication error"))?;

        Ok(plaintext.to_vec())
    }
}

impl Drop for Cipher {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_derivation() {
        let password = b"test-password";
        let salt = generate_salt().unwrap();
        let params = Argon2Params::default();

        let key1 = derive_key(password, &salt, &params).unwrap();
        let key2 = derive_key(password, &salt, &params).unwrap();

        // Same password and salt should produce same key
        assert_eq!(key1, key2);

        // Different salt should produce different key
        let salt2 = generate_salt().unwrap();
        let key3 = derive_key(password, &salt2, &params).unwrap();
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = [0x42u8; KEY_SIZE];
        let cipher = Cipher::new(key);

        let plaintext = b"Hello, World!";
        let aad = b"additional data";

        let ciphertext = cipher.encrypt(plaintext, aad).unwrap();
        let decrypted = cipher.decrypt(&ciphertext, aad).unwrap();

        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_encrypt_decrypt_empty() {
        let key = [0x42u8; KEY_SIZE];
        let cipher = Cipher::new(key);

        let plaintext = b"";
        let aad = b"";

        let ciphertext = cipher.encrypt(plaintext, aad).unwrap();
        let decrypted = cipher.decrypt(&ciphertext, aad).unwrap();

        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_decrypt_wrong_aad_fails() {
        let key = [0x42u8; KEY_SIZE];
        let cipher = Cipher::new(key);

        let plaintext = b"Hello, World!";
        let aad = b"correct aad";

        let ciphertext = cipher.encrypt(plaintext, aad).unwrap();
        let result = cipher.decrypt(&ciphertext, b"wrong aad");

        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_tampered_ciphertext_fails() {
        let key = [0x42u8; KEY_SIZE];
        let cipher = Cipher::new(key);

        let plaintext = b"Hello, World!";
        let aad = b"aad";

        let mut ciphertext = cipher.encrypt(plaintext, aad).unwrap();
        // Tamper with the ciphertext
        if let Some(byte) = ciphertext.get_mut(NONCE_SIZE + 5) {
            *byte ^= 0xFF;
        }

        let result = cipher.decrypt(&ciphertext, aad);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_too_short_fails() {
        let key = [0x42u8; KEY_SIZE];
        let cipher = Cipher::new(key);

        let short_ciphertext = vec![0u8; NONCE_SIZE + TAG_SIZE - 1];
        let result = cipher.decrypt(&short_ciphertext, b"");

        assert!(result.is_err());
    }

    #[test]
    fn test_cipher_from_password() {
        let password = b"my-secure-password";
        let salt = generate_salt().unwrap();
        let params = Argon2Params::default();

        let cipher = Cipher::from_password(password, &salt, &params).unwrap();

        let plaintext = b"secret data";
        let ciphertext = cipher.encrypt(plaintext, b"").unwrap();

        // Create another cipher with same password and salt
        let cipher2 = Cipher::from_password(password, &salt, &params).unwrap();
        let decrypted = cipher2.decrypt(&ciphertext, b"").unwrap();

        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_generate_salt_uniqueness() {
        let salt1 = generate_salt().unwrap();
        let salt2 = generate_salt().unwrap();
        assert_ne!(salt1, salt2);
    }

    #[test]
    fn test_generate_nonce_uniqueness() {
        let nonce1 = generate_nonce().unwrap();
        let nonce2 = generate_nonce().unwrap();
        assert_ne!(nonce1, nonce2);
    }
}
