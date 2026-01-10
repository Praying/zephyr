//! Advanced performance metrics for backtesting.
//!
//! Provides calculation of advanced risk-adjusted performance metrics:
//! - Sortino Ratio: Risk-adjusted return using downside deviation
//! - Calmar Ratio: Return relative to maximum drawdown
//! - Information Ratio: Active return relative to tracking error

use std::collections::VecDeque;

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use zephyr_core::types::{Amount, Timestamp};

/// Advanced performance metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdvancedMetrics {
    /// Sortino ratio (risk-adjusted return using downside deviation)
    pub sortino_ratio: Option<Decimal>,
    /// Calmar ratio (annualized return / max drawdown)
    pub calmar_ratio: Option<Decimal>,
    /// Information ratio (active return / tracking error)
    pub information_ratio: Option<Decimal>,
    /// Downside deviation (standard deviation of negative returns)
    pub downside_deviation: Option<Decimal>,
    /// Annualized return
    pub annualized_return: Option<Decimal>,
    /// Annualized volatility
    pub annualized_volatility: Option<Decimal>,
    /// Tracking error (if benchmark provided)
    pub tracking_error: Option<Decimal>,
    /// Active return (if benchmark provided)
    pub active_return: Option<Decimal>,
    /// Omega ratio
    pub omega_ratio: Option<Decimal>,
    /// Tail ratio (95th percentile / 5th percentile)
    pub tail_ratio: Option<Decimal>,
}

/// Configuration for advanced metrics calculation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedMetricsConfig {
    /// Risk-free rate (annualized, e.g., 0.02 for 2%)
    #[serde(default = "default_risk_free_rate")]
    pub risk_free_rate: Decimal,
    /// Minimum acceptable return (MAR) for Sortino calculation
    #[serde(default)]
    pub minimum_acceptable_return: Decimal,
    /// Trading days per year for annualization
    #[serde(default = "default_trading_days")]
    pub trading_days_per_year: u32,
    /// Threshold for Omega ratio calculation
    #[serde(default)]
    pub omega_threshold: Decimal,
}

fn default_risk_free_rate() -> Decimal {
    dec!(0.02)
}

fn default_trading_days() -> u32 {
    365 // Crypto markets trade 24/7
}

impl Default for AdvancedMetricsConfig {
    fn default() -> Self {
        Self {
            risk_free_rate: dec!(0.02),
            minimum_acceptable_return: Decimal::ZERO,
            trading_days_per_year: 365,
            omega_threshold: Decimal::ZERO,
        }
    }
}

/// Calculator for advanced performance metrics.
pub struct AdvancedMetricsCalculator {
    config: AdvancedMetricsConfig,
    /// Daily returns
    returns: VecDeque<Decimal>,
    /// Benchmark returns (optional)
    benchmark_returns: Option<VecDeque<Decimal>>,
    /// Equity curve for drawdown calculation
    equity_curve: Vec<(Timestamp, Amount)>,
    /// Initial equity
    initial_equity: Amount,
    /// Peak equity
    peak_equity: Amount,
    /// Maximum drawdown
    max_drawdown: Decimal,
    /// Start timestamp
    start_time: Option<Timestamp>,
    /// End timestamp
    end_time: Option<Timestamp>,
}

impl AdvancedMetricsCalculator {
    /// Creates a new advanced metrics calculator.
    #[must_use]
    pub fn new(config: AdvancedMetricsConfig, initial_equity: Amount) -> Self {
        Self {
            config,
            returns: VecDeque::new(),
            benchmark_returns: None,
            equity_curve: Vec::new(),
            initial_equity,
            peak_equity: initial_equity,
            max_drawdown: Decimal::ZERO,
            start_time: None,
            end_time: None,
        }
    }

    /// Enables benchmark tracking.
    pub fn enable_benchmark(&mut self) {
        self.benchmark_returns = Some(VecDeque::new());
    }

    /// Records a daily return.
    pub fn record_return(&mut self, daily_return: Decimal) {
        self.returns.push_back(daily_return);
    }

    /// Records a benchmark return.
    pub fn record_benchmark_return(&mut self, benchmark_return: Decimal) {
        if let Some(ref mut benchmark) = self.benchmark_returns {
            benchmark.push_back(benchmark_return);
        }
    }

