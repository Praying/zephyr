//! K-line (candlestick) data structures.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

use crate::types::{Amount, Price, Quantity, Symbol, Timestamp};

use super::DataValidationError;

/// K-line time period enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KlinePeriod {
    /// 1 minute
    #[serde(rename = "1m")]
    Minute1,
    /// 5 minutes
    #[serde(rename = "5m")]
    Minute5,
    /// 15 minutes
    #[serde(rename = "15m")]
    Minute15,
    /// 30 minutes
    #[serde(rename = "30m")]
    Minute30,
    /// 1 hour
    #[serde(rename = "1h")]
    Hour1,
    /// 4 hours
    #[serde(rename = "4h")]
    Hour4,
    /// 1 day
    #[serde(rename = "1d")]
    Day1,
    /// 1 week
    #[serde(rename = "1w")]
    Week1,
}

impl KlinePeriod {
    /// Returns the duration of this period.
    #[must_use]
    pub const fn duration(&self) -> Duration {
        match self {
            Self::Minute1 => Duration::from_secs(60),
            Self::Minute5 => Duration::from_secs(5 * 60),
            Self::Minute15 => Duration::from_secs(15 * 60),
            Self::Minute30 => Duration::from_secs(30 * 60),
            Self::Hour1 => Duration::from_secs(60 * 60),
            Self::Hour4 => Duration::from_secs(4 * 60 * 60),
            Self::Day1 => Duration::from_secs(24 * 60 * 60),
            Self::Week1 => Duration::from_secs(7 * 24 * 60 * 60),
        }
    }

    /// Returns the duration in milliseconds.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn millis(&self) -> i64 {
        self.duration().as_millis() as i64
    }

    /// Returns the duration in seconds.
    #[must_use]
    #[allow(clippy::cast_possible_wrap)]
    pub const fn secs(&self) -> i64 {
        self.duration().as_secs() as i64
    }

    /// Returns a short string representation (e.g., "1m", "1h").
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Minute1 => "1m",
            Self::Minute5 => "5m",
            Self::Minute15 => "15m",
            Self::Minute30 => "30m",
            Self::Hour1 => "1h",
            Self::Hour4 => "4h",
            Self::Day1 => "1d",
            Self::Week1 => "1w",
        }
    }
}

impl fmt::Display for KlinePeriod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// K-line (candlestick/OHLCV) data structure.
///
/// Represents aggregated price and volume data over a time period.
///
/// # Examples
///
/// ```
/// use zephyr_core::data::{KlineData, KlinePeriod};
/// use zephyr_core::types::{Symbol, Timestamp, Price, Quantity, Amount};
/// use rust_decimal_macros::dec;
///
/// let kline = KlineData::builder()
///     .symbol(Symbol::new("BTC-USDT").unwrap())
///     .timestamp(Timestamp::new(1_704_067_200_000).unwrap())
///     .period(KlinePeriod::Hour1)
///     .open(Price::new(dec!(42000)).unwrap())
///     .high(Price::new(dec!(42500)).unwrap())
///     .low(Price::new(dec!(41800)).unwrap())
///     .close(Price::new(dec!(42300)).unwrap())
///     .volume(Quantity::new(dec!(100)).unwrap())
///     .turnover(Amount::new(dec!(4200000)).unwrap())
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KlineData {
    /// Trading pair symbol
    pub symbol: Symbol,
    /// K-line open timestamp (start of period)
    pub timestamp: Timestamp,
    /// K-line period
    pub period: KlinePeriod,
    /// Opening price
    pub open: Price,
    /// Highest price
    pub high: Price,
    /// Lowest price
    pub low: Price,
    /// Closing price
    pub close: Price,
    /// Trading volume (in base asset)
    pub volume: Quantity,
    /// Turnover (in quote asset)
    pub turnover: Amount,
}

impl KlineData {
    /// Creates a new builder for `KlineData`.
    #[must_use]
    pub fn builder() -> KlineDataBuilder {
        KlineDataBuilder::default()
    }

