//! Mark price type for representing mark price values.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, Sub};
use std::str::FromStr;

use super::{Price, ValidationError};

/// Mark price type - used for representing mark price values.
///
/// Mark price is distinct from the last traded price and is used
/// for calculating unrealized `PnL` and liquidation prices.
///
/// # Examples
///
/// ```
/// use zephyr_core::types::MarkPrice;
/// use rust_decimal_macros::dec;
///
/// let mark_price = MarkPrice::new(dec!(50000.50)).unwrap();
/// assert_eq!(mark_price.as_decimal(), dec!(50000.50));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct MarkPrice(Decimal);

impl MarkPrice {
    /// Zero mark price constant.
    pub const ZERO: Self = Self(Decimal::ZERO);

    /// Creates a new `MarkPrice` from a `Decimal` value.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::NegativeMarkPrice` if the value is negative.
    ///
    /// # Examples
    ///
    /// ```
    /// use zephyr_core::types::MarkPrice;
    /// use rust_decimal_macros::dec;
    ///
    /// let mark_price = MarkPrice::new(dec!(50000.50)).unwrap();
    /// assert!(MarkPrice::new(dec!(-1.0)).is_err());
    /// ```
    pub fn new(value: Decimal) -> Result<Self, ValidationError> {
        if value < Decimal::ZERO {
            return Err(ValidationError::NegativeMarkPrice(value));
        }
        Ok(Self(value))
    }

    /// Creates a new `MarkPrice` without validation.
    ///
    /// # Safety
    ///
    /// The caller must ensure the value is non-negative.
    #[must_use]
    pub const fn new_unchecked(value: Decimal) -> Self {
        Self(value)
    }

    /// Creates a `MarkPrice` from a `Price`.
    #[must_use]
    pub fn from_price(price: Price) -> Self {
        Self(price.as_decimal())
    }

    /// Returns the underlying `Decimal` value.
    #[must_use]
    pub const fn as_decimal(&self) -> Decimal {
        self.0
    }

    /// Returns true if the mark price is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Converts to a `Price`.
    ///
    /// # Errors
    ///
    /// This should never fail since `MarkPrice` is always non-negative.
    pub fn to_price(&self) -> Result<Price, ValidationError> {
        Price::new(self.0)
    }
}

impl fmt::Display for MarkPrice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for MarkPrice {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let decimal =
            Decimal::from_str(s).map_err(|_| ValidationError::NegativeMarkPrice(Decimal::ZERO))?;
        Self::new(decimal)
    }
}

impl Add for MarkPrice {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for MarkPrice {
    type Output = Decimal;

    fn sub(self, rhs: Self) -> Self::Output {
        self.0 - rhs.0
    }
}

impl From<MarkPrice> for Decimal {
    fn from(mark_price: MarkPrice) -> Self {
        mark_price.0
    }
}

impl From<Price> for MarkPrice {
    fn from(price: Price) -> Self {
        Self::from_price(price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_mark_price_new_valid() {
        let mark_price = MarkPrice::new(dec!(50000.50)).unwrap();
        assert_eq!(mark_price.as_decimal(), dec!(50000.50));
    }

    #[test]
    fn test_mark_price_new_zero() {
        let mark_price = MarkPrice::new(dec!(0)).unwrap();
        assert!(mark_price.is_zero());
    }

    #[test]
    fn test_mark_price_new_negative() {
        let result = MarkPrice::new(dec!(-1.0));
        assert!(matches!(result, Err(ValidationError::NegativeMarkPrice(_))));
    }

    #[test]
    fn test_mark_price_from_price() {
        let price = Price::new(dec!(50000)).unwrap();
        let mark_price = MarkPrice::from_price(price);
        assert_eq!(mark_price.as_decimal(), dec!(50000));
    }

    #[test]
    fn test_mark_price_to_price() {
        let mark_price = MarkPrice::new(dec!(50000)).unwrap();
        let price = mark_price.to_price().unwrap();
        assert_eq!(price.as_decimal(), dec!(50000));
    }

    #[test]
    fn test_mark_price_display() {
        let mark_price = MarkPrice::new(dec!(50000.50)).unwrap();
        assert_eq!(format!("{mark_price}"), "50000.50");
    }

    #[test]
    fn test_mark_price_from_str() {
        let mark_price: MarkPrice = "50000.50".parse().unwrap();
        assert_eq!(mark_price.as_decimal(), dec!(50000.50));
    }

    #[test]
    fn test_mark_price_arithmetic() {
        let mp1 = MarkPrice::new(dec!(50000)).unwrap();
        let mp2 = MarkPrice::new(dec!(1000)).unwrap();
        let sum = mp1 + mp2;
        assert_eq!(sum.as_decimal(), dec!(51000));

        let diff = mp1 - mp2;
        assert_eq!(diff, dec!(49000));
    }

    #[test]
    fn test_mark_price_serde_roundtrip() {
        let mark_price = MarkPrice::new(dec!(50000.123456789)).unwrap();
        let json = serde_json::to_string(&mark_price).unwrap();
        let parsed: MarkPrice = serde_json::from_str(&json).unwrap();
        assert_eq!(mark_price, parsed);
    }
}