    /// Updates equity and tracks drawdown.
    pub fn update_equity(&mut self, timestamp: Timestamp, equity: Amount) {
        if self.start_time.is_none() {
            self.start_time = Some(timestamp);
        }
        self.end_time = Some(timestamp);

        // Update peak and drawdown
        if equity > self.peak_equity {
            self.peak_equity = equity;
        }

        if !self.peak_equity.is_zero() {
            let drawdown = (self.peak_equity.as_decimal() - equity.as_decimal())
                / self.peak_equity.as_decimal();
            if drawdown > self.max_drawdown {
                self.max_drawdown = drawdown;
            }
        }

        self.equity_curve.push((timestamp, equity));
    }

    /// Calculates all advanced metrics.
    #[must_use]
    pub fn calculate(&self) -> AdvancedMetrics {
        let mut metrics = AdvancedMetrics::default();

        if self.returns.len() < 2 {
            return metrics;
        }

        // Calculate basic statistics
        let (mean_return, volatility) = self.calculate_mean_and_volatility();
        let downside_dev = self.calculate_downside_deviation();

        // Annualize
        let trading_days = Decimal::from(self.config.trading_days_per_year);
        let annualized_return = mean_return * trading_days;
        let annualized_volatility =
            decimal_sqrt(trading_days).map(|sqrt_days| volatility * sqrt_days);

        metrics.annualized_return = Some(annualized_return);
        metrics.annualized_volatility = annualized_volatility;
        metrics.downside_deviation = downside_dev;

        // Sortino Ratio
        metrics.sortino_ratio = self.calculate_sortino_ratio(annualized_return, downside_dev);

        // Calmar Ratio
        metrics.calmar_ratio = self.calculate_calmar_ratio(annualized_return);

        // Information Ratio (if benchmark available)
        if self.benchmark_returns.is_some() {
            let (active_return, tracking_error) = self.calculate_active_metrics();
            metrics.active_return = active_return;
            metrics.tracking_error = tracking_error;
            metrics.information_ratio =
                self.calculate_information_ratio(active_return, tracking_error);
        }

        // Omega Ratio
        metrics.omega_ratio = self.calculate_omega_ratio();

        // Tail Ratio
        metrics.tail_ratio = self.calculate_tail_ratio();

        metrics
    }

    /// Calculates mean return and volatility.
    fn calculate_mean_and_volatility(&self) -> (Decimal, Decimal) {
        let n = Decimal::from(self.returns.len() as u64);
        let sum: Decimal = self.returns.iter().copied().sum();
        let mean = sum / n;

        let variance: Decimal = self
            .returns
            .iter()
            .map(|r| {
                let diff = *r - mean;
                diff * diff
            })
            .sum::<Decimal>()
            / n;

        let volatility = decimal_sqrt(variance).unwrap_or(Decimal::ZERO);
        (mean, volatility)
    }

    /// Calculates downside deviation (standard deviation of negative returns).
    fn calculate_downside_deviation(&self) -> Option<Decimal> {
        let mar = self.config.minimum_acceptable_return;
        let downside_returns: Vec<Decimal> = self
            .returns
            .iter()
            .filter(|&&r| r < mar)
            .map(|&r| {
                let diff = r - mar;
                diff * diff
            })
            .collect();

        if downside_returns.is_empty() {
            return Some(Decimal::ZERO);
        }

        let n = Decimal::from(self.returns.len() as u64); // Use total count for proper scaling
        let sum: Decimal = downside_returns.iter().copied().sum();
        let downside_variance = sum / n;

        decimal_sqrt(downside_variance)
    }

    /// Calculates Sortino ratio.
    ///
    /// Sortino = (Return - Risk-free rate) / Downside Deviation
    fn calculate_sortino_ratio(
        &self,
        annualized_return: Decimal,
        downside_dev: Option<Decimal>,
    ) -> Option<Decimal> {
        let downside = downside_dev?;
        if downside.is_zero() {
            return None;
        }

        // Annualize downside deviation
        let trading_days = Decimal::from(self.config.trading_days_per_year);
        let annualized_downside = downside * decimal_sqrt(trading_days)?;

        let excess_return = annualized_return - self.config.risk_free_rate;
        Some(excess_return / annualized_downside)
    }

    /// Calculates Calmar ratio.
    ///
    /// Calmar = Annualized Return / Maximum Drawdown
    fn calculate_calmar_ratio(&self, annualized_return: Decimal) -> Option<Decimal> {
        if self.max_drawdown.is_zero() {
            return None;
        }
        Some(annualized_return / self.max_drawdown)
    }

