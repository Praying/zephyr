//! Performance metrics calculation for backtesting.
//!
//! Provides calculation of key performance indicators including:
//! - PnL (Profit and Loss)
//! - Sharpe Ratio
//! - Maximum Drawdown
//! - Win Rate and other statistics

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use zephyr_core::types::{Amount, Timestamp};

/// A single trade record for metrics calculation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    /// Trade timestamp
    pub timestamp: Timestamp,
    /// Realized PnL from this trade
    pub pnl: Amount,
    /// Trade value (notional)
    pub value: Amount,
    /// Whether this was a winning trade
    pub is_win: bool,
}

/// Performance statistics calculated from backtest results.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceStats {
    /// Total number of trades
    pub total_trades: u64,
    /// Number of winning trades
    pub winning_trades: u64,
    /// Number of losing trades
    pub losing_trades: u64,
    /// Win rate (0.0 to 1.0)
    pub win_rate: Decimal,
    /// Total profit from winning trades
    pub gross_profit: Amount,
    /// Total loss from losing trades
    pub gross_loss: Amount,
    /// Net profit (gross_profit - gross_loss)
    pub net_profit: Amount,
    /// Profit factor (gross_profit / gross_loss)
    pub profit_factor: Option<Decimal>,
    /// Average profit per winning trade
    pub avg_win: Option<Amount>,
    /// Average loss per losing trade
    pub avg_loss: Option<Amount>,
    /// Largest single winning trade
    pub largest_win: Option<Amount>,
    /// Largest single losing trade
    pub largest_loss: Option<Amount>,
    /// Average trade duration (if available)
    pub avg_trade_duration_ms: Option<i64>,
}

/// Backtest metrics calculator.
///
/// Tracks equity curve and calculates performance metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestMetrics {
    /// Initial capital
    initial_capital: Amount,
    /// Current equity
    current_equity: Amount,
    /// Peak equity (for drawdown calculation)
    peak_equity: Amount,
    /// Maximum drawdown amount
    max_drawdown: Amount,
    /// Maximum drawdown percentage
    max_drawdown_pct: Decimal,
    /// Equity curve (timestamp, equity)
    equity_curve: Vec<(Timestamp, Amount)>,
    /// Daily returns for Sharpe calculation
    daily_returns: VecDeque<Decimal>,
    /// Trade records
    trades: Vec<TradeRecord>,
    /// Last equity update timestamp
    last_update: Option<Timestamp>,
    /// Risk-free rate for Sharpe calculation (annualized)
    risk_free_rate: Decimal,
}

impl BacktestMetrics {
    /// Creates a new metrics tracker with initial capital.
    #[must_use]
    pub fn new(initial_capital: Amount) -> Self {
        Self {
            initial_capital,
            current_equity: initial_capital,
            peak_equity: initial_capital,
            max_drawdown: Amount::ZERO,
            max_drawdown_pct: Decimal::ZERO,
            equity_curve: vec![],
            daily_returns: VecDeque::new(),
            trades: Vec::new(),
            last_update: None,
            risk_free_rate: dec!(0.02), // 2% default risk-free rate
        }
    }

    /// Sets the risk-free rate for Sharpe ratio calculation.
    pub fn set_risk_free_rate(&mut self, rate: Decimal) {
        self.risk_free_rate = rate;
    }

    /// Records a trade.
    pub fn record_trade(&mut self, trade: TradeRecord) {
        // Update equity
        self.current_equity =
            Amount::new_unchecked(self.current_equity.as_decimal() + trade.pnl.as_decimal());

        // Update peak and drawdown
        if self.current_equity > self.peak_equity {
            self.peak_equity = self.current_equity;
        }

        let drawdown =
            Amount::new_unchecked(self.peak_equity.as_decimal() - self.current_equity.as_decimal());
        if drawdown > self.max_drawdown {
            self.max_drawdown = drawdown;
            if !self.peak_equity.is_zero() {
                self.max_drawdown_pct =
                    drawdown.as_decimal() / self.peak_equity.as_decimal() * dec!(100);
            }
        }

        // Record equity point
        self.equity_curve
            .push((trade.timestamp, self.current_equity));

        // Store trade
        self.trades.push(trade);
    }

