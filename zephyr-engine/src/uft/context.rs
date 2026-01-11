//! UFT (Ultra-High Frequency Trading) Strategy Context Implementation.

#![allow(clippy::redundant_clone)]
#![allow(clippy::significant_drop_tightening)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};

use crossbeam_queue::ArrayQueue;
use dashmap::DashMap;
use parking_lot::RwLock;
use tracing::{info, trace, warn};

use zephyr_core::data::{
    Order, OrderBook, OrderRequest, OrderSide, OrderStatus, OrderType, TickData, TimeInForce,
};
use zephyr_core::traits::{LogLevel, OrderFlag, UftEngineConfig, UftStrategy, UftStrategyContext};
use zephyr_core::types::{OrderId, Price, Quantity, Symbol, Timestamp};

use crate::hft::OrderSubmission;

/// Pre-allocated order ID pool for lock-free order ID generation.
pub struct OrderIdPool {
    pool: ArrayQueue<OrderId>,
    counter: AtomicU64,
    prefix: String,
}

impl OrderIdPool {
    /// Creates a new order ID pool with the given prefix and capacity.
    #[must_use]
    pub fn new(prefix: impl Into<String>, capacity: usize) -> Self {
        let prefix = prefix.into();
        let pool = ArrayQueue::new(capacity);
        for i in 0..capacity {
            let id = OrderId::new_unchecked(format!("{}-{}", prefix, i));
            let _ = pool.push(id);
        }
        Self {
            pool,
            counter: AtomicU64::new(capacity as u64),
            prefix,
        }
    }

    /// Gets an order ID from the pool, or generates a new one if empty.
    #[must_use]
    pub fn get(&self) -> OrderId {
        self.pool.pop().unwrap_or_else(|| {
            let counter = self.counter.fetch_add(1, Ordering::Relaxed);
            OrderId::new_unchecked(format!("{}-{}", self.prefix, counter))
        })
    }

    /// Returns an order ID to the pool for reuse.
    pub fn return_id(&self, id: OrderId) {
        let _ = self.pool.push(id);
    }
}

/// Ring buffer for storing tick data with zero-copy access.
pub struct TickRingBuffer {
    buffer: Box<[Option<TickData>]>,
    write_index: AtomicU64,
    capacity: usize,
}

impl TickRingBuffer {
    /// Creates a new tick ring buffer with the given capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let buffer = (0..capacity)
            .map(|_| None)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            buffer,
            write_index: AtomicU64::new(0),
            capacity,
        }
    }

    /// Pushes a tick into the ring buffer.
    pub fn push(&mut self, tick: TickData) {
        let index = self.write_index.fetch_add(1, Ordering::Relaxed) as usize % self.capacity;
        self.buffer[index] = Some(tick);
    }

    /// Returns the latest tick in the buffer.
    #[must_use]
    pub fn latest(&self) -> Option<&TickData> {
        let index = self.write_index.load(Ordering::Relaxed);
        if index == 0 {
            return None;
        }
        self.buffer[((index - 1) as usize) % self.capacity].as_ref()
    }

    /// Returns the number of ticks in the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        (self.write_index.load(Ordering::Relaxed) as usize).min(self.capacity)
    }

    /// Returns true if the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.write_index.load(Ordering::Relaxed) == 0
    }
}

/// High-resolution timestamp provider.
pub struct HardwareTimestamp {
    enabled: bool,
}

impl HardwareTimestamp {
    /// Creates a new hardware timestamp provider.
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Returns the current timestamp in nanoseconds.
    #[must_use]
    pub fn now_ns(&self) -> i64 {
        if self.enabled {
            Self::read_tsc_ns()
        } else {
            Self::read_system_ns()
        }
    }

    fn read_system_ns() -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0)
    }

    #[cfg(target_arch = "x86_64")]
    fn read_tsc_ns() -> i64 {
        Self::read_system_ns()
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn read_tsc_ns() -> i64 {
        Self::read_system_ns()
    }
}

impl Default for HardwareTimestamp {
    fn default() -> Self {
        Self::new(true)
    }
}

