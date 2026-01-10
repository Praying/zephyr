//! Simulated order matching engine for backtesting.
//!
//! Provides order matching simulation with configurable slippage models.

use std::collections::HashMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use zephyr_core::data::{Order, OrderRequest, OrderSide, OrderStatus, OrderType, TickData};
use zephyr_core::types::{OrderId, Price, Quantity, Timestamp};

use crate::error::BacktestError;

/// Slippage model for simulating market impact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SlippageModel {
    /// No slippage
    None,
    /// Fixed slippage in price units
    Fixed(Decimal),
    /// Percentage-based slippage (e.g., 0.001 = 0.1%)
    Percentage(Decimal),
    /// Volume-based slippage (larger orders have more slippage)
    VolumeImpact {
        /// Base slippage percentage
        base_pct: Decimal,
        /// Impact factor per unit volume
        impact_factor: Decimal,
    },
}

impl Default for SlippageModel {
    fn default() -> Self {
        Self::None
    }
}

impl SlippageModel {
    /// Calculates the slippage amount for a given price and quantity.
    #[must_use]
    pub fn calculate(&self, price: Price, quantity: Quantity, side: OrderSide) -> Decimal {
        let base_slippage = match self {
            Self::None => Decimal::ZERO,
            Self::Fixed(amount) => *amount,
            Self::Percentage(pct) => price.as_decimal() * pct,
            Self::VolumeImpact {
                base_pct,
                impact_factor,
            } => {
                let base = price.as_decimal() * base_pct;
                let volume_impact = quantity.as_decimal().abs() * impact_factor;
                base + volume_impact
            }
        };

        // Slippage direction: worse for the trader
        match side {
            OrderSide::Buy => base_slippage,   // Pay more
            OrderSide::Sell => -base_slippage, // Receive less
        }
    }

    /// Applies slippage to a price.
    #[must_use]
    pub fn apply(&self, price: Price, quantity: Quantity, side: OrderSide) -> Price {
        let slippage = self.calculate(price, quantity, side);
        let adjusted = price.as_decimal() + slippage;
        Price::new_unchecked(adjusted.max(Decimal::ZERO))
    }
}

/// Result of an order match attempt.
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// The matched order (updated with fill info)
    pub order: Order,
    /// Fill price (including slippage)
    pub fill_price: Price,
    /// Fill quantity
    pub fill_quantity: Quantity,
    /// Slippage amount
    pub slippage: Decimal,
    /// Whether the order is fully filled
    pub is_complete: bool,
}

/// Configuration for the match engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchEngineConfig {
    /// Slippage model to use
    #[serde(default)]
    pub slippage_model: SlippageModel,
    /// Whether to allow partial fills
    #[serde(default = "default_true")]
    pub allow_partial_fills: bool,
    /// Minimum fill quantity (to avoid dust)
    pub min_fill_quantity: Option<Quantity>,
}

fn default_true() -> bool {
    true
}

impl Default for MatchEngineConfig {
    fn default() -> Self {
        Self {
            slippage_model: SlippageModel::None,
            allow_partial_fills: true,
            min_fill_quantity: None,
        }
    }
}

/// Simulated order matching engine.
///
/// Matches orders against historical price data with configurable slippage.
pub struct MatchEngine {
    config: MatchEngineConfig,
    /// Pending orders by order ID
    pending_orders: HashMap<OrderId, Order>,
    /// Order counter for generating IDs
    order_counter: u64,
    /// Random seed for deterministic behavior
    seed: u64,
}

impl MatchEngine {
    /// Creates a new match engine with the given configuration.
    #[must_use]
    pub fn new(config: MatchEngineConfig) -> Self {
        Self {
            config,
            pending_orders: HashMap::new(),
            order_counter: 0,
            seed: 0,
        }
    }

    /// Creates a new match engine with a specific seed for determinism.
    #[must_use]
    pub fn with_seed(config: MatchEngineConfig, seed: u64) -> Self {
        Self {
            config,
            pending_orders: HashMap::new(),
            order_counter: 0,
            seed,
        }
    }

    /// Submits an order request and returns the created order.
    ///
    /// # Errors
    ///
    /// Returns an error if the order request is invalid.
    pub fn submit_order(
        &mut self,
        request: &OrderRequest,
        timestamp: Timestamp,
    ) -> Result<Order, BacktestError> {
        // Validate the request
        request
            .validate()
            .map_err(|e| BacktestError::OrderValidation(e.to_string()))?;

        // Generate order ID
        self.order_counter += 1;
        let order_id = OrderId::new(format!("BT-{:016X}", self.order_counter ^ self.seed))
            .map_err(|e| BacktestError::Internal(e.to_string()))?;

        // Create order from request
        let order = Order::from_request(request, order_id.clone(), timestamp);

        // For market orders, they stay pending until matched
        // For limit orders, add to pending
        self.pending_orders.insert(order_id, order.clone());

        Ok(order)
    }

