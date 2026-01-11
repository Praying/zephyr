//! Portfolio performance metrics and attribution.
//!
//! This module provides portfolio-level performance tracking including:
//! - Total value and P&L tracking
//! - Risk-adjusted metrics (Sharpe ratio, max drawdown)
//! - Strategy attribution analysis

#![allow(clippy::disallowed_types)]
#![allow(clippy::branches_sharing_code)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]

use std::collections::HashMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use zephyr_core::types::{Amount, Symbol, Timestamp};

use super::StrategyId;

/// Strategy attribution - P&L contribution from a single strategy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrategyAttribution {
    /// Strategy identifier.
    pub strategy_id: StrategyId,
    /// Realized P&L from this strategy.
    pub realized_pnl: Amount,
    /// Unrealized P&L from this strategy.
    pub unrealized_pnl: Amount,
    /// Total P&L (realized + unrealized).
    pub total_pnl: Amount,
    /// Contribution percentage to portfolio P&L.
    pub contribution_pct: Decimal,
    /// Number of trades executed.
    pub trade_count: u64,
    /// Win rate (0.0 to 1.0).
    pub win_rate: Decimal,
}

impl StrategyAttribution {
    /// Creates a new strategy attribution.
    #[must_use]
    pub fn new(strategy_id: StrategyId) -> Self {
        Self {
            strategy_id,
            realized_pnl: Amount::ZERO,
            unrealized_pnl: Amount::ZERO,
            total_pnl: Amount::ZERO,
            contribution_pct: Decimal::ZERO,
            trade_count: 0,
            win_rate: Decimal::ZERO,
        }
    }

    /// Updates the P&L values.
    pub fn update_pnl(&mut self, realized: Amount, unrealized: Amount) {
        self.realized_pnl = realized;
        self.unrealized_pnl = unrealized;
        self.total_pnl = realized + unrealized;
    }

    /// Records a trade result.
    pub fn record_trade(&mut self, pnl: Amount) {
        self.trade_count += 1;
        if pnl.is_positive() {
            // Update win rate incrementally
            let wins = (self.win_rate * Decimal::from(self.trade_count - 1)).round();
            let new_wins = wins + Decimal::ONE;
            self.win_rate = new_wins / Decimal::from(self.trade_count);
        } else {
            let wins = (self.win_rate * Decimal::from(self.trade_count - 1)).round();
            self.win_rate = wins / Decimal::from(self.trade_count);
        }
    }
}

/// Portfolio performance metrics.
///
/// Provides comprehensive performance tracking for a portfolio including
/// value, P&L, risk metrics, and strategy attribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioMetrics {
    /// Total portfolio value (equity).
    pub total_value: Amount,
    /// Total unrealized P&L across all positions.
    pub unrealized_pnl: Amount,
    /// Total realized P&L.
    pub realized_pnl: Amount,
    /// Sharpe ratio (annualized).
    pub sharpe_ratio: Decimal,
    /// Maximum drawdown as a decimal (e.g., 0.15 for 15%).
    pub max_drawdown: Decimal,
    /// Current drawdown from peak.
    pub current_drawdown: Decimal,
    /// Win rate across all trades (0.0 to 1.0).
    pub win_rate: Decimal,
    /// Total number of trades.
    pub total_trades: u64,
    /// Profit factor (gross profit / gross loss).
    pub profit_factor: Decimal,
    /// Strategy-level attribution.
    pub strategy_attribution: HashMap<StrategyId, StrategyAttribution>,
    /// Position values by symbol.
    pub position_values: HashMap<Symbol, Amount>,
    /// Peak portfolio value (for drawdown calculation).
    pub peak_value: Amount,
    /// Daily P&L.
    pub daily_pnl: Amount,
    /// Last update timestamp.
    pub updated_at: Timestamp,
}

impl PortfolioMetrics {
    /// Creates new empty portfolio metrics.
    #[must_use]
    pub fn new() -> Self {
        Self {
            total_value: Amount::ZERO,
            unrealized_pnl: Amount::ZERO,
            realized_pnl: Amount::ZERO,
            sharpe_ratio: Decimal::ZERO,
            max_drawdown: Decimal::ZERO,
            current_drawdown: Decimal::ZERO,
            win_rate: Decimal::ZERO,
            total_trades: 0,
            profit_factor: Decimal::ZERO,
            strategy_attribution: HashMap::new(),
            position_values: HashMap::new(),
            peak_value: Amount::ZERO,
            daily_pnl: Amount::ZERO,
            updated_at: Timestamp::now(),
        }
    }