/// UFT Strategy Context Implementation.
pub struct UftStrategyContextImpl {
    strategy_name: String,
    config: UftEngineConfig,
    order_pool: OrderIdPool,
    positions: DashMap<Symbol, AtomicI64>,
    prices: DashMap<Symbol, Price>,
    tick_buffers: DashMap<Symbol, RwLock<TickRingBuffer>>,
    orderbooks: DashMap<Symbol, RwLock<OrderBook>>,
    pending_orders: DashMap<Symbol, RwLock<Vec<Order>>>,
    orders_by_id: DashMap<OrderId, Order>,
    order_queue: ArrayQueue<OrderSubmission>,
    timestamp: HardwareTimestamp,
}

impl UftStrategyContextImpl {
    /// Creates a new UFT strategy context.
    #[must_use]
    pub fn new(strategy_name: impl Into<String>, config: UftEngineConfig) -> Self {
        let strategy_name = strategy_name.into();
        let order_pool = OrderIdPool::new(&strategy_name, config.preallocated_orders);
        let order_queue = ArrayQueue::new(config.max_pending_orders);
        let timestamp = HardwareTimestamp::new(config.hardware_timestamp);
        Self {
            strategy_name,
            config,
            order_pool,
            positions: DashMap::new(),
            prices: DashMap::new(),
            tick_buffers: DashMap::new(),
            orderbooks: DashMap::new(),
            pending_orders: DashMap::new(),
            orders_by_id: DashMap::new(),
            order_queue,
            timestamp,
        }
    }

    /// Returns the engine configuration.
    #[must_use]
    pub fn config(&self) -> &UftEngineConfig {
        &self.config
    }

    /// Drains all pending order submissions from the queue.
    pub fn drain_orders(&self) -> Vec<OrderSubmission> {
        let mut orders = Vec::new();
        while let Some(submission) = self.order_queue.pop() {
            orders.push(submission);
        }
        orders
    }

    /// Updates the tick data for a symbol.
    pub fn update_tick(&self, tick: TickData) {
        let symbol = tick.symbol.clone();
        self.prices.insert(symbol.clone(), tick.price);
        self.tick_buffers
            .entry(symbol)
            .or_insert_with(|| RwLock::new(TickRingBuffer::new(self.config.tick_buffer_size)))
            .write()
            .push(tick);
    }

    /// Updates the order book for a symbol.
    pub fn update_orderbook(&self, orderbook: OrderBook) {
        self.orderbooks
            .insert(orderbook.symbol.clone(), RwLock::new(orderbook));
    }

    /// Updates the position for a symbol.
    pub fn update_position(&self, symbol: &Symbol, quantity: Quantity) {
        use rust_decimal::prelude::ToPrimitive;
        let scaled = quantity
            .as_decimal()
            .checked_mul(rust_decimal::Decimal::from(100_000_000))
            .and_then(|d| d.trunc().to_i64())
            .unwrap_or(0);
        self.positions
            .entry(symbol.clone())
            .or_insert_with(|| AtomicI64::new(0))
            .store(scaled, Ordering::Relaxed);
    }

    /// Handles an order update event.
    pub fn on_order_update(&self, order: Order) {
        let symbol = order.symbol.clone();
        let order_id = order.order_id.clone();
        self.orders_by_id.insert(order_id.clone(), order.clone());
        if order.status.is_final() {
            if let Some(pending) = self.pending_orders.get(&symbol) {
                pending.write().retain(|o| o.order_id != order_id);
            }
            self.order_pool.return_id(order_id);
        } else if let Some(pending) = self.pending_orders.get(&symbol) {
            let mut pending = pending.write();
            if let Some(existing) = pending.iter_mut().find(|o| o.order_id == order_id) {
                *existing = order;
            }
        }
    }