    /// Cancels a pending order.
    ///
    /// # Errors
    ///
    /// Returns an error if the order is not found or already filled.
    pub fn cancel_order(&mut self, order_id: &OrderId) -> Result<Order, BacktestError> {
        let mut order = self
            .pending_orders
            .remove(order_id)
            .ok_or_else(|| BacktestError::InvalidOrderState("order not found".to_string()))?;

        if order.status.is_final() {
            return Err(BacktestError::InvalidOrderState(format!(
                "order already in final state: {}",
                order.status
            )));
        }

        order.status = OrderStatus::Canceled;
        order.update_time = Timestamp::now();

        Ok(order)
    }

    /// Processes a tick and attempts to match pending orders.
    ///
    /// Returns a list of match results for orders that were filled.
    pub fn process_tick(&mut self, tick: &TickData) -> Vec<MatchResult> {
        let mut results = Vec::new();
        let mut filled_orders = Vec::new();

        for (order_id, order) in &self.pending_orders {
            if order.symbol != tick.symbol {
                continue;
            }

            if let Some(result) = self.try_match(order, tick) {
                results.push(result.clone());
                if result.is_complete {
                    filled_orders.push(order_id.clone());
                }
            }
        }

        // Remove fully filled orders
        for order_id in filled_orders {
            self.pending_orders.remove(&order_id);
        }

        // Update partially filled orders
        for result in &results {
            if !result.is_complete {
                if let Some(order) = self.pending_orders.get_mut(&result.order.order_id) {
                    *order = result.order.clone();
                }
            }
        }

        results
    }

    /// Attempts to match an order against a tick.
    fn try_match(&self, order: &Order, tick: &TickData) -> Option<MatchResult> {
        match order.order_type {
            OrderType::Market => self.match_market_order(order, tick),
            OrderType::Limit => self.match_limit_order(order, tick),
            _ => None, // Stop orders not implemented yet
        }
    }

    /// Matches a market order against a tick.
    fn match_market_order(&self, order: &Order, tick: &TickData) -> Option<MatchResult> {
        // Market orders fill at the current price with slippage
        let base_price = match order.side {
            OrderSide::Buy => tick.best_ask().unwrap_or(tick.price),
            OrderSide::Sell => tick.best_bid().unwrap_or(tick.price),
        };

        let fill_quantity = order.remaining_quantity();
        let fill_price = self
            .config
            .slippage_model
            .apply(base_price, fill_quantity, order.side);
        let slippage = self
            .config
            .slippage_model
            .calculate(base_price, fill_quantity, order.side);

        let mut filled_order = order.clone();
        filled_order.record_fill(fill_quantity, fill_price).ok()?;

        Some(MatchResult {
            order: filled_order,
            fill_price,
            fill_quantity,
            slippage,
            is_complete: true,
        })
    }

    /// Matches a limit order against a tick.
    fn match_limit_order(&self, order: &Order, tick: &TickData) -> Option<MatchResult> {
        let limit_price = order.price?;

        // Check if the limit price is crossed
        let is_crossed = match order.side {
            OrderSide::Buy => {
                // Buy limit: fill if market price <= limit price
                let ask = tick.best_ask().unwrap_or(tick.price);
                ask <= limit_price
            }
            OrderSide::Sell => {
                // Sell limit: fill if market price >= limit price
                let bid = tick.best_bid().unwrap_or(tick.price);
                bid >= limit_price
            }
        };

        if !is_crossed {
            return None;
        }

        // Determine fill price (limit price or better, with slippage)
        let base_fill_price = match order.side {
            OrderSide::Buy => {
                let ask = tick.best_ask().unwrap_or(tick.price);
                // Fill at the better of limit price or ask
                if ask < limit_price { ask } else { limit_price }
            }
            OrderSide::Sell => {
                let bid = tick.best_bid().unwrap_or(tick.price);
                // Fill at the better of limit price or bid
                if bid > limit_price { bid } else { limit_price }
            }
        };

        let fill_quantity = order.remaining_quantity();
        let fill_price =
            self.config
                .slippage_model
                .apply(base_fill_price, fill_quantity, order.side);
        let slippage =
            self.config
                .slippage_model
                .calculate(base_fill_price, fill_quantity, order.side);

        let mut filled_order = order.clone();
        filled_order.record_fill(fill_quantity, fill_price).ok()?;

        Some(MatchResult {
            order: filled_order,
            fill_price,
            fill_quantity,
            slippage,
            is_complete: true,
        })
    }

