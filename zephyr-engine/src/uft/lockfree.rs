//! Lock-Free Data Structures for UFT Strategies.
//!
//! This module provides lock-free data structures optimized for
//! ultra-high frequency trading scenarios.
//!
//! # Data Structures
//!
//! - [`LockFreeOrderPool`] - Pre-allocated pool of orders for zero-allocation submission
//! - [`SpscQueue`] - Single-producer single-consumer queue for order routing
//! - [`AtomicPrice`] - Atomic price storage for lock-free price updates
//!
//! # Design Principles
//!
//! - No heap allocations in hot path
//! - Cache-line aligned structures to avoid false sharing
//! - Memory ordering optimized for `x86_64` architecture
//!
//! # Safety
//!
//! This module contains unsafe code for performance-critical lock-free operations.
//! All unsafe code is carefully documented with safety invariants.

// Allow unsafe code in this module - it's required for lock-free data structures
#![allow(unsafe_code)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::uninlined_format_args)]

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};

use rust_decimal::Decimal;

use zephyr_core::data::OrderRequest;
use zephyr_core::types::{OrderId, Price, Quantity};

/// Cache line size for alignment (64 bytes on most modern CPUs).
const CACHE_LINE_SIZE: usize = 64;

/// Atomic price storage.
///
/// Stores a price as an atomic i64 (scaled by 1e8 for precision).
/// This allows lock-free price updates and reads.
#[repr(align(64))]
#[derive(Debug)]
pub struct AtomicPrice {
    /// Scaled price value (price * 1e8).
    value: AtomicI64,
}

impl AtomicPrice {
    /// Scale factor for price conversion.
    const SCALE: i64 = 100_000_000;

    /// Creates a new atomic price with the given value.
    #[must_use]
    pub fn new(price: Price) -> Self {
        Self {
            value: AtomicI64::new(Self::price_to_scaled(price)),
        }
    }

    /// Creates a new atomic price with zero value.
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            value: AtomicI64::new(0),
        }
    }

    /// Stores a price value.
    pub fn store(&self, price: Price) {
        self.value
            .store(Self::price_to_scaled(price), Ordering::Release);
    }

    /// Loads the current price value.
    #[must_use]
    pub fn load(&self) -> Option<Price> {
        let scaled = self.value.load(Ordering::Acquire);
        if scaled == 0 {
            return None;
        }
        Self::scaled_to_price(scaled)
    }

    /// Converts a Price to scaled i64.
    fn price_to_scaled(price: Price) -> i64 {
        use rust_decimal::prelude::ToPrimitive;
        price
            .as_decimal()
            .checked_mul(Decimal::from(Self::SCALE))
            .and_then(|d| d.trunc().to_i64())
            .unwrap_or(0)
    }

    /// Converts scaled i64 to Price.
    fn scaled_to_price(scaled: i64) -> Option<Price> {
        let decimal = Decimal::from(scaled) / Decimal::from(Self::SCALE);
        Price::new(decimal).ok()
    }
}

impl Default for AtomicPrice {
    fn default() -> Self {
        Self::zero()
    }
}

/// Atomic quantity storage.
///
/// Stores a quantity as an atomic i64 (scaled by 1e8 for precision).
#[repr(align(64))]
#[derive(Debug)]
pub struct AtomicQuantity {
    /// Scaled quantity value (quantity * 1e8).
    value: AtomicI64,
}

impl AtomicQuantity {
    /// Scale factor for quantity conversion.
    const SCALE: i64 = 100_000_000;

    /// Creates a new atomic quantity with the given value.
    #[must_use]
    pub fn new(qty: Quantity) -> Self {
        Self {
            value: AtomicI64::new(Self::qty_to_scaled(qty)),
        }
    }

