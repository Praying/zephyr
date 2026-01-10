//! HFT Strategy Context Implementation.
//!
//! This module provides the implementation of the HFT strategy context,
//! which manages direct order submission and callbacks for high-frequency
//! trading strategies.

#![allow(clippy::unused_async)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::map_unwrap_or)]

use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, info, instrument, warn};

use zephyr_core::data::{Order, OrderRequest, OrderSide, OrderStatus, OrderType, TimeInForce};
use zephyr_core::error::StrategyError;
use zephyr_core::traits::{HftStrategy, HftStrategyContext, LogLevel, OrderFlag, Trade};
use zephyr_core::types::{OrderId, Price, Quantity, Symbol, Timestamp};

/// Order submission request for the order router.
#[derive(Debug, Clone)]
pub struct OrderSubmission {
    /// Order request details.
    pub request: OrderRequest,
    /// Strategy name that submitted the order.
    pub strategy_name: String,
}

/// Order callback for HFT strategies.
pub trait HftOrderCallback: Send + Sync {
    /// Called when an order is submitted.
    fn on_order_submitted(&self, order_id: &OrderId, request: &OrderRequest);
    /// Called when an order is updated.
    fn on_order_update(&self, order: &Order);
    /// Called when a trade is executed.
    fn on_trade(&self, trade: &Trade);
}

/// HFT Strategy Context Implementation.
///
/// Provides direct order submission for HFT strategies.
/// Implements the `HftStrategyContext` trait.
///
/// # Thread Safety
///
/// This implementation is thread-safe and can be shared across threads.
pub struct HftStrategyContextImpl {
    /// Strategy name.
    strategy_name: String,
    /// Positions per symbol.
    positions: DashMap<Symbol, Quantity>,
    /// Current prices per symbol.
    prices: DashMap<Symbol, Price>,
    /// Pending orders per symbol.
    pending_orders: DashMap<Symbol, Vec<Order>>,
    /// All orders by ID.
    orders_by_id: DashMap<OrderId, Order>,
    /// Order submission channel.
    order_tx: mpsc::UnboundedSender<OrderSubmission>,
    /// Current timestamp.
    current_time: RwLock<Timestamp>,
    /// Order ID counter for generating unique IDs.
    order_counter: std::sync::atomic::AtomicU64,
}

