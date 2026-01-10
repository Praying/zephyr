//! Margin and liquidation calculations for perpetual contracts.
//!
//! This module provides functionality for calculating:
//! - Initial and maintenance margin requirements
//! - Liquidation prices
//! - Unrealized profit and loss (`PnL`)
//!
//! # Margin Types
//!
//! - **Cross Margin**: Margin is shared across all positions
//! - **Isolated Margin**: Margin is isolated per position

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use crate::data::PositionSide;
use crate::types::{Amount, Leverage, MarkPrice, Price, Quantity};

/// Margin requirement result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarginRequirement {
    /// Initial margin required to open the position
    pub initial_margin: Amount,
    /// Maintenance margin required to keep the position open
    pub maintenance_margin: Amount,
    /// Margin ratio (`maintenance_margin` / `notional_value`)
    pub margin_ratio: Decimal,
}

/// Margin calculator for perpetual contracts.
///
/// Calculates margin requirements based on position size, leverage, and exchange rules.
///
/// # Examples
///
/// ```
/// use zephyr_core::crypto::MarginCalculator;
/// use zephyr_core::types::{Price, Quantity, Leverage};
/// use rust_decimal_macros::dec;
///
/// let calculator = MarginCalculator::new(dec!(0.004)); // 0.4% maintenance margin rate
///
/// let requirement = calculator.calculate_margin(
///     Price::new(dec!(50000)).unwrap(),
///     Quantity::new(dec!(1.0)).unwrap(),
///     Leverage::new(10).unwrap(),
/// );
///
/// // Initial margin = 50000 * 1.0 / 10 = 5000
/// assert_eq!(requirement.initial_margin.as_decimal(), dec!(5000));
/// ```
#[derive(Debug, Clone)]
pub struct MarginCalculator {
    /// Maintenance margin rate (e.g., 0.004 for 0.4%)
    pub maintenance_margin_rate: Decimal,
    /// Taker fee rate for liquidation calculation
    pub taker_fee_rate: Decimal,
}

impl MarginCalculator {
    /// Creates a new margin calculator with the specified maintenance margin rate.
    #[must_use]
    pub fn new(maintenance_margin_rate: Decimal) -> Self {
        Self {
            maintenance_margin_rate,
            taker_fee_rate: Decimal::ZERO,
        }
    }

    /// Creates a new margin calculator with maintenance margin and taker fee rates.
    #[must_use]
    pub fn with_fees(maintenance_margin_rate: Decimal, taker_fee_rate: Decimal) -> Self {
        Self {
            maintenance_margin_rate,
            taker_fee_rate,
        }
    }

    /// Creates a calculator with Binance-like default rates.
    #[must_use]
    pub fn binance_defaults() -> Self {
        Self {
            maintenance_margin_rate: dec!(0.004), // 0.4%
            taker_fee_rate: dec!(0.0004),         // 0.04%
        }
    }

    /// Calculates margin requirements for a position.
    ///
    /// # Formula
    ///
    /// - Initial Margin = Notional Value / Leverage
    /// - Maintenance Margin = Notional Value * Maintenance Margin Rate
    /// - Margin Ratio = Maintenance Margin / Notional Value
    #[must_use]
    pub fn calculate_margin(
        &self,
        price: Price,
        quantity: Quantity,
        leverage: Leverage,
    ) -> MarginRequirement {
        let notional_value = price.as_decimal() * quantity.as_decimal().abs();
        let initial_margin = notional_value / Decimal::from(leverage.as_u8());
        let maintenance_margin = notional_value * self.maintenance_margin_rate;

        MarginRequirement {
            initial_margin: Amount::new_unchecked(initial_margin),
            maintenance_margin: Amount::new_unchecked(maintenance_margin),
            margin_ratio: self.maintenance_margin_rate,
        }
    }

    /// Calculates the margin ratio for a position.
    ///
    /// Margin Ratio = (Maintenance Margin + Unrealized Loss) / Account Equity
    #[must_use]
    pub fn calculate_margin_ratio(
        &self,
        maintenance_margin: Amount,
        unrealized_pnl: Amount,
        account_equity: Amount,
    ) -> Decimal {
        if account_equity.is_zero() {
            return Decimal::MAX;
        }

        let unrealized_loss = if unrealized_pnl.is_negative() {
            unrealized_pnl.abs()
        } else {
            Amount::ZERO
        };

        (maintenance_margin.as_decimal() + unrealized_loss.as_decimal())
            / account_equity.as_decimal()
    }
}