    /// Creates a new atomic quantity with zero value.
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            value: AtomicI64::new(0),
        }
    }

    /// Stores a quantity value.
    pub fn store(&self, qty: Quantity) {
        self.value
            .store(Self::qty_to_scaled(qty), Ordering::Release);
    }

    /// Loads the current quantity value.
    #[must_use]
    pub fn load(&self) -> Quantity {
        let scaled = self.value.load(Ordering::Acquire);
        Self::scaled_to_qty(scaled)
    }

    /// Atomically adds to the quantity.
    pub fn fetch_add(&self, qty: Quantity) -> Quantity {
        let scaled = Self::qty_to_scaled(qty);
        let prev = self.value.fetch_add(scaled, Ordering::AcqRel);
        Self::scaled_to_qty(prev)
    }

    /// Converts a Quantity to scaled i64.
    fn qty_to_scaled(qty: Quantity) -> i64 {
        use rust_decimal::prelude::ToPrimitive;
        qty.as_decimal()
            .checked_mul(Decimal::from(Self::SCALE))
            .and_then(|d| d.trunc().to_i64())
            .unwrap_or(0)
    }

    /// Converts scaled i64 to Quantity.
    fn scaled_to_qty(scaled: i64) -> Quantity {
        let decimal = Decimal::from(scaled) / Decimal::from(Self::SCALE);
        Quantity::new(decimal).unwrap_or(Quantity::ZERO)
    }
}

impl Default for AtomicQuantity {
    fn default() -> Self {
        Self::zero()
    }
}

/// Pre-allocated order slot for lock-free order pool.
///
/// Each slot is cache-line aligned to avoid false sharing.
#[repr(align(64))]
pub struct OrderSlot {
    /// Whether this slot is in use.
    in_use: AtomicU64,
    /// The order data (only valid when in_use is true).
    order: UnsafeCell<Option<OrderRequest>>,
    /// Pre-generated order ID for this slot.
    order_id: OrderId,
}

// Safety: OrderSlot is safe to share between threads because:
// - in_use is atomic and provides synchronization
// - order is only accessed when in_use indicates ownership
unsafe impl Send for OrderSlot {}
unsafe impl Sync for OrderSlot {}

impl OrderSlot {
    /// Creates a new order slot with a pre-generated order ID.
    fn new(order_id: OrderId) -> Self {
        Self {
            in_use: AtomicU64::new(0),
            order: UnsafeCell::new(None),
            order_id,
        }
    }

    /// Tries to acquire this slot.
    ///
    /// Returns true if the slot was successfully acquired.
    fn try_acquire(&self) -> bool {
        self.in_use
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    }

    /// Releases this slot.
    fn release(&self) {
        // Clear the order data
        unsafe {
            *self.order.get() = None;
        }
        self.in_use.store(0, Ordering::Release);
    }

    /// Gets the order ID for this slot.
    fn order_id(&self) -> &OrderId {
        &self.order_id
    }

    /// Sets the order request (must be acquired first).
    ///
    /// # Safety
    ///
    /// Caller must have acquired this slot first.
    unsafe fn set_order(&self, order: OrderRequest) {
        unsafe {
            *self.order.get() = Some(order);
        }
    }

    /// Gets the order request (must be acquired first).
    ///
    /// # Safety
    ///
    /// Caller must have acquired this slot first.
    unsafe fn get_order(&self) -> Option<&OrderRequest> {
        unsafe { (*self.order.get()).as_ref() }
    }
}

/// Lock-free order pool.
///
/// Pre-allocates order slots to avoid heap allocation in the hot path.
/// Uses atomic operations for lock-free slot acquisition.
pub struct LockFreeOrderPool {
    /// Pre-allocated order slots.
    slots: Box<[OrderSlot]>,
    /// Current search index (hint for finding free slots).
    search_hint: AtomicUsize,
    /// Pool capacity.
    capacity: usize,
}

impl LockFreeOrderPool {
    /// Creates a new order pool with the specified capacity.
    #[must_use]
    pub fn new(prefix: &str, capacity: usize) -> Self {
        let slots: Vec<OrderSlot> = (0..capacity)
            .map(|i| {
                let order_id = OrderId::new_unchecked(format!("{}-{}", prefix, i));
                OrderSlot::new(order_id)
            })
            .collect();

        Self {
            slots: slots.into_boxed_slice(),
            search_hint: AtomicUsize::new(0),
            capacity,
        }
    }

    /// Acquires an order slot and returns its index and order ID.
    ///
    /// Returns `None` if no slots are available.
    pub fn acquire(&self) -> Option<(usize, OrderId)> {
        let start = self.search_hint.load(Ordering::Relaxed);

        // Search from hint to end
        for i in start..self.capacity {
            if self.slots[i].try_acquire() {
                self.search_hint
                    .store((i + 1) % self.capacity, Ordering::Relaxed);
                return Some((i, self.slots[i].order_id().clone()));
            }
        }

        // Search from beginning to hint
        for i in 0..start {
            if self.slots[i].try_acquire() {
                self.search_hint
                    .store((i + 1) % self.capacity, Ordering::Relaxed);
                return Some((i, self.slots[i].order_id().clone()));
            }
        }

        None
    }

