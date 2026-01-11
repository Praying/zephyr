//! Monte Carlo simulation for strategy robustness testing.
//!
//! Provides Monte Carlo simulation capabilities:
//! - Random path generation from historical returns
//! - Bootstrap resampling of trade sequences
//! - Confidence interval estimation
//! - Robustness testing across multiple scenarios

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use zephyr_core::types::Amount;

/// Monte Carlo simulation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonteCarloConfig {
    /// Number of simulation paths to generate
    #[serde(default = "default_num_simulations")]
    pub num_simulations: usize,
    /// Random seed for reproducibility (None for random)
    pub seed: Option<u64>,
    /// Confidence level for interval estimation (e.g., 0.95 for 95%)
    #[serde(default = "default_confidence_level")]
    pub confidence_level: Decimal,
    /// Whether to use bootstrap resampling (vs parametric)
    #[serde(default = "default_use_bootstrap")]
    pub use_bootstrap: bool,
    /// Block size for block bootstrap (preserves autocorrelation)
    #[serde(default = "default_block_size")]
    pub block_size: usize,
}

fn default_num_simulations() -> usize {
    1000
}

fn default_confidence_level() -> Decimal {
    dec!(0.95)
}

fn default_use_bootstrap() -> bool {
    true
}

fn default_block_size() -> usize {
    5
}

impl Default for MonteCarloConfig {
    fn default() -> Self {
        Self {
            num_simulations: 1000,
            seed: None,
            confidence_level: dec!(0.95),
            use_bootstrap: true,
            block_size: 5,
        }
    }
}

/// Results from a single simulation path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationPath {
    /// Final equity
    pub final_equity: Amount,
    /// Total return
    pub total_return: Decimal,
    /// Maximum drawdown
    pub max_drawdown: Decimal,
    /// Sharpe ratio (if calculable)
    pub sharpe_ratio: Option<Decimal>,
    /// Number of trades
    pub num_trades: usize,
    /// Win rate
    pub win_rate: Decimal,
}

/// Monte Carlo simulation results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonteCarloResults {
    /// Number of simulations run
    pub num_simulations: usize,
    /// Mean final equity
    pub mean_final_equity: Amount,
    /// Median final equity
    pub median_final_equity: Amount,
    /// Standard deviation of final equity
    pub std_final_equity: Amount,
    /// Mean total return
    pub mean_return: Decimal,
    /// Median total return
    pub median_return: Decimal,
    /// Mean maximum drawdown
    pub mean_max_drawdown: Decimal,
    /// Worst case maximum drawdown
    pub worst_max_drawdown: Decimal,
    /// Probability of profit
    pub probability_of_profit: Decimal,
    /// Value at Risk (VaR) at confidence level
    pub var: Decimal,
    /// Conditional VaR (Expected Shortfall)
    pub cvar: Decimal,
    /// Lower confidence bound for return
    pub return_lower_bound: Decimal,
    /// Upper confidence bound for return
    pub return_upper_bound: Decimal,
    /// Individual simulation paths (optional, for detailed analysis)
    pub paths: Option<Vec<SimulationPath>>,
}

/// Simple linear congruential generator for deterministic randomness.
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        // LCG parameters from Numerical Recipes
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        self.state
    }

    fn next_usize(&mut self, max: usize) -> usize {
        (self.next_u64() as usize) % max
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() as f64) / (u64::MAX as f64)
    }
}

/// Monte Carlo simulator.
pub struct MonteCarloSimulator {
    config: MonteCarloConfig,
    /// Historical returns for resampling
    returns: Vec<Decimal>,
    /// Historical trade PnLs for bootstrap
    trade_pnls: Vec<Decimal>,
    /// Initial equity
    initial_equity: Amount,
    /// Random number generator
    rng: SimpleRng,
}

