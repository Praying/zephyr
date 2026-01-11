//! Multi-asset portfolio backtesting module.
//!
//! Provides portfolio-level backtesting with:
//! - Multi-asset position tracking
//! - Correlation analysis between assets
//! - Portfolio-level metrics calculation

use std::collections::HashMap;

use rust_decimal::Decimal;
use rust_decimal::prelude::Signed;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use zephyr_core::types::{Amount, Price, Quantity, Symbol, Timestamp};

use crate::error::BacktestError;
use crate::metrics::{BacktestMetrics, TradeRecord};

/// Portfolio position for a single asset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetPosition {
    /// Symbol
    pub symbol: Symbol,
    /// Current quantity (positive = long, negative = short)
    pub quantity: Quantity,
    /// Average entry price
    pub avg_entry_price: Price,
    /// Current market price
    pub market_price: Price,
    /// Realized PnL
    pub realized_pnl: Amount,
    /// Unrealized PnL
    pub unrealized_pnl: Amount,
    /// Last update timestamp
    pub last_update: Timestamp,
}

impl AssetPosition {
    /// Creates a new empty position.
    #[must_use]
    pub fn new(symbol: Symbol) -> Self {
        Self {
            symbol,
            quantity: Quantity::ZERO,
            avg_entry_price: Price::ZERO,
            market_price: Price::ZERO,
            realized_pnl: Amount::ZERO,
            unrealized_pnl: Amount::ZERO,
            last_update: Timestamp::now(),
        }
    }

    /// Returns the position value (quantity * market_price).
    #[must_use]
    pub fn market_value(&self) -> Amount {
        Amount::new_unchecked(self.quantity.as_decimal().abs() * self.market_price.as_decimal())
    }

    /// Updates the market price and recalculates unrealized PnL.
    pub fn update_price(&mut self, price: Price, timestamp: Timestamp) {
        self.market_price = price;
        self.last_update = timestamp;
        self.recalculate_unrealized_pnl();
    }

    /// Recalculates unrealized PnL based on current position and prices.
    fn recalculate_unrealized_pnl(&mut self) {
        if self.quantity.is_zero() {
            self.unrealized_pnl = Amount::ZERO;
            return;
        }

        let pnl = (self.market_price.as_decimal() - self.avg_entry_price.as_decimal())
            * self.quantity.as_decimal();
        self.unrealized_pnl = Amount::new_unchecked(pnl);
    }

    /// Records a trade and updates position.
    pub fn record_trade(
        &mut self,
        quantity: Quantity,
        price: Price,
        timestamp: Timestamp,
    ) -> Amount {
        let trade_qty = quantity.as_decimal();
        let trade_price = price.as_decimal();
        let current_qty = self.quantity.as_decimal();

        let realized_pnl = if current_qty.signum() != trade_qty.signum() && !current_qty.is_zero() {
            // Closing or reversing position
            let close_qty = trade_qty.abs().min(current_qty.abs());
            let pnl = (trade_price - self.avg_entry_price.as_decimal())
                * close_qty
                * current_qty.signum();
            Amount::new_unchecked(pnl)
        } else {
            Amount::ZERO
        };

        // Update position
        let new_qty = current_qty + trade_qty;

        if new_qty.is_zero() {
            // Position closed
            self.quantity = Quantity::ZERO;
            self.avg_entry_price = Price::ZERO;
        } else if current_qty.signum() == trade_qty.signum() || current_qty.is_zero() {
            // Adding to position
            let total_cost = current_qty.abs() * self.avg_entry_price.as_decimal()
                + trade_qty.abs() * trade_price;
            let new_avg = total_cost / new_qty.abs();
            self.quantity = Quantity::new_unchecked(new_qty);
            self.avg_entry_price = Price::new_unchecked(new_avg);
        } else {
            // Partial close or reversal
            self.quantity = Quantity::new_unchecked(new_qty);
            if new_qty.signum() != current_qty.signum() {
                // Position reversed
                self.avg_entry_price = price;
            }
        }

        self.realized_pnl =
            Amount::new_unchecked(self.realized_pnl.as_decimal() + realized_pnl.as_decimal());
        self.market_price = price;
        self.last_update = timestamp;
        self.recalculate_unrealized_pnl();

        realized_pnl
    }
}

