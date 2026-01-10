//! Quantity type for representing trading quantities.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::iter::Sum;
use std::ops::{Add, Neg, Sub};
use std::str::FromStr;

use super::ValidationError;

/// Quantity type - used for representing trading quantities.
///
/// Wraps a `Decimal` value to ensure type safety. Unlike `Price`,
/// quantities can be negative to represent short positions.
///
/// # Examples
///
/// ```
/// use zephyr_core::types::Quantity;
/// use rust_decimal_macros::dec;
///
/// let qty = Quantity::new(dec!(10.5)).unwrap();
/// assert_eq!(qty.as_decimal(), dec!(10.5));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct Quantity(Decimal);

impl Quantity {
    /// Zero quantity constant.
    pub const ZERO: Self = Self(Decimal::ZERO);

    /// Creates a new `Quantity` from a `Decimal` value.
    ///
    /// Quantities can be negative (for short positions).
    ///
    /// # Examples
    ///
    /// ```
    /// use zephyr_core::types::Quantity;
    /// use rust_decimal_macros::dec;
    ///
    /// let qty = Quantity::new(dec!(10.5)).unwrap();
    /// let short = Quantity::new(dec!(-5.0)).unwrap();
    /// ```
    pub fn new(value: Decimal) -> Result<Self, ValidationError> {
        Ok(Self(value))
    }

    /// Creates a new non-negative `Quantity`.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::NegativeQuantity` if the value is negative.
    pub fn new_unsigned(value: Decimal) -> Result<Self, ValidationError> {
        if value < Decimal::ZERO {
            return Err(ValidationError::NegativeQuantity(value));
        }
        Ok(Self(value))
    }

    /// Creates a new `Quantity` without validation.
    #[must_use]
    pub const fn new_unchecked(value: Decimal) -> Self {
        Self(value)
    }

    /// Returns the underlying `Decimal` value.
    #[must_use]
    pub const fn as_decimal(&self) -> Decimal {
        self.0
    }

    /// Returns true if the quantity is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Returns the absolute value of the quantity.
    #[must_use]
    pub fn abs(&self) -> Self {
        Self(self.0.abs())
    }

    /// Returns true if the quantity is positive.
    #[must_use]
    pub fn is_positive(&self) -> bool {
        self.0 > Decimal::ZERO
    }

    /// Returns true if the quantity is negative.
    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.0 < Decimal::ZERO
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Quantity {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let decimal =
            Decimal::from_str(s).map_err(|_| ValidationError::NegativeQuantity(Decimal::ZERO))?;
        Self::new(decimal)
    }
}

impl Add for Quantity {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Quantity {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Neg for Quantity {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl Sum for Quantity {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, |acc, x| acc + x)
    }
}

impl From<Quantity> for Decimal {
    fn from(qty: Quantity) -> Self {
        qty.0
    }
}

impl Default for Quantity {
    fn default() -> Self {
        Self::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_quantity_new() {
        let qty = Quantity::new(dec!(10.5)).unwrap();
        assert_eq!(qty.as_decimal(), dec!(10.5));
    }

    #[test]
    fn test_quantity_negative() {
        let qty = Quantity::new(dec!(-5.0)).unwrap();
        assert!(qty.is_negative());
        assert_eq!(qty.abs().as_decimal(), dec!(5.0));
    }

    #[test]
    fn test_quantity_unsigned_rejects_negative() {
        let result = Quantity::new_unsigned(dec!(-1.0));
        assert!(matches!(result, Err(ValidationError::NegativeQuantity(_))));
    }

    #[test]
    fn test_quantity_arithmetic() {
        let q1 = Quantity::new(dec!(10)).unwrap();
        let q2 = Quantity::new(dec!(3)).unwrap();
        assert_eq!((q1 + q2).as_decimal(), dec!(13));
        assert_eq!((q1 - q2).as_decimal(), dec!(7));
        assert_eq!((-q1).as_decimal(), dec!(-10));
    }

    #[test]
    fn test_quantity_sum() {
        let quantities = vec![
            Quantity::new(dec!(1)).unwrap(),
            Quantity::new(dec!(2)).unwrap(),
            Quantity::new(dec!(3)).unwrap(),
        ];
        let total: Quantity = quantities.into_iter().sum();
        assert_eq!(total.as_decimal(), dec!(6));
    }

    #[test]
    fn test_quantity_serde_roundtrip() {
        let qty = Quantity::new(dec!(10.123456789)).unwrap();
        let json = serde_json::to_string(&qty).unwrap();
        let parsed: Quantity = serde_json::from_str(&json).unwrap();
        assert_eq!(qty, parsed);
    }
}