    /// Updates equity at a timestamp (for equity curve tracking).
    pub fn update_equity(&mut self, timestamp: Timestamp, equity: Amount) {
        // Calculate daily return if we have a previous update
        if let Some(last_ts) = self.last_update {
            let time_diff = timestamp.as_millis() - last_ts.as_millis();
            // If at least 1 day has passed (86400000 ms)
            if time_diff >= 86_400_000 && !self.current_equity.is_zero() {
                let daily_return = (equity.as_decimal() - self.current_equity.as_decimal())
                    / self.current_equity.as_decimal();
                self.daily_returns.push_back(daily_return);

                // Keep only last 365 days for rolling calculation
                if self.daily_returns.len() > 365 {
                    self.daily_returns.pop_front();
                }
            }
        }

        self.current_equity = equity;
        self.last_update = Some(timestamp);

        // Update peak and drawdown
        if equity > self.peak_equity {
            self.peak_equity = equity;
        }

        let drawdown = Amount::new_unchecked(self.peak_equity.as_decimal() - equity.as_decimal());
        if drawdown > self.max_drawdown {
            self.max_drawdown = drawdown;
            if !self.peak_equity.is_zero() {
                self.max_drawdown_pct =
                    drawdown.as_decimal() / self.peak_equity.as_decimal() * dec!(100);
            }
        }

        self.equity_curve.push((timestamp, equity));
    }

    /// Returns the current equity.
    #[must_use]
    pub fn current_equity(&self) -> Amount {
        self.current_equity
    }

    /// Returns the total PnL.
    #[must_use]
    pub fn total_pnl(&self) -> Amount {
        Amount::new_unchecked(self.current_equity.as_decimal() - self.initial_capital.as_decimal())
    }

    /// Returns the total return percentage.
    #[must_use]
    pub fn total_return_pct(&self) -> Decimal {
        if self.initial_capital.is_zero() {
            return Decimal::ZERO;
        }
        (self.current_equity.as_decimal() - self.initial_capital.as_decimal())
            / self.initial_capital.as_decimal()
            * dec!(100)
    }

    /// Returns the maximum drawdown amount.
    #[must_use]
    pub fn max_drawdown(&self) -> Amount {
        self.max_drawdown
    }

    /// Returns the maximum drawdown percentage.
    #[must_use]
    pub fn max_drawdown_pct(&self) -> Decimal {
        self.max_drawdown_pct
    }

    /// Calculates the Sharpe ratio.
    ///
    /// Uses daily returns and annualizes the result.
    /// Returns None if there are insufficient data points.
    #[must_use]
    pub fn sharpe_ratio(&self) -> Option<Decimal> {
        if self.daily_returns.len() < 2 {
            return None;
        }

        let n = Decimal::from(self.daily_returns.len() as u64);
        let sum: Decimal = self.daily_returns.iter().copied().sum();
        let mean = sum / n;

        // Calculate standard deviation
        let variance: Decimal = self
            .daily_returns
            .iter()
            .map(|r| {
                let diff = *r - mean;
                diff * diff
            })
            .sum::<Decimal>()
            / n;

        let std_dev = decimal_sqrt(variance)?;

        if std_dev.is_zero() {
            return None;
        }

        // Annualize: multiply mean by 252 (trading days), std by sqrt(252)
        let annualized_return = mean * dec!(252);
        let annualized_std = std_dev * decimal_sqrt(dec!(252))?;

        // Sharpe = (return - risk_free) / std
        let excess_return = annualized_return - self.risk_free_rate;
        Some(excess_return / annualized_std)
    }

    /// Calculates performance statistics from recorded trades.
    #[must_use]
    pub fn calculate_stats(&self) -> PerformanceStats {
        let total_trades = self.trades.len() as u64;

        if total_trades == 0 {
            return PerformanceStats::default();
        }

        let winning_trades: Vec<_> = self.trades.iter().filter(|t| t.is_win).collect();
        let losing_trades: Vec<_> = self.trades.iter().filter(|t| !t.is_win).collect();

        let winning_count = winning_trades.len() as u64;
        let losing_count = losing_trades.len() as u64;

        let gross_profit: Amount = winning_trades
            .iter()
            .map(|t| t.pnl)
            .fold(Amount::ZERO, |acc, p| {
                Amount::new_unchecked(acc.as_decimal() + p.as_decimal())
            });

        let gross_loss: Amount = losing_trades
            .iter()
            .map(|t| t.pnl.abs())
            .fold(Amount::ZERO, |acc, p| {
                Amount::new_unchecked(acc.as_decimal() + p.as_decimal())
            });

        let net_profit = Amount::new_unchecked(gross_profit.as_decimal() - gross_loss.as_decimal());

        let win_rate = if total_trades > 0 {
            Decimal::from(winning_count) / Decimal::from(total_trades)
        } else {
            Decimal::ZERO
        };

        let profit_factor = if gross_loss.is_zero() {
            None
        } else {
            Some(gross_profit.as_decimal() / gross_loss.as_decimal())
        };

        let avg_win = if winning_count > 0 {
            Some(Amount::new_unchecked(
                gross_profit.as_decimal() / Decimal::from(winning_count),
            ))
        } else {
            None
        };

        let avg_loss = if losing_count > 0 {
            Some(Amount::new_unchecked(
                gross_loss.as_decimal() / Decimal::from(losing_count),
            ))
        } else {
            None
        };

        let largest_win = winning_trades.iter().map(|t| t.pnl).max();
        let largest_loss = losing_trades.iter().map(|t| t.pnl.abs()).max();

        PerformanceStats {
            total_trades,
            winning_trades: winning_count,
            losing_trades: losing_count,
            win_rate,
            gross_profit,
            gross_loss,
            net_profit,
            profit_factor,
            avg_win,
            avg_loss,
            largest_win,
            largest_loss,
            avg_trade_duration_ms: None,
        }
    }