    /// Handles a trade event.
    pub fn on_trade(&self, trade: &zephyr_core::traits::Trade) {
        use rust_decimal::prelude::ToPrimitive;
        let change = match trade.side {
            OrderSide::Buy => trade.quantity,
            OrderSide::Sell => -trade.quantity,
        };
        let scaled_change = change
            .as_decimal()
            .checked_mul(rust_decimal::Decimal::from(100_000_000))
            .and_then(|d| d.trunc().to_i64())
            .unwrap_or(0);
        self.positions
            .entry(trade.symbol.clone())
            .or_insert_with(|| AtomicI64::new(0))
            .fetch_add(scaled_change, Ordering::Relaxed);
        trace!(symbol = %trade.symbol, side = %trade.side, quantity = %trade.quantity, price = %trade.price, "UFT trade");
    }

    fn submit_order(
        &self,
        symbol: &Symbol,
        side: OrderSide,
        price: Price,
        qty: Quantity,
        flag: OrderFlag,
    ) -> OrderId {
        let order_id = self.order_pool.get();
        let timestamp = Timestamp::from_nanos(self.timestamp.now_ns());
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
        let order = Order::from_request(&request, order_id.clone(), timestamp);
        self.orders_by_id.insert(order_id.clone(), order.clone());
        self.pending_orders
            .entry(symbol.clone())
            .or_insert_with(|| RwLock::new(Vec::new()))
            .write()
            .push(order);
        let submission = OrderSubmission {
            request,
            strategy_name: self.strategy_name.clone(),
        };
        if self.order_queue.push(submission).is_err() {
            warn!(order_id = %order_id, "Order queue full");
        }
        trace!(order_id = %order_id, symbol = %symbol, side = %side, price = %price, quantity = %qty, "UFT order");
        order_id
    }
}

