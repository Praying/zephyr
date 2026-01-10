//! Configuration for the key manager.

use super::crypto::Argon2Params;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the key manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyManagerConfig {
    /// Path to the encrypted secrets file.
    pub secrets_path: PathBuf,
    /// Argon2id parameters for key derivation.
    #[serde(default)]
    pub argon2_params: Argon2ParamsConfig,
    /// Whether to verify file permissions on Unix systems.
    #[serde(default = "default_verify_permissions")]
    pub verify_permissions: bool,
    /// Maximum number of secrets to cache in memory.
    #[serde(default = "default_max_cache_size")]
    pub max_cache_size: usize,
}

fn default_verify_permissions() -> bool {
    true
}

fn default_max_cache_size() -> usize {
    100
}

impl Default for KeyManagerConfig {
    fn default() -> Self {
        Self {
            secrets_path: PathBuf::from("secrets.enc"),
            argon2_params: Argon2ParamsConfig::default(),
            verify_permissions: true,
            max_cache_size: 100,
        }
    }
}

impl KeyManagerConfig {
    /// Creates a new configuration with the given secrets path.
    #[must_use]
    pub fn new(secrets_path: impl Into<PathBuf>) -> Self {
        Self {
            secrets_path: secrets_path.into(),
            ..Default::default()
        }
    }

    /// Sets the Argon2 parameters.
    #[must_use]
    pub fn with_argon2_params(mut self, params: Argon2ParamsConfig) -> Self {
        self.argon2_params = params;
        self
    }

    /// Sets whether to verify file permissions.
    #[must_use]
    pub const fn with_verify_permissions(mut self, verify: bool) -> Self {
        self.verify_permissions = verify;
        self
    }

    /// Sets the maximum cache size.
    #[must_use]
    pub const fn with_max_cache_size(mut self, size: usize) -> Self {
        self.max_cache_size = size;
        self
    }
}

/// Serializable Argon2 parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Argon2ParamsConfig {
    /// Memory cost in KiB.
    #[serde(default = "default_memory_cost")]
    pub memory_cost: u32,
    /// Time cost (iterations).
    #[serde(default = "default_time_cost")]
    pub time_cost: u32,
    /// Parallelism factor.
    #[serde(default = "default_parallelism")]
    pub parallelism: u32,
}

fn default_memory_cost() -> u32 {
    65536 // 64 MiB
}

fn default_time_cost() -> u32 {
    3
}

fn default_parallelism() -> u32 {
    4
}

impl Default for Argon2ParamsConfig {
    fn default() -> Self {
        Self {
            memory_cost: default_memory_cost(),
            time_cost: default_time_cost(),
            parallelism: default_parallelism(),
        }
    }
}

impl From<Argon2ParamsConfig> for Argon2Params {
    fn from(config: Argon2ParamsConfig) -> Self {
        Self {
            memory_cost: config.memory_cost,
            time_cost: config.time_cost,
            parallelism: config.parallelism,
        }
    }
}

impl From<&Argon2ParamsConfig> for Argon2Params {
    fn from(config: &Argon2ParamsConfig) -> Self {
        Self {
            memory_cost: config.memory_cost,
            time_cost: config.time_cost,
            parallelism: config.parallelism,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = KeyManagerConfig::default();
        assert_eq!(config.secrets_path, PathBuf::from("secrets.enc"));
        assert!(config.verify_permissions);
        assert_eq!(config.max_cache_size, 100);
    }

    #[test]
    fn test_config_builder() {
        let config = KeyManagerConfig::new("/path/to/secrets")
            .with_verify_permissions(false)
            .with_max_cache_size(50);

        assert_eq!(config.secrets_path, PathBuf::from("/path/to/secrets"));
        assert!(!config.verify_permissions);
        assert_eq!(config.max_cache_size, 50);
    }

    #[test]
    fn test_argon2_params_conversion() {
        let config = Argon2ParamsConfig {
            memory_cost: 32768,
            time_cost: 2,
            parallelism: 2,
        };

        let params: Argon2Params = config.into();
        assert_eq!(params.memory_cost, 32768);
        assert_eq!(params.time_cost, 2);
        assert_eq!(params.parallelism, 2);
    }

    #[test]
    fn test_config_serde() {
        let config = KeyManagerConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: KeyManagerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.secrets_path, parsed.secrets_path);
    }
}
