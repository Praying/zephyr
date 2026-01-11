//! Walk-Forward optimization for strategy validation.
//!
//! Provides walk-forward analysis capabilities:
//! - Rolling window testing with in-sample/out-of-sample splits
//! - Out-of-sample validation for strategy robustness
//! - Parameter stability analysis across windows
//! - Anchored and rolling walk-forward modes

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use zephyr_core::types::{Amount, Timestamp};

/// Walk-forward optimization configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardConfig {
    /// Total data start timestamp
    pub start_time: Timestamp,
    /// Total data end timestamp
    pub end_time: Timestamp,
    /// In-sample window size in milliseconds
    pub in_sample_size_ms: i64,
    /// Out-of-sample window size in milliseconds
    pub out_of_sample_size_ms: i64,
    /// Step size for rolling windows in milliseconds (0 = non-overlapping)
    #[serde(default)]
    pub step_size_ms: i64,
    /// Whether to use anchored walk-forward (expanding in-sample window)
    #[serde(default)]
    pub anchored: bool,
    /// Minimum number of trades required in each window
    #[serde(default = "default_min_trades")]
    pub min_trades_per_window: usize,
}

fn default_min_trades() -> usize {
    10
}

impl WalkForwardConfig {
    /// Creates a new walk-forward configuration.
    #[must_use]
    pub fn new(
        start_time: Timestamp,
        end_time: Timestamp,
        in_sample_size_ms: i64,
        out_of_sample_size_ms: i64,
    ) -> Self {
        Self {
            start_time,
            end_time,
            in_sample_size_ms,
            out_of_sample_size_ms,
            step_size_ms: 0,
            anchored: false,
            min_trades_per_window: default_min_trades(),
        }
    }

    /// Sets the step size for rolling windows.
    #[must_use]
    pub fn with_step_size(mut self, step_size_ms: i64) -> Self {
        self.step_size_ms = step_size_ms;
        self
    }

    /// Enables anchored walk-forward mode.
    #[must_use]
    pub fn with_anchored(mut self, anchored: bool) -> Self {
        self.anchored = anchored;
        self
    }

    /// Sets minimum trades per window.
    #[must_use]
    pub fn with_min_trades(mut self, min_trades: usize) -> Self {
        self.min_trades_per_window = min_trades;
        self
    }

    /// Validates the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    pub fn validate(&self) -> Result<(), WalkForwardError> {
        if self.end_time <= self.start_time {
            return Err(WalkForwardError::InvalidTimeRange);
        }

        if self.in_sample_size_ms <= 0 {
            return Err(WalkForwardError::InvalidWindowSize(
                "in-sample size must be positive".to_string(),
            ));
        }

        if self.out_of_sample_size_ms <= 0 {
            return Err(WalkForwardError::InvalidWindowSize(
                "out-of-sample size must be positive".to_string(),
            ));
        }

        let total_duration = self.end_time.as_millis() - self.start_time.as_millis();
        let min_required = self.in_sample_size_ms + self.out_of_sample_size_ms;

        if total_duration < min_required {
            return Err(WalkForwardError::InsufficientData);
        }

        Ok(())
    }
}

/// A single walk-forward window definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardWindow {
    /// Window index (0-based)
    pub index: usize,
    /// In-sample period start
    pub in_sample_start: Timestamp,
    /// In-sample period end
    pub in_sample_end: Timestamp,
    /// Out-of-sample period start
    pub out_of_sample_start: Timestamp,
    /// Out-of-sample period end
    pub out_of_sample_end: Timestamp,
}

impl WalkForwardWindow {
    /// Returns the in-sample duration in milliseconds.
    #[must_use]
    pub fn in_sample_duration_ms(&self) -> i64 {
        self.in_sample_end.as_millis() - self.in_sample_start.as_millis()
    }

    /// Returns the out-of-sample duration in milliseconds.
    #[must_use]
    pub fn out_of_sample_duration_ms(&self) -> i64 {
        self.out_of_sample_end.as_millis() - self.out_of_sample_start.as_millis()
    }
}