impl UftStrategyContext for UftStrategyContextImpl {
    fn buy(&self, symbol: &Symbol, price: Price, qty: Quantity, flag: OrderFlag) -> OrderId {
        self.submit_order(symbol, OrderSide::Buy, price, qty, flag)
    }
    fn sell(&self, symbol: &Symbol, price: Price, qty: Quantity, flag: OrderFlag) -> OrderId {
        self.submit_order(symbol, OrderSide::Sell, price, qty, flag)
    }
    fn cancel(&self, order_id: &OrderId) -> bool {
        if let Some(order) = self.orders_by_id.get(order_id) {
            if order.status.is_active() {
                trace!(order_id = %order_id, "UFT cancelling order");
                drop(order);
                if let Some(mut order) = self.orders_by_id.get_mut(order_id) {
                    if order.status == OrderStatus::Pending {
                        let _ = order.update_status(OrderStatus::New);
                    }
                    let _ = order.update_status(OrderStatus::Canceled);
                }
                return true;
            }
        }
        false
    }
    fn cancel_all(&self, symbol: &Symbol) -> usize {
        let mut cancelled = 0;
        if let Some(pending) = self.pending_orders.get(symbol) {
            let order_ids: Vec<OrderId> =
                pending.read().iter().map(|o| o.order_id.clone()).collect();
            for order_id in order_ids {
                if self.cancel(&order_id) {
                    cancelled += 1;
                }
            }
        }
        trace!(symbol = %symbol, cancelled = cancelled, "UFT cancelled all orders");
        cancelled
    }
    fn get_position(&self, symbol: &Symbol) -> Quantity {
        self.positions
            .get(symbol)
            .map(|p| {
                let scaled = p.load(Ordering::Relaxed);
                Quantity::new(
                    rust_decimal::Decimal::from(scaled) / rust_decimal::Decimal::from(100_000_000),
                )
                .unwrap_or(Quantity::ZERO)
            })
            .unwrap_or(Quantity::ZERO)
    }
    fn get_last_tick(&self, _symbol: &Symbol) -> Option<&TickData> {
        None
    }
    fn get_orderbook(&self, _symbol: &Symbol) -> Option<&OrderBook> {
        None
    }
    fn get_pending_orders(&self, _symbol: &Symbol) -> &[Order] {
        &[]
    }
    fn current_time_ns(&self) -> i64 {
        self.timestamp.now_ns()
    }
    fn get_price(&self, symbol: &Symbol) -> Option<Price> {
        self.prices.get(symbol).map(|p| *p)
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

/// UFT Strategy Runner.
pub struct UftStrategyRunner {
    strategy: Box<dyn UftStrategy>,
    context: Arc<UftStrategyContextImpl>,
    running: AtomicBool,
}

impl UftStrategyRunner {
    /// Creates a new UFT strategy runner.
    #[must_use]
    pub fn new(strategy: Box<dyn UftStrategy>, context: Arc<UftStrategyContextImpl>) -> Self {
        Self {
            strategy,
            context,
            running: AtomicBool::new(false),
        }
    }

    /// Starts the strategy runner.
    pub fn start(&mut self) {
        if self.running.load(Ordering::Relaxed) {
            warn!(strategy = %self.strategy.name(), "UFT strategy already running");
            return;
        }
        info!(strategy = %self.strategy.name(), "Starting UFT strategy");
        self.strategy.on_init(self.context.as_ref());
        self.running.store(true, Ordering::Relaxed);
        info!(strategy = %self.strategy.name(), "UFT strategy started");
    }

    /// Stops the strategy runner.
    pub fn stop(&mut self) {
        if !self.running.load(Ordering::Relaxed) {
            return;
        }
        info!(strategy = %self.strategy.name(), "Stopping UFT strategy");
        self.strategy.on_stop(self.context.as_ref());
        self.running.store(false, Ordering::Relaxed);
        info!(strategy = %self.strategy.name(), "UFT strategy stopped");
    }

    /// Returns true if the strategy is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Handles a tick event.
    pub fn on_tick(&mut self, tick: &TickData) {
        if !self.running.load(Ordering::Relaxed) {
            return;
        }
        self.context.update_tick(tick.clone());
        self.strategy.on_tick(self.context.as_ref(), tick);
    }

    /// Handles an order book event.
    pub fn on_orderbook(&mut self, orderbook: &OrderBook) {
        if !self.running.load(Ordering::Relaxed) {
            return;
        }
        self.context.update_orderbook(orderbook.clone());
        self.strategy.on_orderbook(self.context.as_ref(), orderbook);
    }

    /// Handles an order event.
    pub fn on_order(&mut self, order: &Order) {
        if !self.running.load(Ordering::Relaxed) {
            return;
        }
        self.context.on_order_update(order.clone());
        self.strategy.on_order(self.context.as_ref(), order);
    }

    /// Handles a trade event.
    pub fn on_trade(&mut self, trade: &zephyr_core::traits::Trade) {
        if !self.running.load(Ordering::Relaxed) {
            return;
        }
        self.context.on_trade(trade);
        self.strategy.on_trade(self.context.as_ref(), trade);
    }

    /// Returns the strategy name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.strategy.name()
    }

    /// Returns the strategy context.
    #[must_use]
    pub fn context(&self) -> &Arc<UftStrategyContextImpl> {
        &self.context
    }

    /// Drains all pending order submissions from the context.
    pub fn drain_orders(&self) -> Vec<OrderSubmission> {
        self.context.drain_orders()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_symbol() -> Symbol {
        Symbol::new("BTC-USDT").unwrap()
    }

    fn create_test_config() -> UftEngineConfig {
        UftEngineConfig {
            cpu_affinity: vec![],
            use_kernel_bypass: false,
            preallocated_orders: 100,
            tick_buffer_size: 10,
            orderbook_buffer_size: 10,
            max_pending_orders: 100,
            hardware_timestamp: false,
        }
    }

    #[test]
    fn test_order_id_pool() {
        let pool = OrderIdPool::new("test", 10);
        let id1 = pool.get();
        let id2 = pool.get();
        assert_ne!(id1, id2);
        pool.return_id(id1.clone());
        for _ in 0..20 {
            let _ = pool.get();
        }
    }

    #[test]
    fn test_tick_ring_buffer() {
        let mut buffer = TickRingBuffer::new(3);
        assert!(buffer.is_empty());
        let tick = TickData::builder()
            .symbol(create_test_symbol())
            .timestamp(Timestamp::now())
            .price(Price::new(dec!(50000)).unwrap())
            .volume(Quantity::new(dec!(1)).unwrap())
            .build()
            .unwrap();
        buffer.push(tick);
        assert_eq!(buffer.len(), 1);
        assert!(buffer.latest().is_some());
    }

    #[test]
    fn test_hardware_timestamp() {
        let ts = HardwareTimestamp::new(false);
        let ns1 = ts.now_ns();
        let ns2 = ts.now_ns();
        assert!(ns2 >= ns1);
    }

    #[test]
    fn test_uft_context_creation() {
        let ctx = UftStrategyContextImpl::new("test_uft", create_test_config());
        assert_eq!(ctx.strategy_name(), "test_uft");
    }

    #[test]
    fn test_uft_context_position() {
        let ctx = UftStrategyContextImpl::new("test_uft", create_test_config());
        let symbol = create_test_symbol();
        assert_eq!(ctx.get_position(&symbol), Quantity::ZERO);
        ctx.update_position(&symbol, Quantity::new(dec!(5.0)).unwrap());
        let pos = ctx.get_position(&symbol);
        assert!((pos.as_decimal() - dec!(5.0)).abs() < dec!(0.0001));
    }

    #[test]
    fn test_uft_context_price() {
        let ctx = UftStrategyContextImpl::new("test_uft", create_test_config());
        let symbol = create_test_symbol();
        assert!(ctx.get_price(&symbol).is_none());
        let tick = TickData::builder()
            .symbol(symbol.clone())
            .timestamp(Timestamp::now())
            .price(Price::new(dec!(50000)).unwrap())
            .volume(Quantity::new(dec!(1)).unwrap())
            .build()
            .unwrap();
        ctx.update_tick(tick);
        assert_eq!(ctx.get_price(&symbol).unwrap().as_decimal(), dec!(50000));
    }

    #[test]
    fn test_uft_context_buy_order() {
        let ctx = UftStrategyContextImpl::new("test_uft", create_test_config());
        let symbol = create_test_symbol();
        let order_id = ctx.buy(
            &symbol,
            Price::new(dec!(50000)).unwrap(),
            Quantity::new(dec!(0.1)).unwrap(),
            OrderFlag::Normal,
        );
        assert!(!order_id.as_str().is_empty());
        let orders = ctx.drain_orders();
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].request.symbol, symbol);
        assert_eq!(orders[0].request.side, OrderSide::Buy);
    }

