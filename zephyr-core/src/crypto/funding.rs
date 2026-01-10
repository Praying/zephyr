//! Funding rate calculations for perpetual contracts.
//!
//! This module provides functionality for calculating funding payments
//! and adjusting position costs based on funding rates.
//!
//! # Overview
//!
//! Perpetual contracts use funding rates to keep the contract price
//! close to the spot price. Funding is exchanged between long and short
//! positions periodically (typically every 8 hours).
//!
//! - Positive funding rate: longs pay shorts
//! - Negative funding rate: shorts pay longs

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::data::PositionSide;
use crate::types::{Amount, FundingRate, MarkPrice, Quantity, Symbol, Timestamp};

/// Funding payment result.
///
/// Represents the funding payment for a position at a specific time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FundingPayment {
    /// Trading symbol
    pub symbol: Symbol,
    /// Funding rate applied
    pub funding_rate: FundingRate,
    /// Position size at funding time
    pub position_size: Quantity,
    /// Mark price at funding time
    pub mark_price: MarkPrice,
    /// Calculated payment amount (positive = received, negative = paid)
    pub payment: Amount,
    /// Timestamp of funding settlement
    pub timestamp: Timestamp,
}

impl FundingPayment {
    /// Returns true if this payment was received (positive).
    #[must_use]
    pub fn is_received(&self) -> bool {
        self.payment.is_positive()
    }

    /// Returns true if this payment was paid (negative).
    #[must_use]
    pub fn is_paid(&self) -> bool {
        self.payment.is_negative()
    }
}

/// Funding schedule configuration.
///
/// Defines when funding settlements occur for a contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FundingSchedule {
    /// Interval between funding settlements
    pub interval: Duration,
    /// Settlement times in UTC (hours, e.g., [0, 8, 16] for 8-hour intervals)
    pub settlement_hours: Vec<u8>,
}

impl Default for FundingSchedule {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(8 * 60 * 60), // 8 hours
            settlement_hours: vec![0, 8, 16],
        }
    }
}

impl FundingSchedule {
    /// Creates a new funding schedule with 8-hour intervals.
    #[must_use]
    pub fn eight_hourly() -> Self {
        Self::default()
    }

    /// Creates a new funding schedule with 4-hour intervals.
    #[must_use]
    pub fn four_hourly() -> Self {
        Self {
            interval: Duration::from_secs(4 * 60 * 60),
            settlement_hours: vec![0, 4, 8, 12, 16, 20],
        }
    }

    /// Creates a new funding schedule with 1-hour intervals.
    #[must_use]
    pub fn hourly() -> Self {
        Self {
            interval: Duration::from_secs(60 * 60),
            settlement_hours: (0..24).collect(),
        }
    }

    /// Returns the number of funding settlements per day.
    #[must_use]
    pub fn settlements_per_day(&self) -> usize {
        self.settlement_hours.len()
    }
}

/// Funding rate calculator.
///
/// Calculates funding payments and adjusts position costs based on funding rates.
///
/// # Examples
///
/// ```
/// use zephyr_core::crypto::FundingCalculator;
/// use zephyr_core::types::{FundingRate, MarkPrice, Quantity, Amount};
/// use zephyr_core::data::PositionSide;
/// use rust_decimal_macros::dec;
///
/// let calculator = FundingCalculator::new();
///
/// let payment = calculator.calculate_funding_payment(
///     FundingRate::new(dec!(0.0001)).unwrap(),
///     Quantity::new(dec!(1.0)).unwrap(),
///     MarkPrice::new(dec!(50000)).unwrap(),
///     PositionSide::Long,
/// );
///
/// // Long position pays when funding rate is positive
/// // Payment = -1.0 * 50000 * 0.0001 = -5.0 USDT
/// assert_eq!(payment.as_decimal(), dec!(-5.0));
/// ```
#[derive(Debug, Clone, Default)]
pub struct FundingCalculator {
    /// Funding schedule
    pub schedule: FundingSchedule,
}

impl FundingCalculator {
    /// Creates a new funding calculator with default 8-hour schedule.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new funding calculator with a custom schedule.
    #[must_use]
    pub fn with_schedule(schedule: FundingSchedule) -> Self {
        Self { schedule }
    }

    /// Calculates the funding payment for a position.
    ///
    /// # Formula
    ///
    /// For long positions:
    /// - `payment = -position_size * mark_price * funding_rate`
    ///
    /// For short positions:
    /// - `payment = position_size * mark_price * funding_rate`
    ///
    /// A positive payment means the position receives funding.
    /// A negative payment means the position pays funding.
    ///
    /// # Arguments
    ///
    /// * `funding_rate` - The current funding rate
    /// * `position_size` - The absolute position size
    /// * `mark_price` - The mark price at funding time
    /// * `side` - The position side (Long or Short)
    #[must_use]
    pub fn calculate_funding_payment(
        &self,
        funding_rate: FundingRate,
        position_size: Quantity,
        mark_price: MarkPrice,
        side: PositionSide,
    ) -> Amount {
        let notional_value = position_size.as_decimal().abs() * mark_price.as_decimal();
        let funding_amount = notional_value * funding_rate.as_decimal();

        // Long pays when rate is positive, short receives
        // Short pays when rate is negative, long receives
        let direction = match side {
            PositionSide::Long => -Decimal::ONE,
            PositionSide::Short => Decimal::ONE,
            PositionSide::Both => {
                // For one-way mode, determine direction from position size
                if position_size.is_positive() {
                    -Decimal::ONE // Long
                } else if position_size.is_negative() {
                    Decimal::ONE // Short
                } else {
                    Decimal::ZERO // No position
                }
            }
        };

        Amount::new_unchecked(funding_amount * direction)
    }