    /// Calculates active return and tracking error relative to benchmark.
    fn calculate_active_metrics(&self) -> (Option<Decimal>, Option<Decimal>) {
        let benchmark = match &self.benchmark_returns {
            Some(b) if !b.is_empty() => b,
            _ => return (None, None),
        };

        let n = self.returns.len().min(benchmark.len());
        if n < 2 {
            return (None, None);
        }

        // Calculate active returns (portfolio return - benchmark return)
        let active_returns: Vec<Decimal> = self
            .returns
            .iter()
            .zip(benchmark.iter())
            .take(n)
            .map(|(r, b)| *r - *b)
            .collect();

        let n_dec = Decimal::from(n as u64);
        let sum: Decimal = active_returns.iter().copied().sum();
        let mean_active = sum / n_dec;

        // Tracking error = std dev of active returns
        let variance: Decimal = active_returns
            .iter()
            .map(|r| {
                let diff = *r - mean_active;
                diff * diff
            })
            .sum::<Decimal>()
            / n_dec;

        let tracking_error = decimal_sqrt(variance);

        // Annualize
        let trading_days = Decimal::from(self.config.trading_days_per_year);
        let annualized_active = mean_active * trading_days;
        let annualized_te =
            tracking_error.map(|te| te * decimal_sqrt(trading_days).unwrap_or(Decimal::ONE));

        (Some(annualized_active), annualized_te)
    }

    /// Calculates Information ratio.
    ///
    /// IR = Active Return / Tracking Error
    fn calculate_information_ratio(
        &self,
        active_return: Option<Decimal>,
        tracking_error: Option<Decimal>,
    ) -> Option<Decimal> {
        let ar = active_return?;
        let te = tracking_error?;
        if te.is_zero() {
            return None;
        }
        Some(ar / te)
    }

    /// Calculates Omega ratio.
    ///
    /// Omega = Sum of returns above threshold / Sum of returns below threshold
    fn calculate_omega_ratio(&self) -> Option<Decimal> {
        let threshold = self.config.omega_threshold;

        let gains: Decimal = self
            .returns
            .iter()
            .filter(|&&r| r > threshold)
            .map(|&r| r - threshold)
            .sum();

        let losses: Decimal = self
            .returns
            .iter()
            .filter(|&&r| r < threshold)
            .map(|&r| threshold - r)
            .sum();

        if losses.is_zero() {
            return None;
        }

        Some(gains / losses)
    }

    /// Calculates tail ratio (95th percentile / 5th percentile absolute value).
    fn calculate_tail_ratio(&self) -> Option<Decimal> {
        if self.returns.len() < 20 {
            return None;
        }

        let mut sorted: Vec<Decimal> = self.returns.iter().copied().collect();
        sorted.sort();

        let n = sorted.len();
        let p5_idx = n * 5 / 100;
        let p95_idx = n * 95 / 100;

        let p5 = sorted.get(p5_idx)?;
        let p95 = sorted.get(p95_idx)?;

        if p5.abs().is_zero() {
            return None;
        }

        Some(p95.abs() / p5.abs())
    }

    /// Returns the maximum drawdown.
    #[must_use]
    pub fn max_drawdown(&self) -> Decimal {
        self.max_drawdown
    }

    /// Returns the number of recorded returns.
    #[must_use]
    pub fn return_count(&self) -> usize {
        self.returns.len()
    }

