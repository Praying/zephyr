//! Value at Risk (`VaR`) calculation module.
//!
//! This module provides `VaR` calculation methods:
//! - Historical `VaR`: Based on historical returns distribution
//! - Parametric `VaR`: Based on normal distribution assumption
//!
//! # Example
//!
//! ```
//! use zephyr_risk::var::{VarCalculator, VarConfig, VarMethod};
//! use rust_decimal_macros::dec;
//!
//! let config = VarConfig::default();
//! let calculator = VarCalculator::new(config);
//!
//! let returns = vec![dec!(-0.02), dec!(0.01), dec!(-0.015), dec!(0.03), dec!(-0.01)];
//! let var = calculator.calculate_historical_var(&returns, dec!(0.95));
//! ```

#![allow(clippy::module_name_repetitions)]

use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// `VaR` calculation method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VarMethod {
    /// Historical simulation method
    #[default]
    Historical,
    /// Parametric (variance-covariance) method
    Parametric,
    /// Monte Carlo simulation method
    MonteCarlo,
}

/// `VaR` calculation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarConfig {
    /// Default confidence level (e.g., 0.95 for 95%)
    pub confidence_level: Decimal,
    /// Time horizon in days
    pub time_horizon_days: u32,
    /// Minimum number of observations required
    pub min_observations: usize,
    /// Whether to use exponentially weighted moving average
    pub use_ewma: bool,
    /// EWMA decay factor (lambda)
    pub ewma_lambda: Decimal,
}

impl Default for VarConfig {
    fn default() -> Self {
        Self {
            confidence_level: Decimal::new(95, 2), // 0.95
            time_horizon_days: 1,
            min_observations: 30,
            use_ewma: false,
            ewma_lambda: Decimal::new(94, 2), // 0.94
        }
    }
}

impl VarConfig {
    /// Creates a new `VaR` configuration with the specified confidence level.
    #[must_use]
    pub fn with_confidence(confidence_level: Decimal) -> Self {
        Self {
            confidence_level,
            ..Default::default()
        }
    }

    /// Sets the time horizon in days.
    #[must_use]
    pub const fn with_time_horizon(mut self, days: u32) -> Self {
        self.time_horizon_days = days;
        self
    }

    /// Enables EWMA weighting.
    #[must_use]
    pub fn with_ewma(mut self, lambda: Decimal) -> Self {
        self.use_ewma = true;
        self.ewma_lambda = lambda;
        self
    }
}

/// `VaR` calculation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VarResult {
    /// `VaR` value (positive number representing potential loss)
    pub var: Decimal,
    /// Confidence level used
    pub confidence_level: Decimal,
    /// Time horizon in days
    pub time_horizon_days: u32,
    /// Method used for calculation
    pub method: VarMethod,
    /// Number of observations used
    pub observations: usize,
    /// Expected Shortfall (`CVaR`) if calculated
    pub expected_shortfall: Option<Decimal>,
}

/// `VaR` calculator.
#[derive(Debug, Clone)]
pub struct VarCalculator {
    config: VarConfig,
}

impl VarCalculator {
    /// Creates a new `VaR` calculator with the given configuration.
    #[must_use]
    pub const fn new(config: VarConfig) -> Self {
        Self { config }
    }

