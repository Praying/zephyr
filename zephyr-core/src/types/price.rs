//! Price type for representing asset prices.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, Sub};
use std::str::FromStr;

use super::ValidationError;

/// Price type - used for representing asset prices.
///
/// Wraps a `Decimal` value to ensure type safety and prevent
/// mixing price values with other numeric types.
///
/// # Examples
///
/// ```
/// use zephyr_core::types::Price;
/// use rust_decimal_macros::dec;
///
/// let price = Price::new(dec!(100.50)).unwrap();
/// assert_eq!(price.as_decimal(), dec!(100.50));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct Price(Decimal);

impl Price {
    /// Zero price constant.
    pub const ZERO: Self = Self(Decimal::ZERO);

    /// Creates a new `Price` from a `Decimal` value.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::NegativePrice` if the value is negative.
    ///
    /// # Examples
    ///
    /// ```
    /// use zephyr_core::types::Price;
    /// use rust_decimal_macros::dec;
    ///
    /// let price = Price::new(dec!(100.50)).unwrap();
    /// assert!(Price::new(dec!(-1.0)).is_err());
    /// ```
    pub fn new(value: Decimal) -> Result<Self, ValidationError> {
        if value < Decimal::ZERO {
            return Err(ValidationError::NegativePrice(value));
        }
        Ok(Self(value))
    }

    /// Creates a new `Price` without validation.
    ///
    /// # Safety
    ///
    /// The caller must ensure the value is non-negative.
    #[must_use]
    pub const fn new_unchecked(value: Decimal) -> Self {
        Self(value)
    }

    /// Returns the underlying `Decimal` value.
    #[must_use]
    pub const fn as_decimal(&self) -> Decimal {
        self.0
    }

    /// Returns true if the price is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Price {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let decimal =
            Decimal::from_str(s).map_err(|_| ValidationError::NegativePrice(Decimal::ZERO))?;
        Self::new(decimal)
    }
}

impl Add for Price {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Price {
    type Output = Decimal;

    fn sub(self, rhs: Self) -> Self::Output {
        self.0 - rhs.0
    }
}

impl From<Price> for Decimal {
    fn from(price: Price) -> Self {
        price.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_price_new_valid() {
        let price = Price::new(dec!(100.50)).unwrap();
        assert_eq!(price.as_decimal(), dec!(100.50));
    }

    #[test]
    fn test_price_new_zero() {
        let price = Price::new(dec!(0)).unwrap();
        assert!(price.is_zero());
    }

    #[test]
    fn test_price_new_negative() {
        let result = Price::new(dec!(-1.0));
        assert!(matches!(result, Err(ValidationError::NegativePrice(_))));
    }

    #[test]
    fn test_price_display() {
        let price = Price::new(dec!(100.50)).unwrap();
        assert_eq!(format!("{price}"), "100.50");
    }

    #[test]
    fn test_price_from_str() {
        let price: Price = "100.50".parse().unwrap();
        assert_eq!(price.as_decimal(), dec!(100.50));
    }

    #[test]
    fn test_price_add() {
        let p1 = Price::new(dec!(100)).unwrap();
        let p2 = Price::new(dec!(50)).unwrap();
        let sum = p1 + p2;
        assert_eq!(sum.as_decimal(), dec!(150));
    }

    #[test]
    fn test_price_serde_roundtrip() {
        let price = Price::new(dec!(100.123456789)).unwrap();
        let json = serde_json::to_string(&price).unwrap();
        let parsed: Price = serde_json::from_str(&json).unwrap();
        assert_eq!(price, parsed);
    }
}
