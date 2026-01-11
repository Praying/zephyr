//! Sensitive data masking for logs.
//!
//! Automatically masks API keys, secrets, and other sensitive information
//! in log output to prevent accidental exposure.

use regex::Regex;
use std::borrow::Cow;
use std::sync::LazyLock;

/// Patterns for detecting sensitive data.
static PATTERNS: LazyLock<Vec<SensitivePattern>> = LazyLock::new(|| {
    vec![
        // API keys (common formats)
        SensitivePattern {
            name: "api_key",
            regex: Regex::new(r#"(?i)(api[_-]?key|apikey)["\s:=]+["']?([a-zA-Z0-9]{16,64})["']?"#)
                .unwrap(),
            group: 2,
        },
        // Secret keys
        SensitivePattern {
            name: "secret_key",
            regex: Regex::new(
                r#"(?i)(secret[_-]?key|secretkey|api[_-]?secret)["\s:=]+["']?([a-zA-Z0-9]{16,64})["']?"#,
            )
            .unwrap(),
            group: 2,
        },
        // Passwords
        SensitivePattern {
            name: "password",
            regex: Regex::new(r#"(?i)(password|passwd|pwd)["\s:=]+["']?([^\s"']{4,})["']?"#)
                .unwrap(),
            group: 2,
        },
        // Bearer tokens
        SensitivePattern {
            name: "bearer_token",
            regex: Regex::new(r"(?i)bearer\s+([a-zA-Z0-9._-]{20,})").unwrap(),
            group: 1,
        },
        // JWT tokens
        SensitivePattern {
            name: "jwt",
            regex: Regex::new(r"eyJ[a-zA-Z0-9_-]*\.eyJ[a-zA-Z0-9_-]*\.[a-zA-Z0-9_-]*").unwrap(),
            group: 0,
        },
        // Private keys (hex format)
        SensitivePattern {
            name: "private_key",
            regex: Regex::new(r#"(?i)(private[_-]?key)["\s:=]+["']?(0x)?([a-fA-F0-9]{64})["']?"#)
                .unwrap(),
            group: 3,
        },
        // Binance API key format
        SensitivePattern {
            name: "binance_key",
            regex: Regex::new(r"[a-zA-Z0-9]{64}").unwrap(),
            group: 0,
        },
    ]
});

struct SensitivePattern {
    #[allow(dead_code)]
    name: &'static str,
    regex: Regex,
    group: usize,
}

/// Masks sensitive data in strings.
#[derive(Debug, Clone)]
pub struct SensitiveDataMasker {
    /// Minimum length of string to consider for masking
    min_length: usize,
    /// Characters to show at start of masked value
    show_start: usize,
    /// Characters to show at end of masked value
    show_end: usize,
    /// Mask character
    mask_char: char,
}

impl Default for SensitiveDataMasker {
    fn default() -> Self {
        Self::new()
    }
}

impl SensitiveDataMasker {
    /// Create a new masker with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            min_length: 8,
            show_start: 3,
            show_end: 3,
            mask_char: '*',
        }
    }

    /// Create a masker with custom settings.
    #[must_use]
    pub fn with_settings(min_length: usize, show_start: usize, show_end: usize) -> Self {
        Self {
            min_length,
            show_start,
            show_end,
            mask_char: '*',
        }
    }

    /// Mask a known sensitive value.
    ///
    /// # Example
    ///
    /// ```
    /// use zephyr_telemetry::masking::SensitiveDataMasker;
    ///
    /// let masker = SensitiveDataMasker::new();
    /// let masked = masker.mask_value("my_secret_api_key_12345");
    /// assert!(masked.contains("***"));
    /// assert!(masked.starts_with("my_"));
    /// ```
    #[must_use]
    pub fn mask_value(&self, value: &str) -> String {
        if value.len() < self.min_length {
            return self.mask_char.to_string().repeat(value.len().max(3));
        }

        let start = &value[..self.show_start.min(value.len())];
        let end = if value.len() > self.show_end {
            &value[value.len() - self.show_end..]
        } else {
            ""
        };

        format!("{}{}{}", start, self.mask_char.to_string().repeat(3), end)
    }

    /// Mask sensitive data in a string using pattern detection.
    ///
    /// # Example
    ///
    /// ```
    /// use zephyr_telemetry::masking::SensitiveDataMasker;
    ///
    /// let masker = SensitiveDataMasker::new();
    /// let input = r#"{"api_key": "abcdefghijklmnop1234567890"}"#;
    /// let masked = masker.mask_string(input);
    /// assert!(masked.contains("***"));
    /// ```
    #[must_use]
    pub fn mask_string<'a>(&self, input: &'a str) -> Cow<'a, str> {
        let mut result = input.to_string();
        let mut modified = false;

        for pattern in PATTERNS.iter() {
            if let Some(captures) = pattern.regex.captures(&result)
                && let Some(matched) = captures.get(pattern.group)
            {
                let masked = self.mask_value(matched.as_str());
                result = result.replace(matched.as_str(), &masked);
                modified = true;
            }
        }

        if modified {
            Cow::Owned(result)
        } else {
            Cow::Borrowed(input)
        }
    }

    /// Check if a string contains sensitive patterns.
    #[must_use]
    pub fn contains_sensitive(&self, input: &str) -> bool {
        PATTERNS.iter().any(|p| p.regex.is_match(input))
    }
}