/// Results from a single walk-forward window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowResult {
    /// Window definition
    pub window: WalkForwardWindow,
    /// In-sample performance metrics
    pub in_sample_metrics: WindowMetrics,
    /// Out-of-sample performance metrics
    pub out_of_sample_metrics: WindowMetrics,
    /// Efficiency ratio (OOS return / IS return)
    pub efficiency_ratio: Option<Decimal>,
    /// Whether this window passed validation
    pub passed: bool,
}

/// Performance metrics for a window period.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowMetrics {
    /// Total return percentage
    pub total_return: Decimal,
    /// Number of trades
    pub num_trades: usize,
    /// Win rate
    pub win_rate: Decimal,
    /// Maximum drawdown percentage
    pub max_drawdown: Decimal,
    /// Sharpe ratio (if calculable)
    pub sharpe_ratio: Option<Decimal>,
    /// Profit factor
    pub profit_factor: Option<Decimal>,
    /// Final equity
    pub final_equity: Amount,
}

/// Aggregate results from walk-forward optimization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardResults {
    /// Configuration used
    pub config: WalkForwardConfig,
    /// Number of windows tested
    pub num_windows: usize,
    /// Number of windows that passed validation
    pub num_passed: usize,
    /// Individual window results
    pub window_results: Vec<WindowResult>,
    /// Aggregate in-sample metrics
    pub aggregate_in_sample: AggregateMetrics,
    /// Aggregate out-of-sample metrics
    pub aggregate_out_of_sample: AggregateMetrics,
    /// Overall efficiency ratio
    pub overall_efficiency: Decimal,
    /// Walk-forward efficiency (% of windows where OOS > 0)
    pub walk_forward_efficiency: Decimal,
    /// Parameter stability score (0-1)
    pub stability_score: Decimal,
    /// Robustness score (0-1)
    pub robustness_score: Decimal,
}

/// Aggregate metrics across all windows.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregateMetrics {
    /// Mean return
    pub mean_return: Decimal,
    /// Median return
    pub median_return: Decimal,
    /// Standard deviation of returns
    pub std_return: Decimal,
    /// Mean win rate
    pub mean_win_rate: Decimal,
    /// Mean max drawdown
    pub mean_max_drawdown: Decimal,
    /// Worst max drawdown
    pub worst_max_drawdown: Decimal,
    /// Total trades across all windows
    pub total_trades: usize,
}

/// Walk-forward optimizer.
pub struct WalkForwardOptimizer {
    config: WalkForwardConfig,
    windows: Vec<WalkForwardWindow>,
}

impl WalkForwardOptimizer {
    /// Creates a new walk-forward optimizer.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    pub fn new(config: WalkForwardConfig) -> Result<Self, WalkForwardError> {
        config.validate()?;

        let windows = Self::generate_windows(&config);
        if windows.is_empty() {
            return Err(WalkForwardError::InsufficientData);
        }