    /// Creates metrics with initial value.
    #[must_use]
    pub fn with_initial_value(initial_value: Amount) -> Self {
        Self {
            total_value: initial_value,
            peak_value: initial_value,
            ..Self::new()
        }
    }

    /// Returns the total P&L (realized + unrealized).
    #[must_use]
    pub fn total_pnl(&self) -> Amount {
        self.realized_pnl + self.unrealized_pnl
    }

    /// Returns the return percentage.
    #[must_use]
    pub fn return_pct(&self) -> Decimal {
        let initial = self.total_value - self.total_pnl();
        if initial.is_zero() {
            return Decimal::ZERO;
        }
        (self.total_pnl().as_decimal() / initial.as_decimal()) * Decimal::ONE_HUNDRED
    }

    /// Updates the portfolio value and recalculates drawdown.
    pub fn update_value(&mut self, new_value: Amount) {
        self.total_value = new_value;

        // Update peak value
        if new_value.as_decimal() > self.peak_value.as_decimal() {
            self.peak_value = new_value;
        }

        // Calculate current drawdown
        if !self.peak_value.is_zero() {
            let drawdown = (self.peak_value.as_decimal() - new_value.as_decimal())
                / self.peak_value.as_decimal();
            self.current_drawdown = drawdown.max(Decimal::ZERO);

            // Update max drawdown
            if self.current_drawdown > self.max_drawdown {
                self.max_drawdown = self.current_drawdown;
            }
        }

        self.updated_at = Timestamp::now();
    }

    /// Records a trade and updates metrics.
    pub fn record_trade(&mut self, strategy_id: &StrategyId, pnl: Amount) {
        self.total_trades += 1;
        self.realized_pnl = self.realized_pnl + pnl;

        // Update win rate
        if pnl.is_positive() {
            let wins = (self.win_rate * Decimal::from(self.total_trades - 1)).round();
            let new_wins = wins + Decimal::ONE;
            self.win_rate = new_wins / Decimal::from(self.total_trades);
        } else {
            let wins = (self.win_rate * Decimal::from(self.total_trades - 1)).round();
            self.win_rate = wins / Decimal::from(self.total_trades);
        }

        // Update strategy attribution
        self.strategy_attribution
            .entry(strategy_id.clone())
            .or_insert_with(|| StrategyAttribution::new(strategy_id.clone()))
            .record_trade(pnl);

        self.updated_at = Timestamp::now();
    }

    /// Updates strategy attribution P&L.
    pub fn update_strategy_pnl(
        &mut self,
        strategy_id: &StrategyId,
        realized: Amount,
        unrealized: Amount,
    ) {
        self.strategy_attribution
            .entry(strategy_id.clone())
            .or_insert_with(|| StrategyAttribution::new(strategy_id.clone()))
            .update_pnl(realized, unrealized);

        // Recalculate contribution percentages
        self.recalculate_contributions();
        self.updated_at = Timestamp::now();
    }

    /// Updates position value for a symbol.
    pub fn update_position_value(&mut self, symbol: Symbol, value: Amount) {
        if value.is_zero() {
            self.position_values.remove(&symbol);
        } else {
            self.position_values.insert(symbol, value);
        }
        self.updated_at = Timestamp::now();
    }

    /// Recalculates strategy contribution percentages.
    fn recalculate_contributions(&mut self) {
        let total_pnl = self.total_pnl();
        if total_pnl.is_zero() {
            return;
        }

        for attribution in self.strategy_attribution.values_mut() {
            attribution.contribution_pct = (attribution.total_pnl.as_decimal()
                / total_pnl.as_decimal())
                * Decimal::ONE_HUNDRED;
        }
    }

