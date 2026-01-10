//! Symbol type for representing trading pair identifiers.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::ValidationError;

/// Symbol type - used for representing trading pair identifiers.
///
/// Wraps a `String` value with validation to ensure proper format.
/// Symbols are typically in the format "BTC-USDT" or "BTCUSDT".
///
/// # Examples
///
/// ```
/// use zephyr_core::types::Symbol;
///
/// let symbol = Symbol::new("BTC-USDT").unwrap();
/// assert_eq!(symbol.as_str(), "BTC-USDT");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Symbol(String);

impl Symbol {
    /// Creates a new `Symbol` from a string.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::EmptySymbol` if the string is empty.
    /// Returns `ValidationError::InvalidSymbol` if the format is invalid.
    ///
    /// # Examples
    ///
    /// ```
    /// use zephyr_core::types::Symbol;
    ///
    /// let symbol = Symbol::new("BTC-USDT").unwrap();
    /// assert!(Symbol::new("").is_err());
    /// ```
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let s = value.into();
        if s.is_empty() {
            return Err(ValidationError::EmptySymbol);
        }
        // Basic validation: must contain only alphanumeric chars, hyphens, underscores
        if !s
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ValidationError::InvalidSymbol(s));
        }
        Ok(Self(s))
    }

    /// Creates a new `Symbol` without validation.
    ///
    /// # Safety
    ///
    /// The caller must ensure the value is a valid symbol format.
    #[must_use]
    pub fn new_unchecked(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the symbol as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns true if this symbol is valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.0.is_empty()
            && self
                .0
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    }

    /// Returns the base asset from the symbol (e.g., "BTC" from "BTC-USDT").
    ///
    /// Returns `None` if the symbol doesn't contain a separator.
    #[must_use]
    pub fn base_asset(&self) -> Option<&str> {
        self.0.split(&['-', '_', '/'][..]).next()
    }

    /// Returns the quote asset from the symbol (e.g., "USDT" from "BTC-USDT").
    ///
    /// Returns `None` if the symbol doesn't contain a separator.
    #[must_use]
    pub fn quote_asset(&self) -> Option<&str> {
        let parts: Vec<&str> = self.0.split(&['-', '_', '/'][..]).collect();
        if parts.len() >= 2 {
            Some(parts[1])
        } else {
            None
        }
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Symbol {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Symbol> for String {
    fn from(symbol: Symbol) -> Self {
        symbol.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_new_valid() {
        let symbol = Symbol::new("BTC-USDT").unwrap();
        assert_eq!(symbol.as_str(), "BTC-USDT");
    }

    #[test]
    fn test_symbol_new_empty() {
        let result = Symbol::new("");
        assert!(matches!(result, Err(ValidationError::EmptySymbol)));
    }

    #[test]
    fn test_symbol_new_invalid_chars() {
        let result = Symbol::new("BTC@USDT");
        assert!(matches!(result, Err(ValidationError::InvalidSymbol(_))));
    }

    #[test]
    fn test_symbol_base_quote_assets() {
        let symbol = Symbol::new("BTC-USDT").unwrap();
        assert_eq!(symbol.base_asset(), Some("BTC"));
        assert_eq!(symbol.quote_asset(), Some("USDT"));
    }

    #[test]
    fn test_symbol_no_separator() {
        let symbol = Symbol::new("BTCUSDT").unwrap();
        assert_eq!(symbol.base_asset(), Some("BTCUSDT"));
        assert_eq!(symbol.quote_asset(), None);
    }

    #[test]
    fn test_symbol_display() {
        let symbol = Symbol::new("ETH-USDT").unwrap();
        assert_eq!(format!("{symbol}"), "ETH-USDT");
    }

    #[test]
    fn test_symbol_serde_roundtrip() {
        let symbol = Symbol::new("BTC-USDT").unwrap();
        let json = serde_json::to_string(&symbol).unwrap();
        let parsed: Symbol = serde_json::from_str(&json).unwrap();
        assert_eq!(symbol, parsed);
    }
}
