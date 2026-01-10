//! Configuration validation utilities.
//!
//! This module provides utilities for validating configuration values
//! with descriptive error messages.

use crate::error::ConfigError;
#[allow(clippy::disallowed_types)]
use std::collections::HashMap;

/// Result type for validation operations.
pub type ValidationResult = Result<(), ConfigError>;

/// Context for validation operations.
///
/// Tracks the current path in the configuration tree for better error messages.
#[derive(Debug, Clone, Default)]
pub struct ValidationContext {
    /// Current path in the configuration (e.g., "server.database.host").
    path: Vec<String>,
    /// Collected validation errors.
    errors: Vec<ConfigError>,
}

impl ValidationContext {
    /// Creates a new validation context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enters a new section in the configuration.
    pub fn enter(&mut self, section: impl Into<String>) {
        self.path.push(section.into());
    }

    /// Exits the current section.
    pub fn exit(&mut self) {
        self.path.pop();
    }

    /// Returns the current path as a dot-separated string.
    #[must_use]
    pub fn current_path(&self) -> String {
        self.path.join(".")
    }

    /// Adds a validation error.
    pub fn add_error(&mut self, error: ConfigError) {
        self.errors.push(error);
    }

    /// Returns true if there are no validation errors.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns the collected validation errors.
    #[must_use]
    pub fn errors(&self) -> &[ConfigError] {
        &self.errors
    }

    /// Consumes the context and returns the first error, if any.
    pub fn into_result(self) -> ValidationResult {
        self.errors.into_iter().next().map_or(Ok(()), Err)
    }

    /// Creates a missing field error with the current path context.
    #[must_use]
    pub fn missing_field(&self, field: impl Into<String>) -> ConfigError {
        let field = field.into();
        let section = if self.path.is_empty() {
            None
        } else {
            Some(self.current_path())
        };
        ConfigError::MissingField { field, section }
    }

    /// Creates an invalid value error with the current path context.
    #[must_use]
    pub fn invalid_value(
        &self,
        field: impl Into<String>,
        reason: impl Into<String>,
    ) -> ConfigError {
        let field_name = field.into();
        let full_field = if self.path.is_empty() {
            field_name
        } else {
            format!("{}.{}", self.current_path(), field_name)
        };
        ConfigError::InvalidValue {
            field: full_field,
            reason: reason.into(),
        }
    }
}

/// Validator for configuration values.
///
/// Provides fluent API for validating configuration fields.
#[derive(Debug)]
pub struct Validator<'a> {
    ctx: &'a mut ValidationContext,
}

impl<'a> Validator<'a> {
    /// Creates a new validator with the given context.
    pub fn new(ctx: &'a mut ValidationContext) -> Self {
        Self { ctx }
    }

    /// Validates that a required field is present (not None).
    pub fn require<T>(&mut self, field: &str, value: &Option<T>) -> &mut Self {
        if value.is_none() {
            self.ctx.add_error(self.ctx.missing_field(field));
        }
        self
    }

    /// Validates that a string field is not empty.
    pub fn require_non_empty(&mut self, field: &str, value: &str) -> &mut Self {
        if value.is_empty() {
            self.ctx.add_error(self.ctx.missing_field(field));
        }
        self
    }

    /// Validates that a numeric value is within a range.
    pub fn in_range<T: PartialOrd + std::fmt::Display>(
        &mut self,
        field: &str,
        value: &T,
        min: &T,
        max: &T,
    ) -> &mut Self {
        if value < min || value > max {
            self.ctx.add_error(self.ctx.invalid_value(
                field,
                format!("Value {value} must be between {min} and {max}"),
            ));
        }
        self
    }

    /// Validates that a numeric value is positive.
    pub fn positive<T: PartialOrd + Default + std::fmt::Display>(
        &mut self,
        field: &str,
        value: &T,
    ) -> &mut Self {
        if *value <= T::default() {
            self.ctx.add_error(
                self.ctx
                    .invalid_value(field, format!("Value {value} must be positive")),
            );
        }
        self
    }

    /// Validates that a numeric value is non-negative.
    pub fn non_negative<T: PartialOrd + Default + std::fmt::Display>(
        &mut self,
        field: &str,
        value: &T,
    ) -> &mut Self {
        if *value < T::default() {
            self.ctx.add_error(
                self.ctx
                    .invalid_value(field, format!("Value {value} must be non-negative")),
            );
        }
        self
    }

    /// Validates using a custom predicate.
    pub fn custom<F>(&mut self, field: &str, predicate: F, error_msg: &str) -> &mut Self
    where
        F: FnOnce() -> bool,
    {
        if !predicate() {
            self.ctx.add_error(self.ctx.invalid_value(field, error_msg));
        }
        self
    }

    /// Validates a URL format.
    pub fn valid_url(&mut self, field: &str, value: &str) -> &mut Self {
        if !value.is_empty()
            && !value.starts_with("http://")
            && !value.starts_with("https://")
            && !value.starts_with("ws://")
            && !value.starts_with("wss://")
        {
            self.ctx.add_error(self.ctx.invalid_value(
                field,
                "Must be a valid URL (http://, https://, ws://, or wss://)",
            ));
        }
        self
    }

    /// Returns the validation result.
    pub fn result(&self) -> ValidationResult {
        if self.ctx.is_valid() {
            Ok(())
        } else {
            // Return the first error
            Err(self.ctx.errors()[0].clone())
        }
    }
}

