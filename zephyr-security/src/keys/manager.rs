//! Key manager for secure secret storage and retrieval.

// Allow clippy warnings that are acceptable in this module
#![allow(clippy::disallowed_types)]
#![allow(clippy::future_not_send)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::await_holding_lock)]
#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::explicit_iter_loop)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::unused_async)]

use super::config::KeyManagerConfig;
use super::crypto::{Argon2Params, Cipher, SALT_SIZE, generate_salt};
use super::secret::{Secret, SecretMetadata};
use crate::error::{Result, SecurityError};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tracing::{debug, info, warn};

/// Encrypted secrets store format.
#[derive(Serialize, Deserialize)]
struct SecretsStore {
    /// Version of the store format.
    version: u32,
    /// Salt used for key derivation.
    #[serde(with = "hex_serde")]
    salt: Vec<u8>,
    /// Encrypted secrets data.
    #[serde(with = "hex_serde")]
    encrypted_data: Vec<u8>,
}

/// Inner secrets data (decrypted).
#[derive(Serialize, Deserialize, Default)]
struct SecretsData {
    secrets: HashMap<String, StoredSecret>,
}

/// A stored secret with metadata.
#[derive(Serialize, Deserialize)]
struct StoredSecret {
    /// The encrypted secret value (base64 encoded).
    #[serde(with = "base64_serde")]
    value: Vec<u8>,
    /// Metadata about the secret.
    metadata: SecretMetadata,
}

mod hex_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        hex::decode(&s).map_err(serde::de::Error::custom)
    }
}

mod base64_serde {
    use base64::prelude::*;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&BASE64_STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        BASE64_STANDARD.decode(&s).map_err(serde::de::Error::custom)
    }
}

/// Key manager for secure secret storage and retrieval.
///
/// The key manager provides:
/// - AES-256-GCM encryption for secrets at rest
/// - Argon2id key derivation from master password
/// - In-memory caching with automatic zeroing
/// - Thread-safe concurrent access
///
/// # Example
///
/// ```no_run
/// use zephyr_security::keys::{KeyManager, KeyManagerConfig};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = KeyManagerConfig::default();
/// let manager = KeyManager::new(config)?;
///
/// // Initialize with master password
/// manager.initialize(b"master-password").await?;
///
/// // Store a secret
/// manager.store_secret("api_key", b"secret-value").await?;
///
/// // Retrieve a secret
/// let secret = manager.get_secret("api_key").await?;
/// # Ok(())
/// # }
/// ```
pub struct KeyManager {
    config: KeyManagerConfig,
    cipher: RwLock<Option<Cipher>>,
    salt: RwLock<Option<[u8; SALT_SIZE]>>,
    cache: DashMap<String, Arc<Secret>>,
    metadata_cache: DashMap<String, SecretMetadata>,
}