    /// Calculates the adjusted entry price after funding payment.
    ///
    /// This adjusts the position's effective entry price to account for
    /// funding payments received or paid.
    ///
    /// # Formula
    ///
    /// `adjusted_price = entry_price - (cumulative_funding / position_size)`
    ///
    /// # Arguments
    ///
    /// * `entry_price` - The original entry price
    /// * `cumulative_funding` - Total funding received (positive) or paid (negative)
    /// * `position_size` - The position size
    /// * `side` - The position side
    #[must_use]
    pub fn adjust_entry_price(
        &self,
        entry_price: MarkPrice,
        cumulative_funding: Amount,
        position_size: Quantity,
        side: PositionSide,
    ) -> MarkPrice {
        if position_size.is_zero() {
            return entry_price;
        }

        let adjustment = cumulative_funding.as_decimal() / position_size.as_decimal().abs();

        let direction = match side {
            PositionSide::Long => Decimal::ONE,
            PositionSide::Short => -Decimal::ONE,
            PositionSide::Both => {
                if position_size.is_positive() {
                    Decimal::ONE
                } else {
                    -Decimal::ONE
                }
            }
        };

        let adjusted = entry_price.as_decimal() - (adjustment * direction);
        MarkPrice::new(adjusted.max(Decimal::ZERO)).unwrap_or(MarkPrice::ZERO)
    }

    /// Estimates the daily funding cost for a position.
    ///
    /// # Arguments
    ///
    /// * `funding_rate` - The current funding rate (per interval)
    /// * `position_size` - The position size
    /// * `mark_price` - The current mark price
    /// * `side` - The position side
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn estimate_daily_funding(
        &self,
        funding_rate: FundingRate,
        position_size: Quantity,
        mark_price: MarkPrice,
        side: PositionSide,
    ) -> Amount {
        let per_settlement =
            self.calculate_funding_payment(funding_rate, position_size, mark_price, side);
        let settlements = self.schedule.settlements_per_day();
        Amount::new_unchecked(per_settlement.as_decimal() * Decimal::from(settlements as u32))
    }

    /// Calculates the annualized funding rate.
    ///
    /// Converts the per-interval funding rate to an annualized percentage.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn annualized_rate(&self, funding_rate: FundingRate) -> Decimal {
        let settlements_per_year = self.schedule.settlements_per_day() * 365;
        funding_rate.as_decimal()
            * Decimal::from(settlements_per_year as u32)
            * Decimal::ONE_HUNDRED
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn test_symbol() -> Symbol {
        Symbol::new("BTC-USDT").unwrap()
    }

    #[test]
    fn test_funding_schedule_default() {
        let schedule = FundingSchedule::default();
        assert_eq!(schedule.settlements_per_day(), 3);
        assert_eq!(schedule.settlement_hours, vec![0, 8, 16]);
    }

    #[test]
    fn test_funding_schedule_four_hourly() {
        let schedule = FundingSchedule::four_hourly();
        assert_eq!(schedule.settlements_per_day(), 6);
    }

    #[test]
    fn test_funding_schedule_hourly() {
        let schedule = FundingSchedule::hourly();
        assert_eq!(schedule.settlements_per_day(), 24);
    }

    #[test]
    fn test_calculate_funding_payment_long_positive_rate() {
        let calculator = FundingCalculator::new();
        let payment = calculator.calculate_funding_payment(
            FundingRate::new(dec!(0.0001)).unwrap(), // 0.01%
            Quantity::new(dec!(1.0)).unwrap(),
            MarkPrice::new(dec!(50000)).unwrap(),
            PositionSide::Long,
        );

        // Long pays when rate is positive: -1.0 * 50000 * 0.0001 = -5.0
        assert_eq!(payment.as_decimal(), dec!(-5.0));
        assert!(payment.is_negative());
    }

    #[test]
    fn test_calculate_funding_payment_short_positive_rate() {
        let calculator = FundingCalculator::new();
        let payment = calculator.calculate_funding_payment(
            FundingRate::new(dec!(0.0001)).unwrap(),
            Quantity::new(dec!(1.0)).unwrap(),
            MarkPrice::new(dec!(50000)).unwrap(),
            PositionSide::Short,
        );

        // Short receives when rate is positive: 1.0 * 50000 * 0.0001 = 5.0
        assert_eq!(payment.as_decimal(), dec!(5.0));
        assert!(payment.is_positive());
    }