/// Helper function to validate a configuration value.
///
/// # Example
///
/// ```rust
/// use zephyr_core::config::validation::validate_range;
/// use zephyr_core::error::ConfigError;
///
/// let result = validate_range("port", &8080, &1, &65535);
/// assert!(result.is_ok());
///
/// let result = validate_range("port", &0, &1, &65535);
/// assert!(result.is_err());
/// ```
pub fn validate_range<T: PartialOrd + std::fmt::Display>(
    field: &str,
    value: &T,
    min: &T,
    max: &T,
) -> ValidationResult {
    if value < min || value > max {
        Err(ConfigError::invalid_value(
            field,
            format!("Value {value} must be between {min} and {max}"),
        ))
    } else {
        Ok(())
    }
}

/// Helper function to validate a required string field.
pub fn validate_required(field: &str, value: &str) -> ValidationResult {
    if value.is_empty() {
        Err(ConfigError::missing_field(field))
    } else {
        Ok(())
    }
}

/// Helper function to validate an optional required field.
pub fn validate_required_option<T>(field: &str, value: &Option<T>) -> ValidationResult {
    if value.is_none() {
        Err(ConfigError::missing_field(field))
    } else {
        Ok(())
    }
}

/// Environment variable helper for applying overrides.
///
/// # Example
///
/// ```rust
/// use zephyr_core::config::validation::EnvOverride;
///
/// let mut value = "default".to_string();
/// EnvOverride::apply_string("MY_VAR", &mut value);
/// // If MY_VAR is set, value will be updated
/// ```
pub struct EnvOverride;

impl EnvOverride {
    /// Applies an environment variable override to a string value.
    pub fn apply_string(var_name: &str, target: &mut String) {
        if let Ok(value) = std::env::var(var_name) {
            *target = value;
        }
    }

    /// Applies an environment variable override to an optional string value.
    pub fn apply_optional_string(var_name: &str, target: &mut Option<String>) {
        if let Ok(value) = std::env::var(var_name) {
            *target = Some(value);
        }
    }

    /// Applies an environment variable override to a numeric value.
    pub fn apply_number<T: std::str::FromStr>(var_name: &str, target: &mut T) {
        if let Ok(value) = std::env::var(var_name)
            && let Ok(parsed) = value.parse()
        {
            *target = parsed;
        }
    }

    /// Applies an environment variable override to a boolean value.
    pub fn apply_bool(var_name: &str, target: &mut bool) {
        if let Ok(value) = std::env::var(var_name) {
            match value.to_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => *target = true,
                "false" | "0" | "no" | "off" => *target = false,
                _ => {}
            }
        }
    }

    /// Applies an environment variable override to a duration in milliseconds.
    pub fn apply_duration_ms(var_name: &str, target: &mut u64) {
        if let Ok(value) = std::env::var(var_name)
            && let Ok(parsed) = value.parse()
        {
            *target = parsed;
        }
    }

    /// Applies environment variable overrides from a map.
    #[allow(clippy::disallowed_types)]
    pub fn apply_map(var_name: &str, target: &mut HashMap<String, String>) {
        if let Ok(value) = std::env::var(var_name) {
            // Parse as KEY1=VALUE1,KEY2=VALUE2
            for pair in value.split(',') {
                if let Some((k, v)) = pair.split_once('=') {
                    target.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_context_path() {
        let mut ctx = ValidationContext::new();
        assert_eq!(ctx.current_path(), "");

        ctx.enter("server");
        assert_eq!(ctx.current_path(), "server");

        ctx.enter("database");
        assert_eq!(ctx.current_path(), "server.database");

        ctx.exit();
        assert_eq!(ctx.current_path(), "server");

        ctx.exit();
        assert_eq!(ctx.current_path(), "");
    }

    #[test]
    fn test_validation_context_errors() {
        let mut ctx = ValidationContext::new();
        assert!(ctx.is_valid());

        ctx.add_error(ConfigError::missing_field("test"));
        assert!(!ctx.is_valid());
        assert_eq!(ctx.errors().len(), 1);
    }

    #[test]
    fn test_validator_require() {
        let mut ctx = ValidationContext::new();
        let mut validator = Validator::new(&mut ctx);

        let some_value: Option<i32> = Some(42);
        let none_value: Option<i32> = None;

        validator.require("present", &some_value);
        assert!(validator.result().is_ok());

        let mut ctx = ValidationContext::new();
        let mut validator = Validator::new(&mut ctx);
        validator.require("missing", &none_value);
        assert!(validator.result().is_err());
    }

    #[test]
    fn test_validator_in_range() {
        let mut ctx = ValidationContext::new();
        let mut validator = Validator::new(&mut ctx);

        validator.in_range("port", &8080, &1, &65535);
        assert!(validator.result().is_ok());

        let mut ctx = ValidationContext::new();
        let mut validator = Validator::new(&mut ctx);
        validator.in_range("port", &0, &1, &65535);
        assert!(validator.result().is_err());
    }

    #[test]
    fn test_validate_range() {
        assert!(validate_range("test", &50, &0, &100).is_ok());
        assert!(validate_range("test", &0, &0, &100).is_ok());
        assert!(validate_range("test", &100, &0, &100).is_ok());
        assert!(validate_range("test", &-1, &0, &100).is_err());
        assert!(validate_range("test", &101, &0, &100).is_err());
    }

    #[test]
    fn test_validate_required() {
        assert!(validate_required("test", "value").is_ok());
        assert!(validate_required("test", "").is_err());
    }

    #[test]
    fn test_env_override_bool() {
        let mut value = false;

        // Test without env var set
        EnvOverride::apply_bool("NONEXISTENT_VAR_12345", &mut value);
        assert!(!value);

        // We can't easily test with env vars set in unit tests
        // without affecting other tests, so we just verify the function compiles
    }
}