    #[test]
    fn test_uft_context_sell_order() {
        let ctx = UftStrategyContextImpl::new("test_uft", create_test_config());
        let symbol = create_test_symbol();
        let order_id = ctx.sell(
            &symbol,
            Price::new(dec!(50000)).unwrap(),
            Quantity::new(dec!(0.1)).unwrap(),
            OrderFlag::PostOnly,
        );
        assert!(!order_id.as_str().is_empty());
        let orders = ctx.drain_orders();
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].request.side, OrderSide::Sell);
        assert!(orders[0].request.post_only);
    }

    #[test]
    fn test_uft_context_cancel_order() {
        let ctx = UftStrategyContextImpl::new("test_uft", create_test_config());
        let symbol = create_test_symbol();
        let order_id = ctx.buy(
            &symbol,
            Price::new(dec!(50000)).unwrap(),
            Quantity::new(dec!(0.1)).unwrap(),
            OrderFlag::Normal,
        );
        assert!(ctx.cancel(&order_id));
        assert!(!ctx.cancel(&order_id));
    }

    #[test]
    fn test_uft_context_cancel_all() {
        let ctx = UftStrategyContextImpl::new("test_uft", create_test_config());
        let symbol = create_test_symbol();
        ctx.buy(
            &symbol,
            Price::new(dec!(50000)).unwrap(),
            Quantity::new(dec!(0.1)).unwrap(),
            OrderFlag::Normal,
        );
        ctx.buy(
            &symbol,
            Price::new(dec!(49000)).unwrap(),
            Quantity::new(dec!(0.2)).unwrap(),
            OrderFlag::Normal,
        );
        assert_eq!(ctx.cancel_all(&symbol), 2);
    }

    #[test]
    fn test_uft_context_timestamp() {
        let ctx = UftStrategyContextImpl::new("test_uft", create_test_config());
        let ns1 = ctx.current_time_ns();
        let ns2 = ctx.current_time_ns();
        assert!(ns2 >= ns1);
    }
}