impl MonteCarloSimulator {
    /// Creates a new Monte Carlo simulator.
    #[must_use]
    pub fn new(config: MonteCarloConfig, initial_equity: Amount) -> Self {
        let seed = config.seed.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(12345)
        });

        Self {
            config,
            returns: Vec::new(),
            trade_pnls: Vec::new(),
            initial_equity,
            rng: SimpleRng::new(seed),
        }
    }

    /// Adds historical returns for simulation.
    pub fn add_returns(&mut self, returns: &[Decimal]) {
        self.returns.extend_from_slice(returns);
    }

    /// Adds historical trade PnLs for bootstrap.
    pub fn add_trade_pnls(&mut self, pnls: &[Decimal]) {
        self.trade_pnls.extend_from_slice(pnls);
    }

    /// Runs the Monte Carlo simulation.
    ///
    /// # Errors
    ///
    /// Returns an error if there is insufficient data for simulation.
    pub fn run(&mut self) -> Result<MonteCarloResults, MonteCarloError> {
        if self.returns.is_empty() && self.trade_pnls.is_empty() {
            return Err(MonteCarloError::InsufficientData);
        }

        let mut paths = Vec::with_capacity(self.config.num_simulations);

        for _ in 0..self.config.num_simulations {
            let path = if self.config.use_bootstrap {
                self.run_bootstrap_simulation()?
            } else {
                self.run_parametric_simulation()?
            };
            paths.push(path);
        }

        Ok(self.calculate_results(paths))
    }

    /// Runs a single bootstrap simulation.
    fn run_bootstrap_simulation(&mut self) -> Result<SimulationPath, MonteCarloError> {
        if self.trade_pnls.is_empty() {
            self.bootstrap_returns()
        } else {
            self.bootstrap_trades()
        }
    }

    /// Bootstrap resampling of trades.
    fn bootstrap_trades(&mut self) -> Result<SimulationPath, MonteCarloError> {
        let n = self.trade_pnls.len();
        if n == 0 {
            return Err(MonteCarloError::InsufficientData);
        }

        let mut equity = self.initial_equity.as_decimal();
        let mut peak = equity;
        let mut max_drawdown = Decimal::ZERO;
        let mut wins = 0usize;
        let mut total_trades = 0usize;

        // Resample trades with replacement
        let num_trades = n; // Same number of trades as original
        for _ in 0..num_trades {
            let idx = self.rng.next_usize(n);
            let pnl = self.trade_pnls[idx];

            equity += pnl;
            total_trades += 1;

            if pnl > Decimal::ZERO {
                wins += 1;
            }

            if equity > peak {
                peak = equity;
            }

            let drawdown = if peak > Decimal::ZERO {
                (peak - equity) / peak
            } else {
                Decimal::ZERO
            };

            if drawdown > max_drawdown {
                max_drawdown = drawdown;
            }
        }

        let total_return = if self.initial_equity.is_zero() {
            Decimal::ZERO
        } else {
            (equity - self.initial_equity.as_decimal()) / self.initial_equity.as_decimal()
        };

        let win_rate = if total_trades > 0 {
            Decimal::from(wins as u64) / Decimal::from(total_trades as u64)
        } else {
            Decimal::ZERO
        };

        Ok(SimulationPath {
            final_equity: Amount::new_unchecked(equity),
            total_return,
            max_drawdown,
            sharpe_ratio: None,
            num_trades: total_trades,
            win_rate,
        })
    }

    /// Bootstrap resampling of returns (block bootstrap).
    fn bootstrap_returns(&mut self) -> Result<SimulationPath, MonteCarloError> {
        let n = self.returns.len();
        if n == 0 {
            return Err(MonteCarloError::InsufficientData);
        }

        let block_size = self.config.block_size.min(n);
        let num_blocks = n.div_ceil(block_size);

        let mut equity = self.initial_equity.as_decimal();
        let mut peak = equity;
        let mut max_drawdown = Decimal::ZERO;
        let mut sampled_returns = Vec::new();

        // Block bootstrap
        for _ in 0..num_blocks {
            let start_idx = self.rng.next_usize(n.saturating_sub(block_size) + 1);
            for i in 0..block_size {
                if start_idx + i < n {
                    sampled_returns.push(self.returns[start_idx + i]);
                }
            }
        }

        // Simulate equity path
        for ret in &sampled_returns {
            equity *= Decimal::ONE + *ret;

            if equity > peak {
                peak = equity;
            }

            let drawdown = if peak > Decimal::ZERO {
                (peak - equity) / peak
            } else {
                Decimal::ZERO
            };

            if drawdown > max_drawdown {
                max_drawdown = drawdown;
            }
        }

        let total_return = if self.initial_equity.is_zero() {
            Decimal::ZERO
        } else {
            (equity - self.initial_equity.as_decimal()) / self.initial_equity.as_decimal()
        };

        // Calculate Sharpe ratio from sampled returns
        let sharpe = self.calculate_sharpe(&sampled_returns);

        Ok(SimulationPath {
            final_equity: Amount::new_unchecked(equity),
            total_return,
            max_drawdown,
            sharpe_ratio: sharpe,
            num_trades: sampled_returns.len(),
            win_rate: Decimal::ZERO, // Not applicable for returns
        })
    }

    /// Runs a parametric simulation using normal distribution.
    fn run_parametric_simulation(&mut self) -> Result<SimulationPath, MonteCarloError> {
        if self.returns.is_empty() {
            return Err(MonteCarloError::InsufficientData);
        }

        // Calculate mean and std of returns
        let n = Decimal::from(self.returns.len() as u64);
        let mean: Decimal = self.returns.iter().copied().sum::<Decimal>() / n;

        let variance: Decimal = self
            .returns
            .iter()
            .map(|r| {
                let diff = *r - mean;
                diff * diff
            })
            .sum::<Decimal>()
            / n;

        let std = decimal_sqrt(variance).unwrap_or(Decimal::ZERO);

        // Generate random returns using Box-Muller transform
        let mut equity = self.initial_equity.as_decimal();
        let mut peak = equity;
        let mut max_drawdown = Decimal::ZERO;
        let mut sampled_returns = Vec::new();

        let num_periods = self.returns.len();
        for _ in 0..num_periods {
            // Box-Muller transform for normal distribution
            let u1 = self.rng.next_f64().max(1e-10);
            let u2 = self.rng.next_f64();
            let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();

            let ret = mean + std * Decimal::try_from(z).unwrap_or(Decimal::ZERO);
            sampled_returns.push(ret);

            equity *= Decimal::ONE + ret;

            if equity > peak {
                peak = equity;
            }

            let drawdown = if peak > Decimal::ZERO {
                (peak - equity) / peak
            } else {
                Decimal::ZERO
            };

            if drawdown > max_drawdown {
                max_drawdown = drawdown;
            }
        }

        let total_return = if self.initial_equity.is_zero() {
            Decimal::ZERO
        } else {
            (equity - self.initial_equity.as_decimal()) / self.initial_equity.as_decimal()
        };

        let sharpe = self.calculate_sharpe(&sampled_returns);

        Ok(SimulationPath {
            final_equity: Amount::new_unchecked(equity),
            total_return,
            max_drawdown,
            sharpe_ratio: sharpe,
            num_trades: num_periods,
            win_rate: Decimal::ZERO,
        })
    }

    /// Calculates Sharpe ratio from returns.
    fn calculate_sharpe(&self, returns: &[Decimal]) -> Option<Decimal> {
        if returns.len() < 2 {
            return None;
        }

        let n = Decimal::from(returns.len() as u64);
        let mean: Decimal = returns.iter().copied().sum::<Decimal>() / n;

        let variance: Decimal = returns
            .iter()
            .map(|r| {
                let diff = *r - mean;
                diff * diff
            })
            .sum::<Decimal>()
            / n;

        let std = decimal_sqrt(variance)?;
        if std.is_zero() {
            return None;
        }

        // Annualize (assuming daily returns, 365 days for crypto)
        let annualized_return = mean * dec!(365);
        let annualized_std = std * decimal_sqrt(dec!(365))?;

        Some(annualized_return / annualized_std)
    }

    /// Calculates aggregate results from simulation paths.
    fn calculate_results(&self, mut paths: Vec<SimulationPath>) -> MonteCarloResults {
        let n = paths.len();
        if n == 0 {
            return MonteCarloResults {
                num_simulations: 0,
                mean_final_equity: Amount::ZERO,
                median_final_equity: Amount::ZERO,
                std_final_equity: Amount::ZERO,
                mean_return: Decimal::ZERO,
                median_return: Decimal::ZERO,
                mean_max_drawdown: Decimal::ZERO,
                worst_max_drawdown: Decimal::ZERO,
                probability_of_profit: Decimal::ZERO,
                var: Decimal::ZERO,
                cvar: Decimal::ZERO,
                return_lower_bound: Decimal::ZERO,
                return_upper_bound: Decimal::ZERO,
                paths: None,
            };
        }

        // Sort paths by return for percentile calculations
        paths.sort_by(|a, b| a.total_return.partial_cmp(&b.total_return).unwrap());

        let n_dec = Decimal::from(n as u64);

        // Calculate means
        let mean_equity: Decimal = paths
            .iter()
            .map(|p| p.final_equity.as_decimal())
            .sum::<Decimal>()
            / n_dec;
        let mean_return: Decimal = paths.iter().map(|p| p.total_return).sum::<Decimal>() / n_dec;
        let mean_drawdown: Decimal = paths.iter().map(|p| p.max_drawdown).sum::<Decimal>() / n_dec;

        // Calculate medians
        let median_idx = n / 2;
        let median_equity = paths[median_idx].final_equity;
        let median_return = paths[median_idx].total_return;

        // Calculate standard deviation of equity
        let equity_variance: Decimal = paths
            .iter()
            .map(|p| {
                let diff = p.final_equity.as_decimal() - mean_equity;
                diff * diff
            })
            .sum::<Decimal>()
            / n_dec;
        let std_equity = decimal_sqrt(equity_variance).unwrap_or(Decimal::ZERO);

        // Worst drawdown
        let worst_drawdown = paths
            .iter()
            .map(|p| p.max_drawdown)
            .max()
            .unwrap_or(Decimal::ZERO);

        // Probability of profit
        let profitable = paths
            .iter()
            .filter(|p| p.total_return > Decimal::ZERO)
            .count();
        let prob_profit = Decimal::from(profitable as u64) / n_dec;

        // VaR and CVaR at confidence level
        let var_idx = ((Decimal::ONE - self.config.confidence_level) * n_dec)
            .to_string()
            .parse::<usize>()
            .unwrap_or(0);
        let var = -paths[var_idx].total_return; // VaR is typically positive

        // CVaR (Expected Shortfall) - average of returns below VaR
        let cvar_returns: Decimal = paths[..=var_idx].iter().map(|p| p.total_return).sum();
        let cvar = if var_idx > 0 {
            -cvar_returns / Decimal::from((var_idx + 1) as u64)
        } else {
            var
        };

        // Confidence bounds
        let lower_idx = ((Decimal::ONE - self.config.confidence_level) / dec!(2) * n_dec)
            .to_string()
            .parse::<usize>()
            .unwrap_or(0);
        let upper_idx = n.saturating_sub(lower_idx + 1);

        let return_lower = paths[lower_idx].total_return;
        let return_upper = paths[upper_idx].total_return;

        MonteCarloResults {
            num_simulations: n,
            mean_final_equity: Amount::new_unchecked(mean_equity),
            median_final_equity: median_equity,
            std_final_equity: Amount::new_unchecked(std_equity),
            mean_return,
            median_return,
            mean_max_drawdown: mean_drawdown,
            worst_max_drawdown: worst_drawdown,
            probability_of_profit: prob_profit,
            var,
            cvar,
            return_lower_bound: return_lower,
            return_upper_bound: return_upper,
            paths: Some(paths),
        }
    }

    /// Resets the simulator.
    pub fn reset(&mut self) {
        self.returns.clear();
        self.trade_pnls.clear();
    }
}