        Ok(Self { config, windows })
    }

    /// Generates walk-forward windows based on configuration.
    fn generate_windows(config: &WalkForwardConfig) -> Vec<WalkForwardWindow> {
        let mut windows = Vec::new();
        let mut index = 0;

        let step = if config.step_size_ms > 0 {
            config.step_size_ms
        } else {
            config.out_of_sample_size_ms
        };

        let mut current_start = config.start_time.as_millis();

        loop {
            let in_sample_start = if config.anchored {
                config.start_time.as_millis()
            } else {
                current_start
            };

            let in_sample_end = current_start + config.in_sample_size_ms;
            let oos_start = in_sample_end;
            let oos_end = oos_start + config.out_of_sample_size_ms;

            // Check if we've exceeded the data range
            if oos_end > config.end_time.as_millis() {
                break;
            }

            if let (Ok(is_start), Ok(is_end), Ok(oos_s), Ok(oos_e)) = (
                Timestamp::new(in_sample_start),
                Timestamp::new(in_sample_end),
                Timestamp::new(oos_start),
                Timestamp::new(oos_end),
            ) {
                windows.push(WalkForwardWindow {
                    index,
                    in_sample_start: is_start,
                    in_sample_end: is_end,
                    out_of_sample_start: oos_s,
                    out_of_sample_end: oos_e,
                });
                index += 1;
            }

            current_start += step;
        }

        windows
    }

    /// Returns the generated windows.
    #[must_use]
    pub fn windows(&self) -> &[WalkForwardWindow] {
        &self.windows
    }

    /// Returns the number of windows.
    #[must_use]
    pub fn num_windows(&self) -> usize {
        self.windows.len()
    }

    /// Calculates aggregate results from window results.
    #[must_use]
    pub fn calculate_results(&self, window_results: Vec<WindowResult>) -> WalkForwardResults {
        let num_windows = window_results.len();
        let num_passed = window_results.iter().filter(|w| w.passed).count();

        // Calculate aggregate in-sample metrics
        let aggregate_in_sample = self.calculate_aggregate(&window_results, true);

        // Calculate aggregate out-of-sample metrics
        let aggregate_out_of_sample = self.calculate_aggregate(&window_results, false);

        // Overall efficiency ratio
        let overall_efficiency = if aggregate_in_sample.mean_return.is_zero() {
            Decimal::ZERO
        } else {
            aggregate_out_of_sample.mean_return / aggregate_in_sample.mean_return
        };

        // Walk-forward efficiency (% of windows with positive OOS return)
        let positive_oos = window_results
            .iter()
            .filter(|w| w.out_of_sample_metrics.total_return > Decimal::ZERO)
            .count();
        let walk_forward_efficiency = if num_windows > 0 {
            Decimal::from(positive_oos as u64) / Decimal::from(num_windows as u64)
        } else {
            Decimal::ZERO
        };

        // Stability score based on return consistency
        let stability_score = self.calculate_stability_score(&window_results);

        // Robustness score combining multiple factors
        let robustness_score = self.calculate_robustness_score(
            walk_forward_efficiency,
            overall_efficiency,
            stability_score,
        );

        WalkForwardResults {
            config: self.config.clone(),
            num_windows,
            num_passed,
            window_results,
            aggregate_in_sample,
            aggregate_out_of_sample,
            overall_efficiency,
            walk_forward_efficiency,
            stability_score,
            robustness_score,
        }
    }

    /// Calculates aggregate metrics from window results.
    fn calculate_aggregate(&self, results: &[WindowResult], in_sample: bool) -> AggregateMetrics {
        if results.is_empty() {
            return AggregateMetrics::default();
        }

        let metrics: Vec<&WindowMetrics> = results
            .iter()
            .map(|r| {
                if in_sample {
                    &r.in_sample_metrics
                } else {
                    &r.out_of_sample_metrics
                }
            })
            .collect();

        let n = Decimal::from(metrics.len() as u64);

        // Calculate returns
        let mut returns: Vec<Decimal> = metrics.iter().map(|m| m.total_return).collect();
        returns.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mean_return: Decimal = returns.iter().copied().sum::<Decimal>() / n;
        let median_return = returns[returns.len() / 2];

        // Standard deviation
        let variance: Decimal = returns
            .iter()
            .map(|r| {
                let diff = *r - mean_return;
                diff * diff
            })
            .sum::<Decimal>()
            / n;
        let std_return = decimal_sqrt(variance).unwrap_or(Decimal::ZERO);

        // Other aggregates
        let mean_win_rate: Decimal = metrics.iter().map(|m| m.win_rate).sum::<Decimal>() / n;
        let mean_max_drawdown: Decimal =
            metrics.iter().map(|m| m.max_drawdown).sum::<Decimal>() / n;
        let worst_max_drawdown = metrics
            .iter()
            .map(|m| m.max_drawdown)
            .max()
            .unwrap_or(Decimal::ZERO);
        let total_trades: usize = metrics.iter().map(|m| m.num_trades).sum();

        AggregateMetrics {
            mean_return,
            median_return,
            std_return,
            mean_win_rate,
            mean_max_drawdown,
            worst_max_drawdown,
            total_trades,
        }
    }

    /// Calculates stability score based on return consistency.
    fn calculate_stability_score(&self, results: &[WindowResult]) -> Decimal {
        if results.len() < 2 {
            return Decimal::ONE;
        }

        let returns: Vec<Decimal> = results
            .iter()
            .map(|r| r.out_of_sample_metrics.total_return)
            .collect();

        let n = Decimal::from(returns.len() as u64);
        let mean: Decimal = returns.iter().copied().sum::<Decimal>() / n;

        if mean.is_zero() {
            return Decimal::ZERO;
        }

        let variance: Decimal = returns
            .iter()
            .map(|r| {
                let diff = *r - mean;
                diff * diff
            })
            .sum::<Decimal>()
            / n;

        let std = decimal_sqrt(variance).unwrap_or(Decimal::ZERO);

        // Coefficient of variation (lower is more stable)
        let cv = if mean.is_zero() {
            dec!(2)
        } else {
            (std / mean.abs()).min(dec!(2))
        };

        // Convert to 0-1 score (1 = most stable)
        (Decimal::ONE - cv / dec!(2)).max(Decimal::ZERO)
    }

    /// Calculates overall robustness score.
    fn calculate_robustness_score(
        &self,
        wf_efficiency: Decimal,
        overall_efficiency: Decimal,
        stability: Decimal,
    ) -> Decimal {
        // Weighted combination of factors
        let efficiency_weight = dec!(0.4);
        let overall_weight = dec!(0.3);
        let stability_weight = dec!(0.3);

        // Normalize overall efficiency (cap at 1.5 for scoring)
        let normalized_efficiency = overall_efficiency.min(dec!(1.5)) / dec!(1.5);

        wf_efficiency * efficiency_weight
            + normalized_efficiency * overall_weight
            + stability * stability_weight
    }

    /// Validates a single window result.
    #[must_use]
    pub fn validate_window(&self, result: &WindowResult) -> bool {
        // Check minimum trades
        if result.in_sample_metrics.num_trades < self.config.min_trades_per_window {
            return false;
        }

        // Check if in-sample was profitable (basic requirement)
        if result.in_sample_metrics.total_return <= Decimal::ZERO {
            return false;
        }

        true
    }
}