/// A wrapper type for sensitive values that masks them in Display/Debug.
#[derive(Clone)]
pub struct Sensitive<T>(T);

impl<T> Sensitive<T> {
    /// Wrap a value as sensitive.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Get the inner value (use with caution).
    pub fn expose(&self) -> &T {
        &self.0
    }

    /// Consume and return the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> std::fmt::Debug for Sensitive<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl<T> std::fmt::Display for Sensitive<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl<T: serde::Serialize> serde::Serialize for Sensitive<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("[REDACTED]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_value() {
        let masker = SensitiveDataMasker::new();

        // Long value
        let result = masker.mask_value("abcdefghijklmnop");
        assert_eq!(result, "abc***nop");

        // Short value
        let result = masker.mask_value("short");
        assert_eq!(result, "*****");
    }

    #[test]
    fn test_mask_api_key() {
        let masker = SensitiveDataMasker::new();
        let input = r#"{"api_key": "abcdefghijklmnop1234567890123456"}"#;
        let result = masker.mask_string(input);

        assert!(result.contains("***"));
        assert!(!result.contains("abcdefghijklmnop1234567890123456"));
    }

    #[test]
    fn test_mask_secret_key() {
        let masker = SensitiveDataMasker::new();
        let input = r#"secret_key = "mysupersecretkey1234567890123456""#;
        let result = masker.mask_string(input);

        assert!(result.contains("***"));
    }

    #[test]
    fn test_mask_bearer_token() {
        let masker = SensitiveDataMasker::new();
        let input = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.test";
        let result = masker.mask_string(input);

        assert!(result.contains("***"));
    }

    #[test]
    fn test_no_sensitive_data() {
        let masker = SensitiveDataMasker::new();
        let input = "This is a normal log message without secrets";
        let result = masker.mask_string(input);

        assert_eq!(result, input);
    }

    #[test]
    fn test_sensitive_wrapper() {
        let secret = Sensitive::new("my_api_key");
        assert_eq!(format!("{secret}"), "[REDACTED]");
        assert_eq!(format!("{secret:?}"), "[REDACTED]");
        assert_eq!(*secret.expose(), "my_api_key");
    }

    #[test]
    fn test_contains_sensitive() {
        let masker = SensitiveDataMasker::new();

        assert!(masker.contains_sensitive(r#"api_key: "test1234567890123456""#));
        assert!(!masker.contains_sensitive("normal message"));
    }
}
