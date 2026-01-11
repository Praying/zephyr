//! Stress testing module for risk analysis.
//!
//! This module provides stress testing capabilities:
//! - Historical scenario testing: Replay historical market events
//! - Hypothetical scenario testing: Custom stress scenarios
//!
//! # Example
//!
//! ```
//! use zephyr_risk::stress::{StressTester, StressScenario, ScenarioType};
//! use zephyr_core::types::{Symbol, Amount};
//! use rust_decimal_macros::dec;
//!
//! let tester = StressTester::new();
//!
//! // Create a hypothetical scenario
//! let scenario = StressScenario::hypothetical("Market Crash")
//!     .with_price_shock(Symbol::new("BTC-USDT").unwrap(), dec!(-0.30))
//!     .with_volatility_multiplier(dec!(2.0));
//!
//! // Run stress test
//! // let result = tester.run_scenario(&scenario, &portfolio);
//! ```

#![allow(clippy::module_name_repetitions)]

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use zephyr_core::types::{Amount, Symbol, Timestamp};

/// Type of stress scenario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioType {
    /// Historical scenario based on past market events
    Historical {
        /// Name of the historical event
        event_name: String,
        /// Start date of the event
        start_date: Timestamp,
        /// End date of the event
        end_date: Timestamp,
    },
    /// Hypothetical scenario with custom parameters
    Hypothetical {
        /// Description of the scenario
        description: String,
    },
}

/// Price shock definition for a symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceShock {
    /// Symbol affected
    pub symbol: Symbol,
    /// Price change as a decimal (e.g., -0.30 for -30%)
    pub price_change: Decimal,
    /// Optional volatility change multiplier
    pub volatility_multiplier: Option<Decimal>,
}

/// Correlation shock definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationShock {
    /// First symbol
    pub symbol_a: Symbol,
    /// Second symbol
    pub symbol_b: Symbol,
    /// New correlation value (-1 to 1)
    pub correlation: Decimal,
}

/// Stress scenario definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressScenario {
    /// Unique identifier for the scenario
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Scenario type
    pub scenario_type: ScenarioType,
    /// Price shocks to apply
    pub price_shocks: Vec<PriceShock>,
    /// Global volatility multiplier
    pub volatility_multiplier: Decimal,
    /// Correlation shocks
    pub correlation_shocks: Vec<CorrelationShock>,
    /// Liquidity reduction factor (0-1, where 0 = no liquidity)
    pub liquidity_factor: Decimal,
    /// Whether this scenario is enabled
    pub enabled: bool,
}

impl StressScenario {
    /// Creates a new hypothetical stress scenario.
    #[must_use]
    pub fn hypothetical(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            id: uuid_v4(),
            name: name.clone(),
            scenario_type: ScenarioType::Hypothetical { description: name },
            price_shocks: Vec::new(),
            volatility_multiplier: Decimal::ONE,
            correlation_shocks: Vec::new(),
            liquidity_factor: Decimal::ONE,
            enabled: true,
        }
    }

    /// Creates a new historical stress scenario.
    #[must_use]
    pub fn historical(
        name: impl Into<String>,
        event_name: impl Into<String>,
        start_date: Timestamp,
        end_date: Timestamp,
    ) -> Self {
        Self {
            id: uuid_v4(),
            name: name.into(),
            scenario_type: ScenarioType::Historical {
                event_name: event_name.into(),
                start_date,
                end_date,
            },
            price_shocks: Vec::new(),
            volatility_multiplier: Decimal::ONE,
            correlation_shocks: Vec::new(),
            liquidity_factor: Decimal::ONE,
            enabled: true,
        }
    }

    /// Adds a price shock to the scenario.
    #[must_use]
    pub fn with_price_shock(mut self, symbol: Symbol, price_change: Decimal) -> Self {
        self.price_shocks.push(PriceShock {
            symbol,
            price_change,
            volatility_multiplier: None,
        });
        self
    }

    /// Adds a price shock with volatility change.
    #[must_use]
    pub fn with_price_and_vol_shock(
        mut self,
        symbol: Symbol,
        price_change: Decimal,
        vol_multiplier: Decimal,
    ) -> Self {
        self.price_shocks.push(PriceShock {
            symbol,
            price_change,
            volatility_multiplier: Some(vol_multiplier),
        });
        self
    }

    /// Sets the global volatility multiplier.
    #[must_use]
    pub fn with_volatility_multiplier(mut self, multiplier: Decimal) -> Self {
        self.volatility_multiplier = multiplier;
        self
    }

    /// Adds a correlation shock.
    #[must_use]
    pub fn with_correlation_shock(
        mut self,
        symbol_a: Symbol,
        symbol_b: Symbol,
        correlation: Decimal,
    ) -> Self {
        self.correlation_shocks.push(CorrelationShock {
            symbol_a,
            symbol_b,
            correlation,
        });
        self
    }

    /// Sets the liquidity factor.
    #[must_use]
    pub fn with_liquidity_factor(mut self, factor: Decimal) -> Self {
        self.liquidity_factor = factor;
        self
    }

    /// Gets the price shock for a specific symbol.
    #[must_use]
    pub fn get_price_shock(&self, symbol: &Symbol) -> Option<&PriceShock> {
        self.price_shocks.iter().find(|s| &s.symbol == symbol)
    }
}