    /// Releases an order slot.
    pub fn release(&self, index: usize) {
        if index < self.capacity {
            self.slots[index].release();
        }
    }

    /// Sets the order request for a slot.
    ///
    /// # Safety
    ///
    /// The slot must have been acquired first.
    pub fn set_order(&self, index: usize, order: OrderRequest) {
        if index < self.capacity {
            unsafe {
                self.slots[index].set_order(order);
            }
        }
    }

    /// Gets the order request from a slot.
    ///
    /// # Safety
    ///
    /// The slot must have been acquired first.
    pub fn get_order(&self, index: usize) -> Option<&OrderRequest> {
        if index < self.capacity {
            unsafe { self.slots[index].get_order() }
        } else {
            None
        }
    }

    /// Returns the pool capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the number of available slots (approximate).
    #[must_use]
    pub fn available(&self) -> usize {
        self.slots
            .iter()
            .filter(|s| s.in_use.load(Ordering::Relaxed) == 0)
            .count()
    }
}

/// Single-producer single-consumer bounded queue.
///
/// Optimized for the case where one thread produces orders
/// and another thread consumes them.
#[repr(align(64))]
pub struct SpscQueue<T> {
    /// Buffer for queue elements.
    buffer: Box<[UnsafeCell<Option<T>>]>,
    /// Capacity (must be power of 2).
    capacity: usize,
    /// Mask for index wrapping.
    mask: usize,
    /// Write index (only modified by producer).
    write_idx: AtomicUsize,
    /// Padding to avoid false sharing.
    _pad1: [u8; CACHE_LINE_SIZE - std::mem::size_of::<AtomicUsize>()],
    /// Read index (only modified by consumer).
    read_idx: AtomicUsize,
    /// Padding to avoid false sharing.
    _pad2: [u8; CACHE_LINE_SIZE - std::mem::size_of::<AtomicUsize>()],
}

// Safety: SpscQueue is safe to share between threads because:
// - write_idx is only modified by producer
// - read_idx is only modified by consumer
// - buffer access is synchronized via indices
unsafe impl<T: Send> Send for SpscQueue<T> {}
unsafe impl<T: Send> Sync for SpscQueue<T> {}

impl<T> SpscQueue<T> {
    /// Creates a new SPSC queue with the specified capacity.
    ///
    /// Capacity will be rounded up to the next power of 2.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.next_power_of_two();
        let buffer: Vec<UnsafeCell<Option<T>>> =
            (0..capacity).map(|_| UnsafeCell::new(None)).collect();

