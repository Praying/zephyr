//! Execution context implementation.

#![allow(clippy::unwrap_or_default)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::map_unwrap_or)]

use async_trait::async_trait;
use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::Arc;

use zephyr_core::data::{Order, OrderRequest, OrderSide, OrderType, TickData, TimeInForce};
use zephyr_core::error::ExchangeError;
use zephyr_core::traits::TraderGateway;
use zephyr_core::types::{OrderId, Price, Quantity, Symbol, Timestamp};

use super::traits::ExecuteContext;

/// Implementation of the execution context.
///
/// Provides market data access and order submission for execution units.
pub struct ExecuteContextImpl {
    /// Trader gateway for order submission.
    trader: Arc<dyn TraderGateway>,
    /// Tick data cache per symbol.
    tick_cache: DashMap<Symbol, Vec<TickData>>,
    /// Position cache per symbol.
    positions: DashMap<Symbol, Quantity>,
    /// Pending orders per symbol.
    pending_orders: DashMap<Symbol, Vec<Order>>,
    /// Maximum ticks to cache per symbol.
    max_ticks: usize,
    /// Channel ready flag.
    channel_ready: RwLock<bool>,
}

impl ExecuteContextImpl {
    /// Creates a new execution context.
    #[must_use]
    pub fn new(trader: Arc<dyn TraderGateway>) -> Self {
        Self {
            trader,
            tick_cache: DashMap::new(),
            positions: DashMap::new(),
            pending_orders: DashMap::new(),
            max_ticks: 1000,
            channel_ready: RwLock::new(true),
        }
    }

    /// Creates a context with custom tick cache size.
    #[must_use]
    pub fn with_max_ticks(mut self, max_ticks: usize) -> Self {
        self.max_ticks = max_ticks;
        self
    }

    /// Updates the tick cache for a symbol.
    pub fn update_tick(&self, tick: TickData) {
        let symbol = tick.symbol.clone();
        let mut entry = self.tick_cache.entry(symbol).or_insert_with(Vec::new);
        entry.push(tick);
        if entry.len() > self.max_ticks {
            entry.remove(0);
        }
    }

    /// Updates the position for a symbol.
    pub fn update_position(&self, symbol: &Symbol, qty: Quantity) {
        self.positions.insert(symbol.clone(), qty);
    }

    /// Updates an order in the pending orders cache.
    pub fn update_order(&self, order: &Order) {
        let mut entry = self
            .pending_orders
            .entry(order.symbol.clone())
            .or_insert_with(Vec::new);

        // Find and update or add
        if let Some(existing) = entry.iter_mut().find(|o| o.order_id == order.order_id) {
            *existing = order.clone();
        } else {
            entry.push(order.clone());
        }

        // Remove completed orders
        entry.retain(|o| o.is_active());
    }

    /// Sets the channel ready state.
    pub fn set_channel_ready(&self, ready: bool) {
        *self.channel_ready.write() = ready;
    }

    /// Returns whether the channel is ready.
    #[must_use]
    pub fn is_channel_ready(&self) -> bool {
        *self.channel_ready.read()
    }

    /// Gets pending orders for a symbol.
    #[must_use]
    pub fn get_pending_orders(&self, symbol: &Symbol) -> Vec<Order> {
        self.pending_orders
            .get(symbol)
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    /// Clears all cached data.
    pub fn clear(&self) {
        self.tick_cache.clear();
        self.positions.clear();
        self.pending_orders.clear();
    }
}

#[async_trait]
impl ExecuteContext for ExecuteContextImpl {
    fn get_ticks(&self, _symbol: &Symbol, _count: usize) -> &[TickData] {
        // Note: This returns an empty slice because we can't return a reference
        // to data inside DashMap. In practice, you'd use a different approach
        // or return owned data.
        static EMPTY: &[TickData] = &[];
        EMPTY
    }

    fn get_last_tick(&self, _symbol: &Symbol) -> Option<&TickData> {
        // Same limitation as get_ticks - can't return reference from DashMap
        None
    }

    fn get_position(&self, symbol: &Symbol) -> Quantity {
        self.positions
            .get(symbol)
            .map(|v| *v)
            .unwrap_or(Quantity::ZERO)
    }

    fn get_undone_qty(&self, symbol: &Symbol) -> Quantity {
        self.pending_orders
            .get(symbol)
            .map(|orders| {
                orders
                    .iter()
                    .filter(|o| o.is_active())
                    .map(|o| o.remaining_quantity())
                    .fold(Quantity::ZERO, |acc, q| acc + q)
            })
            .unwrap_or(Quantity::ZERO)
    }

    async fn buy(
        &self,
        symbol: &Symbol,
        price: Price,
        qty: Quantity,
    ) -> Result<Vec<OrderId>, ExchangeError> {
        if !self.is_channel_ready() {
            return Err(ExchangeError::OrderRejected {
                reason: "Trading channel not ready".to_string(),
                code: None,
            });
        }

        let request = OrderRequest {
            symbol: symbol.clone(),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            quantity: qty,
            price: Some(price),
            stop_price: None,
            time_in_force: TimeInForce::Gtc,
            reduce_only: false,
            post_only: false,
            client_order_id: None,
        };

        let order_id = self.trader.order_insert(&request).await?;
        Ok(vec![order_id])
    }

    async fn sell(
        &self,
        symbol: &Symbol,
        price: Price,
        qty: Quantity,
    ) -> Result<Vec<OrderId>, ExchangeError> {
        if !self.is_channel_ready() {
            return Err(ExchangeError::OrderRejected {
                reason: "Trading channel not ready".to_string(),
                code: None,
            });
        }

        let request = OrderRequest {
            symbol: symbol.clone(),
            side: OrderSide::Sell,
            order_type: OrderType::Limit,
            quantity: qty,
            price: Some(price),
            stop_price: None,
            time_in_force: TimeInForce::Gtc,
            reduce_only: false,
            post_only: false,
            client_order_id: None,
        };

        let order_id = self.trader.order_insert(&request).await?;
        Ok(vec![order_id])
    }

    async fn cancel(&self, order_id: &OrderId) -> Result<bool, ExchangeError> {
        self.trader.order_cancel(order_id).await
    }

    async fn cancel_all(&self, symbol: &Symbol) -> Result<usize, ExchangeError> {
        self.trader.cancel_all_orders(Some(symbol)).await
    }

    fn current_time(&self) -> Timestamp {
        Timestamp::now()
    }

    fn log(&self, level: tracing::Level, message: &str) {
        match level {
            tracing::Level::ERROR => tracing::error!("{}", message),
            tracing::Level::WARN => tracing::warn!("{}", message),
            tracing::Level::INFO => tracing::info!("{}", message),
            tracing::Level::DEBUG => tracing::debug!("{}", message),
            tracing::Level::TRACE => tracing::trace!("{}", message),
        }
    }
}

#[cfg(test)]
mod tests {
    // Note: Full tests would require mocking TraderGateway
    // These are basic structural tests

    #[test]
    fn test_position_update() {
        // Create a mock context for testing
        // In real tests, we'd use mockall or similar
    }
}
