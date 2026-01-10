//! Amount type for representing monetary amounts.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::iter::Sum;
use std::ops::{Add, Neg, Sub};
use std::str::FromStr;

use super::{Price, Quantity, ValidationError};

/// Amount type - used for representing monetary amounts (price × quantity).
///
/// Wraps a `Decimal` value to ensure type safety. Amounts can be
/// negative to represent losses or debits.
///
/// # Examples
///
/// ```
/// use zephyr_core::types::{Amount, Price, Quantity};
/// use rust_decimal_macros::dec;
///
/// let price = Price::new(dec!(100)).unwrap();
/// let qty = Quantity::new(dec!(10)).unwrap();
/// let amount = Amount::from_price_qty(price, qty);
/// assert_eq!(amount.as_decimal(), dec!(1000));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct Amount(Decimal);

impl Amount {
    /// Zero amount constant.
    pub const ZERO: Self = Self(Decimal::ZERO);

    /// Creates a new `Amount` from a `Decimal` value.
    ///
    /// Amounts can be negative (for losses/debits).
    ///
    /// # Examples
    ///
    /// ```
    /// use zephyr_core::types::Amount;
    /// use rust_decimal_macros::dec;
    ///
    /// let amount = Amount::new(dec!(1000.50)).unwrap();
    /// ```
    pub fn new(value: Decimal) -> Result<Self, ValidationError> {
        Ok(Self(value))
    }

    /// Creates a new non-negative `Amount`.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::NegativeAmount` if the value is negative.
    pub fn new_unsigned(value: Decimal) -> Result<Self, ValidationError> {
        if value < Decimal::ZERO {
            return Err(ValidationError::NegativeAmount(value));
        }
        Ok(Self(value))
    }

    /// Creates a new `Amount` without validation.
    #[must_use]
    pub const fn new_unchecked(value: Decimal) -> Self {
        Self(value)
    }

    /// Creates an `Amount` from price and quantity.
    #[must_use]
    pub fn from_price_qty(price: Price, qty: Quantity) -> Self {
        Self(price.as_decimal() * qty.as_decimal())
    }

    /// Returns the underlying `Decimal` value.
    #[must_use]
    pub const fn as_decimal(&self) -> Decimal {
        self.0
    }

    /// Returns true if the amount is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Returns the absolute value of the amount.
    #[must_use]
    pub fn abs(&self) -> Self {
        Self(self.0.abs())
    }

    /// Returns true if the amount is positive.
    #[must_use]
    pub fn is_positive(&self) -> bool {
        self.0 > Decimal::ZERO
    }

    /// Returns true if the amount is negative.
    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.0 < Decimal::ZERO
    }
}

impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Amount {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let decimal =
            Decimal::from_str(s).map_err(|_| ValidationError::NegativeAmount(Decimal::ZERO))?;
        Self::new(decimal)
    }
}

impl Add for Amount {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Amount {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Neg for Amount {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl Sum for Amount {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, |acc, x| acc + x)
    }
}

impl From<Amount> for Decimal {
    fn from(amount: Amount) -> Self {
        amount.0
    }
}

impl Default for Amount {
    fn default() -> Self {
        Self::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_amount_new() {
        let amount = Amount::new(dec!(1000.50)).unwrap();
        assert_eq!(amount.as_decimal(), dec!(1000.50));
    }

    #[test]
    fn test_amount_from_price_qty() {
        let price = Price::new(dec!(100)).unwrap();
        let qty = Quantity::new(dec!(10)).unwrap();
        let amount = Amount::from_price_qty(price, qty);
        assert_eq!(amount.as_decimal(), dec!(1000));
    }

    #[test]
    fn test_amount_negative() {
        let amount = Amount::new(dec!(-500)).unwrap();
        assert!(amount.is_negative());
        assert_eq!(amount.abs().as_decimal(), dec!(500));
    }

    #[test]
    fn test_amount_unsigned_rejects_negative() {
        let result = Amount::new_unsigned(dec!(-1.0));
        assert!(matches!(result, Err(ValidationError::NegativeAmount(_))));
    }

    #[test]
    fn test_amount_arithmetic() {
        let a1 = Amount::new(dec!(1000)).unwrap();
        let a2 = Amount::new(dec!(300)).unwrap();
        assert_eq!((a1 + a2).as_decimal(), dec!(1300));
        assert_eq!((a1 - a2).as_decimal(), dec!(700));
        assert_eq!((-a1).as_decimal(), dec!(-1000));
    }

    #[test]
    fn test_amount_serde_roundtrip() {
        let amount = Amount::new(dec!(1000.123456789)).unwrap();
        let json = serde_json::to_string(&amount).unwrap();
        let parsed: Amount = serde_json::from_str(&json).unwrap();
        assert_eq!(amount, parsed);
    }
}