    /// Returns the number of pending orders.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending_orders.len()
    }

    /// Returns all pending orders.
    #[must_use]
    pub fn pending_orders(&self) -> Vec<&Order> {
        self.pending_orders.values().collect()
    }

    /// Returns a pending order by ID.
    #[must_use]
    pub fn get_order(&self, order_id: &OrderId) -> Option<&Order> {
        self.pending_orders.get(order_id)
    }

    /// Resets the match engine state.
    pub fn reset(&mut self) {
        self.pending_orders.clear();
        self.order_counter = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use zephyr_core::data::TimeInForce;
    use zephyr_core::types::Symbol;

    fn create_tick(price: Decimal, bid: Decimal, ask: Decimal) -> TickData {
        TickData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1704067200000).unwrap())
            .price(Price::new(price).unwrap())
            .volume(Quantity::new(dec!(1)).unwrap())
            .bid_price(Price::new(bid).unwrap())
            .bid_quantity(Quantity::new(dec!(10)).unwrap())
            .ask_price(Price::new(ask).unwrap())
            .ask_quantity(Quantity::new(dec!(10)).unwrap())
            .build()
            .unwrap()
    }

    fn create_market_buy(qty: Decimal) -> OrderRequest {
        OrderRequest::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .side(OrderSide::Buy)
            .order_type(OrderType::Market)
            .quantity(Quantity::new(qty).unwrap())
            .build()
            .unwrap()
    }

    fn create_limit_buy(qty: Decimal, price: Decimal) -> OrderRequest {
        OrderRequest::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(Quantity::new(qty).unwrap())
            .price(Price::new(price).unwrap())
            .time_in_force(TimeInForce::Gtc)
            .build()
            .unwrap()
    }

    fn create_limit_sell(qty: Decimal, price: Decimal) -> OrderRequest {
        OrderRequest::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .side(OrderSide::Sell)
            .order_type(OrderType::Limit)
            .quantity(Quantity::new(qty).unwrap())
            .price(Price::new(price).unwrap())
            .time_in_force(TimeInForce::Gtc)
            .build()
            .unwrap()
    }

    #[test]
    fn test_slippage_none() {
        let model = SlippageModel::None;
        let price = Price::new(dec!(100)).unwrap();
        let qty = Quantity::new(dec!(1)).unwrap();

        assert_eq!(model.calculate(price, qty, OrderSide::Buy), Decimal::ZERO);
        assert_eq!(
            model.apply(price, qty, OrderSide::Buy).as_decimal(),
            dec!(100)
        );
    }

    #[test]
    fn test_slippage_fixed() {
        let model = SlippageModel::Fixed(dec!(0.5));
        let price = Price::new(dec!(100)).unwrap();
        let qty = Quantity::new(dec!(1)).unwrap();

        // Buy: pay more
        assert_eq!(model.calculate(price, qty, OrderSide::Buy), dec!(0.5));
        assert_eq!(
            model.apply(price, qty, OrderSide::Buy).as_decimal(),
            dec!(100.5)
        );

        // Sell: receive less
        assert_eq!(model.calculate(price, qty, OrderSide::Sell), dec!(-0.5));
        assert_eq!(
            model.apply(price, qty, OrderSide::Sell).as_decimal(),
            dec!(99.5)
        );
    }

    #[test]
    fn test_slippage_percentage() {
        let model = SlippageModel::Percentage(dec!(0.001)); // 0.1%
        let price = Price::new(dec!(1000)).unwrap();
        let qty = Quantity::new(dec!(1)).unwrap();

        // 0.1% of 1000 = 1
        assert_eq!(model.calculate(price, qty, OrderSide::Buy), dec!(1));
        assert_eq!(
            model.apply(price, qty, OrderSide::Buy).as_decimal(),
            dec!(1001)
        );
    }

    #[test]
    fn test_match_engine_market_order() {
        let mut engine = MatchEngine::new(MatchEngineConfig::default());
        let tick = create_tick(dec!(100), dec!(99), dec!(101));

        let request = create_market_buy(dec!(1));
        let order = engine
            .submit_order(&request, Timestamp::new(1000).unwrap())
            .unwrap();

        assert_eq!(engine.pending_count(), 1);

        let results = engine.process_tick(&tick);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_complete);
        assert_eq!(results[0].fill_price.as_decimal(), dec!(101)); // Filled at ask
        assert_eq!(engine.pending_count(), 0);
    }

    #[test]
    fn test_match_engine_limit_order_not_crossed() {
        let mut engine = MatchEngine::new(MatchEngineConfig::default());
        let tick = create_tick(dec!(100), dec!(99), dec!(101));

        // Buy limit at 98 - should not fill (ask is 101)
        let request = create_limit_buy(dec!(1), dec!(98));
        engine
            .submit_order(&request, Timestamp::new(1000).unwrap())
            .unwrap();

        let results = engine.process_tick(&tick);
        assert!(results.is_empty());
        assert_eq!(engine.pending_count(), 1);
    }

    #[test]
    fn test_match_engine_limit_order_crossed() {
        let mut engine = MatchEngine::new(MatchEngineConfig::default());
        let tick = create_tick(dec!(100), dec!(99), dec!(101));

        // Buy limit at 102 - should fill (ask is 101)
        let request = create_limit_buy(dec!(1), dec!(102));
        engine
            .submit_order(&request, Timestamp::new(1000).unwrap())
            .unwrap();

        let results = engine.process_tick(&tick);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_complete);
        // Should fill at ask (101) since it's better than limit (102)
        assert_eq!(results[0].fill_price.as_decimal(), dec!(101));
    }

    #[test]
    fn test_match_engine_sell_limit_crossed() {
        let mut engine = MatchEngine::new(MatchEngineConfig::default());
        let tick = create_tick(dec!(100), dec!(99), dec!(101));

        // Sell limit at 98 - should fill (bid is 99)
        let request = create_limit_sell(dec!(1), dec!(98));
        engine
            .submit_order(&request, Timestamp::new(1000).unwrap())
            .unwrap();

        let results = engine.process_tick(&tick);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_complete);
        // Should fill at bid (99) since it's better than limit (98)
        assert_eq!(results[0].fill_price.as_decimal(), dec!(99));
    }

    #[test]
    fn test_match_engine_with_slippage() {
        let config = MatchEngineConfig {
            slippage_model: SlippageModel::Fixed(dec!(0.5)),
            ..Default::default()
        };
        let mut engine = MatchEngine::new(config);
        let tick = create_tick(dec!(100), dec!(99), dec!(101));

        let request = create_market_buy(dec!(1));
        engine
            .submit_order(&request, Timestamp::new(1000).unwrap())
            .unwrap();

        let results = engine.process_tick(&tick);
        assert_eq!(results.len(), 1);
        // Ask (101) + slippage (0.5) = 101.5
        assert_eq!(results[0].fill_price.as_decimal(), dec!(101.5));
        assert_eq!(results[0].slippage, dec!(0.5));
    }

    #[test]
    fn test_match_engine_cancel_order() {
        let mut engine = MatchEngine::new(MatchEngineConfig::default());

        let request = create_limit_buy(dec!(1), dec!(98));
        let order = engine
            .submit_order(&request, Timestamp::new(1000).unwrap())
            .unwrap();

        assert_eq!(engine.pending_count(), 1);

        let canceled = engine.cancel_order(&order.order_id).unwrap();
        assert_eq!(canceled.status, OrderStatus::Canceled);
        assert_eq!(engine.pending_count(), 0);
    }

    #[test]
    fn test_match_engine_deterministic_with_seed() {
        let config = MatchEngineConfig::default();

        let mut engine1 = MatchEngine::with_seed(config.clone(), 12345);
        let mut engine2 = MatchEngine::with_seed(config, 12345);

        let request = create_market_buy(dec!(1));

        let order1 = engine1
            .submit_order(&request, Timestamp::new(1000).unwrap())
            .unwrap();
        let order2 = engine2
            .submit_order(&request, Timestamp::new(1000).unwrap())
            .unwrap();

        // Same seed should produce same order IDs
        assert_eq!(order1.order_id, order2.order_id);
    }

    #[test]
    fn test_match_engine_reset() {
        let mut engine = MatchEngine::new(MatchEngineConfig::default());

        let request = create_limit_buy(dec!(1), dec!(98));
        engine
            .submit_order(&request, Timestamp::new(1000).unwrap())
            .unwrap();

        assert_eq!(engine.pending_count(), 1);

        engine.reset();
        assert_eq!(engine.pending_count(), 0);
    }

    #[test]
    fn test_match_engine_multi_symbol() {
        let mut engine = MatchEngine::new(MatchEngineConfig::default());

        // BTC order
        let btc_request = create_limit_buy(dec!(1), dec!(50000));
        engine
            .submit_order(&btc_request, Timestamp::new(1000).unwrap())
            .unwrap();

        // ETH tick - should not match BTC order
        let eth_tick = TickData::builder()
            .symbol(Symbol::new("ETH-USDT").unwrap())
            .timestamp(Timestamp::new(1000).unwrap())
            .price(Price::new(dec!(3000)).unwrap())
            .volume(Quantity::new(dec!(1)).unwrap())
            .bid_price(Price::new(dec!(2999)).unwrap())
            .bid_quantity(Quantity::new(dec!(10)).unwrap())
            .ask_price(Price::new(dec!(3001)).unwrap())
            .ask_quantity(Quantity::new(dec!(10)).unwrap())
            .build()
            .unwrap();

        let results = engine.process_tick(&eth_tick);
        assert!(results.is_empty());
        assert_eq!(engine.pending_count(), 1);
    }
}