/// Portfolio backtest configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioBacktestConfig {
    /// Initial capital
    pub initial_capital: Amount,
    /// Symbols to track
    pub symbols: Vec<Symbol>,
    /// Whether to calculate correlations
    #[serde(default = "default_true")]
    pub calculate_correlations: bool,
    /// Correlation lookback period (number of returns)
    #[serde(default = "default_correlation_lookback")]
    pub correlation_lookback: usize,
}

fn default_true() -> bool {
    true
}

fn default_correlation_lookback() -> usize {
    30
}

impl Default for PortfolioBacktestConfig {
    fn default() -> Self {
        Self {
            initial_capital: Amount::new_unchecked(dec!(100000)),
            symbols: Vec::new(),
            calculate_correlations: true,
            correlation_lookback: 30,
        }
    }
}

/// Correlation matrix between assets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationMatrix {
    /// Symbols in order
    pub symbols: Vec<Symbol>,
    /// Correlation values (row-major order)
    pub values: Vec<Vec<Decimal>>,
}

impl CorrelationMatrix {
    /// Creates a new correlation matrix.
    #[must_use]
    pub fn new(symbols: Vec<Symbol>) -> Self {
        let n = symbols.len();
        let values = vec![vec![Decimal::ONE; n]; n];
        Self { symbols, values }
    }

    /// Gets the correlation between two symbols.
    #[must_use]
    pub fn get(&self, symbol1: &Symbol, symbol2: &Symbol) -> Option<Decimal> {
        let idx1 = self.symbols.iter().position(|s| s == symbol1)?;
        let idx2 = self.symbols.iter().position(|s| s == symbol2)?;
        Some(self.values[idx1][idx2])
    }

    /// Sets the correlation between two symbols.
    pub fn set(&mut self, symbol1: &Symbol, symbol2: &Symbol, correlation: Decimal) {
        if let (Some(idx1), Some(idx2)) = (
            self.symbols.iter().position(|s| s == symbol1),
            self.symbols.iter().position(|s| s == symbol2),
        ) {
            self.values[idx1][idx2] = correlation;
            self.values[idx2][idx1] = correlation; // Symmetric
        }
    }
}

/// Portfolio-level backtest metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioMetrics {
    /// Per-asset metrics
    pub asset_metrics: HashMap<Symbol, BacktestMetrics>,
    /// Portfolio-level metrics
    pub portfolio_metrics: BacktestMetrics,
    /// Correlation matrix
    pub correlations: Option<CorrelationMatrix>,
    /// Asset weights over time
    pub weight_history: Vec<(Timestamp, HashMap<Symbol, Decimal>)>,
    /// Portfolio beta (if benchmark provided)
    pub beta: Option<Decimal>,
    /// Tracking error (if benchmark provided)
    pub tracking_error: Option<Decimal>,
}

/// Multi-asset portfolio backtester.
pub struct PortfolioBacktester {
    config: PortfolioBacktestConfig,
    /// Current positions by symbol
    positions: HashMap<Symbol, AssetPosition>,
    /// Cash balance
    cash: Amount,
    /// Per-asset metrics trackers
    asset_metrics: HashMap<Symbol, BacktestMetrics>,
    /// Portfolio-level metrics
    portfolio_metrics: BacktestMetrics,
    /// Return history for correlation calculation
    return_history: HashMap<Symbol, Vec<Decimal>>,
    /// Previous prices for return calculation
    prev_prices: HashMap<Symbol, Price>,
    /// Weight history
    weight_history: Vec<(Timestamp, HashMap<Symbol, Decimal>)>,
}

impl PortfolioBacktester {
    /// Creates a new portfolio backtester.
    #[must_use]
    pub fn new(config: PortfolioBacktestConfig) -> Self {
        let mut positions = HashMap::new();
        let mut asset_metrics = HashMap::new();
        let mut return_history = HashMap::new();

        for symbol in &config.symbols {
            positions.insert(symbol.clone(), AssetPosition::new(symbol.clone()));
            asset_metrics.insert(symbol.clone(), BacktestMetrics::new(Amount::ZERO));
            return_history.insert(symbol.clone(), Vec::new());
        }

        Self {
            cash: config.initial_capital,
            portfolio_metrics: BacktestMetrics::new(config.initial_capital),
            config,
            positions,
            asset_metrics,
            return_history,
            prev_prices: HashMap::new(),
            weight_history: Vec::new(),
        }
    }