        Self {
            buffer: buffer.into_boxed_slice(),
            capacity,
            mask: capacity - 1,
            write_idx: AtomicUsize::new(0),
            _pad1: [0; CACHE_LINE_SIZE - std::mem::size_of::<AtomicUsize>()],
            read_idx: AtomicUsize::new(0),
            _pad2: [0; CACHE_LINE_SIZE - std::mem::size_of::<AtomicUsize>()],
        }
    }

    /// Tries to push an element to the queue.
    ///
    /// Returns `Err(value)` if the queue is full.
    pub fn try_push(&self, value: T) -> Result<(), T> {
        let write = self.write_idx.load(Ordering::Relaxed);
        let read = self.read_idx.load(Ordering::Acquire);

        // Check if queue is full
        if write.wrapping_sub(read) >= self.capacity {
            return Err(value);
        }

        let idx = write & self.mask;
        unsafe {
            *self.buffer[idx].get() = Some(value);
        }

        self.write_idx
            .store(write.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// Tries to pop an element from the queue.
    ///
    /// Returns `None` if the queue is empty.
    pub fn try_pop(&self) -> Option<T> {
        let read = self.read_idx.load(Ordering::Relaxed);
        let write = self.write_idx.load(Ordering::Acquire);

        // Check if queue is empty
        if read == write {
            return None;
        }

        let idx = read & self.mask;
        let value = unsafe { (*self.buffer[idx].get()).take() };

        self.read_idx.store(read.wrapping_add(1), Ordering::Release);
        value
    }

    /// Returns true if the queue is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let read = self.read_idx.load(Ordering::Relaxed);
        let write = self.write_idx.load(Ordering::Acquire);
        read == write
    }

    /// Returns the number of elements in the queue (approximate).
    #[must_use]
    pub fn len(&self) -> usize {
        let read = self.read_idx.load(Ordering::Relaxed);
        let write = self.write_idx.load(Ordering::Acquire);
        write.wrapping_sub(read)
    }

    /// Returns the queue capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_atomic_price() {
        let price = AtomicPrice::new(Price::new(dec!(50000.12345678)).unwrap());
        let loaded = price.load().unwrap();
        assert!((loaded.as_decimal() - dec!(50000.12345678)).abs() < dec!(0.00000001));
    }

    #[test]
    fn test_atomic_price_zero() {
        let price = AtomicPrice::zero();
        assert!(price.load().is_none());
    }

    #[test]
    fn test_atomic_price_store() {
        let price = AtomicPrice::zero();
        price.store(Price::new(dec!(42000)).unwrap());
        assert_eq!(price.load().unwrap().as_decimal(), dec!(42000));
    }

    #[test]
    fn test_atomic_quantity() {
        let qty = AtomicQuantity::new(Quantity::new(dec!(1.5)).unwrap());
        let loaded = qty.load();
        assert!((loaded.as_decimal() - dec!(1.5)).abs() < dec!(0.00000001));
    }

    #[test]
    fn test_atomic_quantity_fetch_add() {
        let qty = AtomicQuantity::new(Quantity::new(dec!(1.0)).unwrap());
        let prev = qty.fetch_add(Quantity::new(dec!(0.5)).unwrap());
        assert!((prev.as_decimal() - dec!(1.0)).abs() < dec!(0.00000001));
        assert!((qty.load().as_decimal() - dec!(1.5)).abs() < dec!(0.00000001));
    }

    #[test]
    fn test_lockfree_order_pool() {
        let pool = LockFreeOrderPool::new("test", 10);
        assert_eq!(pool.capacity(), 10);
        assert_eq!(pool.available(), 10);

        // Acquire a slot
        let (idx, order_id) = pool.acquire().unwrap();
        assert!(idx < 10);
        assert!(!order_id.as_str().is_empty());
        assert_eq!(pool.available(), 9);

        // Release the slot
        pool.release(idx);
        assert_eq!(pool.available(), 10);
    }

    #[test]
    fn test_lockfree_order_pool_exhaustion() {
        let pool = LockFreeOrderPool::new("test", 2);

        let (idx1, _) = pool.acquire().unwrap();
        let (idx2, _) = pool.acquire().unwrap();

        // Pool should be exhausted
        assert!(pool.acquire().is_none());

        // Release one slot
        pool.release(idx1);

        // Should be able to acquire again
        assert!(pool.acquire().is_some());

        pool.release(idx2);
    }

    #[test]
    fn test_spsc_queue() {
        let queue: SpscQueue<i32> = SpscQueue::new(4);
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);

        // Push elements
        assert!(queue.try_push(1).is_ok());
        assert!(queue.try_push(2).is_ok());
        assert_eq!(queue.len(), 2);

        // Pop elements
        assert_eq!(queue.try_pop(), Some(1));
        assert_eq!(queue.try_pop(), Some(2));
        assert!(queue.is_empty());
    }

    #[test]
    fn test_spsc_queue_full() {
        let queue: SpscQueue<i32> = SpscQueue::new(2);

        assert!(queue.try_push(1).is_ok());
        assert!(queue.try_push(2).is_ok());

        // Queue should be full
        assert!(queue.try_push(3).is_err());

        // Pop one element
        assert_eq!(queue.try_pop(), Some(1));

        // Should be able to push again
        assert!(queue.try_push(3).is_ok());
    }

    #[test]
    fn test_spsc_queue_wrap_around() {
        let queue: SpscQueue<i32> = SpscQueue::new(4);

        // Fill and drain multiple times to test wrap-around
        for round in 0..3 {
            for i in 0..4 {
                assert!(queue.try_push(round * 4 + i).is_ok());
            }
            for i in 0..4 {
                assert_eq!(queue.try_pop(), Some(round * 4 + i));
            }
        }
    }
}