impl KeyManager {
    /// Creates a new key manager with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    pub fn new(config: KeyManagerConfig) -> Result<Self> {
        Ok(Self {
            config,
            cipher: RwLock::new(None),
            salt: RwLock::new(None),
            cache: DashMap::new(),
            metadata_cache: DashMap::new(),
        })
    }

    /// Initializes the key manager with a master password.
    ///
    /// If a secrets file exists, it will be loaded and decrypted.
    /// Otherwise, a new secrets file will be created.
    ///
    /// # Arguments
    ///
    /// * `master_password` - The master password for encryption/decryption
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
    pub async fn initialize(&self, master_password: &[u8]) -> Result<()> {
        let path = &self.config.secrets_path;

        if path.exists() {
            self.load_from_file(master_password).await?;
            info!("Loaded secrets from {}", path.display());
        } else {
            self.create_new_store(master_password).await?;
            info!("Created new secrets store at {}", path.display());
        }

        Ok(())
    }

    /// Creates a new secrets store with the given master password.
    async fn create_new_store(&self, master_password: &[u8]) -> Result<()> {
        let salt = generate_salt()?;
        let params: Argon2Params = (&self.config.argon2_params).into();
        let cipher = Cipher::from_password(master_password, &salt, &params)?;

        *self.salt.write() = Some(salt);
        *self.cipher.write() = Some(cipher);

        // Save empty store
        self.save_to_file().await?;

        Ok(())
    }

    /// Loads secrets from the encrypted file.
    async fn load_from_file(&self, master_password: &[u8]) -> Result<()> {
        let path = &self.config.secrets_path;

        // Verify file permissions on Unix
        #[cfg(unix)]
        if self.config.verify_permissions {
            self.verify_file_permissions(path).await?;
        }

        let content = fs::read_to_string(path).await.map_err(|e| {
            SecurityError::storage_error(format!("Failed to read secrets file: {e}"))
        })?;

        let store: SecretsStore = serde_json::from_str(&content).map_err(|e| {
            SecurityError::invalid_secret_format(format!("Invalid secrets file format: {e}"))
        })?;

        if store.version != 1 {
            return Err(SecurityError::invalid_secret_format(format!(
                "Unsupported secrets file version: {}",
                store.version
            )));
        }

        let salt: [u8; SALT_SIZE] = store
            .salt
            .try_into()
            .map_err(|_| SecurityError::invalid_secret_format("Invalid salt length"))?;

        let params: Argon2Params = (&self.config.argon2_params).into();
        let cipher = Cipher::from_password(master_password, &salt, &params)?;

        // Decrypt the secrets data
        let decrypted = cipher.decrypt(&store.encrypted_data, b"zephyr-secrets")?;
        let data: SecretsData = serde_json::from_slice(&decrypted)
            .map_err(|e| SecurityError::encryption(format!("Failed to decrypt secrets: {e}")))?;

        // Populate caches
        for (name, stored) in data.secrets {
            let secret = Secret::new(stored.value);
            self.cache.insert(name.clone(), Arc::new(secret));
            self.metadata_cache.insert(name, stored.metadata);
        }

        *self.salt.write() = Some(salt);
        *self.cipher.write() = Some(cipher);

        Ok(())
    }

    /// Saves secrets to the encrypted file.
    async fn save_to_file(&self) -> Result<()> {
        let cipher_guard = self.cipher.read();
        let cipher = cipher_guard
            .as_ref()
            .ok_or_else(|| SecurityError::encryption("Key manager not initialized"))?;

        let salt_guard = self.salt.read();
        let salt = salt_guard
            .as_ref()
            .ok_or_else(|| SecurityError::encryption("Key manager not initialized"))?;

        // Build secrets data
        let mut data = SecretsData::default();
        for entry in self.cache.iter() {
            let name = entry.key().clone();
            let secret = entry.value();
            let metadata = self
                .metadata_cache
                .get(&name)
                .map(|m| m.clone())
                .unwrap_or_else(|| SecretMetadata::new(&name));

            data.secrets.insert(
                name,
                StoredSecret {
                    value: secret.expose().to_vec(),
                    metadata,
                },
            );
        }

        let json = serde_json::to_vec(&data).map_err(|e| {
            SecurityError::storage_error(format!("Failed to serialize secrets: {e}"))
        })?;

        let encrypted = cipher.encrypt(&json, b"zephyr-secrets")?;

        let store = SecretsStore {
            version: 1,
            salt: salt.to_vec(),
            encrypted_data: encrypted,
        };

        let content = serde_json::to_string_pretty(&store)
            .map_err(|e| SecurityError::storage_error(format!("Failed to serialize store: {e}")))?;

        // Write atomically using a temp file
        let path = &self.config.secrets_path;
        let temp_path = path.with_extension("tmp");

        fs::write(&temp_path, &content)
            .await
            .map_err(|e| SecurityError::storage_error(format!("Failed to write temp file: {e}")))?;

        fs::rename(&temp_path, path).await.map_err(|e| {
            SecurityError::storage_error(format!("Failed to rename temp file: {e}"))
        })?;

        // Set restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(path, permissions).map_err(|e| {
                SecurityError::storage_error(format!("Failed to set permissions: {e}"))
            })?;
        }

        debug!("Saved secrets to {}", path.display());
        Ok(())
    }

    /// Verifies that the file has restrictive permissions (Unix only).
    #[cfg(unix)]
    async fn verify_file_permissions(&self, path: &Path) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let metadata = fs::metadata(path).await.map_err(|e| {
            SecurityError::storage_error(format!("Failed to read file metadata: {e}"))
        })?;

        let mode = metadata.permissions().mode();
        let other_perms = mode & 0o077;

        if other_perms != 0 {
            warn!(
                "Secrets file {} has insecure permissions: {:o}",
                path.display(),
                mode & 0o777
            );
            return Err(SecurityError::configuration_error(format!(
                "Secrets file has insecure permissions: {:o}. Expected 600.",
                mode & 0o777
            )));
        }

        Ok(())
    }

    /// Stores a secret.
    ///
    /// # Arguments
    ///
    /// * `name` - The name/identifier for the secret
    /// * `value` - The secret value
    ///
    /// # Errors
    ///
    /// Returns an error if the key manager is not initialized or storage fails.
    pub async fn store_secret(&self, name: &str, value: &[u8]) -> Result<()> {
        if self.cipher.read().is_none() {
            return Err(SecurityError::encryption("Key manager not initialized"));
        }

        let secret = Secret::new(value.to_vec());
        let metadata = SecretMetadata::new(name);

        self.cache.insert(name.to_string(), Arc::new(secret));
        self.metadata_cache.insert(name.to_string(), metadata);

        self.save_to_file().await?;

        info!("Stored secret: {}", name);
        Ok(())
    }

    /// Stores a secret with metadata.
    ///
    /// # Arguments
    ///
    /// * `name` - The name/identifier for the secret
    /// * `value` - The secret value
    /// * `metadata` - Metadata for the secret
    ///
    /// # Errors
    ///
    /// Returns an error if the key manager is not initialized or storage fails.
    pub async fn store_secret_with_metadata(
        &self,
        name: &str,
        value: &[u8],
        metadata: SecretMetadata,
    ) -> Result<()> {
        if self.cipher.read().is_none() {
            return Err(SecurityError::encryption("Key manager not initialized"));
        }

        let secret = Secret::new(value.to_vec());

        self.cache.insert(name.to_string(), Arc::new(secret));
        self.metadata_cache.insert(name.to_string(), metadata);

        self.save_to_file().await?;

        info!("Stored secret with metadata: {}", name);
        Ok(())
    }

    /// Retrieves a secret.
    ///
    /// # Arguments
    ///
    /// * `name` - The name/identifier of the secret
    ///
    /// # Returns
    ///
    /// The secret value wrapped in an Arc for safe sharing.
    ///
    /// # Errors
    ///
    /// Returns an error if the secret is not found.
    pub async fn get_secret(&self, name: &str) -> Result<Arc<Secret>> {
        let secret = self
            .cache
            .get(name)
            .map(|entry| Arc::clone(entry.value()))
            .ok_or_else(|| SecurityError::secret_not_found(name))?;

        // Update access metadata
        if let Some(mut metadata) = self.metadata_cache.get_mut(name) {
            metadata.record_access();
        }

        debug!("Retrieved secret: {}", name);
        Ok(secret)
    }

    /// Retrieves secret metadata.
    ///
    /// # Arguments
    ///
    /// * `name` - The name/identifier of the secret
    ///
    /// # Returns
    ///
    /// The secret metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the secret is not found.
    pub fn get_metadata(&self, name: &str) -> Result<SecretMetadata> {
        self.metadata_cache
            .get(name)
            .map(|entry| entry.clone())
            .ok_or_else(|| SecurityError::secret_not_found(name))
    }

    /// Deletes a secret.
    ///
    /// # Arguments
    ///
    /// * `name` - The name/identifier of the secret
    ///
    /// # Errors
    ///
    /// Returns an error if the secret is not found or deletion fails.
    pub async fn delete_secret(&self, name: &str) -> Result<()> {
        if self.cache.remove(name).is_none() {
            return Err(SecurityError::secret_not_found(name));
        }
        self.metadata_cache.remove(name);

        self.save_to_file().await?;

        info!("Deleted secret: {}", name);
        Ok(())
    }

    /// Lists all secret names.
    #[must_use]
    pub fn list_secrets(&self) -> Vec<String> {
        self.cache.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Checks if a secret exists.
    #[must_use]
    pub fn has_secret(&self, name: &str) -> bool {
        self.cache.contains_key(name)
    }

    /// Returns the number of stored secrets.
    #[must_use]
    pub fn secret_count(&self) -> usize {
        self.cache.len()
    }

    /// Changes the master password.
    ///
    /// This re-encrypts all secrets with a new key derived from the new password.
    ///
    /// # Arguments
    ///
    /// * `new_password` - The new master password
    ///
    /// # Errors
    ///
    /// Returns an error if re-encryption fails.
    pub async fn change_password(&self, new_password: &[u8]) -> Result<()> {
        let new_salt = generate_salt()?;
        let params: Argon2Params = (&self.config.argon2_params).into();
        let new_cipher = Cipher::from_password(new_password, &new_salt, &params)?;

        // Replace cipher and salt
        {
            let mut cipher_guard = self.cipher.write();
            let mut salt_guard = self.salt.write();

            // Zeroize old cipher by dropping it
            if let Some(old_cipher) = cipher_guard.take() {
                // The cipher will be zeroized on drop
                drop(old_cipher);
            }

            *cipher_guard = Some(new_cipher);
            *salt_guard = Some(new_salt);
        }

        // Re-save with new encryption
        self.save_to_file().await?;

        info!("Changed master password");
        Ok(())
    }

    /// Clears all cached secrets from memory.
    ///
    /// This does not delete secrets from storage.
    pub fn clear_cache(&self) {
        self.cache.clear();
        self.metadata_cache.clear();
        debug!("Cleared secret cache");
    }
}

