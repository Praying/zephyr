//! Timestamp type for representing Unix millisecond timestamps.

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use super::ValidationError;

/// Timestamp type - used for representing Unix millisecond timestamps.
///
/// Wraps an `i64` value representing milliseconds since Unix epoch.
///
/// # Examples
///
/// ```
/// use zephyr_core::types::Timestamp;
///
/// let ts = Timestamp::now();
/// assert!(ts.as_millis() > 0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Zero timestamp constant.
    pub const ZERO: Self = Self(0);

    /// Creates a new `Timestamp` from milliseconds since Unix epoch.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::InvalidTimestamp` if the value is negative.
    ///
    /// # Examples
    ///
    /// ```
    /// use zephyr_core::types::Timestamp;
    ///
    /// let ts = Timestamp::new(1_704_067_200_000).unwrap();
    /// assert!(Timestamp::new(-1).is_err());
    /// ```
    pub fn new(millis: i64) -> Result<Self, ValidationError> {
        if millis < 0 {
            return Err(ValidationError::InvalidTimestamp(millis));
        }
        Ok(Self(millis))
    }

    /// Creates a new `Timestamp` without validation.
    ///
    /// # Safety
    ///
    /// The caller must ensure the value is non-negative.
    #[must_use]
    pub const fn new_unchecked(millis: i64) -> Self {
        Self(millis)
    }

    /// Returns the current timestamp.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn now() -> Self {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time before Unix epoch");
        Self(duration.as_millis() as i64)
    }

    /// Creates a `Timestamp` from seconds since Unix epoch.
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::InvalidTimestamp` if the value is negative.
    pub fn from_secs(secs: i64) -> Result<Self, ValidationError> {
        Self::new(secs * 1000)
    }

    /// Creates a `Timestamp` from nanoseconds since Unix epoch.
    ///
    /// Converts nanoseconds to milliseconds (truncating sub-millisecond precision).
    #[must_use]
    pub fn from_nanos(nanos: i64) -> Self {
        Self::new_unchecked(nanos / 1_000_000)
    }

    /// Returns the timestamp as milliseconds since Unix epoch.
    #[must_use]
    pub const fn as_millis(&self) -> i64 {
        self.0
    }

    /// Returns the timestamp as seconds since Unix epoch.
    #[must_use]
    pub const fn as_secs(&self) -> i64 {
        self.0 / 1000
    }

    /// Returns true if the timestamp is zero.
    #[must_use]
    pub const fn is_zero(&self) -> bool {
        self.0 == 0
    }

    /// Converts to a `DateTime<Utc>`.
    #[must_use]
    pub fn to_datetime(&self) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(self.0)
            .single()
            .unwrap_or_else(|| Utc.timestamp_millis_opt(0).unwrap())
    }

    /// Creates a `Timestamp` from a `DateTime<Utc>`.
    #[must_use]
    pub fn from_datetime(dt: DateTime<Utc>) -> Self {
        Self(dt.timestamp_millis())
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Timestamp {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let millis: i64 = s
            .parse()
            .map_err(|_| ValidationError::InvalidTimestamp(0))?;
        Self::new(millis)
    }
}

impl From<Timestamp> for i64 {
    fn from(ts: Timestamp) -> Self {
        ts.0
    }
}

impl From<Timestamp> for DateTime<Utc> {
    fn from(ts: Timestamp) -> Self {
        ts.to_datetime()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_new_valid() {
        let ts = Timestamp::new(1_704_067_200_000).unwrap();
        assert_eq!(ts.as_millis(), 1_704_067_200_000);
    }

    #[test]
    fn test_timestamp_new_zero() {
        let ts = Timestamp::new(0).unwrap();
        assert!(ts.is_zero());
    }

    #[test]
    fn test_timestamp_new_negative() {
        let result = Timestamp::new(-1);
        assert!(matches!(result, Err(ValidationError::InvalidTimestamp(_))));
    }

    #[test]
    fn test_timestamp_now() {
        let ts = Timestamp::now();
        assert!(ts.as_millis() > 0);
    }

    #[test]
    fn test_timestamp_from_secs() {
        let ts = Timestamp::from_secs(1_704_067_200).unwrap();
        assert_eq!(ts.as_millis(), 1_704_067_200_000);
        assert_eq!(ts.as_secs(), 1_704_067_200);
    }

    #[test]
    fn test_timestamp_to_datetime() {
        let ts = Timestamp::new(1_704_067_200_000).unwrap();
        let dt = ts.to_datetime();
        assert_eq!(dt.timestamp_millis(), 1_704_067_200_000);
    }

    #[test]
    fn test_timestamp_display() {
        let ts = Timestamp::new(1_704_067_200_000).unwrap();
        assert_eq!(format!("{ts}"), "1_704_067_200_000");
    }

    #[test]
    fn test_timestamp_serde_roundtrip() {
        let ts = Timestamp::new(1_704_067_200_000).unwrap();
        let json = serde_json::to_string(&ts).unwrap();
        let parsed: Timestamp = serde_json::from_str(&json).unwrap();
        assert_eq!(ts, parsed);
    }
}