    /// Returns the equity curve.
    #[must_use]
    pub fn equity_curve(&self) -> &[(Timestamp, Amount)] {
        &self.equity_curve
    }

    /// Returns all recorded trades.
    #[must_use]
    pub fn trades(&self) -> &[TradeRecord] {
        &self.trades
    }

    /// Resets the metrics tracker.
    pub fn reset(&mut self) {
        self.current_equity = self.initial_capital;
        self.peak_equity = self.initial_capital;
        self.max_drawdown = Amount::ZERO;
        self.max_drawdown_pct = Decimal::ZERO;
        self.equity_curve.clear();
        self.daily_returns.clear();
        self.trades.clear();
        self.last_update = None;
    }
}

/// Calculates the square root of a Decimal using Newton's method.
fn decimal_sqrt(value: Decimal) -> Option<Decimal> {
    if value < Decimal::ZERO {
        return None;
    }
    if value.is_zero() {
        return Some(Decimal::ZERO);
    }

    let mut guess = value / dec!(2);
    let epsilon = dec!(0.0000001);

    for _ in 0..100 {
        let new_guess = (guess + value / guess) / dec!(2);
        if (new_guess - guess).abs() < epsilon {
            return Some(new_guess);
        }
        guess = new_guess;
    }

    Some(guess)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_trade(pnl: Decimal, is_win: bool) -> TradeRecord {
        TradeRecord {
            timestamp: Timestamp::new(1_704_067_200_000).unwrap(),
            pnl: Amount::new(pnl).unwrap(),
            value: Amount::new(dec!(1000)).unwrap(),
            is_win,
        }
    }

    #[test]
    fn test_metrics_initial_state() {
        let metrics = BacktestMetrics::new(Amount::new(dec!(10000)).unwrap());
        assert_eq!(metrics.current_equity().as_decimal(), dec!(10000));
        assert_eq!(metrics.total_pnl().as_decimal(), dec!(0));
        assert_eq!(metrics.total_return_pct(), dec!(0));
    }

    #[test]
    fn test_metrics_record_winning_trade() {
        let mut metrics = BacktestMetrics::new(Amount::new(dec!(10000)).unwrap());

        metrics.record_trade(create_trade(dec!(500), true));

        assert_eq!(metrics.current_equity().as_decimal(), dec!(10500));
        assert_eq!(metrics.total_pnl().as_decimal(), dec!(500));
        assert_eq!(metrics.total_return_pct(), dec!(5));
    }

    #[test]
    fn test_metrics_record_losing_trade() {
        let mut metrics = BacktestMetrics::new(Amount::new(dec!(10000)).unwrap());

        metrics.record_trade(create_trade(dec!(-300), false));

        assert_eq!(metrics.current_equity().as_decimal(), dec!(9700));
        assert_eq!(metrics.total_pnl().as_decimal(), dec!(-300));
    }

    #[test]
    fn test_metrics_max_drawdown() {
        let mut metrics = BacktestMetrics::new(Amount::new(dec!(10000)).unwrap());

        // Win, then lose
        metrics.record_trade(create_trade(dec!(1000), true)); // Equity: 11000
        metrics.record_trade(create_trade(dec!(-500), false)); // Equity: 10500
        metrics.record_trade(create_trade(dec!(-1000), false)); // Equity: 9500

        // Max drawdown should be 11000 - 9500 = 1500
        assert_eq!(metrics.max_drawdown().as_decimal(), dec!(1500));

        // Max drawdown % = 1500 / 11000 * 100 ≈ 13.636...
        let expected_pct = dec!(1500) / dec!(11000) * dec!(100);
        assert_eq!(metrics.max_drawdown_pct(), expected_pct);
    }

    #[test]
    fn test_metrics_calculate_stats() {
        let mut metrics = BacktestMetrics::new(Amount::new(dec!(10000)).unwrap());

        metrics.record_trade(create_trade(dec!(500), true));
        metrics.record_trade(create_trade(dec!(300), true));
        metrics.record_trade(create_trade(dec!(-200), false));
        metrics.record_trade(create_trade(dec!(-100), false));

        let stats = metrics.calculate_stats();

        assert_eq!(stats.total_trades, 4);
        assert_eq!(stats.winning_trades, 2);
        assert_eq!(stats.losing_trades, 2);
        assert_eq!(stats.win_rate, dec!(0.5));
        assert_eq!(stats.gross_profit.as_decimal(), dec!(800)); // 500 + 300
        assert_eq!(stats.gross_loss.as_decimal(), dec!(300)); // 200 + 100
        assert_eq!(stats.net_profit.as_decimal(), dec!(500)); // 800 - 300

        // Profit factor = 800 / 300 ≈ 2.666...
        let expected_pf = dec!(800) / dec!(300);
        assert_eq!(stats.profit_factor, Some(expected_pf));

        // Avg win = 800 / 2 = 400
        assert_eq!(stats.avg_win.unwrap().as_decimal(), dec!(400));

        // Avg loss = 300 / 2 = 150
        assert_eq!(stats.avg_loss.unwrap().as_decimal(), dec!(150));
    }

    #[test]
    fn test_metrics_empty_stats() {
        let metrics = BacktestMetrics::new(Amount::new(dec!(10000)).unwrap());
        let stats = metrics.calculate_stats();

        assert_eq!(stats.total_trades, 0);
        assert_eq!(stats.win_rate, Decimal::ZERO);
    }

    #[test]
    fn test_metrics_reset() {
        let mut metrics = BacktestMetrics::new(Amount::new(dec!(10000)).unwrap());

        metrics.record_trade(create_trade(dec!(500), true));
        assert_eq!(metrics.current_equity().as_decimal(), dec!(10500));

        metrics.reset();

        assert_eq!(metrics.current_equity().as_decimal(), dec!(10000));
        assert_eq!(metrics.trades().len(), 0);
        assert_eq!(metrics.equity_curve().len(), 0);
    }

    #[test]
    fn test_decimal_sqrt() {
        assert_eq!(decimal_sqrt(dec!(0)), Some(dec!(0)));
        assert_eq!(decimal_sqrt(dec!(-1)), None);

        // sqrt(4) = 2
        let sqrt4 = decimal_sqrt(dec!(4)).unwrap();
        assert!((sqrt4 - dec!(2)).abs() < dec!(0.0001));

        // sqrt(2) ≈ 1.414
        let sqrt2 = decimal_sqrt(dec!(2)).unwrap();
        assert!((sqrt2 - dec!(1.41421356)).abs() < dec!(0.0001));
    }

    #[test]
    fn test_metrics_equity_curve() {
        let mut metrics = BacktestMetrics::new(Amount::new(dec!(10000)).unwrap());

        let trade1 = TradeRecord {
            timestamp: Timestamp::new(1000).unwrap(),
            pnl: Amount::new(dec!(500)).unwrap(),
            value: Amount::new(dec!(1000)).unwrap(),
            is_win: true,
        };

        let trade2 = TradeRecord {
            timestamp: Timestamp::new(2000).unwrap(),
            pnl: Amount::new(dec!(-200)).unwrap(),
            value: Amount::new(dec!(1000)).unwrap(),
            is_win: false,
        };

        metrics.record_trade(trade1);
        metrics.record_trade(trade2);

        let curve = metrics.equity_curve();
        assert_eq!(curve.len(), 2);
        assert_eq!(curve[0].0.as_millis(), 1000);
        assert_eq!(curve[0].1.as_decimal(), dec!(10500));
        assert_eq!(curve[1].0.as_millis(), 2000);
        assert_eq!(curve[1].1.as_decimal(), dec!(10300));
    }

    #[test]
    fn test_metrics_largest_win_loss() {
        let mut metrics = BacktestMetrics::new(Amount::new(dec!(10000)).unwrap());

        metrics.record_trade(create_trade(dec!(500), true));
        metrics.record_trade(create_trade(dec!(1000), true));
        metrics.record_trade(create_trade(dec!(-200), false));
        metrics.record_trade(create_trade(dec!(-800), false));

        let stats = metrics.calculate_stats();

        assert_eq!(stats.largest_win.unwrap().as_decimal(), dec!(1000));
        assert_eq!(stats.largest_loss.unwrap().as_decimal(), dec!(800));
    }
}