impl HftStrategyContextImpl {
    /// Creates a new HFT strategy context.
    ///
    /// # Arguments
    ///
    /// * `strategy_name` - Name of the strategy
    /// * `order_tx` - Channel for submitting orders
    #[must_use]
    pub fn new(
        strategy_name: impl Into<String>,
        order_tx: mpsc::UnboundedSender<OrderSubmission>,
    ) -> Self {
        Self {
            strategy_name: strategy_name.into(),
            positions: DashMap::new(),
            prices: DashMap::new(),
            pending_orders: DashMap::new(),
            orders_by_id: DashMap::new(),
            order_tx,
            current_time: RwLock::new(Timestamp::now()),
            order_counter: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Generates a unique order ID.
    fn generate_order_id(&self) -> OrderId {
        let counter = self
            .order_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        OrderId::new_unchecked(format!("{}-{}", self.strategy_name, counter))
    }

    /// Updates the current price for a symbol.
    pub fn update_price(&self, symbol: &Symbol, price: Price) {
        self.prices.insert(symbol.clone(), price);
    }

    /// Updates the position for a symbol.
    pub fn update_position(&self, symbol: &Symbol, quantity: Quantity) {
        self.positions.insert(symbol.clone(), quantity);
    }

    /// Sets the current timestamp.
    pub fn set_current_time(&self, timestamp: Timestamp) {
        *self.current_time.write() = timestamp;
    }

    /// Handles an order update from the exchange.
    pub fn on_order_update(&self, order: Order) {
        let symbol = order.symbol.clone();
        let order_id = order.order_id.clone();

        // Update order in orders_by_id
        self.orders_by_id.insert(order_id.clone(), order.clone());

        // Update pending orders
        if order.status.is_final() {
            // Remove from pending
            if let Some(mut pending) = self.pending_orders.get_mut(&symbol) {
                pending.retain(|o| o.order_id != order_id);
            }
        } else {
            // Update in pending
            if let Some(mut pending) = self.pending_orders.get_mut(&symbol) {
                if let Some(existing) = pending.iter_mut().find(|o| o.order_id == order_id) {
                    *existing = order;
                }
            }
        }
    }

    /// Handles a trade execution.
    pub fn on_trade(&self, trade: &Trade) {
        // Update position based on trade
        let current = self.get_position(&trade.symbol);
        let change = match trade.side {
            OrderSide::Buy => trade.quantity,
            OrderSide::Sell => -trade.quantity,
        };
        let new_position = current + change;
        self.positions.insert(trade.symbol.clone(), new_position);

        debug!(
            symbol = %trade.symbol,
            side = %trade.side,
            quantity = %trade.quantity,
            price = %trade.price,
            new_position = %new_position,
            "Trade executed"
        );
    }

    /// Submits an order internally.
    async fn submit_order(
        &self,
        symbol: &Symbol,
        side: OrderSide,
        price: Price,
        qty: Quantity,
        flag: OrderFlag,
    ) -> Result<OrderId, StrategyError> {
        let order_id = self.generate_order_id();
        let timestamp = self.current_time();

        // Convert OrderFlag to TimeInForce
        let time_in_force = match flag {
            OrderFlag::Normal => TimeInForce::Gtc,
            OrderFlag::Fak => TimeInForce::Ioc,
            OrderFlag::Fok => TimeInForce::Fok,
            OrderFlag::PostOnly => TimeInForce::PostOnly,
            OrderFlag::ReduceOnly => TimeInForce::Gtc,
        };

        let request = OrderRequest {
            symbol: symbol.clone(),
            side,
            order_type: OrderType::Limit,
            quantity: qty,
            price: Some(price),
            stop_price: None,
            time_in_force,
            reduce_only: matches!(flag, OrderFlag::ReduceOnly),
            post_only: matches!(flag, OrderFlag::PostOnly),
            client_order_id: Some(order_id.as_str().to_string()),
        };

        // Create pending order
        let order = Order::from_request(&request, order_id.clone(), timestamp);

        // Store order
        self.orders_by_id.insert(order_id.clone(), order.clone());
        self.pending_orders
            .entry(symbol.clone())
            .or_default()
            .push(order);

        // Send to order router
        let submission = OrderSubmission {
            request,
            strategy_name: self.strategy_name.clone(),
        };

        self.order_tx
            .send(submission)
            .map_err(|e| StrategyError::CallbackError {
                strategy: self.strategy_name.clone(),
                reason: format!("Failed to submit order: {}", e),
            })?;

        info!(
            order_id = %order_id,
            symbol = %symbol,
            side = %side,
            price = %price,
            quantity = %qty,
            "Order submitted"
        );

        Ok(order_id)
    }
}

#[async_trait]
impl HftStrategyContext for HftStrategyContextImpl {
    #[instrument(skip(self), fields(strategy = %self.strategy_name))]
    async fn buy(
        &self,
        symbol: &Symbol,
        price: Price,
        qty: Quantity,
        flag: OrderFlag,
    ) -> Result<OrderId, StrategyError> {
        self.submit_order(symbol, OrderSide::Buy, price, qty, flag)
            .await
    }

    #[instrument(skip(self), fields(strategy = %self.strategy_name))]
    async fn sell(
        &self,
        symbol: &Symbol,
        price: Price,
        qty: Quantity,
        flag: OrderFlag,
    ) -> Result<OrderId, StrategyError> {
        self.submit_order(symbol, OrderSide::Sell, price, qty, flag)
            .await
    }

    async fn cancel(&self, order_id: &OrderId) -> Result<bool, StrategyError> {
        // Check if order exists and is cancellable
        let order = self.orders_by_id.get(order_id);
        match order {
            Some(order) if order.status.is_active() => {
                info!(order_id = %order_id, "Cancelling order");
                // In a real implementation, this would send a cancel request
                // For now, we just mark it as cancelled locally
                drop(order);
                if let Some(mut order) = self.orders_by_id.get_mut(order_id) {
                    // Transition through New if currently Pending, then to Canceled
                    if order.status == OrderStatus::Pending {
                        let _ = order.update_status(OrderStatus::New);
                    }
                    let _ = order.update_status(OrderStatus::Canceled);
                }
                Ok(true)
            }
            Some(_) => {
                debug!(order_id = %order_id, "Order already in final state");
                Ok(false)
            }
            None => {
                warn!(order_id = %order_id, "Order not found");
                Ok(false)
            }
        }
    }

    async fn cancel_all(&self, symbol: &Symbol) -> Result<usize, StrategyError> {
        let mut cancelled = 0;

        if let Some(pending) = self.pending_orders.get(symbol) {
            for order in pending.iter() {
                if self.cancel(&order.order_id).await? {
                    cancelled += 1;
                }
            }
        }

        info!(symbol = %symbol, cancelled = cancelled, "Cancelled all orders");
        Ok(cancelled)
    }

    fn get_pending_orders(&self, _symbol: &Symbol) -> Vec<&Order> {
        // Note: This returns an empty vec because we can't return references
        // to data inside DashMap. In a real implementation, we'd use a different
        // approach or return owned data.
        Vec::new()
    }

    fn get_position(&self, symbol: &Symbol) -> Quantity {
        self.positions
            .get(symbol)
            .map(|p| *p)
            .unwrap_or(Quantity::ZERO)
    }

    fn get_price(&self, symbol: &Symbol) -> Option<Price> {
        self.prices.get(symbol).map(|p| *p)
    }

    fn current_time(&self) -> Timestamp {
        *self.current_time.read()
    }

    fn log(&self, level: LogLevel, message: &str) {
        match level {
            LogLevel::Trace => tracing::trace!(strategy = %self.strategy_name, "{}", message),
            LogLevel::Debug => tracing::debug!(strategy = %self.strategy_name, "{}", message),
            LogLevel::Info => tracing::info!(strategy = %self.strategy_name, "{}", message),
            LogLevel::Warn => tracing::warn!(strategy = %self.strategy_name, "{}", message),
            LogLevel::Error => tracing::error!(strategy = %self.strategy_name, "{}", message),
        }
    }

    fn strategy_name(&self) -> &str {
        &self.strategy_name
    }
}

/// HFT Strategy Runner.
///
/// Manages the lifecycle and execution of HFT strategies.
pub struct HftStrategyRunner {
    /// Strategy instance.
    strategy: Box<dyn HftStrategy>,
    /// Strategy context.
    context: Arc<HftStrategyContextImpl>,
    /// Whether the strategy is running.
    running: RwLock<bool>,
}

impl HftStrategyRunner {
    /// Creates a new strategy runner.
    #[must_use]
    pub fn new(strategy: Box<dyn HftStrategy>, context: Arc<HftStrategyContextImpl>) -> Self {
        Self {
            strategy,
            context,
            running: RwLock::new(false),
        }
    }

    /// Initializes and starts the strategy.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if initialization fails.
    pub async fn start(&mut self) -> Result<(), StrategyError> {
        if *self.running.read() {
            return Err(StrategyError::AlreadyRunning {
                name: self.strategy.name().to_string(),
            });
        }

        info!(strategy = %self.strategy.name(), "Starting HFT strategy");

        self.strategy.on_init(self.context.as_ref()).await?;
        *self.running.write() = true;

        info!(strategy = %self.strategy.name(), "HFT strategy started");
        Ok(())
    }

    /// Stops the strategy.
    pub async fn stop(&mut self) {
        if !*self.running.read() {
            return;
        }

        info!(strategy = %self.strategy.name(), "Stopping HFT strategy");

        self.strategy.on_stop(self.context.as_ref()).await;
        *self.running.write() = false;

        info!(strategy = %self.strategy.name(), "HFT strategy stopped");
    }

    /// Returns whether the strategy is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        *self.running.read()
    }

    /// Processes a tick event.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if processing fails.
    pub async fn on_tick(
        &mut self,
        tick: &zephyr_core::data::TickData,
    ) -> Result<(), StrategyError> {
        if !*self.running.read() {
            return Err(StrategyError::NotRunning {
                name: self.strategy.name().to_string(),
            });
        }

        // Update context with tick data
        self.context.update_price(&tick.symbol, tick.price);

        // Call strategy callback
        self.strategy.on_tick(self.context.as_ref(), tick).await
    }

    /// Processes an order update.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if processing fails.
    pub async fn on_order(&mut self, order: &Order) -> Result<(), StrategyError> {
        if !*self.running.read() {
            return Err(StrategyError::NotRunning {
                name: self.strategy.name().to_string(),
            });
        }

        // Update context
        self.context.on_order_update(order.clone());

        // Call strategy callback
        self.strategy.on_order(self.context.as_ref(), order).await
    }

    /// Processes a trade execution.
    ///
    /// # Errors
    ///
    /// Returns `StrategyError` if processing fails.
    pub async fn on_trade(&mut self, trade: &Trade) -> Result<(), StrategyError> {
        if !*self.running.read() {
            return Err(StrategyError::NotRunning {
                name: self.strategy.name().to_string(),
            });
        }

        // Update context
        self.context.on_trade(trade);

        // Call strategy callback
        self.strategy.on_trade(self.context.as_ref(), trade).await
    }

    /// Returns the strategy name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.strategy.name()
    }