/// Liquidation price calculator.
///
/// Calculates the price at which a position will be liquidated.
///
/// # Examples
///
/// ```
/// use zephyr_core::crypto::LiquidationCalculator;
/// use zephyr_core::types::{Price, Quantity, Leverage, Amount};
/// use zephyr_core::data::PositionSide;
/// use rust_decimal_macros::dec;
///
/// let calculator = LiquidationCalculator::new(dec!(0.004), dec!(0.0004));
///
/// let liq_price = calculator.calculate_liquidation_price(
///     Price::new(dec!(50000)).unwrap(),
///     Quantity::new(dec!(1.0)).unwrap(),
///     Leverage::new(10).unwrap(),
///     PositionSide::Long,
///     Amount::ZERO, // No extra margin
/// );
///
/// // For 10x long, liquidation is approximately 9.6% below entry
/// assert!(liq_price.is_some());
/// ```
#[derive(Debug, Clone)]
pub struct LiquidationCalculator {
    /// Maintenance margin rate
    pub maintenance_margin_rate: Decimal,
    /// Taker fee rate (charged on liquidation)
    pub taker_fee_rate: Decimal,
}

impl LiquidationCalculator {
    /// Creates a new liquidation calculator.
    #[must_use]
    pub fn new(maintenance_margin_rate: Decimal, taker_fee_rate: Decimal) -> Self {
        Self {
            maintenance_margin_rate,
            taker_fee_rate,
        }
    }

    /// Creates a calculator with Binance-like default rates.
    #[must_use]
    pub fn binance_defaults() -> Self {
        Self {
            maintenance_margin_rate: dec!(0.004),
            taker_fee_rate: dec!(0.0004),
        }
    }

    /// Calculates the liquidation price for a position.
    ///
    /// # Formula (Isolated Margin)
    ///
    /// For Long:
    /// `Liq Price = Entry Price * (1 - Initial Margin Rate + Maintenance Margin Rate + Taker Fee Rate)`
    ///
    /// For Short:
    /// `Liq Price = Entry Price * (1 + Initial Margin Rate - Maintenance Margin Rate - Taker Fee Rate)`
    ///
    /// # Arguments
    ///
    /// * `entry_price` - The position entry price
    /// * `quantity` - The position size
    /// * `leverage` - The position leverage
    /// * `side` - The position side
    /// * `extra_margin` - Additional margin added to the position
    ///
    /// # Returns
    ///
    /// The liquidation price, or `None` if the position cannot be liquidated
    /// (e.g., fully collateralized).
    #[must_use]
    pub fn calculate_liquidation_price(
        &self,
        entry_price: Price,
        quantity: Quantity,
        leverage: Leverage,
        side: PositionSide,
        extra_margin: Amount,
    ) -> Option<Price> {
        let entry = entry_price.as_decimal();
        let qty = quantity.as_decimal().abs();
        let initial_margin_rate = Decimal::ONE / Decimal::from(leverage.as_u8());

        // Extra margin as a rate of notional value
        let notional = entry * qty;
        let extra_margin_rate = if notional.is_zero() {
            Decimal::ZERO
        } else {
            extra_margin.as_decimal() / notional
        };

        let total_margin_rate = initial_margin_rate + extra_margin_rate;
        let fee_and_maintenance = self.maintenance_margin_rate + self.taker_fee_rate;

        let liq_price = match side {
            PositionSide::Long | PositionSide::Both if quantity.is_positive() => {
                // Long: price must fall to liquidate
                let multiplier = Decimal::ONE - total_margin_rate + fee_and_maintenance;
                if multiplier <= Decimal::ZERO {
                    return None; // Cannot be liquidated
                }
                entry * multiplier
            }
            PositionSide::Short | PositionSide::Both => {
                // Short: price must rise to liquidate
                let multiplier = Decimal::ONE + total_margin_rate - fee_and_maintenance;
                entry * multiplier
            }
            PositionSide::Long => return None,
        };

        if liq_price <= Decimal::ZERO {
            None
        } else {
            Price::new(liq_price).ok()
        }
    }