    /// Calculates Sharpe ratio from returns.
    ///
    /// # Arguments
    ///
    /// * `returns` - Daily returns as decimals
    /// * `risk_free_rate` - Annual risk-free rate (e.g., 0.02 for 2%)
    pub fn calculate_sharpe(&mut self, returns: &[Decimal], risk_free_rate: Decimal) {
        if returns.is_empty() {
            self.sharpe_ratio = Decimal::ZERO;
            return;
        }

        let n = Decimal::from(returns.len());
        let mean: Decimal = returns.iter().sum::<Decimal>() / n;

        // Calculate standard deviation
        let variance: Decimal = returns
            .iter()
            .map(|r| (*r - mean) * (*r - mean))
            .sum::<Decimal>()
            / n;

        // Approximate square root using Newton's method
        let std_dev = sqrt_decimal(variance);

        if std_dev.is_zero() {
            self.sharpe_ratio = Decimal::ZERO;
            return;
        }

        // Annualize (assuming daily returns, 252 trading days)
        let annualized_return = mean * Decimal::from(252);
        let annualized_std = std_dev * sqrt_decimal(Decimal::from(252));

        self.sharpe_ratio = (annualized_return - risk_free_rate) / annualized_std;
    }

    /// Returns the number of open positions.
    #[must_use]
    pub fn position_count(&self) -> usize {
        self.position_values.len()
    }

    /// Returns the total position value.
    #[must_use]
    pub fn total_position_value(&self) -> Amount {
        self.position_values.values().copied().sum()
    }
}

