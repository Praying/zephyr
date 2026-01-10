//! Funding rate type for perpetual contract funding rates.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, Neg, Sub};
use std::str::FromStr;

use super::ValidationError;

/// Funding rate type - used for perpetual contract funding rates.
///
/// Wraps a `Decimal` value representing the funding rate as a decimal
/// (e.g., 0.0001 for 0.01%).
///
/// # Examples
///
/// ```
/// use zephyr_core::types::FundingRate;
/// use rust_decimal_macros::dec;
///
/// let rate = FundingRate::new(dec!(0.0001)).unwrap();
/// assert_eq!(rate.as_decimal(), dec!(0.0001));
/// assert_eq!(rate.as_percentage(), dec!(0.01));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct FundingRate(Decimal);

impl FundingRate {
    /// Zero funding rate constant.
    pub const ZERO: Self = Self(Decimal::ZERO);

    /// Creates a new `FundingRate` from a `Decimal` value.
    ///
    /// The value should be in decimal form (e.g., 0.0001 for 0.01%).
    /// Funding rates can be negative (shorts pay longs).
    ///
    /// # Examples
    ///
    /// ```
    /// use zephyr_core::types::FundingRate;
    /// use rust_decimal_macros::dec;
    ///
    /// let positive = FundingRate::new(dec!(0.0001)).unwrap();
    /// let negative = FundingRate::new(dec!(-0.0001)).unwrap();
    /// ```
    pub fn new(value: Decimal) -> Result<Self, ValidationError> {
        Ok(Self(value))
    }

    /// Creates a new `FundingRate` without validation.
    #[must_use]
    pub const fn new_unchecked(value: Decimal) -> Self {
        Self(value)
    }

    /// Creates a `FundingRate` from a percentage value.
    ///
    /// # Examples
    ///
    /// ```
    /// use zephyr_core::types::FundingRate;
    /// use rust_decimal_macros::dec;
    ///
    /// let rate = FundingRate::from_percentage(dec!(0.01)).unwrap();
    /// assert_eq!(rate.as_decimal(), dec!(0.0001));
    /// ```
    pub fn from_percentage(percentage: Decimal) -> Result<Self, ValidationError> {
        Ok(Self(percentage / Decimal::from(100)))
    }

    /// Returns the underlying `Decimal` value.
    #[must_use]
    pub const fn as_decimal(&self) -> Decimal {
        self.0
    }

    /// Returns the funding rate as a percentage.
    ///
    /// # Examples
    ///
    /// ```
    /// use zephyr_core::types::FundingRate;
    /// use rust_decimal_macros::dec;
    ///
    /// let rate = FundingRate::new(dec!(0.0001)).unwrap();
    /// assert_eq!(rate.as_percentage(), dec!(0.01));
    /// ```
    #[must_use]
    pub fn as_percentage(&self) -> Decimal {
        self.0 * Decimal::from(100)
    }

    /// Returns true if the funding rate is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Returns true if the funding rate is positive (longs pay shorts).
    #[must_use]
    pub fn is_positive(&self) -> bool {
        self.0 > Decimal::ZERO
    }

    /// Returns true if the funding rate is negative (shorts pay longs).
    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.0 < Decimal::ZERO
    }

    /// Returns the absolute value of the funding rate.
    #[must_use]
    pub fn abs(&self) -> Self {
        Self(self.0.abs())
    }
}

impl fmt::Display for FundingRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}%", self.as_percentage())
    }
}

impl FromStr for FundingRate {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Handle percentage format "0.01%"
        let has_percent = s.ends_with('%');
        let s = s.trim_end_matches('%');
        let decimal =
            Decimal::from_str(s).map_err(|_| ValidationError::NegativePrice(Decimal::ZERO))?;

        // If it looks like a percentage (has %), convert from percentage
        if has_percent {
            Self::from_percentage(decimal)
        } else {
            Self::new(decimal)
        }
    }
}

impl Add for FundingRate {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for FundingRate {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Neg for FundingRate {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl From<FundingRate> for Decimal {
    fn from(rate: FundingRate) -> Self {
        rate.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_funding_rate_new() {
        let rate = FundingRate::new(dec!(0.0001)).unwrap();
        assert_eq!(rate.as_decimal(), dec!(0.0001));
    }

    #[test]
    fn test_funding_rate_from_percentage() {
        let rate = FundingRate::from_percentage(dec!(0.01)).unwrap();
        assert_eq!(rate.as_decimal(), dec!(0.0001));
        assert_eq!(rate.as_percentage(), dec!(0.01));
    }

    #[test]
    fn test_funding_rate_negative() {
        let rate = FundingRate::new(dec!(-0.0001)).unwrap();
        assert!(rate.is_negative());
        assert_eq!(rate.abs().as_decimal(), dec!(0.0001));
    }

    #[test]
    fn test_funding_rate_arithmetic() {
        let r1 = FundingRate::new(dec!(0.0001)).unwrap();
        let r2 = FundingRate::new(dec!(0.00005)).unwrap();
        assert_eq!((r1 + r2).as_decimal(), dec!(0.00015));
        assert_eq!((r1 - r2).as_decimal(), dec!(0.00005));
        assert_eq!((-r1).as_decimal(), dec!(-0.0001));
    }

    #[test]
    fn test_funding_rate_display() {
        let rate = FundingRate::new(dec!(0.0001)).unwrap();
        // Display shows percentage with full precision
        assert!(format!("{rate}").ends_with('%'));
        assert!(format!("{rate}").contains("0.01"));
    }

    #[test]
    fn test_funding_rate_serde_roundtrip() {
        let rate = FundingRate::new(dec!(0.000123456)).unwrap();
        let json = serde_json::to_string(&rate).unwrap();
        let parsed: FundingRate = serde_json::from_str(&json).unwrap();
        assert_eq!(rate, parsed);
    }
}