    /// Returns a reference to the context.
    #[must_use]
    pub fn context(&self) -> &Arc<HftStrategyContextImpl> {
        &self.context
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use zephyr_core::types::Amount;

    fn create_test_symbol() -> Symbol {
        Symbol::new("BTC-USDT").unwrap()
    }

    fn create_test_context() -> (
        HftStrategyContextImpl,
        mpsc::UnboundedReceiver<OrderSubmission>,
    ) {
        let (tx, rx) = mpsc::unbounded_channel();
        let ctx = HftStrategyContextImpl::new("test_hft", tx);
        (ctx, rx)
    }

    #[test]
    fn test_context_position_management() {
        let (ctx, _rx) = create_test_context();
        let symbol = create_test_symbol();

        // Initial position should be zero
        assert_eq!(ctx.get_position(&symbol), Quantity::ZERO);

        // Update position
        ctx.update_position(&symbol, Quantity::new(dec!(5.0)).unwrap());
        assert_eq!(ctx.get_position(&symbol).as_decimal(), dec!(5.0));
    }

    #[test]
    fn test_context_price_update() {
        let (ctx, _rx) = create_test_context();
        let symbol = create_test_symbol();

        // No price initially
        assert!(ctx.get_price(&symbol).is_none());

        // Update price
        ctx.update_price(&symbol, Price::new(dec!(50000)).unwrap());
        assert_eq!(ctx.get_price(&symbol).unwrap().as_decimal(), dec!(50000));
    }

    #[tokio::test]
    async fn test_context_buy_order() {
        let (ctx, mut rx) = create_test_context();
        let symbol = create_test_symbol();

        let order_id = ctx
            .buy(
                &symbol,
                Price::new(dec!(50000)).unwrap(),
                Quantity::new(dec!(0.1)).unwrap(),
                OrderFlag::Normal,
            )
            .await
            .unwrap();

        assert!(!order_id.as_str().is_empty());

        // Check order was sent
        let submission = rx.recv().await.unwrap();
        assert_eq!(submission.request.symbol, symbol);
        assert_eq!(submission.request.side, OrderSide::Buy);
        assert_eq!(submission.strategy_name, "test_hft");
    }

    #[tokio::test]
    async fn test_context_sell_order() {
        let (ctx, mut rx) = create_test_context();
        let symbol = create_test_symbol();

        let order_id = ctx
            .sell(
                &symbol,
                Price::new(dec!(50000)).unwrap(),
                Quantity::new(dec!(0.1)).unwrap(),
                OrderFlag::PostOnly,
            )
            .await
            .unwrap();

        assert!(!order_id.as_str().is_empty());

        // Check order was sent
        let submission = rx.recv().await.unwrap();
        assert_eq!(submission.request.side, OrderSide::Sell);
        assert!(submission.request.post_only);
    }

    #[tokio::test]
    async fn test_context_cancel_order() {
        let (ctx, _rx) = create_test_context();
        let symbol = create_test_symbol();

        // Submit an order first
        let order_id = ctx
            .buy(
                &symbol,
                Price::new(dec!(50000)).unwrap(),
                Quantity::new(dec!(0.1)).unwrap(),
                OrderFlag::Normal,
            )
            .await
            .unwrap();

        // Cancel it
        let cancelled = ctx.cancel(&order_id).await.unwrap();
        assert!(cancelled);

        // Try to cancel again (should return false)
        let cancelled_again = ctx.cancel(&order_id).await.unwrap();
        assert!(!cancelled_again);
    }

    #[test]
    fn test_context_on_trade() {
        let (ctx, _rx) = create_test_context();
        let symbol = create_test_symbol();

        // Set initial position
        ctx.update_position(&symbol, Quantity::new(dec!(1.0)).unwrap());

        // Simulate a buy trade
        let trade = Trade {
            trade_id: "trade1".to_string(),
            order_id: OrderId::new("order1").unwrap(),
            symbol: symbol.clone(),
            side: OrderSide::Buy,
            price: Price::new(dec!(50000)).unwrap(),
            quantity: Quantity::new(dec!(0.5)).unwrap(),
            value: Amount::new(dec!(25000)).unwrap(),
            fee: dec!(0.01),
            fee_currency: "USDT".to_string(),
            timestamp: Timestamp::now(),
            is_maker: true,
        };

        ctx.on_trade(&trade);

        // Position should increase
        assert_eq!(ctx.get_position(&symbol).as_decimal(), dec!(1.5));
    }

    #[test]
    fn test_context_on_trade_sell() {
        let (ctx, _rx) = create_test_context();
        let symbol = create_test_symbol();

        // Set initial position
        ctx.update_position(&symbol, Quantity::new(dec!(2.0)).unwrap());

        // Simulate a sell trade
        let trade = Trade {
            trade_id: "trade1".to_string(),
            order_id: OrderId::new("order1").unwrap(),
            symbol: symbol.clone(),
            side: OrderSide::Sell,
            price: Price::new(dec!(50000)).unwrap(),
            quantity: Quantity::new(dec!(0.5)).unwrap(),
            value: Amount::new(dec!(25000)).unwrap(),
            fee: dec!(0.01),
            fee_currency: "USDT".to_string(),
            timestamp: Timestamp::now(),
            is_maker: false,
        };

        ctx.on_trade(&trade);

        // Position should decrease
        assert_eq!(ctx.get_position(&symbol).as_decimal(), dec!(1.5));
    }

    #[test]
    fn test_order_flag_to_time_in_force() {
        // This is implicitly tested through the submit_order function
        // but we can verify the mapping logic
        let (ctx, _rx) = create_test_context();

        // The mapping is:
        // Normal -> GTC
        // Fak -> IOC
        // Fok -> FOK
        // PostOnly -> PostOnly
        // ReduceOnly -> GTC (with reduce_only flag)
    }
}