    /// Validates the K-line data.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - High price is less than low price
    /// - Open or close price is outside high-low range
    /// - Timestamp is zero
    pub fn validate(&self) -> Result<(), DataValidationError> {
        if self.timestamp.is_zero() {
            return Err(DataValidationError::InvalidTimestamp(
                "timestamp cannot be zero".to_string(),
            ));
        }

        if self.high < self.low {
            return Err(DataValidationError::InvalidPriceRelation(format!(
                "high ({}) < low ({})",
                self.high, self.low
            )));
        }

        if self.open > self.high || self.open < self.low {
            return Err(DataValidationError::InvalidPriceRelation(format!(
                "open ({}) outside high-low range ({}-{})",
                self.open, self.low, self.high
            )));
        }

        if self.close > self.high || self.close < self.low {
            return Err(DataValidationError::InvalidPriceRelation(format!(
                "close ({}) outside high-low range ({}-{})",
                self.close, self.low, self.high
            )));
        }

        Ok(())
    }

    /// Returns true if this is a bullish (green) candle.
    #[must_use]
    pub fn is_bullish(&self) -> bool {
        self.close >= self.open
    }

    /// Returns true if this is a bearish (red) candle.
    #[must_use]
    pub fn is_bearish(&self) -> bool {
        self.close < self.open
    }

    /// Returns the price range (high - low).
    #[must_use]
    pub fn range(&self) -> rust_decimal::Decimal {
        self.high - self.low
    }

    /// Returns the body size (|close - open|).
    #[must_use]
    pub fn body(&self) -> rust_decimal::Decimal {
        (self.close - self.open).abs()
    }
}

/// Builder for `KlineData`.
#[derive(Debug, Default)]
pub struct KlineDataBuilder {
    symbol: Option<Symbol>,
    timestamp: Option<Timestamp>,
    period: Option<KlinePeriod>,
    open: Option<Price>,
    high: Option<Price>,
    low: Option<Price>,
    close: Option<Price>,
    volume: Option<Quantity>,
    turnover: Option<Amount>,
}

impl KlineDataBuilder {
    /// Sets the symbol.
    #[must_use]
    pub fn symbol(mut self, symbol: Symbol) -> Self {
        self.symbol = Some(symbol);
        self
    }

    /// Sets the timestamp.
    #[must_use]
    pub fn timestamp(mut self, timestamp: Timestamp) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Sets the period.
    #[must_use]
    pub fn period(mut self, period: KlinePeriod) -> Self {
        self.period = Some(period);
        self
    }

    /// Sets the open price.
    #[must_use]
    pub fn open(mut self, open: Price) -> Self {
        self.open = Some(open);
        self
    }

    /// Sets the high price.
    #[must_use]
    pub fn high(mut self, high: Price) -> Self {
        self.high = Some(high);
        self
    }

    /// Sets the low price.
    #[must_use]
    pub fn low(mut self, low: Price) -> Self {
        self.low = Some(low);
        self
    }

    /// Sets the close price.
    #[must_use]
    pub fn close(mut self, close: Price) -> Self {
        self.close = Some(close);
        self
    }

    /// Sets the volume.
    #[must_use]
    pub fn volume(mut self, volume: Quantity) -> Self {
        self.volume = Some(volume);
        self
    }

    /// Sets the turnover.
    #[must_use]
    pub fn turnover(mut self, turnover: Amount) -> Self {
        self.turnover = Some(turnover);
        self
    }