    /// Calculates the liquidation price for cross margin mode.
    ///
    /// In cross margin, the entire account balance is used as margin.
    ///
    /// # Arguments
    ///
    /// * `entry_price` - The position entry price
    /// * `quantity` - The position size
    /// * `side` - The position side
    /// * `wallet_balance` - Total wallet balance
    /// * `unrealized_pnl` - Current unrealized `PnL` from other positions
    #[must_use]
    pub fn calculate_cross_liquidation_price(
        &self,
        entry_price: Price,
        quantity: Quantity,
        side: PositionSide,
        wallet_balance: Amount,
        unrealized_pnl: Amount,
    ) -> Option<Price> {
        let entry = entry_price.as_decimal();
        let qty = quantity.as_decimal().abs();

        if qty.is_zero() {
            return None;
        }

        let available_margin = wallet_balance.as_decimal() + unrealized_pnl.as_decimal();
        let notional = entry * qty;
        let maintenance_margin = notional * self.maintenance_margin_rate;

        // Distance to liquidation in price terms
        let margin_buffer = available_margin - maintenance_margin;

        let liq_price = match side {
            PositionSide::Long | PositionSide::Both if quantity.is_positive() => {
                // Long: liquidation when price drops
                entry - (margin_buffer / qty)
            }
            PositionSide::Short | PositionSide::Both => {
                // Short: liquidation when price rises
                entry + (margin_buffer / qty)
            }
            PositionSide::Long => return None,
        };

        if liq_price <= Decimal::ZERO {
            None
        } else {
            Price::new(liq_price).ok()
        }
    }

    /// Checks if a position is at risk of liquidation.
    ///
    /// # Arguments
    ///
    /// * `current_price` - Current mark price
    /// * `liquidation_price` - Calculated liquidation price
    /// * `side` - Position side
    /// * `warning_threshold` - Percentage threshold for warning (e.g., 0.05 for 5%)
    #[must_use]
    pub fn is_at_risk(
        &self,
        current_price: MarkPrice,
        liquidation_price: Price,
        side: PositionSide,
        warning_threshold: Decimal,
    ) -> bool {
        let current = current_price.as_decimal();
        let liq = liquidation_price.as_decimal();

        if liq.is_zero() {
            return false;
        }

        let distance_ratio = match side {
            PositionSide::Long | PositionSide::Both => {
                // For long, check how close current price is to liquidation (below)
                (current - liq) / current
            }
            PositionSide::Short => {
                // For short, check how close current price is to liquidation (above)
                (liq - current) / current
            }
        };

        distance_ratio <= warning_threshold
    }
}

/// Calculates unrealized `PnL` for a position.
///
/// # Formula
///
/// For Long: `(Mark Price - Entry Price) * Quantity`
/// For Short: `(Entry Price - Mark Price) * Quantity`
///
/// # Arguments
///
/// * `entry_price` - The position entry price
/// * `mark_price` - The current mark price
/// * `quantity` - The position size (absolute value)
/// * `side` - The position side
#[must_use]
pub fn calculate_unrealized_pnl(
    entry_price: Price,
    mark_price: MarkPrice,
    quantity: Quantity,
    side: PositionSide,
) -> Amount {
    let price_diff = mark_price.as_decimal() - entry_price.as_decimal();
    let qty = quantity.as_decimal().abs();

    let direction = match side {
        PositionSide::Long => Decimal::ONE,
        PositionSide::Short => -Decimal::ONE,
        PositionSide::Both => {
            if quantity.is_positive() {
                Decimal::ONE
            } else if quantity.is_negative() {
                -Decimal::ONE
            } else {
                Decimal::ZERO
            }
        }
    };

    Amount::new_unchecked(price_diff * qty * direction)
}

/// Calculates the ROE (Return on Equity) percentage.
///
/// # Formula
///
/// `ROE = (Unrealized PnL / Initial Margin) * 100`
#[must_use]
pub fn calculate_roe(unrealized_pnl: Amount, initial_margin: Amount) -> Decimal {
    if initial_margin.is_zero() {
        return Decimal::ZERO;
    }
    (unrealized_pnl.as_decimal() / initial_margin.as_decimal()) * Decimal::ONE_HUNDRED
}