    /// Updates the price for a symbol.
    pub fn update_price(&mut self, symbol: &Symbol, price: Price, timestamp: Timestamp) {
        // Calculate return for correlation
        if self.config.calculate_correlations {
            if let Some(prev_price) = self.prev_prices.get(symbol)
                && !prev_price.is_zero()
            {
                let ret = (price.as_decimal() - prev_price.as_decimal()) / prev_price.as_decimal();
                if let Some(returns) = self.return_history.get_mut(symbol) {
                    returns.push(ret);
                    // Keep only lookback period
                    if returns.len() > self.config.correlation_lookback {
                        returns.remove(0);
                    }
                }
            }
            self.prev_prices.insert(symbol.clone(), price);
        }

        // Update position
        if let Some(position) = self.positions.get_mut(symbol) {
            position.update_price(price, timestamp);
        }

        // Update portfolio equity
        self.update_portfolio_equity(timestamp);
    }

    /// Records a trade for a symbol.
    ///
    /// # Errors
    ///
    /// Returns an error if the symbol is not tracked.
    pub fn record_trade(
        &mut self,
        symbol: &Symbol,
        quantity: Quantity,
        price: Price,
        timestamp: Timestamp,
    ) -> Result<Amount, BacktestError> {
        let position = self
            .positions
            .get_mut(symbol)
            .ok_or_else(|| BacktestError::PositionNotFound(symbol.clone()))?;

        let realized_pnl = position.record_trade(quantity, price, timestamp);

        // Update cash
        let trade_value = quantity.as_decimal().abs() * price.as_decimal();
        if quantity.as_decimal() > Decimal::ZERO {
            // Buying
            self.cash = Amount::new_unchecked(self.cash.as_decimal() - trade_value);
        } else {
            // Selling
            self.cash = Amount::new_unchecked(self.cash.as_decimal() + trade_value);
        }

        // Record trade in asset metrics
        if let Some(metrics) = self.asset_metrics.get_mut(symbol) {
            let trade_record = TradeRecord {
                timestamp,
                pnl: realized_pnl,
                value: Amount::new_unchecked(trade_value),
                is_win: realized_pnl.as_decimal() > Decimal::ZERO,
            };
            metrics.record_trade(trade_record.clone());
        }

        // Record in portfolio metrics if PnL realized
        if !realized_pnl.is_zero() {
            let trade_record = TradeRecord {
                timestamp,
                pnl: realized_pnl,
                value: Amount::new_unchecked(trade_value),
                is_win: realized_pnl.as_decimal() > Decimal::ZERO,
            };
            self.portfolio_metrics.record_trade(trade_record);
        }

        self.update_portfolio_equity(timestamp);

        Ok(realized_pnl)
    }

    /// Updates portfolio equity and records weight history.
    fn update_portfolio_equity(&mut self, timestamp: Timestamp) {
        let total_equity = self.total_equity();
        self.portfolio_metrics
            .update_equity(timestamp, total_equity);

        // Record weights
        if !total_equity.is_zero() {
            let mut weights = HashMap::new();
            for (symbol, position) in &self.positions {
                let weight = position.market_value().as_decimal() / total_equity.as_decimal();
                weights.insert(symbol.clone(), weight);
            }
            self.weight_history.push((timestamp, weights));
        }
    }

    /// Returns the total portfolio equity.
    #[must_use]
    pub fn total_equity(&self) -> Amount {
        let position_value: Decimal = self
            .positions
            .values()
            .map(|p| p.market_value().as_decimal() + p.unrealized_pnl.as_decimal())
            .sum();
        Amount::new_unchecked(self.cash.as_decimal() + position_value)
    }

    /// Returns the current cash balance.
    #[must_use]
    pub fn cash(&self) -> Amount {
        self.cash
    }

    /// Returns a position by symbol.
    #[must_use]
    pub fn get_position(&self, symbol: &Symbol) -> Option<&AssetPosition> {
        self.positions.get(symbol)
    }

    /// Returns all positions.
    #[must_use]
    pub fn positions(&self) -> &HashMap<Symbol, AssetPosition> {
        &self.positions
    }

    /// Calculates the correlation matrix.
    #[must_use]
    pub fn calculate_correlations(&self) -> CorrelationMatrix {
        let symbols: Vec<_> = self.config.symbols.clone();
        let mut matrix = CorrelationMatrix::new(symbols.clone());

        for (i, sym1) in symbols.iter().enumerate() {
            for (j, sym2) in symbols.iter().enumerate() {
                if i == j {
                    continue;
                }

                if let (Some(returns1), Some(returns2)) =
                    (self.return_history.get(sym1), self.return_history.get(sym2))
                    && let Some(corr) = calculate_correlation(returns1, returns2)
                {
                    matrix.set(sym1, sym2, corr);
                }
            }
        }

        matrix
    }

