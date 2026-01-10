//! Leverage type for representing leverage multipliers.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use super::ValidationError;

/// Maximum allowed leverage value.
pub const MAX_LEVERAGE: u8 = 125;

/// Leverage type - used for representing leverage multipliers.
///
/// Wraps a `u8` value with validation to ensure it's within valid range.
///
/// # Examples
///
/// ```
/// use zephyr_core::types::Leverage;
///
/// let leverage = Leverage::new(10).unwrap();
/// assert_eq!(leverage.as_u8(), 10);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct Leverage(u8);

impl Leverage {
    /// 1x leverage (no leverage).
    pub const ONE: Self = Self(1);

    /// Creates a new `Leverage` from a `u8` value.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::ZeroLeverage` if the value is zero.
    /// Returns `ValidationError::LeverageExceedsMax` if the value exceeds maximum.
    ///
    /// # Examples
    ///
    /// ```
    /// use zephyr_core::types::Leverage;
    ///
    /// let leverage = Leverage::new(10).unwrap();
    /// assert!(Leverage::new(0).is_err());
    /// assert!(Leverage::new(200).is_err());
    /// ```
    pub fn new(value: u8) -> Result<Self, ValidationError> {
        if value == 0 {
            return Err(ValidationError::ZeroLeverage);
        }
        if value > MAX_LEVERAGE {
            return Err(ValidationError::LeverageExceedsMax(value, MAX_LEVERAGE));
        }
        Ok(Self(value))
    }

    /// Creates a new `Leverage` with a custom maximum.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::ZeroLeverage` if the value is zero.
    /// Returns `ValidationError::LeverageExceedsMax` if the value exceeds the custom maximum.
    pub fn new_with_max(value: u8, max: u8) -> Result<Self, ValidationError> {
        if value == 0 {
            return Err(ValidationError::ZeroLeverage);
        }
        if value > max {
            return Err(ValidationError::LeverageExceedsMax(value, max));
        }
        Ok(Self(value))
    }

    /// Creates a new `Leverage` without validation.
    ///
    /// # Safety
    ///
    /// The caller must ensure the value is within valid range (1-125).
    #[must_use]
    pub const fn new_unchecked(value: u8) -> Self {
        Self(value)
    }

    /// Returns the leverage as a `u8` value.
    #[must_use]
    pub const fn as_u8(&self) -> u8 {
        self.0
    }

    /// Returns the leverage as a `f64` for calculations.
    #[must_use]
    pub fn as_f64(&self) -> f64 {
        f64::from(self.0)
    }
}

impl fmt::Display for Leverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x", self.0)
    }
}

impl FromStr for Leverage {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Handle "10x" format
        let s = s.trim_end_matches('x').trim_end_matches('X');
        let value: u8 = s.parse().map_err(|_| ValidationError::ZeroLeverage)?;
        Self::new(value)
    }
}

impl From<Leverage> for u8 {
    fn from(leverage: Leverage) -> Self {
        leverage.0
    }
}

impl Default for Leverage {
    fn default() -> Self {
        Self::ONE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leverage_new_valid() {
        let leverage = Leverage::new(10).unwrap();
        assert_eq!(leverage.as_u8(), 10);
    }

    #[test]
    fn test_leverage_new_zero() {
        let result = Leverage::new(0);
        assert!(matches!(result, Err(ValidationError::ZeroLeverage)));
    }

    #[test]
    fn test_leverage_new_exceeds_max() {
        let result = Leverage::new(200);
        assert!(matches!(
            result,
            Err(ValidationError::LeverageExceedsMax(200, 125))
        ));
    }

    #[test]
    fn test_leverage_new_at_max() {
        let leverage = Leverage::new(125).unwrap();
        assert_eq!(leverage.as_u8(), 125);
    }

    #[test]
    fn test_leverage_with_custom_max() {
        let leverage = Leverage::new_with_max(20, 50).unwrap();
        assert_eq!(leverage.as_u8(), 20);

        let result = Leverage::new_with_max(60, 50);
        assert!(matches!(
            result,
            Err(ValidationError::LeverageExceedsMax(60, 50))
        ));
    }

    #[test]
    fn test_leverage_display() {
        let leverage = Leverage::new(10).unwrap();
        assert_eq!(format!("{leverage}"), "10x");
    }

    #[test]
    fn test_leverage_from_str() {
        let leverage: Leverage = "10".parse().unwrap();
        assert_eq!(leverage.as_u8(), 10);

        let leverage: Leverage = "20x".parse().unwrap();
        assert_eq!(leverage.as_u8(), 20);
    }

    #[test]
    fn test_leverage_default() {
        let leverage = Leverage::default();
        assert_eq!(leverage.as_u8(), 1);
    }

    #[test]
    fn test_leverage_serde_roundtrip() {
        let leverage = Leverage::new(50).unwrap();
        let json = serde_json::to_string(&leverage).unwrap();
        let parsed: Leverage = serde_json::from_str(&json).unwrap();
        assert_eq!(leverage, parsed);
    }
}
