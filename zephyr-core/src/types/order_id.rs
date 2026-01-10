//! Order ID type for representing order identifiers.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::ValidationError;

/// Order ID type - used for representing order identifiers.
///
/// Wraps a `String` value with validation to ensure non-empty.
///
/// # Examples
///
/// ```
/// use zephyr_core::types::OrderId;
///
/// let order_id = OrderId::new("12345678").unwrap();
/// assert_eq!(order_id.as_str(), "12345678");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OrderId(String);

impl OrderId {
    /// Creates a new `OrderId` from a string.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::EmptyOrderId` if the string is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use zephyr_core::types::OrderId;
    ///
    /// let order_id = OrderId::new("12345678").unwrap();
    /// assert!(OrderId::new("").is_err());
    /// ```
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let s = value.into();
        if s.is_empty() {
            return Err(ValidationError::EmptyOrderId);
        }
        Ok(Self(s))
    }

    /// Creates a new `OrderId` without validation.
    ///
    /// # Safety
    ///
    /// The caller must ensure the value is non-empty.
    #[must_use]
    pub fn new_unchecked(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Generates a new unique `OrderId` using UUID v4.
    #[must_use]
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Returns the order ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for OrderId {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for OrderId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<OrderId> for String {
    fn from(order_id: OrderId) -> Self {
        order_id.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_id_new_valid() {
        let order_id = OrderId::new("12345678").unwrap();
        assert_eq!(order_id.as_str(), "12345678");
    }

    #[test]
    fn test_order_id_new_empty() {
        let result = OrderId::new("");
        assert!(matches!(result, Err(ValidationError::EmptyOrderId)));
    }

    #[test]
    fn test_order_id_generate() {
        let id1 = OrderId::generate();
        let id2 = OrderId::generate();
        assert_ne!(id1, id2);
        assert!(!id1.as_str().is_empty());
    }

    #[test]
    fn test_order_id_display() {
        let order_id = OrderId::new("ABC123").unwrap();
        assert_eq!(format!("{order_id}"), "ABC123");
    }

    #[test]
    fn test_order_id_serde_roundtrip() {
        let order_id = OrderId::new("test-order-123").unwrap();
        let json = serde_json::to_string(&order_id).unwrap();
        let parsed: OrderId = serde_json::from_str(&json).unwrap();
        assert_eq!(order_id, parsed);
    }
}