impl Drop for KeyManager {
    fn drop(&mut self) {
        // Clear all cached secrets
        self.cache.clear();
        self.metadata_cache.clear();

        // The cipher will be zeroized by its own Drop implementation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    async fn create_test_manager() -> (KeyManager, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        let secrets_path = dir.path().join("secrets.enc");

        let config = KeyManagerConfig::new(&secrets_path).with_verify_permissions(false);

        let manager = KeyManager::new(config).unwrap();
        manager.initialize(b"test-password").await.unwrap();

        (manager, dir)
    }

    #[tokio::test]
    async fn test_store_and_retrieve_secret() {
        let (manager, _dir) = create_test_manager().await;

        manager
            .store_secret("api_key", b"secret-value")
            .await
            .unwrap();

        let secret = manager.get_secret("api_key").await.unwrap();
        assert_eq!(secret.expose(), b"secret-value");
    }

    #[tokio::test]
    async fn test_secret_not_found() {
        let (manager, _dir) = create_test_manager().await;

        let result = manager.get_secret("nonexistent").await;
        assert!(matches!(result, Err(SecurityError::SecretNotFound { .. })));
    }

    #[tokio::test]
    async fn test_delete_secret() {
        let (manager, _dir) = create_test_manager().await;

        manager.store_secret("to_delete", b"value").await.unwrap();
        assert!(manager.has_secret("to_delete"));

        manager.delete_secret("to_delete").await.unwrap();
        assert!(!manager.has_secret("to_delete"));
    }