/// Monte Carlo simulation error.
#[derive(Debug, Clone, thiserror::Error)]
pub enum MonteCarloError {
    /// Insufficient data for simulation
    #[error("insufficient data for Monte Carlo simulation")]
    InsufficientData,
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

    fn create_simulator() -> MonteCarloSimulator {
        let config = MonteCarloConfig {
            num_simulations: 100,
            seed: Some(12345),
            ..Default::default()
        };
        let initial = Amount::new_unchecked(dec!(100000));
        MonteCarloSimulator::new(config, initial)
    }

    #[test]
    fn test_monte_carlo_with_returns() {
        let mut sim = create_simulator();

        // Add some historical returns
        let returns = vec![
            dec!(0.01),
            dec!(-0.005),
            dec!(0.02),
            dec!(-0.01),
            dec!(0.015),
            dec!(-0.008),
            dec!(0.012),
            dec!(-0.003),
            dec!(0.018),
            dec!(-0.007),
        ];

        sim.add_returns(&returns);

        let results = sim.run().unwrap();

        assert_eq!(results.num_simulations, 100);
        assert!(results.mean_final_equity.as_decimal() > Decimal::ZERO);
        assert!(results.probability_of_profit >= Decimal::ZERO);
        assert!(results.probability_of_profit <= Decimal::ONE);
    }