    #[test]
    fn test_calculate_funding_payment_long_negative_rate() {
        let calculator = FundingCalculator::new();
        let payment = calculator.calculate_funding_payment(
            FundingRate::new(dec!(-0.0001)).unwrap(),
            Quantity::new(dec!(1.0)).unwrap(),
            MarkPrice::new(dec!(50000)).unwrap(),
            PositionSide::Long,
        );

        // Long receives when rate is negative: -1.0 * 50000 * -0.0001 = 5.0
        assert_eq!(payment.as_decimal(), dec!(5.0));
        assert!(payment.is_positive());
    }

    #[test]
    fn test_calculate_funding_payment_short_negative_rate() {
        let calculator = FundingCalculator::new();
        let payment = calculator.calculate_funding_payment(
            FundingRate::new(dec!(-0.0001)).unwrap(),
            Quantity::new(dec!(1.0)).unwrap(),
            MarkPrice::new(dec!(50000)).unwrap(),
            PositionSide::Short,
        );

        // Short pays when rate is negative: 1.0 * 50000 * -0.0001 = -5.0
        assert_eq!(payment.as_decimal(), dec!(-5.0));
        assert!(payment.is_negative());
    }

    #[test]
    fn test_calculate_funding_payment_both_mode_long() {
        let calculator = FundingCalculator::new();
        let payment = calculator.calculate_funding_payment(
            FundingRate::new(dec!(0.0001)).unwrap(),
            Quantity::new(dec!(1.0)).unwrap(), // Positive = long
            MarkPrice::new(dec!(50000)).unwrap(),
            PositionSide::Both,
        );

        // Treated as long, pays when rate is positive
        assert_eq!(payment.as_decimal(), dec!(-5.0));
    }

    #[test]
    fn test_calculate_funding_payment_both_mode_short() {
        let calculator = FundingCalculator::new();
        let payment = calculator.calculate_funding_payment(
            FundingRate::new(dec!(0.0001)).unwrap(),
            Quantity::new(dec!(-1.0)).unwrap(), // Negative = short
            MarkPrice::new(dec!(50000)).unwrap(),
            PositionSide::Both,
        );

        // Treated as short, receives when rate is positive
        assert_eq!(payment.as_decimal(), dec!(5.0));
    }

    #[test]
    fn test_adjust_entry_price_received_funding() {
        let calculator = FundingCalculator::new();
        let adjusted = calculator.adjust_entry_price(
            MarkPrice::new(dec!(50000)).unwrap(),
            Amount::new(dec!(100)).unwrap(), // Received 100 USDT
            Quantity::new(dec!(1.0)).unwrap(),
            PositionSide::Long,
        );

        // Entry price reduced by funding received: 50000 - 100 = 49900
        assert_eq!(adjusted.as_decimal(), dec!(49900));
    }

    #[test]
    fn test_adjust_entry_price_paid_funding() {
        let calculator = FundingCalculator::new();
        let adjusted = calculator.adjust_entry_price(
            MarkPrice::new(dec!(50000)).unwrap(),
            Amount::new(dec!(-100)).unwrap(), // Paid 100 USDT
            Quantity::new(dec!(1.0)).unwrap(),
            PositionSide::Long,
        );

        // Entry price increased by funding paid: 50000 + 100 = 50100
        assert_eq!(adjusted.as_decimal(), dec!(50100));
    }

    #[test]
    fn test_estimate_daily_funding() {
        let calculator = FundingCalculator::new(); // 3 settlements per day
        let daily = calculator.estimate_daily_funding(
            FundingRate::new(dec!(0.0001)).unwrap(),
            Quantity::new(dec!(1.0)).unwrap(),
            MarkPrice::new(dec!(50000)).unwrap(),
            PositionSide::Long,
        );

        // -5.0 per settlement * 3 settlements = -15.0
        assert_eq!(daily.as_decimal(), dec!(-15.0));
    }

    #[test]
    fn test_annualized_rate() {
        let calculator = FundingCalculator::new(); // 3 settlements per day
        let annualized = calculator.annualized_rate(FundingRate::new(dec!(0.0001)).unwrap());

        // 0.0001 * 3 * 365 * 100 = 10.95%
        assert_eq!(annualized, dec!(10.95));
    }

    #[test]
    fn test_funding_payment_serde_roundtrip() {
        let payment = FundingPayment {
            symbol: test_symbol(),
            funding_rate: FundingRate::new(dec!(0.0001)).unwrap(),
            position_size: Quantity::new(dec!(1.0)).unwrap(),
            mark_price: MarkPrice::new(dec!(50000)).unwrap(),
            payment: Amount::new(dec!(-5.0)).unwrap(),
            timestamp: Timestamp::new(1704067200000).unwrap(),
        };

        let json = serde_json::to_string(&payment).unwrap();
        let parsed: FundingPayment = serde_json::from_str(&json).unwrap();
        assert_eq!(payment, parsed);
    }
}