/// Portfolio position for stress testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressPosition {
    /// Symbol
    pub symbol: Symbol,
    /// Position quantity (positive for long, negative for short)
    pub quantity: Decimal,
    /// Current market value
    pub market_value: Amount,
    /// Current price
    pub current_price: Decimal,
}

/// Portfolio for stress testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressPortfolio {
    /// Portfolio positions
    pub positions: Vec<StressPosition>,
    /// Total portfolio value
    pub total_value: Amount,
    /// Available cash
    pub cash: Amount,
}

impl StressPortfolio {
    /// Creates a new empty portfolio.
    #[must_use]
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            total_value: Amount::ZERO,
            cash: Amount::ZERO,
        }
    }

    /// Adds a position to the portfolio.
    #[must_use]
    pub fn with_position(mut self, position: StressPosition) -> Self {
        self.total_value = self.total_value + position.market_value;
        self.positions.push(position);
        self
    }

    /// Sets the cash balance.
    #[must_use]
    pub fn with_cash(mut self, cash: Amount) -> Self {
        self.cash = cash;
        self.total_value = self.total_value + cash;
        self
    }

    /// Gets a position by symbol.
    #[must_use]
    pub fn get_position(&self, symbol: &Symbol) -> Option<&StressPosition> {
        self.positions.iter().find(|p| &p.symbol == symbol)
    }
}

impl Default for StressPortfolio {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a stress test for a single position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionStressResult {
    /// Symbol
    pub symbol: Symbol,
    /// Original market value
    pub original_value: Amount,
    /// Stressed market value
    pub stressed_value: Amount,
    /// Profit/Loss from stress
    pub pnl: Amount,
    /// Percentage change
    pub pnl_percent: Decimal,
}

/// Result of a stress test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressTestResult {
    /// Scenario that was tested
    pub scenario_id: String,
    /// Scenario name
    pub scenario_name: String,
    /// Original portfolio value
    pub original_value: Amount,
    /// Stressed portfolio value
    pub stressed_value: Amount,
    /// Total P&L
    pub total_pnl: Amount,
    /// Total P&L as percentage
    pub total_pnl_percent: Decimal,
    /// Per-position results
    pub position_results: Vec<PositionStressResult>,
    /// Timestamp of the test
    pub timestamp: Timestamp,
    /// Whether the portfolio would breach risk limits
    pub breaches_limits: bool,
    /// Risk limit breaches if any
    pub limit_breaches: Vec<String>,
}

/// Stress tester configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressTesterConfig {
    /// Maximum acceptable loss percentage
    pub max_loss_threshold: Decimal,
    /// Whether to include second-order effects
    pub include_second_order: bool,
    /// Whether to apply liquidity adjustments
    pub apply_liquidity_adjustment: bool,
}

impl Default for StressTesterConfig {
    fn default() -> Self {
        Self {
            max_loss_threshold: Decimal::new(-20, 2), // -20%
            include_second_order: false,
            apply_liquidity_adjustment: true,
        }
    }
}

/// Stress tester for running stress scenarios.
#[derive(Debug, Clone)]
pub struct StressTester {
    config: StressTesterConfig,
    scenarios: Vec<StressScenario>,
}