    /// Creates a new `VaR` calculator with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(VarConfig::default())
    }

    /// Returns the current configuration.
    #[must_use]
    pub const fn config(&self) -> &VarConfig {
        &self.config
    }

    /// Calculates Historical `VaR` from a series of returns.
    ///
    /// Historical `VaR` uses the empirical distribution of past returns
    /// to estimate potential losses at a given confidence level.
    ///
    /// # Arguments
    ///
    /// * `returns` - Historical returns (as decimals, e.g., -0.02 for -2%)
    /// * `confidence_level` - Confidence level (e.g., 0.95 for 95%)
    ///
    /// # Returns
    ///
    /// `VaR` value as a positive decimal representing potential loss.
    /// Returns `None` if insufficient data.
    #[must_use]
    pub fn calculate_historical_var(
        &self,
        returns: &[Decimal],
        confidence_level: Decimal,
    ) -> Option<VarResult> {
        if returns.len() < self.config.min_observations {
            return None;
        }

        // Sort returns in ascending order
        let mut sorted_returns: Vec<Decimal> = returns.to_vec();
        sorted_returns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));

        // Calculate the index for the percentile
        let n = sorted_returns.len();
        let alpha = Decimal::ONE - confidence_level;
        let index_decimal = alpha * Decimal::from(n);
        let index = index_decimal.to_usize().unwrap_or(0).saturating_sub(1);
        let index = index.min(n.saturating_sub(1));

        // VaR is the negative of the return at the percentile (loss is positive)
        let var = -sorted_returns[index];

        // Calculate Expected Shortfall (average of losses beyond VaR)
        let es = Some(Self::calculate_expected_shortfall(&sorted_returns, index));

        // Scale for time horizon using square root of time rule
        let time_scale = self.time_scale_factor();
        let scaled_var = var * time_scale;
        let scaled_es = es.map(|e| e * time_scale);

        Some(VarResult {
            var: scaled_var,
            confidence_level,
            time_horizon_days: self.config.time_horizon_days,
            method: VarMethod::Historical,
            observations: n,
            expected_shortfall: scaled_es,
        })
    }

    /// Calculates Parametric `VaR` assuming normal distribution.
    ///
    /// Parametric `VaR` uses the mean and standard deviation of returns
    /// with a normal distribution assumption.
    ///
    /// # Arguments
    ///
    /// * `returns` - Historical returns (as decimals)
    /// * `confidence_level` - Confidence level (e.g., 0.95 for 95%)
    ///
    /// # Returns
    ///
    /// `VaR` value as a positive decimal representing potential loss.
    #[must_use]
    pub fn calculate_parametric_var(
        &self,
        returns: &[Decimal],
        confidence_level: Decimal,
    ) -> Option<VarResult> {
        if returns.len() < self.config.min_observations {
            return None;
        }

        let n = returns.len();

        // Calculate mean
        let sum: Decimal = returns.iter().copied().sum();
        let mean = sum / Decimal::from(n);

        // Calculate variance
        let variance = if self.config.use_ewma {
            self.calculate_ewma_variance(returns)
        } else {
            Self::calculate_sample_variance(returns, mean)
        };

        // Calculate standard deviation
        let std_dev = decimal_sqrt(variance)?;

        // Get z-score for confidence level
        let z_score = Self::get_z_score(confidence_level);

        // VaR = -mean + z * std_dev (for loss, we want positive value)
        let var = z_score * std_dev - mean;

        // Scale for time horizon
        let time_scale = self.time_scale_factor();
        let scaled_var = var * time_scale;

        // Calculate Expected Shortfall for normal distribution
        // ES = mean + std_dev * phi(z) / (1 - confidence)
        // where phi is the standard normal PDF
        let es = Self::calculate_parametric_es(mean, std_dev, confidence_level, z_score);

        Some(VarResult {
            var: scaled_var.max(Decimal::ZERO),
            confidence_level,
            time_horizon_days: self.config.time_horizon_days,
            method: VarMethod::Parametric,
            observations: n,
            expected_shortfall: es.map(|e| (e * time_scale).max(Decimal::ZERO)),
        })
    }

    /// Calculates `VaR` using the configured default method.
    #[must_use]
    pub fn calculate_var(&self, returns: &[Decimal]) -> Option<VarResult> {
        self.calculate_historical_var(returns, self.config.confidence_level)
    }

    /// Calculates sample variance.
    fn calculate_sample_variance(returns: &[Decimal], mean: Decimal) -> Decimal {
        let n = returns.len();
        if n <= 1 {
            return Decimal::ZERO;
        }

        let sum_sq: Decimal = returns
            .iter()
            .map(|r| {
                let diff = *r - mean;
                diff * diff
            })
            .sum();
        sum_sq / Decimal::from(n - 1)
    }

    /// Calculates EWMA variance.
    fn calculate_ewma_variance(&self, returns: &[Decimal]) -> Decimal {
        if returns.is_empty() {
            return Decimal::ZERO;
        }

        let lambda = self.config.ewma_lambda;
        let one_minus_lambda = Decimal::ONE - lambda;

        // Initialize with first return squared
        let mut variance = returns[0] * returns[0];

        // Apply EWMA recursively
        for r in returns.iter().skip(1) {
            variance = lambda * variance + one_minus_lambda * (*r * *r);
        }

        variance
    }

    /// Calculates Expected Shortfall from sorted returns.
    fn calculate_expected_shortfall(sorted_returns: &[Decimal], var_index: usize) -> Decimal {
        if var_index == 0 {
            return -sorted_returns[0];
        }

        // Average of all returns worse than VaR
        let tail_returns: Vec<Decimal> = sorted_returns[..=var_index].to_vec();
        let sum: Decimal = tail_returns.iter().copied().sum();
        -sum / Decimal::from(tail_returns.len())
    }

    /// Calculates parametric Expected Shortfall.
    fn calculate_parametric_es(
        mean: Decimal,
        std_dev: Decimal,
        confidence_level: Decimal,
        z_score: Decimal,
    ) -> Option<Decimal> {
        // For normal distribution: ES = mean + std_dev * phi(z) / (1 - confidence)
        // phi(z) is the standard normal PDF at z
        let alpha = Decimal::ONE - confidence_level;
        if alpha.is_zero() {
            return None;
        }

        // Approximate phi(z) = exp(-z^2/2) / sqrt(2*pi)
        let phi = normal_pdf(z_score)?;
        let es = -mean + std_dev * phi / alpha;

        Some(es)
    }

    /// Gets the z-score for a given confidence level.
    fn get_z_score(confidence_level: Decimal) -> Decimal {
        // Common z-scores for standard confidence levels
        // Using lookup table for common values, approximation for others
        let cl_100 = (confidence_level * Decimal::ONE_HUNDRED)
            .round_dp(0)
            .to_i32()
            .unwrap_or(95);

        match cl_100 {
            90 => Decimal::new(1282, 3), // 1.282
            95 => Decimal::new(1645, 3), // 1.645
            99 => Decimal::new(2326, 3), // 2.326
            _ => {
                // Linear interpolation for other values
                if cl_100 < 95 {
                    Decimal::new(1282, 3)
                        + (Decimal::new(1645, 3) - Decimal::new(1282, 3))
                            * (confidence_level - Decimal::new(90, 2))
                            / Decimal::new(5, 2)
                } else {
                    Decimal::new(1645, 3)
                        + (Decimal::new(2326, 3) - Decimal::new(1645, 3))
                            * (confidence_level - Decimal::new(95, 2))
                            / Decimal::new(4, 2)
                }
            }
        }
    }

    /// Calculates the time scaling factor.
    fn time_scale_factor(&self) -> Decimal {
        if self.config.time_horizon_days <= 1 {
            return Decimal::ONE;
        }

        // Square root of time rule
        decimal_sqrt(Decimal::from(self.config.time_horizon_days)).unwrap_or(Decimal::ONE)
    }
}