/// Walk-forward optimization error.
#[derive(Debug, Clone, thiserror::Error)]
pub enum WalkForwardError {
    /// Invalid time range
    #[error("invalid time range: end time must be after start time")]
    InvalidTimeRange,

    /// Invalid window size
    #[error("invalid window size: {0}")]
    InvalidWindowSize(String),

    /// Insufficient data for walk-forward analysis
    #[error("insufficient data for walk-forward analysis")]
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

    fn create_config() -> WalkForwardConfig {
        // 100 days of data, 30 day IS, 10 day OOS
        let start = Timestamp::new(0).unwrap();
        let end = Timestamp::new(100 * 86_400_000).unwrap(); // 100 days in ms
        let is_size = 30 * 86_400_000; // 30 days
        let oos_size = 10 * 86_400_000; // 10 days

        WalkForwardConfig::new(start, end, is_size, oos_size)
    }

    fn create_window_metrics(return_pct: Decimal, trades: usize) -> WindowMetrics {
        WindowMetrics {
            total_return: return_pct,
            num_trades: trades,
            win_rate: dec!(0.55),
            max_drawdown: dec!(0.10),
            sharpe_ratio: Some(dec!(1.5)),
            profit_factor: Some(dec!(1.8)),
            final_equity: Amount::new_unchecked(dec!(110000)),
        }
    }

