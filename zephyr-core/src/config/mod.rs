//! Configuration management module.
//!
//! This module provides a flexible configuration system supporting:
//! - YAML and TOML configuration file formats
//! - Configuration validation with descriptive error messages
//! - Environment variable overrides for sensitive configuration
//! - Type-safe configuration loading and validation
//!
//! # Example
//!
//! ```rust,ignore
//! use zephyr_core::config::{ConfigLoader, ConfigFormat};
//!
//! // Load from YAML file
//! let config: MyConfig = ConfigLoader::new()
//!     .with_env_prefix("ZEPHYR")
//!     .load_file("config.yaml")?;
//!
//! // Load from TOML string
//! let config: MyConfig = ConfigLoader::new()
//!     .load_str(toml_content, ConfigFormat::Toml)?;
//! ```

mod loader;
mod traits;
pub mod validation;
mod zephyr_config;

pub use loader::{ConfigFormat, ConfigLoader};
pub use traits::{Configurable, Validatable};
pub use validation::{EnvOverride, ValidationContext, ValidationResult, Validator};
pub use zephyr_config::{
    ExchangeConfig, LoggingConfig, RiskConfig, ServerConfig, StorageConfig, ZephyrConfig,
};