impl Default for PortfolioMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Approximate square root using Newton's method.
fn sqrt_decimal(value: Decimal) -> Decimal {
    if value.is_zero() || value < Decimal::ZERO {
        return Decimal::ZERO;
    }

    let mut guess = value / Decimal::TWO;
    for _ in 0..20 {
        let new_guess = (guess + value / guess) / Decimal::TWO;
        if (new_guess - guess).abs() < Decimal::new(1, 10) {
            return new_guess;
        }
        guess = new_guess;
    }
    guess
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_strategy_attribution_new() {
        let attr = StrategyAttribution::new(StrategyId::new("s1"));
        assert_eq!(attr.strategy_id.as_str(), "s1");
        assert!(attr.total_pnl.is_zero());
        assert_eq!(attr.trade_count, 0);
    }

    #[test]
    fn test_strategy_attribution_update_pnl() {
        let mut attr = StrategyAttribution::new(StrategyId::new("s1"));
        attr.update_pnl(
            Amount::new(dec!(1000)).unwrap(),
            Amount::new(dec!(500)).unwrap(),
        );
        assert_eq!(attr.realized_pnl.as_decimal(), dec!(1000));
        assert_eq!(attr.unrealized_pnl.as_decimal(), dec!(500));
        assert_eq!(attr.total_pnl.as_decimal(), dec!(1500));
    }

    #[test]
    fn test_strategy_attribution_record_trade() {
        let mut attr = StrategyAttribution::new(StrategyId::new("s1"));

        // Win
        attr.record_trade(Amount::new(dec!(100)).unwrap());
        assert_eq!(attr.trade_count, 1);
        assert_eq!(attr.win_rate, dec!(1));

        // Loss
        attr.record_trade(Amount::new(dec!(-50)).unwrap());
        assert_eq!(attr.trade_count, 2);
        assert_eq!(attr.win_rate, dec!(0.5));
    }

    #[test]
    fn test_portfolio_metrics_new() {
        let metrics = PortfolioMetrics::new();
        assert!(metrics.total_value.is_zero());
        assert!(metrics.max_drawdown.is_zero());
        assert_eq!(metrics.total_trades, 0);
    }

    #[test]
    fn test_portfolio_metrics_with_initial_value() {
        let metrics = PortfolioMetrics::with_initial_value(Amount::new(dec!(10000)).unwrap());
        assert_eq!(metrics.total_value.as_decimal(), dec!(10000));
        assert_eq!(metrics.peak_value.as_decimal(), dec!(10000));
    }

    #[test]
    fn test_portfolio_metrics_total_pnl() {
        let mut metrics = PortfolioMetrics::new();
        metrics.realized_pnl = Amount::new(dec!(1000)).unwrap();
        metrics.unrealized_pnl = Amount::new(dec!(500)).unwrap();
        assert_eq!(metrics.total_pnl().as_decimal(), dec!(1500));
    }

    #[test]
    fn test_portfolio_metrics_update_value_drawdown() {
        let mut metrics = PortfolioMetrics::with_initial_value(Amount::new(dec!(10000)).unwrap());

        // Value increases - no drawdown
        metrics.update_value(Amount::new(dec!(12000)).unwrap());
        assert_eq!(metrics.peak_value.as_decimal(), dec!(12000));
        assert!(metrics.current_drawdown.is_zero());

        // Value decreases - drawdown
        metrics.update_value(Amount::new(dec!(10800)).unwrap());
        assert_eq!(metrics.peak_value.as_decimal(), dec!(12000));
        assert_eq!(metrics.current_drawdown, dec!(0.1)); // 10% drawdown
        assert_eq!(metrics.max_drawdown, dec!(0.1));
    }

    #[test]
    fn test_portfolio_metrics_record_trade() {
        let mut metrics = PortfolioMetrics::new();
        let strategy_id = StrategyId::new("s1");

        metrics.record_trade(&strategy_id, Amount::new(dec!(100)).unwrap());
        assert_eq!(metrics.total_trades, 1);
        assert_eq!(metrics.realized_pnl.as_decimal(), dec!(100));
        assert_eq!(metrics.win_rate, dec!(1));

        metrics.record_trade(&strategy_id, Amount::new(dec!(-50)).unwrap());
        assert_eq!(metrics.total_trades, 2);
        assert_eq!(metrics.realized_pnl.as_decimal(), dec!(50));
        assert_eq!(metrics.win_rate, dec!(0.5));
    }

    #[test]
    fn test_portfolio_metrics_update_strategy_pnl() {
        let mut metrics = PortfolioMetrics::new();
        let s1 = StrategyId::new("s1");
        let s2 = StrategyId::new("s2");

        metrics.update_strategy_pnl(&s1, Amount::new(dec!(1000)).unwrap(), Amount::ZERO);
        metrics.update_strategy_pnl(&s2, Amount::new(dec!(500)).unwrap(), Amount::ZERO);

        assert_eq!(metrics.strategy_attribution.len(), 2);

        let attr1 = metrics.strategy_attribution.get(&s1).unwrap();
        assert_eq!(attr1.total_pnl.as_decimal(), dec!(1000));
    }

    #[test]
    fn test_portfolio_metrics_position_tracking() {
        let mut metrics = PortfolioMetrics::new();
        let btc = Symbol::new("BTC-USDT").unwrap();
        let eth = Symbol::new("ETH-USDT").unwrap();

        metrics.update_position_value(btc.clone(), Amount::new(dec!(5000)).unwrap());
        metrics.update_position_value(eth.clone(), Amount::new(dec!(3000)).unwrap());

        assert_eq!(metrics.position_count(), 2);
        assert_eq!(metrics.total_position_value().as_decimal(), dec!(8000));

        // Remove position
        metrics.update_position_value(btc, Amount::ZERO);
        assert_eq!(metrics.position_count(), 1);
    }

    #[test]
    fn test_sqrt_decimal() {
        assert_eq!(sqrt_decimal(Decimal::ZERO), Decimal::ZERO);

        let result = sqrt_decimal(dec!(4));
        assert!((result - dec!(2)).abs() < dec!(0.0001));

        let result = sqrt_decimal(dec!(9));
        assert!((result - dec!(3)).abs() < dec!(0.0001));
    }

    #[test]
    fn test_portfolio_metrics_serde_roundtrip() {
        let mut metrics = PortfolioMetrics::with_initial_value(Amount::new(dec!(10000)).unwrap());
        metrics.realized_pnl = Amount::new(dec!(500)).unwrap();
        metrics.max_drawdown = dec!(0.05);

        let json = serde_json::to_string(&metrics).unwrap();
        let parsed: PortfolioMetrics = serde_json::from_str(&json).unwrap();

        assert_eq!(metrics.total_value, parsed.total_value);
        assert_eq!(metrics.realized_pnl, parsed.realized_pnl);
        assert_eq!(metrics.max_drawdown, parsed.max_drawdown);
    }
}