    #[test]
    fn test_config_validation() {
        let config = create_config();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_invalid_time_range() {
        let start = Timestamp::new(1000).unwrap();
        let end = Timestamp::new(500).unwrap();
        let config = WalkForwardConfig::new(start, end, 100, 50);

        assert!(matches!(
            config.validate(),
            Err(WalkForwardError::InvalidTimeRange)
        ));
    }

    #[test]
    fn test_config_invalid_window_size() {
        let start = Timestamp::new(0).unwrap();
        let end = Timestamp::new(1000).unwrap();
        let config = WalkForwardConfig::new(start, end, -100, 50);

        assert!(matches!(
            config.validate(),
            Err(WalkForwardError::InvalidWindowSize(_))
        ));
    }

    #[test]
    fn test_config_insufficient_data() {
        let start = Timestamp::new(0).unwrap();
        let end = Timestamp::new(100).unwrap();
        let config = WalkForwardConfig::new(start, end, 80, 50);

        assert!(matches!(
            config.validate(),
            Err(WalkForwardError::InsufficientData)
        ));
    }

    #[test]
    fn test_window_generation() {
        let config = create_config();
        let optimizer = WalkForwardOptimizer::new(config).unwrap();

        // With 100 days, 30 IS + 10 OOS, step = 10, we should get multiple windows
        assert!(optimizer.num_windows() > 0);

        // Verify first window
        let first = &optimizer.windows()[0];
        assert_eq!(first.index, 0);
        assert_eq!(first.in_sample_start.as_millis(), 0);
    }

    #[test]
    fn test_anchored_walk_forward() {
        let config = create_config().with_anchored(true);
        let optimizer = WalkForwardOptimizer::new(config).unwrap();

        // All windows should start from the same point
        for window in optimizer.windows() {
            assert_eq!(window.in_sample_start.as_millis(), 0);
        }
    }

    #[test]
    fn test_rolling_walk_forward() {
        let config = create_config().with_anchored(false);
        let optimizer = WalkForwardOptimizer::new(config).unwrap();

        // Windows should have different start times
        if optimizer.num_windows() > 1 {
            let first = &optimizer.windows()[0];
            let second = &optimizer.windows()[1];
            assert!(second.in_sample_start > first.in_sample_start);
        }
    }

    #[test]
    fn test_custom_step_size() {
        let step = 5 * 86_400_000; // 5 days
        let config = create_config().with_step_size(step);
        let optimizer = WalkForwardOptimizer::new(config).unwrap();

        // With smaller step, we should get more windows
        let default_optimizer = WalkForwardOptimizer::new(create_config()).unwrap();
        assert!(optimizer.num_windows() >= default_optimizer.num_windows());
    }

    #[test]
    fn test_window_duration() {
        let config = create_config();
        let optimizer = WalkForwardOptimizer::new(config.clone()).unwrap();

        let window = &optimizer.windows()[0];
        assert_eq!(window.in_sample_duration_ms(), config.in_sample_size_ms);
        assert_eq!(
            window.out_of_sample_duration_ms(),
            config.out_of_sample_size_ms
        );
    }

    #[test]
    fn test_calculate_results() {
        let config = create_config();
        let optimizer = WalkForwardOptimizer::new(config).unwrap();

        let window_results: Vec<WindowResult> = optimizer
            .windows()
            .iter()
            .map(|w| WindowResult {
                window: w.clone(),
                in_sample_metrics: create_window_metrics(dec!(0.15), 20),
                out_of_sample_metrics: create_window_metrics(dec!(0.08), 8),
                efficiency_ratio: Some(dec!(0.53)),
                passed: true,
            })
            .collect();

        let results = optimizer.calculate_results(window_results);

        assert!(results.num_windows > 0);
        assert_eq!(results.num_passed, results.num_windows);
        assert!(results.aggregate_in_sample.mean_return > Decimal::ZERO);
        assert!(results.aggregate_out_of_sample.mean_return > Decimal::ZERO);
    }

    #[test]
    fn test_walk_forward_efficiency() {
        let config = create_config();
        let optimizer = WalkForwardOptimizer::new(config).unwrap();

        // All positive OOS returns
        let all_positive: Vec<WindowResult> = optimizer
            .windows()
            .iter()
            .map(|w| WindowResult {
                window: w.clone(),
                in_sample_metrics: create_window_metrics(dec!(0.15), 20),
                out_of_sample_metrics: create_window_metrics(dec!(0.05), 8),
                efficiency_ratio: Some(dec!(0.33)),
                passed: true,
            })
            .collect();

        let results = optimizer.calculate_results(all_positive);
        assert_eq!(results.walk_forward_efficiency, Decimal::ONE);
    }

    #[test]
    fn test_validate_window() {
        let config = create_config().with_min_trades(10);
        let optimizer = WalkForwardOptimizer::new(config).unwrap();

        let window = optimizer.windows()[0].clone();

        // Valid window
        let valid = WindowResult {
            window: window.clone(),
            in_sample_metrics: create_window_metrics(dec!(0.15), 20),
            out_of_sample_metrics: create_window_metrics(dec!(0.05), 8),
            efficiency_ratio: Some(dec!(0.33)),
            passed: true,
        };
        assert!(optimizer.validate_window(&valid));

        // Invalid: not enough trades
        let low_trades = WindowResult {
            window: window.clone(),
            in_sample_metrics: create_window_metrics(dec!(0.15), 5),
            out_of_sample_metrics: create_window_metrics(dec!(0.05), 2),
            efficiency_ratio: Some(dec!(0.33)),
            passed: true,
        };
        assert!(!optimizer.validate_window(&low_trades));

        // Invalid: negative IS return
        let negative_is = WindowResult {
            window,
            in_sample_metrics: create_window_metrics(dec!(-0.05), 20),
            out_of_sample_metrics: create_window_metrics(dec!(0.05), 8),
            efficiency_ratio: None,
            passed: true,
        };
        assert!(!optimizer.validate_window(&negative_is));
    }

    #[test]
    fn test_stability_score() {
        let config = create_config();
        let optimizer = WalkForwardOptimizer::new(config).unwrap();

        // Consistent returns should have high stability
        let consistent: Vec<WindowResult> = optimizer
            .windows()
            .iter()
            .map(|w| WindowResult {
                window: w.clone(),
                in_sample_metrics: create_window_metrics(dec!(0.15), 20),
                out_of_sample_metrics: create_window_metrics(dec!(0.05), 8),
                efficiency_ratio: Some(dec!(0.33)),
                passed: true,
            })
            .collect();

        let results = optimizer.calculate_results(consistent);
        assert!(results.stability_score > dec!(0.5));
    }

    #[test]
    fn test_robustness_score() {
        let config = create_config();
        let optimizer = WalkForwardOptimizer::new(config).unwrap();

        let good_results: Vec<WindowResult> = optimizer
            .windows()
            .iter()
            .map(|w| WindowResult {
                window: w.clone(),
                in_sample_metrics: create_window_metrics(dec!(0.15), 20),
                out_of_sample_metrics: create_window_metrics(dec!(0.10), 8),
                efficiency_ratio: Some(dec!(0.67)),
                passed: true,
            })
            .collect();

        let results = optimizer.calculate_results(good_results);

        // Good results should have reasonable robustness score
        assert!(results.robustness_score > Decimal::ZERO);
        assert!(results.robustness_score <= Decimal::ONE);
    }

    #[test]
    fn test_aggregate_metrics() {
        let config = create_config();
        let optimizer = WalkForwardOptimizer::new(config).unwrap();

        let window_results: Vec<WindowResult> = optimizer
            .windows()
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let return_pct = dec!(0.05) + Decimal::from(i as u64) * dec!(0.02);
                WindowResult {
                    window: w.clone(),
                    in_sample_metrics: create_window_metrics(return_pct * dec!(2), 20),
                    out_of_sample_metrics: create_window_metrics(return_pct, 8),
                    efficiency_ratio: Some(dec!(0.5)),
                    passed: true,
                }
            })
            .collect();

        let results = optimizer.calculate_results(window_results);

        // Check aggregate calculations
        assert!(results.aggregate_out_of_sample.mean_return > Decimal::ZERO);
        assert!(results.aggregate_out_of_sample.total_trades > 0);
    }

    #[test]
    fn test_empty_results() {
        let config = create_config();
        let optimizer = WalkForwardOptimizer::new(config).unwrap();

        let results = optimizer.calculate_results(vec![]);

        assert_eq!(results.num_windows, 0);
        assert_eq!(results.num_passed, 0);
        assert_eq!(results.overall_efficiency, Decimal::ZERO);
    }
}