/// Approximates the square root of a Decimal.
fn decimal_sqrt(value: Decimal) -> Option<Decimal> {
    if value < Decimal::ZERO {
        return None;
    }
    if value.is_zero() {
        return Some(Decimal::ZERO);
    }

    // Newton-Raphson method
    let mut x = value / Decimal::TWO;
    let tolerance = Decimal::new(1, 10); // 0.0000000001

    for _ in 0..50 {
        let x_new = (x + value / x) / Decimal::TWO;
        if (x_new - x).abs() < tolerance {
            return Some(x_new);
        }
        x = x_new;
    }

    Some(x)
}

/// Approximates the standard normal PDF.
fn normal_pdf(z: Decimal) -> Option<Decimal> {
    // phi(z) = exp(-z^2/2) / sqrt(2*pi)
    // sqrt(2*pi) ≈ 2.5066
    let sqrt_2pi = Decimal::new(25066, 4);
    let z_sq = z * z;
    let exp_arg = -z_sq / Decimal::TWO;

    // Approximate exp using Taylor series for small values
    let exp_val = decimal_exp(exp_arg)?;

    Some(exp_val / sqrt_2pi)
}

/// Approximates exp(x) for a Decimal.
fn decimal_exp(x: Decimal) -> Option<Decimal> {
    if x > Decimal::new(10, 0) {
        return None; // Overflow protection
    }

    // Taylor series: exp(x) = 1 + x + x^2/2! + x^3/3! + ...
    let mut result = Decimal::ONE;
    let mut term = Decimal::ONE;

    for i in 1..20 {
        term = term * x / Decimal::from(i);
        result += term;
        if term.abs() < Decimal::new(1, 15) {
            break;
        }
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn sample_returns() -> Vec<Decimal> {
        vec![
            dec!(-0.02),
            dec!(0.01),
            dec!(-0.015),
            dec!(0.03),
            dec!(-0.01),
            dec!(0.02),
            dec!(-0.025),
            dec!(0.015),
            dec!(-0.005),
            dec!(0.01),
            dec!(-0.03),
            dec!(0.025),
            dec!(-0.02),
            dec!(0.01),
            dec!(-0.015),
            dec!(0.02),
            dec!(-0.01),
            dec!(0.015),
            dec!(-0.025),
            dec!(0.03),
            dec!(-0.02),
            dec!(0.01),
            dec!(-0.015),
            dec!(0.025),
            dec!(-0.01),
            dec!(0.02),
            dec!(-0.03),
            dec!(0.015),
            dec!(-0.005),
            dec!(0.01),
        ]
    }

    #[test]
    fn test_var_config_default() {
        let config = VarConfig::default();
        assert_eq!(config.confidence_level, dec!(0.95));
        assert_eq!(config.time_horizon_days, 1);
        assert_eq!(config.min_observations, 30);
    }

    #[test]
    fn test_var_config_builder() {
        let config = VarConfig::with_confidence(dec!(0.99))
            .with_time_horizon(10)
            .with_ewma(dec!(0.94));

        assert_eq!(config.confidence_level, dec!(0.99));
        assert_eq!(config.time_horizon_days, 10);
        assert!(config.use_ewma);
        assert_eq!(config.ewma_lambda, dec!(0.94));
    }

    #[test]
    fn test_historical_var_calculation() {
        let config = VarConfig::default();
        let calculator = VarCalculator::new(config);
        let returns = sample_returns();

        let result = calculator
            .calculate_historical_var(&returns, dec!(0.95))
            .unwrap();

        assert!(result.var > Decimal::ZERO);
        assert_eq!(result.confidence_level, dec!(0.95));
        assert_eq!(result.method, VarMethod::Historical);
        assert_eq!(result.observations, 30);
    }

    #[test]
    fn test_parametric_var_calculation() {
        let config = VarConfig::default();
        let calculator = VarCalculator::new(config);
        let returns = sample_returns();

        let result = calculator
            .calculate_parametric_var(&returns, dec!(0.95))
            .unwrap();

        assert!(result.var >= Decimal::ZERO);
        assert_eq!(result.confidence_level, dec!(0.95));
        assert_eq!(result.method, VarMethod::Parametric);
    }

    #[test]
    fn test_insufficient_data() {
        let config = VarConfig::default();
        let calculator = VarCalculator::new(config);
        let returns = vec![dec!(-0.01), dec!(0.02)]; // Only 2 observations

        let result = calculator.calculate_historical_var(&returns, dec!(0.95));
        assert!(result.is_none());
    }

    #[test]
    fn test_expected_shortfall() {
        let config = VarConfig::default();
        let calculator = VarCalculator::new(config);
        let returns = sample_returns();

        let result = calculator
            .calculate_historical_var(&returns, dec!(0.95))
            .unwrap();

        // ES should be greater than or equal to VaR
        if let Some(es) = result.expected_shortfall {
            assert!(es >= result.var);
        }
    }

    #[test]
    fn test_time_horizon_scaling() {
        let config = VarConfig::with_confidence(dec!(0.95)).with_time_horizon(10);
        let calculator = VarCalculator::new(config);
        let returns = sample_returns();

        let result_10day = calculator
            .calculate_historical_var(&returns, dec!(0.95))
            .unwrap();

        let config_1d = VarConfig::with_confidence(dec!(0.95)).with_time_horizon(1);
        let calculator_1d = VarCalculator::new(config_1d);
        let result_1d = calculator_1d
            .calculate_historical_var(&returns, dec!(0.95))
            .unwrap();

        // 10-day VaR should be approximately sqrt(10) times 1-day VaR
        let ratio = result_10day.var / result_1d.var;
        let expected_ratio = decimal_sqrt(dec!(10)).unwrap();
        let diff = (ratio - expected_ratio).abs();
        assert!(diff < dec!(0.01));
    }

    #[test]
    fn test_ewma_variance() {
        let config = VarConfig::with_confidence(dec!(0.95)).with_ewma(dec!(0.94));
        let calculator = VarCalculator::new(config);
        let returns = sample_returns();

        let result = calculator
            .calculate_parametric_var(&returns, dec!(0.95))
            .unwrap();

        assert!(result.var >= Decimal::ZERO);
    }

    #[test]
    fn test_decimal_sqrt() {
        let sqrt_4 = decimal_sqrt(dec!(4)).unwrap();
        assert!((sqrt_4 - dec!(2)).abs() < dec!(0.0001));

        let sqrt_2 = decimal_sqrt(dec!(2)).unwrap();
        assert!((sqrt_2 - dec!(1.4142)).abs() < dec!(0.001));
    }

    #[test]
    fn test_z_scores() {
        let config = VarConfig::default();
        let _calculator = VarCalculator::new(config);

        let z_90 = VarCalculator::get_z_score(dec!(0.90));
        assert!((z_90 - dec!(1.282)).abs() < dec!(0.001));

        let z_95 = VarCalculator::get_z_score(dec!(0.95));
        assert!((z_95 - dec!(1.645)).abs() < dec!(0.001));

        let z_99 = VarCalculator::get_z_score(dec!(0.99));
        assert!((z_99 - dec!(2.326)).abs() < dec!(0.001));
    }

    #[test]
    fn test_var_result_serde() {
        let result = VarResult {
            var: dec!(0.025),
            confidence_level: dec!(0.95),
            time_horizon_days: 1,
            method: VarMethod::Historical,
            observations: 100,
            expected_shortfall: Some(dec!(0.035)),
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: VarResult = serde_json::from_str(&json).unwrap();

        assert_eq!(result.var, parsed.var);
        assert_eq!(result.method, parsed.method);
    }
}