    #[test]
    fn test_monte_carlo_with_trades() {
        let mut sim = create_simulator();

        // Add some historical trade PnLs
        let pnls = vec![
            dec!(500),
            dec!(-200),
            dec!(800),
            dec!(-300),
            dec!(600),
            dec!(-150),
            dec!(400),
            dec!(-100),
            dec!(700),
            dec!(-250),
        ];

        sim.add_trade_pnls(&pnls);

        let results = sim.run().unwrap();

        assert_eq!(results.num_simulations, 100);
        assert!(results.mean_final_equity.as_decimal() > Decimal::ZERO);
    }

    #[test]
    fn test_monte_carlo_deterministic() {
        let config = MonteCarloConfig {
            num_simulations: 50,
            seed: Some(42),
            ..Default::default()
        };

        let returns = vec![dec!(0.01), dec!(-0.005), dec!(0.02), dec!(-0.01)];

        let mut sim1 =
            MonteCarloSimulator::new(config.clone(), Amount::new_unchecked(dec!(100000)));
        sim1.add_returns(&returns);
        let results1 = sim1.run().unwrap();

        let mut sim2 = MonteCarloSimulator::new(config, Amount::new_unchecked(dec!(100000)));
        sim2.add_returns(&returns);
        let results2 = sim2.run().unwrap();

        // Same seed should produce same results
        assert_eq!(results1.mean_return, results2.mean_return);
        assert_eq!(results1.mean_final_equity, results2.mean_final_equity);
    }