    /// Resets the calculator.
    pub fn reset(&mut self) {
        self.returns.clear();
        if let Some(ref mut benchmark) = self.benchmark_returns {
            benchmark.clear();
        }
        self.equity_curve.clear();
        self.peak_equity = self.initial_equity;
        self.max_drawdown = Decimal::ZERO;
        self.start_time = None;
        self.end_time = None;
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

    fn create_calculator() -> AdvancedMetricsCalculator {
        let config = AdvancedMetricsConfig::default();
        let initial = Amount::new_unchecked(dec!(100000));
        AdvancedMetricsCalculator::new(config, initial)
    }

    #[test]
    fn test_sortino_ratio_calculation() {
        let mut calc = create_calculator();

        // Add some returns with more negative than positive
        let returns = vec![
            dec!(0.01),
            dec!(-0.02),
            dec!(0.015),
            dec!(-0.01),
            dec!(0.02),
            dec!(-0.005),
            dec!(0.01),
            dec!(-0.015),
            dec!(0.005),
            dec!(-0.01),
        ];

        for r in returns {
            calc.record_return(r);
        }

        let metrics = calc.calculate();

        // Should have a Sortino ratio
        assert!(metrics.sortino_ratio.is_some());
        assert!(metrics.downside_deviation.is_some());
    }

    #[test]
    fn test_calmar_ratio_calculation() {
        let mut calc = create_calculator();

        // Simulate equity curve with drawdown
        let timestamps = [1000i64, 2000, 3000, 4000, 5000];
        let equities = [
            dec!(100000),
            dec!(110000),
            dec!(105000),
            dec!(95000),
            dec!(108000),
        ];

        for (ts, eq) in timestamps.iter().zip(equities.iter()) {
            calc.update_equity(Timestamp::new(*ts).unwrap(), Amount::new_unchecked(*eq));
        }

        // Add returns
        calc.record_return(dec!(0.10));
        calc.record_return(dec!(-0.045));
        calc.record_return(dec!(-0.095));
        calc.record_return(dec!(0.137));

        let metrics = calc.calculate();

        // Max drawdown should be (110000 - 95000) / 110000 ≈ 0.136
        assert!(calc.max_drawdown() > dec!(0.13));
        assert!(calc.max_drawdown() < dec!(0.14));

        // Should have a Calmar ratio
        assert!(metrics.calmar_ratio.is_some());
    }

    #[test]
    fn test_information_ratio_with_benchmark() {
        let mut calc = create_calculator();
        calc.enable_benchmark();

        // Portfolio returns
        let portfolio_returns = vec![dec!(0.02), dec!(-0.01), dec!(0.03), dec!(-0.02), dec!(0.01)];

        // Benchmark returns (slightly different)
        let benchmark_returns = vec![
            dec!(0.015),
            dec!(-0.005),
            dec!(0.025),
            dec!(-0.015),
            dec!(0.005),
        ];

        for (p, b) in portfolio_returns.iter().zip(benchmark_returns.iter()) {
            calc.record_return(*p);
            calc.record_benchmark_return(*b);
        }

        let metrics = calc.calculate();

        // Should have Information ratio components
        assert!(metrics.active_return.is_some());
        assert!(metrics.tracking_error.is_some());
        assert!(metrics.information_ratio.is_some());
    }

    #[test]
    fn test_omega_ratio() {
        let mut calc = create_calculator();

        // Returns with clear gains and losses
        let returns = vec![
            dec!(0.05),
            dec!(-0.02),
            dec!(0.03),
            dec!(-0.01),
            dec!(0.04),
            dec!(-0.03),
        ];

        for r in returns {
            calc.record_return(r);
        }

        let metrics = calc.calculate();

        // Should have Omega ratio
        assert!(metrics.omega_ratio.is_some());
        // Gains > losses, so Omega > 1
        assert!(metrics.omega_ratio.unwrap() > Decimal::ONE);
    }

    #[test]
    fn test_tail_ratio() {
        let mut calc = create_calculator();

        // Need at least 20 returns for tail ratio
        for i in 0..30 {
            let r = if i % 2 == 0 {
                dec!(0.01) * Decimal::from(i as u64 + 1)
            } else {
                dec!(-0.005) * Decimal::from(i as u64 + 1)
            };
            calc.record_return(r);
        }

        let metrics = calc.calculate();

        // Should have tail ratio
        assert!(metrics.tail_ratio.is_some());
    }

    #[test]
    fn test_insufficient_data() {
        let calc = create_calculator();
        let metrics = calc.calculate();

        // With no data, all metrics should be None
        assert!(metrics.sortino_ratio.is_none());
        assert!(metrics.calmar_ratio.is_none());
        assert!(metrics.information_ratio.is_none());
    }

    #[test]
    fn test_reset() {
        let mut calc = create_calculator();

        calc.record_return(dec!(0.01));
        calc.record_return(dec!(-0.02));
        calc.update_equity(
            Timestamp::new(1000).unwrap(),
            Amount::new_unchecked(dec!(101000)),
        );

        assert_eq!(calc.return_count(), 2);

        calc.reset();

        assert_eq!(calc.return_count(), 0);
        assert_eq!(calc.max_drawdown(), Decimal::ZERO);
    }

    #[test]
    fn test_downside_deviation_no_negative_returns() {
        let mut calc = create_calculator();

        // All positive returns
        calc.record_return(dec!(0.01));
        calc.record_return(dec!(0.02));
        calc.record_return(dec!(0.015));

        let metrics = calc.calculate();

        // Downside deviation should be zero
        assert_eq!(metrics.downside_deviation, Some(Decimal::ZERO));
    }

    #[test]
    fn test_decimal_sqrt() {
        assert_eq!(decimal_sqrt(dec!(0)), Some(dec!(0)));
        assert_eq!(decimal_sqrt(dec!(-1)), None);

        let sqrt4 = decimal_sqrt(dec!(4)).unwrap();
        assert!((sqrt4 - dec!(2)).abs() < dec!(0.0001));

        let sqrt2 = decimal_sqrt(dec!(2)).unwrap();
        assert!((sqrt2 - dec!(1.41421356)).abs() < dec!(0.0001));
    }
}