    /// Returns the final portfolio metrics.
    #[must_use]
    pub fn finalize(&self) -> PortfolioMetrics {
        let correlations = if self.config.calculate_correlations {
            Some(self.calculate_correlations())
        } else {
            None
        };

        PortfolioMetrics {
            asset_metrics: self.asset_metrics.clone(),
            portfolio_metrics: self.portfolio_metrics.clone(),
            correlations,
            weight_history: self.weight_history.clone(),
            beta: None,
            tracking_error: None,
        }
    }

    /// Resets the backtester to initial state.
    pub fn reset(&mut self) {
        self.cash = self.config.initial_capital;
        self.portfolio_metrics = BacktestMetrics::new(self.config.initial_capital);

        for symbol in &self.config.symbols {
            self.positions
                .insert(symbol.clone(), AssetPosition::new(symbol.clone()));
            self.asset_metrics
                .insert(symbol.clone(), BacktestMetrics::new(Amount::ZERO));
            self.return_history.insert(symbol.clone(), Vec::new());
        }

        self.prev_prices.clear();
        self.weight_history.clear();
    }
}

/// Calculates Pearson correlation coefficient between two return series.
fn calculate_correlation(returns1: &[Decimal], returns2: &[Decimal]) -> Option<Decimal> {
    let n = returns1.len().min(returns2.len());
    if n < 2 {
        return None;
    }

    let n_dec = Decimal::from(n as u64);

    // Calculate means
    let mean1: Decimal = returns1.iter().take(n).copied().sum::<Decimal>() / n_dec;
    let mean2: Decimal = returns2.iter().take(n).copied().sum::<Decimal>() / n_dec;

    // Calculate covariance and standard deviations
    let mut cov = Decimal::ZERO;
    let mut var1 = Decimal::ZERO;
    let mut var2 = Decimal::ZERO;

    for i in 0..n {
        let d1 = returns1[i] - mean1;
        let d2 = returns2[i] - mean2;
        cov += d1 * d2;
        var1 += d1 * d1;
        var2 += d2 * d2;
    }

    if var1.is_zero() || var2.is_zero() {
        return None;
    }

    // Correlation = cov / (std1 * std2)
    let std1 = decimal_sqrt(var1 / n_dec)?;
    let std2 = decimal_sqrt(var2 / n_dec)?;

    if std1.is_zero() || std2.is_zero() {
        return None;
    }

    Some((cov / n_dec) / (std1 * std2))
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

    fn create_config() -> PortfolioBacktestConfig {
        PortfolioBacktestConfig {
            initial_capital: Amount::new_unchecked(dec!(100000)),
            symbols: vec![
                Symbol::new("BTC-USDT").unwrap(),
                Symbol::new("ETH-USDT").unwrap(),
            ],
            calculate_correlations: true,
            correlation_lookback: 30,
        }
    }

    #[test]
    fn test_portfolio_initial_state() {
        let config = create_config();
        let backtester = PortfolioBacktester::new(config);

        assert_eq!(backtester.cash().as_decimal(), dec!(100000));
        assert_eq!(backtester.total_equity().as_decimal(), dec!(100000));
        assert_eq!(backtester.positions().len(), 2);
    }

    #[test]
    fn test_portfolio_record_trade() {
        let config = create_config();
        let mut backtester = PortfolioBacktester::new(config);

        let symbol = Symbol::new("BTC-USDT").unwrap();
        let timestamp = Timestamp::new(1000).unwrap();

        // Buy 1 BTC at 50000
        backtester
            .record_trade(
                &symbol,
                Quantity::new(dec!(1)).unwrap(),
                Price::new(dec!(50000)).unwrap(),
                timestamp,
            )
            .unwrap();

        // Cash should decrease
        assert_eq!(backtester.cash().as_decimal(), dec!(50000));

        // Position should be updated
        let position = backtester.get_position(&symbol).unwrap();
        assert_eq!(position.quantity.as_decimal(), dec!(1));
        assert_eq!(position.avg_entry_price.as_decimal(), dec!(50000));
    }

    #[test]
    fn test_portfolio_price_update() {
        let config = create_config();
        let mut backtester = PortfolioBacktester::new(config);

        let symbol = Symbol::new("BTC-USDT").unwrap();
        let timestamp = Timestamp::new(1000).unwrap();

        // Buy 1 BTC at 50000
        backtester
            .record_trade(
                &symbol,
                Quantity::new(dec!(1)).unwrap(),
                Price::new(dec!(50000)).unwrap(),
                timestamp,
            )
            .unwrap();

        // Price goes up to 55000
        backtester.update_price(&symbol, Price::new(dec!(55000)).unwrap(), timestamp);

        let position = backtester.get_position(&symbol).unwrap();
        assert_eq!(position.unrealized_pnl.as_decimal(), dec!(5000));
    }

    #[test]
    fn test_portfolio_realized_pnl() {
        let config = create_config();
        let mut backtester = PortfolioBacktester::new(config);

        let symbol = Symbol::new("BTC-USDT").unwrap();
        let timestamp = Timestamp::new(1000).unwrap();

        // Buy 1 BTC at 50000
        backtester
            .record_trade(
                &symbol,
                Quantity::new(dec!(1)).unwrap(),
                Price::new(dec!(50000)).unwrap(),
                timestamp,
            )
            .unwrap();

        // Sell 1 BTC at 55000
        let pnl = backtester
            .record_trade(
                &symbol,
                Quantity::new(dec!(-1)).unwrap(),
                Price::new(dec!(55000)).unwrap(),
                timestamp,
            )
            .unwrap();

        assert_eq!(pnl.as_decimal(), dec!(5000));

        let position = backtester.get_position(&symbol).unwrap();
        assert!(position.quantity.is_zero());
        assert_eq!(position.realized_pnl.as_decimal(), dec!(5000));
    }

    #[test]
    fn test_correlation_calculation() {
        // Perfect positive correlation
        let returns1 = vec![dec!(0.01), dec!(0.02), dec!(0.03), dec!(0.04), dec!(0.05)];
        let returns2 = vec![dec!(0.01), dec!(0.02), dec!(0.03), dec!(0.04), dec!(0.05)];

        let corr = calculate_correlation(&returns1, &returns2).unwrap();
        assert!((corr - dec!(1)).abs() < dec!(0.0001));

        // Perfect negative correlation
        let returns3 = vec![
            dec!(-0.01),
            dec!(-0.02),
            dec!(-0.03),
            dec!(-0.04),
            dec!(-0.05),
        ];
        let corr_neg = calculate_correlation(&returns1, &returns3).unwrap();
        assert!((corr_neg - dec!(-1)).abs() < dec!(0.0001));
    }

    #[test]
    fn test_asset_position_trade() {
        let symbol = Symbol::new("BTC-USDT").unwrap();
        let mut position = AssetPosition::new(symbol);
        let timestamp = Timestamp::new(1000).unwrap();

        // Buy 2 BTC at 50000
        position.record_trade(
            Quantity::new(dec!(2)).unwrap(),
            Price::new(dec!(50000)).unwrap(),
            timestamp,
        );

        assert_eq!(position.quantity.as_decimal(), dec!(2));
        assert_eq!(position.avg_entry_price.as_decimal(), dec!(50000));

        // Buy 1 more BTC at 52000
        position.record_trade(
            Quantity::new(dec!(1)).unwrap(),
            Price::new(dec!(52000)).unwrap(),
            timestamp,
        );

        assert_eq!(position.quantity.as_decimal(), dec!(3));
        // Avg price = (2*50000 + 1*52000) / 3 = 152000/3 ≈ 50666.67
        let expected_avg = (dec!(2) * dec!(50000) + dec!(1) * dec!(52000)) / dec!(3);
        assert_eq!(position.avg_entry_price.as_decimal(), expected_avg);
    }

    #[test]
    fn test_portfolio_reset() {
        let config = create_config();
        let mut backtester = PortfolioBacktester::new(config);

        let symbol = Symbol::new("BTC-USDT").unwrap();
        let timestamp = Timestamp::new(1000).unwrap();

        backtester
            .record_trade(
                &symbol,
                Quantity::new(dec!(1)).unwrap(),
                Price::new(dec!(50000)).unwrap(),
                timestamp,
            )
            .unwrap();

        backtester.reset();

        assert_eq!(backtester.cash().as_decimal(), dec!(100000));
        let position = backtester.get_position(&symbol).unwrap();
        assert!(position.quantity.is_zero());
    }
}