    #[tokio::test]
    async fn test_list_secrets() {
        let (manager, _dir) = create_test_manager().await;

        manager.store_secret("key1", b"value1").await.unwrap();
        manager.store_secret("key2", b"value2").await.unwrap();

        let secrets = manager.list_secrets();
        assert_eq!(secrets.len(), 2);
        assert!(secrets.contains(&"key1".to_string()));
        assert!(secrets.contains(&"key2".to_string()));
    }

    #[tokio::test]
    async fn test_persistence() {
        let dir = tempdir().unwrap();
        let secrets_path = dir.path().join("secrets.enc");

        // Create and store
        {
            let config = KeyManagerConfig::new(&secrets_path).with_verify_permissions(false);
            let manager = KeyManager::new(config).unwrap();
            manager.initialize(b"test-password").await.unwrap();
            manager.store_secret("persistent", b"value").await.unwrap();
        }

        // Load and verify
        {
            let config = KeyManagerConfig::new(&secrets_path).with_verify_permissions(false);
            let manager = KeyManager::new(config).unwrap();
            manager.initialize(b"test-password").await.unwrap();

            let secret = manager.get_secret("persistent").await.unwrap();
            assert_eq!(secret.expose(), b"value");
        }
    }

    #[tokio::test]
    async fn test_wrong_password_fails() {
        let dir = tempdir().unwrap();
        let secrets_path = dir.path().join("secrets.enc");

        // Create with one password
        {
            let config = KeyManagerConfig::new(&secrets_path).with_verify_permissions(false);
            let manager = KeyManager::new(config).unwrap();
            manager.initialize(b"correct-password").await.unwrap();
            manager.store_secret("key", b"value").await.unwrap();
        }

        // Try to load with wrong password
        {
            let config = KeyManagerConfig::new(&secrets_path).with_verify_permissions(false);
            let manager = KeyManager::new(config).unwrap();
            let result = manager.initialize(b"wrong-password").await;
            assert!(result.is_err());
        }
    }

    #[tokio::test]
    async fn test_change_password() {
        let (manager, _dir) = create_test_manager().await;

        manager.store_secret("key", b"value").await.unwrap();
        manager.change_password(b"new-password").await.unwrap();

        // Should still be able to access secrets
        let secret = manager.get_secret("key").await.unwrap();
        assert_eq!(secret.expose(), b"value");
    }

    #[tokio::test]
    async fn test_metadata() {
        let (manager, _dir) = create_test_manager().await;

        let metadata = SecretMetadata::new("api_key")
            .with_description("Test API key")
            .with_tag("test");

        manager
            .store_secret_with_metadata("api_key", b"value", metadata)
            .await
            .unwrap();

        let retrieved = manager.get_metadata("api_key").unwrap();
        assert_eq!(retrieved.description, Some("Test API key".to_string()));
        assert!(retrieved.tags.contains(&"test".to_string()));
    }

    #[tokio::test]
    async fn test_access_count() {
        let (manager, _dir) = create_test_manager().await;

        manager.store_secret("key", b"value").await.unwrap();

        // Access multiple times
        let _ = manager.get_secret("key").await.unwrap();
        let _ = manager.get_secret("key").await.unwrap();
        let _ = manager.get_secret("key").await.unwrap();

        let metadata = manager.get_metadata("key").unwrap();
        assert_eq!(metadata.access_count, 3);
    }
}