    /// Builds the `KlineData`.
    ///
    /// # Errors
    ///
    /// Returns an error if any required field is missing or validation fails.
    pub fn build(self) -> Result<KlineData, DataValidationError> {
        let kline = KlineData {
            symbol: self
                .symbol
                .ok_or(DataValidationError::MissingField("symbol"))?,
            timestamp: self
                .timestamp
                .ok_or(DataValidationError::MissingField("timestamp"))?,
            period: self
                .period
                .ok_or(DataValidationError::MissingField("period"))?,
            open: self.open.ok_or(DataValidationError::MissingField("open"))?,
            high: self.high.ok_or(DataValidationError::MissingField("high"))?,
            low: self.low.ok_or(DataValidationError::MissingField("low"))?,
            close: self
                .close
                .ok_or(DataValidationError::MissingField("close"))?,
            volume: self
                .volume
                .ok_or(DataValidationError::MissingField("volume"))?,
            turnover: self
                .turnover
                .ok_or(DataValidationError::MissingField("turnover"))?,
        };
        kline.validate()?;
        Ok(kline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_valid_kline() -> KlineData {
        KlineData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1_704_067_200_000).unwrap())
            .period(KlinePeriod::Hour1)
            .open(Price::new(dec!(42000)).unwrap())
            .high(Price::new(dec!(42500)).unwrap())
            .low(Price::new(dec!(41800)).unwrap())
            .close(Price::new(dec!(42300)).unwrap())
            .volume(Quantity::new(dec!(100)).unwrap())
            .turnover(Amount::new(dec!(4200000)).unwrap())
            .build()
            .unwrap()
    }

    #[test]
    fn test_kline_builder_valid() {
        let kline = create_valid_kline();
        assert_eq!(kline.symbol.as_str(), "BTC-USDT");
        assert_eq!(kline.period, KlinePeriod::Hour1);
    }

    #[test]
    fn test_kline_builder_missing_field() {
        let result = KlineData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .build();
        assert!(matches!(result, Err(DataValidationError::MissingField(_))));
    }

    #[test]
    fn test_kline_invalid_high_low() {
        let result = KlineData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1_704_067_200_000).unwrap())
            .period(KlinePeriod::Hour1)
            .open(Price::new(dec!(42000)).unwrap())
            .high(Price::new(dec!(41000)).unwrap()) // high < low
            .low(Price::new(dec!(42000)).unwrap())
            .close(Price::new(dec!(42000)).unwrap())
            .volume(Quantity::new(dec!(100)).unwrap())
            .turnover(Amount::new(dec!(4200000)).unwrap())
            .build();
        assert!(matches!(
            result,
            Err(DataValidationError::InvalidPriceRelation(_))
        ));
    }

    #[test]
    fn test_kline_bullish_bearish() {
        let kline = create_valid_kline();
        assert!(kline.is_bullish()); // close > open

        let bearish = KlineData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1_704_067_200_000).unwrap())
            .period(KlinePeriod::Hour1)
            .open(Price::new(dec!(42300)).unwrap())
            .high(Price::new(dec!(42500)).unwrap())
            .low(Price::new(dec!(41800)).unwrap())
            .close(Price::new(dec!(42000)).unwrap())
            .volume(Quantity::new(dec!(100)).unwrap())
            .turnover(Amount::new(dec!(4200000)).unwrap())
            .build()
            .unwrap();
        assert!(bearish.is_bearish());
    }

    #[test]
    fn test_kline_period_duration() {
        assert_eq!(KlinePeriod::Minute1.secs(), 60);
        assert_eq!(KlinePeriod::Hour1.secs(), 3600);
        assert_eq!(KlinePeriod::Day1.secs(), 86400);
    }

    #[test]
    fn test_kline_period_display() {
        assert_eq!(format!("{}", KlinePeriod::Minute1), "1m");
        assert_eq!(format!("{}", KlinePeriod::Hour4), "4h");
        assert_eq!(format!("{}", KlinePeriod::Week1), "1w");
    }

    #[test]
    fn test_kline_serde_roundtrip() {
        let kline = create_valid_kline();
        let json = serde_json::to_string(&kline).unwrap();
        let parsed: KlineData = serde_json::from_str(&json).unwrap();
        assert_eq!(kline, parsed);
    }
}