    #[test]
    fn test_monte_carlo_insufficient_data() {
        let mut sim = create_simulator();
        let result = sim.run();
        assert!(matches!(result, Err(MonteCarloError::InsufficientData)));
    }

    #[test]
    fn test_monte_carlo_parametric() {
        let config = MonteCarloConfig {
            num_simulations: 100,
            seed: Some(12345),
            use_bootstrap: false,
            ..Default::default()
        };

        let mut sim = MonteCarloSimulator::new(config, Amount::new_unchecked(dec!(100000)));

        let returns = vec![
            dec!(0.01),
            dec!(-0.005),
            dec!(0.02),
            dec!(-0.01),
            dec!(0.015),
        ];

        sim.add_returns(&returns);

        let results = sim.run().unwrap();

        assert_eq!(results.num_simulations, 100);
        // VaR can be any value (positive, negative, or zero)
    }

    #[test]
    fn test_monte_carlo_var_cvar() {
        let mut sim = create_simulator();

        // Add returns with clear distribution
        let returns: Vec<Decimal> = (0..50)
            .map(|i| if i % 3 == 0 { dec!(-0.02) } else { dec!(0.01) })
            .collect();

        sim.add_returns(&returns);

        let results = sim.run().unwrap();

        // CVaR should be >= VaR (both are positive for losses)
        assert!(results.cvar >= results.var || results.cvar.abs() >= results.var.abs());
    }

    #[test]
    fn test_monte_carlo_confidence_bounds() {
        let mut sim = create_simulator();

        let returns = vec![
            dec!(0.01),
            dec!(-0.005),
            dec!(0.02),
            dec!(-0.01),
            dec!(0.015),
            dec!(-0.008),
            dec!(0.012),
            dec!(-0.003),
        ];

        sim.add_returns(&returns);

        let results = sim.run().unwrap();

        // Lower bound should be <= median <= upper bound
        assert!(results.return_lower_bound <= results.median_return);
        assert!(results.median_return <= results.return_upper_bound);
    }

    #[test]
    fn test_monte_carlo_reset() {
        let mut sim = create_simulator();

        sim.add_returns(&[dec!(0.01), dec!(-0.005)]);
        sim.add_trade_pnls(&[dec!(100), dec!(-50)]);

        sim.reset();

        let result = sim.run();
        assert!(matches!(result, Err(MonteCarloError::InsufficientData)));
    }
}