impl StressTester {
    /// Creates a new stress tester with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: StressTesterConfig::default(),
            scenarios: Vec::new(),
        }
    }

    /// Creates a new stress tester with custom configuration.
    #[must_use]
    pub const fn with_config(config: StressTesterConfig) -> Self {
        Self {
            config,
            scenarios: Vec::new(),
        }
    }

    /// Adds a scenario to the tester.
    pub fn add_scenario(&mut self, scenario: StressScenario) {
        self.scenarios.push(scenario);
    }

    /// Adds predefined historical scenarios.
    pub fn add_historical_scenarios(&mut self) {
        // COVID-19 Crash (March 2020)
        let covid_crash = StressScenario::historical(
            "COVID-19 Crash",
            "March 2020 Market Crash",
            Timestamp::new(1_583_020_800_000).unwrap_or_else(|_| Timestamp::now()), // March 1, 2020
            Timestamp::new(1_584_835_200_000).unwrap_or_else(|_| Timestamp::now()), // March 22, 2020
        )
        .with_price_shock(
            Symbol::new("BTC-USDT").unwrap_or_else(|_| Symbol::new_unchecked("BTC-USDT")),
            Decimal::new(-50, 2), // -50%
        )
        .with_volatility_multiplier(Decimal::new(300, 2)) // 3x volatility
        .with_liquidity_factor(Decimal::new(30, 2)); // 30% liquidity

        self.scenarios.push(covid_crash);

        // FTX Collapse (November 2022)
        let ftx_collapse = StressScenario::historical(
            "FTX Collapse",
            "November 2022 FTX Bankruptcy",
            Timestamp::new(1_667_779_200_000).unwrap_or_else(|_| Timestamp::now()), // Nov 7, 2022
            Timestamp::new(1_668_556_800_000).unwrap_or_else(|_| Timestamp::now()), // Nov 16, 2022
        )
        .with_price_shock(
            Symbol::new("BTC-USDT").unwrap_or_else(|_| Symbol::new_unchecked("BTC-USDT")),
            Decimal::new(-25, 2), // -25%
        )
        .with_volatility_multiplier(Decimal::new(250, 2)) // 2.5x volatility
        .with_liquidity_factor(Decimal::new(50, 2)); // 50% liquidity

        self.scenarios.push(ftx_collapse);

        // Luna/UST Collapse (May 2022)
        let luna_collapse = StressScenario::historical(
            "Luna/UST Collapse",
            "May 2022 Terra Collapse",
            Timestamp::new(1_652_054_400_000).unwrap_or_else(|_| Timestamp::now()), // May 9, 2022
            Timestamp::new(1_652_659_200_000).unwrap_or_else(|_| Timestamp::now()), // May 16, 2022
        )
        .with_price_shock(
            Symbol::new("BTC-USDT").unwrap_or_else(|_| Symbol::new_unchecked("BTC-USDT")),
            Decimal::new(-30, 2), // -30%
        )
        .with_volatility_multiplier(Decimal::new(400, 2)) // 4x volatility
        .with_liquidity_factor(Decimal::new(40, 2)); // 40% liquidity

        self.scenarios.push(luna_collapse);
    }

    /// Runs a single stress scenario on a portfolio.
    #[must_use]
    pub fn run_scenario(
        &self,
        scenario: &StressScenario,
        portfolio: &StressPortfolio,
    ) -> StressTestResult {
        let mut position_results = Vec::new();
        let mut total_stressed_value = portfolio.cash;

        // Apply stress to each position
        for position in &portfolio.positions {
            let result = self.stress_position(position, scenario);
            total_stressed_value = total_stressed_value + result.stressed_value;
            position_results.push(result);
        }

        let total_pnl = total_stressed_value - portfolio.total_value;
        let total_pnl_percent = if portfolio.total_value.is_zero() {
            Decimal::ZERO
        } else {
            total_pnl.as_decimal() / portfolio.total_value.as_decimal() * Decimal::ONE_HUNDRED
        };

        // Check for limit breaches
        let mut limit_breaches = Vec::new();
        let breaches_limits =
            if total_pnl_percent < self.config.max_loss_threshold * Decimal::ONE_HUNDRED {
                limit_breaches.push(format!(
                    "Loss of {:.2}% exceeds threshold of {:.2}%",
                    total_pnl_percent,
                    self.config.max_loss_threshold * Decimal::ONE_HUNDRED
                ));
                true
            } else {
                false
            };

        StressTestResult {
            scenario_id: scenario.id.clone(),
            scenario_name: scenario.name.clone(),
            original_value: portfolio.total_value,
            stressed_value: total_stressed_value,
            total_pnl,
            total_pnl_percent,
            position_results,
            timestamp: Timestamp::now(),
            breaches_limits,
            limit_breaches,
        }
    }

    /// Runs all enabled scenarios on a portfolio.
    #[must_use]
    pub fn run_all_scenarios(&self, portfolio: &StressPortfolio) -> Vec<StressTestResult> {
        self.scenarios
            .iter()
            .filter(|s| s.enabled)
            .map(|scenario| self.run_scenario(scenario, portfolio))
            .collect()
    }

    /// Stresses a single position.
    fn stress_position(
        &self,
        position: &StressPosition,
        scenario: &StressScenario,
    ) -> PositionStressResult {
        // Get price shock for this symbol, or use default (no change)
        let price_change = scenario
            .get_price_shock(&position.symbol)
            .map_or(Decimal::ZERO, |s| s.price_change);

        // Calculate stressed price
        let stressed_price = position.current_price * (Decimal::ONE + price_change);

        // Calculate stressed value
        let stressed_value_decimal = position.quantity * stressed_price;
        let stressed_value = Amount::new_unchecked(stressed_value_decimal.abs());

        // Apply liquidity adjustment if configured
        let final_stressed_value = if self.config.apply_liquidity_adjustment
            && scenario.liquidity_factor < Decimal::ONE
        {
            // Reduce value further due to liquidity constraints
            let liquidity_impact = (Decimal::ONE - scenario.liquidity_factor) * Decimal::new(5, 2); // 5% additional impact per 100% liquidity reduction
            let adjusted = stressed_value.as_decimal() * (Decimal::ONE - liquidity_impact);
            Amount::new_unchecked(adjusted)
        } else {
            stressed_value
        };

        let pnl = final_stressed_value - position.market_value;
        let pnl_percent = if position.market_value.is_zero() {
            Decimal::ZERO
        } else {
            pnl.as_decimal() / position.market_value.as_decimal() * Decimal::ONE_HUNDRED
        };

        PositionStressResult {
            symbol: position.symbol.clone(),
            original_value: position.market_value,
            stressed_value: final_stressed_value,
            pnl,
            pnl_percent,
        }
    }

    /// Gets all registered scenarios.
    #[must_use]
    pub fn scenarios(&self) -> &[StressScenario] {
        &self.scenarios
    }

    /// Gets the worst-case scenario result.
    #[must_use]
    pub fn worst_case<'a>(&self, results: &'a [StressTestResult]) -> Option<&'a StressTestResult> {
        results.iter().min_by(|a, b| {
            a.total_pnl
                .as_decimal()
                .partial_cmp(&b.total_pnl.as_decimal())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

impl Default for StressTester {
    fn default() -> Self {
        Self::new()
    }
}

/// Generates a simple UUID v4-like string.
fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{timestamp:032x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn test_symbol() -> Symbol {
        Symbol::new("BTC-USDT").unwrap()
    }

    fn test_portfolio() -> StressPortfolio {
        StressPortfolio::new()
            .with_position(StressPosition {
                symbol: test_symbol(),
                quantity: dec!(1.0),
                market_value: Amount::new(dec!(50000)).unwrap(),
                current_price: dec!(50000),
            })
            .with_cash(Amount::new(dec!(10000)).unwrap())
    }

    #[test]
    fn test_stress_scenario_hypothetical() {
        let scenario = StressScenario::hypothetical("Test Crash")
            .with_price_shock(test_symbol(), dec!(-0.30))
            .with_volatility_multiplier(dec!(2.0));

        assert_eq!(scenario.name, "Test Crash");
        assert_eq!(scenario.price_shocks.len(), 1);
        assert_eq!(scenario.volatility_multiplier, dec!(2.0));
    }

    #[test]
    fn test_stress_scenario_historical() {
        let scenario = StressScenario::historical(
            "COVID Crash",
            "March 2020",
            Timestamp::now(),
            Timestamp::now(),
        );

        assert_eq!(scenario.name, "COVID Crash");
        assert!(matches!(
            scenario.scenario_type,
            ScenarioType::Historical { .. }
        ));
    }

    #[test]
    fn test_stress_portfolio_creation() {
        let portfolio = test_portfolio();

        assert_eq!(portfolio.positions.len(), 1);
        assert_eq!(portfolio.total_value.as_decimal(), dec!(60000));
        assert_eq!(portfolio.cash.as_decimal(), dec!(10000));
    }

    #[test]
    fn test_run_scenario() {
        let tester = StressTester::new();
        let portfolio = test_portfolio();

        let scenario =
            StressScenario::hypothetical("30% Crash").with_price_shock(test_symbol(), dec!(-0.30));

        let result = tester.run_scenario(&scenario, &portfolio);

        // Original: 60000, BTC drops 30% (50000 -> 35000), cash stays 10000
        // Stressed: 35000 + 10000 = 45000
        assert!(result.stressed_value.as_decimal() < result.original_value.as_decimal());
        assert!(result.total_pnl.is_negative());
    }

    #[test]
    fn test_run_all_scenarios() {
        let mut tester = StressTester::new();
        tester.add_historical_scenarios();

        let portfolio = test_portfolio();
        let results = tester.run_all_scenarios(&portfolio);

        assert!(!results.is_empty());
        for result in &results {
            assert!(result.total_pnl.is_negative());
        }
    }

    #[test]
    fn test_worst_case() {
        let mut tester = StressTester::new();

        let mild_scenario =
            StressScenario::hypothetical("Mild").with_price_shock(test_symbol(), dec!(-0.10));
        let severe_scenario =
            StressScenario::hypothetical("Severe").with_price_shock(test_symbol(), dec!(-0.50));

        tester.add_scenario(mild_scenario);
        tester.add_scenario(severe_scenario);

        let portfolio = test_portfolio();
        let results = tester.run_all_scenarios(&portfolio);

        let worst = tester.worst_case(&results).unwrap();
        assert_eq!(worst.scenario_name, "Severe");
    }

    #[test]
    fn test_limit_breach_detection() {
        let config = StressTesterConfig {
            max_loss_threshold: dec!(-0.10), // -10%
            ..Default::default()
        };
        let tester = StressTester::with_config(config);

        let portfolio = test_portfolio();
        let scenario =
            StressScenario::hypothetical("Big Crash").with_price_shock(test_symbol(), dec!(-0.30));

        let result = tester.run_scenario(&scenario, &portfolio);

        assert!(result.breaches_limits);
        assert!(!result.limit_breaches.is_empty());
    }

    #[test]
    fn test_liquidity_adjustment() {
        let config = StressTesterConfig {
            apply_liquidity_adjustment: true,
            ..Default::default()
        };
        let tester = StressTester::with_config(config);

        let portfolio = test_portfolio();

        // Scenario with low liquidity
        let low_liq_scenario = StressScenario::hypothetical("Low Liquidity")
            .with_price_shock(test_symbol(), dec!(-0.20))
            .with_liquidity_factor(dec!(0.30));

        // Scenario with normal liquidity
        let normal_liq_scenario = StressScenario::hypothetical("Normal Liquidity")
            .with_price_shock(test_symbol(), dec!(-0.20))
            .with_liquidity_factor(dec!(1.0));

        let low_liq_result = tester.run_scenario(&low_liq_scenario, &portfolio);
        let normal_liq_result = tester.run_scenario(&normal_liq_scenario, &portfolio);

        // Low liquidity should result in worse outcome
        assert!(low_liq_result.total_pnl < normal_liq_result.total_pnl);
    }

    #[test]
    fn test_stress_result_serde() {
        let result = StressTestResult {
            scenario_id: "test-123".to_string(),
            scenario_name: "Test Scenario".to_string(),
            original_value: Amount::new(dec!(100000)).unwrap(),
            stressed_value: Amount::new(dec!(80000)).unwrap(),
            total_pnl: Amount::new(dec!(-20000)).unwrap(),
            total_pnl_percent: dec!(-20),
            position_results: vec![],
            timestamp: Timestamp::now(),
            breaches_limits: true,
            limit_breaches: vec!["Loss exceeds threshold".to_string()],
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: StressTestResult = serde_json::from_str(&json).unwrap();

        assert_eq!(result.scenario_id, parsed.scenario_id);
        assert_eq!(result.total_pnl, parsed.total_pnl);
    }
}