/// Calculates the break-even price including fees.
///
/// # Arguments
///
/// * `entry_price` - The position entry price
/// * `quantity` - The position size
/// * `entry_fee` - Fee paid on entry
/// * `exit_fee_rate` - Expected fee rate on exit
/// * `side` - The position side
#[must_use]
pub fn calculate_break_even_price(
    entry_price: Price,
    quantity: Quantity,
    entry_fee: Amount,
    exit_fee_rate: Decimal,
    side: PositionSide,
) -> Price {
    let entry = entry_price.as_decimal();
    let qty = quantity.as_decimal().abs();

    if qty.is_zero() {
        return entry_price;
    }

    let entry_fee_per_unit = entry_fee.as_decimal() / qty;

    match side {
        PositionSide::Long | PositionSide::Both if quantity.is_positive() => {
            // Long: need price to rise to cover fees
            let exit_fee_per_unit = entry * exit_fee_rate;
            let total_fee_per_unit = entry_fee_per_unit + exit_fee_per_unit;
            Price::new(entry + total_fee_per_unit).unwrap_or(entry_price)
        }
        PositionSide::Short | PositionSide::Both => {
            // Short: need price to fall to cover fees
            let exit_fee_per_unit = entry * exit_fee_rate;
            let total_fee_per_unit = entry_fee_per_unit + exit_fee_per_unit;
            Price::new((entry - total_fee_per_unit).max(Decimal::ZERO)).unwrap_or(Price::ZERO)
        }
        PositionSide::Long => entry_price,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_margin_calculator_basic() {
        let calculator = MarginCalculator::new(dec!(0.004));
        let requirement = calculator.calculate_margin(
            Price::new(dec!(50000)).unwrap(),
            Quantity::new(dec!(1.0)).unwrap(),
            Leverage::new(10).unwrap(),
        );

        // Initial margin = 50000 / 10 = 5000
        assert_eq!(requirement.initial_margin.as_decimal(), dec!(5000));
        // Maintenance margin = 50000 * 0.004 = 200
        assert_eq!(requirement.maintenance_margin.as_decimal(), dec!(200));
        assert_eq!(requirement.margin_ratio, dec!(0.004));
    }

    #[test]
    fn test_margin_calculator_high_leverage() {
        let calculator = MarginCalculator::new(dec!(0.004));
        let requirement = calculator.calculate_margin(
            Price::new(dec!(50000)).unwrap(),
            Quantity::new(dec!(1.0)).unwrap(),
            Leverage::new(100).unwrap(),
        );

        // Initial margin = 50000 / 100 = 500
        assert_eq!(requirement.initial_margin.as_decimal(), dec!(500));
    }

    #[test]
    fn test_liquidation_price_long() {
        let calculator = LiquidationCalculator::new(dec!(0.004), dec!(0.0004));
        let liq_price = calculator
            .calculate_liquidation_price(
                Price::new(dec!(50000)).unwrap(),
                Quantity::new(dec!(1.0)).unwrap(),
                Leverage::new(10).unwrap(),
                PositionSide::Long,
                Amount::ZERO,
            )
            .unwrap();

        // For 10x long: 50000 * (1 - 0.1 + 0.004 + 0.0004) = 50000 * 0.9044 = 45220
        assert_eq!(liq_price.as_decimal(), dec!(45220));
    }

    #[test]
    fn test_liquidation_price_short() {
        let calculator = LiquidationCalculator::new(dec!(0.004), dec!(0.0004));
        let liq_price = calculator
            .calculate_liquidation_price(
                Price::new(dec!(50000)).unwrap(),
                Quantity::new(dec!(1.0)).unwrap(),
                Leverage::new(10).unwrap(),
                PositionSide::Short,
                Amount::ZERO,
            )
            .unwrap();

        // For 10x short: 50000 * (1 + 0.1 - 0.004 - 0.0004) = 50000 * 1.0956 = 54780
        assert_eq!(liq_price.as_decimal(), dec!(54780));
    }

    #[test]
    fn test_liquidation_price_with_extra_margin() {
        let calculator = LiquidationCalculator::new(dec!(0.004), dec!(0.0004));
        let liq_price_no_extra = calculator
            .calculate_liquidation_price(
                Price::new(dec!(50000)).unwrap(),
                Quantity::new(dec!(1.0)).unwrap(),
                Leverage::new(10).unwrap(),
                PositionSide::Long,
                Amount::ZERO,
            )
            .unwrap();

        let liq_price_with_extra = calculator
            .calculate_liquidation_price(
                Price::new(dec!(50000)).unwrap(),
                Quantity::new(dec!(1.0)).unwrap(),
                Leverage::new(10).unwrap(),
                PositionSide::Long,
                Amount::new(dec!(1000)).unwrap(), // Extra 1000 USDT margin
            )
            .unwrap();

        // Extra margin should lower the liquidation price for longs
        assert!(liq_price_with_extra.as_decimal() < liq_price_no_extra.as_decimal());
    }

    #[test]
    fn test_calculate_unrealized_pnl_long_profit() {
        let pnl = calculate_unrealized_pnl(
            Price::new(dec!(50000)).unwrap(),
            MarkPrice::new(dec!(51000)).unwrap(),
            Quantity::new(dec!(1.0)).unwrap(),
            PositionSide::Long,
        );

        // (51000 - 50000) * 1.0 = 1000
        assert_eq!(pnl.as_decimal(), dec!(1000));
        assert!(pnl.is_positive());
    }

    #[test]
    fn test_calculate_unrealized_pnl_long_loss() {
        let pnl = calculate_unrealized_pnl(
            Price::new(dec!(50000)).unwrap(),
            MarkPrice::new(dec!(49000)).unwrap(),
            Quantity::new(dec!(1.0)).unwrap(),
            PositionSide::Long,
        );

        // (49000 - 50000) * 1.0 = -1000
        assert_eq!(pnl.as_decimal(), dec!(-1000));
        assert!(pnl.is_negative());
    }

    #[test]
    fn test_calculate_unrealized_pnl_short_profit() {
        let pnl = calculate_unrealized_pnl(
            Price::new(dec!(50000)).unwrap(),
            MarkPrice::new(dec!(49000)).unwrap(),
            Quantity::new(dec!(1.0)).unwrap(),
            PositionSide::Short,
        );

        // (49000 - 50000) * 1.0 * -1 = 1000
        assert_eq!(pnl.as_decimal(), dec!(1000));
        assert!(pnl.is_positive());
    }

    #[test]
    fn test_calculate_unrealized_pnl_short_loss() {
        let pnl = calculate_unrealized_pnl(
            Price::new(dec!(50000)).unwrap(),
            MarkPrice::new(dec!(51000)).unwrap(),
            Quantity::new(dec!(1.0)).unwrap(),
            PositionSide::Short,
        );

        // (51000 - 50000) * 1.0 * -1 = -1000
        assert_eq!(pnl.as_decimal(), dec!(-1000));
        assert!(pnl.is_negative());
    }

    #[test]
    fn test_calculate_roe() {
        let roe = calculate_roe(
            Amount::new(dec!(500)).unwrap(),
            Amount::new(dec!(5000)).unwrap(),
        );

        // (500 / 5000) * 100 = 10%
        assert_eq!(roe, dec!(10));
    }

    #[test]
    fn test_calculate_roe_negative() {
        let roe = calculate_roe(
            Amount::new(dec!(-500)).unwrap(),
            Amount::new(dec!(5000)).unwrap(),
        );

        // (-500 / 5000) * 100 = -10%
        assert_eq!(roe, dec!(-10));
    }

    #[test]
    fn test_is_at_risk_long() {
        let calculator = LiquidationCalculator::binance_defaults();

        // Current price close to liquidation
        assert!(calculator.is_at_risk(
            MarkPrice::new(dec!(46000)).unwrap(),
            Price::new(dec!(45000)).unwrap(),
            PositionSide::Long,
            dec!(0.05), // 5% threshold
        ));

        // Current price far from liquidation
        assert!(!calculator.is_at_risk(
            MarkPrice::new(dec!(50000)).unwrap(),
            Price::new(dec!(45000)).unwrap(),
            PositionSide::Long,
            dec!(0.05),
        ));
    }

    #[test]
    fn test_calculate_break_even_price_long() {
        let break_even = calculate_break_even_price(
            Price::new(dec!(50000)).unwrap(),
            Quantity::new(dec!(1.0)).unwrap(),
            Amount::new(dec!(20)).unwrap(), // Entry fee
            dec!(0.0004),                   // Exit fee rate
            PositionSide::Long,
        );

        // Entry fee per unit = 20
        // Exit fee per unit = 50000 * 0.0004 = 20
        // Break even = 50000 + 20 + 20 = 50040
        assert_eq!(break_even.as_decimal(), dec!(50040));
    }

    #[test]
    fn test_margin_ratio_calculation() {
        let calculator = MarginCalculator::new(dec!(0.004));
        let ratio = calculator.calculate_margin_ratio(
            Amount::new(dec!(200)).unwrap(),  // Maintenance margin
            Amount::new(dec!(-100)).unwrap(), // Unrealized loss
            Amount::new(dec!(5000)).unwrap(), // Account equity
        );

        // (200 + 100) / 5000 = 0.06
        assert_eq!(ratio, dec!(0.06));
    }

    #[test]
    fn test_cross_liquidation_price() {
        let calculator = LiquidationCalculator::new(dec!(0.004), dec!(0.0004));
        let liq_price = calculator
            .calculate_cross_liquidation_price(
                Price::new(dec!(50000)).unwrap(),
                Quantity::new(dec!(1.0)).unwrap(),
                PositionSide::Long,
                Amount::new(dec!(10000)).unwrap(), // Wallet balance
                Amount::ZERO,                      // No other unrealized PnL
            )
            .unwrap();

        // With 10000 USDT wallet balance and 1 BTC at 50000
        // Maintenance margin = 50000 * 0.004 = 200
        // Margin buffer = 10000 - 200 = 9800
        // Liq price = 50000 - 9800 = 40200
        assert_eq!(liq_price.as_decimal(), dec!(40200));
    }
}
