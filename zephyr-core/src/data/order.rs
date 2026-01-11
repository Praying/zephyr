//! Order types and structures for trading operations.
//!
//! This module provides order-related types including:
//! - [`OrderRequest`] - Request to create a new order
//! - [`Order`] - Represents an order with its current state
//! - [`OrderSide`] - Buy or Sell direction
//! - [`OrderType`] - Limit, Market, Stop, etc.
//! - [`OrderStatus`] - Current status of an order
//! - [`TimeInForce`] - Order validity duration

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::types::{OrderId, Price, Quantity, Symbol, Timestamp};

/// Order side - Buy or Sell direction.
///
/// # Examples
///
/// ```
/// use zephyr_core::data::OrderSide;
///
/// let side = OrderSide::Buy;
/// assert!(side.is_buy());
/// assert!(!side.is_sell());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderSide {
    /// Buy order (long position)
    Buy,
    /// Sell order (short position)
    Sell,
}

impl OrderSide {
    /// Returns true if this is a buy order.
    #[must_use]
    pub const fn is_buy(&self) -> bool {
        matches!(self, Self::Buy)
    }

    /// Returns true if this is a sell order.
    #[must_use]
    pub const fn is_sell(&self) -> bool {
        matches!(self, Self::Sell)
    }

    /// Returns the opposite side.
    #[must_use]
    pub const fn opposite(&self) -> Self {
        match self {
            Self::Buy => Self::Sell,
            Self::Sell => Self::Buy,
        }
    }

    /// Returns the direction multiplier (1 for Buy, -1 for Sell).
    #[must_use]
    pub fn direction(&self) -> Decimal {
        match self {
            Self::Buy => Decimal::ONE,
            Self::Sell => -Decimal::ONE,
        }
    }
}

impl fmt::Display for OrderSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Buy => write!(f, "BUY"),
            Self::Sell => write!(f, "SELL"),
        }
    }
}

/// Order type - specifies how the order should be executed.
///
/// # Examples
///
/// ```
/// use zephyr_core::data::OrderType;
///
/// let order_type = OrderType::Limit;
/// assert!(order_type.requires_price());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderType {
    /// Limit order - executes at specified price or better
    Limit,
    /// Market order - executes immediately at best available price
    Market,
    /// Stop limit order - becomes limit order when stop price is reached
    StopLimit,
    /// Stop market order - becomes market order when stop price is reached
    StopMarket,
    /// Take profit order - limit order triggered at profit target
    TakeProfit,
    /// Take profit market order - market order triggered at profit target
    TakeProfitMarket,
}

impl OrderType {
    /// Returns true if this order type requires a price.
    #[must_use]
    pub const fn requires_price(&self) -> bool {
        matches!(self, Self::Limit | Self::StopLimit | Self::TakeProfit)
    }

    /// Returns true if this order type requires a stop price.
    #[must_use]
    pub const fn requires_stop_price(&self) -> bool {
        matches!(
            self,
            Self::StopLimit | Self::StopMarket | Self::TakeProfit | Self::TakeProfitMarket
        )
    }

    /// Returns true if this is a market order type.
    #[must_use]
    pub const fn is_market(&self) -> bool {
        matches!(
            self,
            Self::Market | Self::StopMarket | Self::TakeProfitMarket
        )
    }
}

impl fmt::Display for OrderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit => write!(f, "LIMIT"),
            Self::Market => write!(f, "MARKET"),
            Self::StopLimit => write!(f, "STOP_LIMIT"),
            Self::StopMarket => write!(f, "STOP_MARKET"),
            Self::TakeProfit => write!(f, "TAKE_PROFIT"),
            Self::TakeProfitMarket => write!(f, "TAKE_PROFIT_MARKET"),
        }
    }
}

/// Order status - current state of an order.
///
/// Uses `TypeState` pattern conceptually - status transitions are validated.
///
/// # State Transitions
///
/// ```text
/// Pending -> New -> PartiallyFilled -> Filled
///                -> Canceled
///                -> Rejected
///                -> Expired
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    /// Order is pending submission
    Pending,
    /// Order has been accepted by the exchange
    New,
    /// Order has been partially filled
    PartiallyFilled,
    /// Order has been completely filled
    Filled,
    /// Order has been canceled
    Canceled,
    /// Order was rejected by the exchange
    Rejected,
    /// Order has expired
    Expired,
}

impl OrderStatus {
    /// Returns true if the order is in a final state.
    #[must_use]
    pub const fn is_final(&self) -> bool {
        matches!(
            self,
            Self::Filled | Self::Canceled | Self::Rejected | Self::Expired
        )
    }

    /// Returns true if the order is active (can still be filled or canceled).
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Pending | Self::New | Self::PartiallyFilled)
    }

    /// Returns true if the order has any fills.
    #[must_use]
    pub const fn has_fills(&self) -> bool {
        matches!(self, Self::PartiallyFilled | Self::Filled)
    }

    /// Checks if transition to the given status is valid.
    #[must_use]
    pub const fn can_transition_to(&self, new_status: Self) -> bool {
        match (self, new_status) {
            // Valid transitions
            (Self::Pending, Self::New | Self::Rejected)
            | (
                Self::New | Self::PartiallyFilled,
                Self::PartiallyFilled | Self::Filled | Self::Canceled,
            )
            | (Self::New, Self::Expired) => true,
            // No transitions from final states
            _ => false,
        }
    }
}

impl fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "PENDING"),
            Self::New => write!(f, "NEW"),
            Self::PartiallyFilled => write!(f, "PARTIALLY_FILLED"),
            Self::Filled => write!(f, "FILLED"),
            Self::Canceled => write!(f, "CANCELED"),
            Self::Rejected => write!(f, "REJECTED"),
            Self::Expired => write!(f, "EXPIRED"),
        }
    }
}

/// Time in force - specifies how long an order remains active.
///
/// # Examples
///
/// ```
/// use zephyr_core::data::TimeInForce;
///
/// let tif = TimeInForce::Gtc;
/// assert!(!tif.is_immediate());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimeInForce {
    /// Good Till Cancel - remains active until filled or canceled
    #[default]
    Gtc,
    /// Immediate or Cancel - fills immediately, cancels unfilled portion
    Ioc,
    /// Fill or Kill - must be filled completely or canceled entirely
    Fok,
    /// Good Till Date - remains active until specified date
    Gtd,
    /// Post Only - only adds liquidity, rejected if would take liquidity
    PostOnly,
}

impl fmt::Display for TimeInForce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gtc => write!(f, "GTC"),
            Self::Ioc => write!(f, "IOC"),
            Self::Fok => write!(f, "FOK"),
            Self::Gtd => write!(f, "GTD"),
            Self::PostOnly => write!(f, "POST_ONLY"),
        }
    }
}

impl TimeInForce {
    /// Returns true if this is an immediate execution type.
    #[must_use]
    pub const fn is_immediate(&self) -> bool {
        matches!(self, Self::Ioc | Self::Fok)
    }

    /// Returns true if partial fills are allowed.
    #[must_use]
    pub const fn allows_partial_fill(&self) -> bool {
        !matches!(self, Self::Fok)
    }
}

/// Order request - used to submit a new order.
///
/// # Examples
///
/// ```
/// use zephyr_core::data::{OrderRequest, OrderSide, OrderType, TimeInForce};
/// use zephyr_core::types::{Symbol, Price, Quantity};
/// use rust_decimal_macros::dec;
///
/// let request = OrderRequest::builder()
///     .symbol(Symbol::new("BTC-USDT").unwrap())
///     .side(OrderSide::Buy)
///     .order_type(OrderType::Limit)
///     .quantity(Quantity::new(dec!(0.1)).unwrap())
///     .price(Price::new(dec!(50000)).unwrap())
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderRequest {
    /// Trading symbol
    pub symbol: Symbol,
    /// Order side (Buy/Sell)
    pub side: OrderSide,
    /// Order type
    pub order_type: OrderType,
    /// Order quantity
    pub quantity: Quantity,
    /// Limit price (required for limit orders)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<Price>,
    /// Stop/trigger price (for stop orders)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_price: Option<Price>,
    /// Time in force
    #[serde(default)]
    pub time_in_force: TimeInForce,
    /// Reduce only flag (only reduce position, don't increase)
    #[serde(default)]
    pub reduce_only: bool,
    /// Post only flag (only add liquidity)
    #[serde(default)]
    pub post_only: bool,
    /// Client-specified order ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
}

impl OrderRequest {
    /// Creates a new builder for `OrderRequest`.
    #[must_use]
    pub fn builder() -> OrderRequestBuilder {
        OrderRequestBuilder::default()
    }

    /// Validates the order request.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Limit order is missing price
    /// - Stop order is missing stop price
    /// - Quantity is zero or negative
    pub fn validate(&self) -> Result<(), OrderValidationError> {
        // Check quantity
        if self.quantity.is_zero() || self.quantity.is_negative() {
            return Err(OrderValidationError::InvalidQuantity(
                "quantity must be positive".to_string(),
            ));
        }

        // Check price for limit orders
        if self.order_type.requires_price() && self.price.is_none() {
            return Err(OrderValidationError::MissingPrice);
        }

        // Check stop price for stop orders
        if self.order_type.requires_stop_price() && self.stop_price.is_none() {
            return Err(OrderValidationError::MissingStopPrice);
        }

        // Check for conflicting flags
        if self.post_only && self.time_in_force.is_immediate() {
            return Err(OrderValidationError::ConflictingFlags(
                "post_only cannot be used with IOC/FOK".to_string(),
            ));
        }

        Ok(())
    }
}

/// Builder for `OrderRequest`.
#[derive(Debug, Default)]
pub struct OrderRequestBuilder {
    symbol: Option<Symbol>,
    side: Option<OrderSide>,
    order_type: Option<OrderType>,
    quantity: Option<Quantity>,
    price: Option<Price>,
    stop_price: Option<Price>,
    time_in_force: TimeInForce,
    reduce_only: bool,
    post_only: bool,
    client_order_id: Option<String>,
}

impl OrderRequestBuilder {
    /// Sets the trading symbol.
    #[must_use]
    pub fn symbol(mut self, symbol: Symbol) -> Self {
        self.symbol = Some(symbol);
        self
    }

    /// Sets the order side.
    #[must_use]
    pub fn side(mut self, side: OrderSide) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets the order type.
    #[must_use]
    pub fn order_type(mut self, order_type: OrderType) -> Self {
        self.order_type = Some(order_type);
        self
    }

    /// Sets the order quantity.
    #[must_use]
    pub fn quantity(mut self, quantity: Quantity) -> Self {
        self.quantity = Some(quantity);
        self
    }

    /// Sets the limit price.
    #[must_use]
    pub fn price(mut self, price: Price) -> Self {
        self.price = Some(price);
        self
    }

    /// Sets the stop/trigger price.
    #[must_use]
    pub fn stop_price(mut self, stop_price: Price) -> Self {
        self.stop_price = Some(stop_price);
        self
    }

    /// Sets the time in force.
    #[must_use]
    pub fn time_in_force(mut self, tif: TimeInForce) -> Self {
        self.time_in_force = tif;
        self
    }

    /// Sets the reduce only flag.
    #[must_use]
    pub fn reduce_only(mut self, reduce_only: bool) -> Self {
        self.reduce_only = reduce_only;
        self
    }

    /// Sets the post only flag.
    #[must_use]
    pub fn post_only(mut self, post_only: bool) -> Self {
        self.post_only = post_only;
        self
    }

    /// Sets the client order ID.
    #[must_use]
    pub fn client_order_id(mut self, id: impl Into<String>) -> Self {
        self.client_order_id = Some(id.into());
        self
    }

    /// Builds the `OrderRequest`.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing.
    pub fn build(self) -> Result<OrderRequest, OrderValidationError> {
        let request = OrderRequest {
            symbol: self
                .symbol
                .ok_or(OrderValidationError::MissingField("symbol"))?,
            side: self
                .side
                .ok_or(OrderValidationError::MissingField("side"))?,
            order_type: self
                .order_type
                .ok_or(OrderValidationError::MissingField("order_type"))?,
            quantity: self
                .quantity
                .ok_or(OrderValidationError::MissingField("quantity"))?,
            price: self.price,
            stop_price: self.stop_price,
            time_in_force: self.time_in_force,
            reduce_only: self.reduce_only,
            post_only: self.post_only,
            client_order_id: self.client_order_id,
        };

        request.validate()?;
        Ok(request)
    }
}

/// Order - represents an order with its current state.
///
/// # Examples
///
/// ```
/// use zephyr_core::data::{Order, OrderSide, OrderType, OrderStatus, TimeInForce};
/// use zephyr_core::types::{Symbol, Price, Quantity, OrderId, Timestamp};
/// use rust_decimal_macros::dec;
///
/// let order = Order::builder()
///     .order_id(OrderId::generate())
///     .symbol(Symbol::new("BTC-USDT").unwrap())
///     .side(OrderSide::Buy)
///     .order_type(OrderType::Limit)
///     .status(OrderStatus::New)
///     .quantity(Quantity::new(dec!(0.1)).unwrap())
///     .price(Price::new(dec!(50000)).unwrap())
///     .create_time(Timestamp::now())
///     .update_time(Timestamp::now())
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    /// Exchange-assigned order ID
    pub order_id: OrderId,
    /// Client-specified order ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
    /// Trading symbol
    pub symbol: Symbol,
    /// Order side
    pub side: OrderSide,
    /// Order type
    pub order_type: OrderType,
    /// Current order status
    pub status: OrderStatus,
    /// Limit price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<Price>,
    /// Stop/trigger price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_price: Option<Price>,
    /// Original order quantity
    pub quantity: Quantity,
    /// Filled quantity
    pub filled_quantity: Quantity,
    /// Average fill price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_price: Option<Price>,
    /// Time in force
    pub time_in_force: TimeInForce,
    /// Reduce only flag
    #[serde(default)]
    pub reduce_only: bool,
    /// Post only flag
    #[serde(default)]
    pub post_only: bool,
    /// Order creation timestamp
    pub create_time: Timestamp,
    /// Last update timestamp
    pub update_time: Timestamp,
}

impl Order {
    /// Creates a new builder for `Order`.
    #[must_use]
    pub fn builder() -> OrderBuilder {
        OrderBuilder::default()
    }

    /// Creates an `Order` from an `OrderRequest` with initial status.
    #[must_use]
    pub fn from_request(request: &OrderRequest, order_id: OrderId, timestamp: Timestamp) -> Self {
        Self {
            order_id,
            client_order_id: request.client_order_id.clone(),
            symbol: request.symbol.clone(),
            side: request.side,
            order_type: request.order_type,
            status: OrderStatus::Pending,
            price: request.price,
            stop_price: request.stop_price,
            quantity: request.quantity,
            filled_quantity: Quantity::ZERO,
            avg_price: None,
            time_in_force: request.time_in_force,
            reduce_only: request.reduce_only,
            post_only: request.post_only,
            create_time: timestamp,
            update_time: timestamp,
        }
    }

    /// Returns the remaining unfilled quantity.
    #[must_use]
    pub fn remaining_quantity(&self) -> Quantity {
        self.quantity - self.filled_quantity
    }

    /// Returns the fill ratio (0.0 to 1.0).
    #[must_use]
    pub fn fill_ratio(&self) -> Decimal {
        if self.quantity.is_zero() {
            return Decimal::ZERO;
        }
        self.filled_quantity.as_decimal() / self.quantity.as_decimal()
    }

    /// Returns true if the order is completely filled.
    #[must_use]
    pub fn is_filled(&self) -> bool {
        self.status == OrderStatus::Filled
    }

    /// Returns true if the order is active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }

    /// Returns true if the order is in a final state.
    #[must_use]
    pub fn is_final(&self) -> bool {
        self.status.is_final()
    }

    /// Updates the order status with validation.
    ///
    /// # Errors
    ///
    /// Returns an error if the status transition is invalid.
    pub fn update_status(&mut self, new_status: OrderStatus) -> Result<(), OrderValidationError> {
        if !self.status.can_transition_to(new_status) {
            return Err(OrderValidationError::InvalidStatusTransition {
                from: self.status,
                to: new_status,
            });
        }
        self.status = new_status;
        self.update_time = Timestamp::now();
        Ok(())
    }

    /// Records a fill on this order.
    ///
    /// # Errors
    ///
    /// Returns an error if the fill would exceed the order quantity.
    pub fn record_fill(
        &mut self,
        fill_qty: Quantity,
        fill_price: Price,
    ) -> Result<(), OrderValidationError> {
        let new_filled = self.filled_quantity + fill_qty;
        if new_filled > self.quantity {
            return Err(OrderValidationError::FillExceedsQuantity {
                fill: fill_qty,
                remaining: self.remaining_quantity(),
            });
        }

        // Update average price
        let old_value = self.avg_price.map_or(Decimal::ZERO, |p| {
            p.as_decimal() * self.filled_quantity.as_decimal()
        });
        let new_value = fill_price.as_decimal() * fill_qty.as_decimal();
        let total_value = old_value + new_value;
        let new_avg = total_value / new_filled.as_decimal();
        self.avg_price = Price::new(new_avg).ok();

        self.filled_quantity = new_filled;
        self.update_time = Timestamp::now();

        // Update status
        if self.filled_quantity == self.quantity {
            self.status = OrderStatus::Filled;
        } else if self.filled_quantity > Quantity::ZERO {
            self.status = OrderStatus::PartiallyFilled;
        }

        Ok(())
    }
}

/// Builder for `Order`.
#[derive(Debug, Default)]
pub struct OrderBuilder {
    order_id: Option<OrderId>,
    client_order_id: Option<String>,
    symbol: Option<Symbol>,
    side: Option<OrderSide>,
    order_type: Option<OrderType>,
    status: Option<OrderStatus>,
    price: Option<Price>,
    stop_price: Option<Price>,
    quantity: Option<Quantity>,
    filled_quantity: Quantity,
    avg_price: Option<Price>,
    time_in_force: TimeInForce,
    reduce_only: bool,
    post_only: bool,
    create_time: Option<Timestamp>,
    update_time: Option<Timestamp>,
}

impl OrderBuilder {
    /// Sets the order ID.
    #[must_use]
    pub fn order_id(mut self, order_id: OrderId) -> Self {
        self.order_id = Some(order_id);
        self
    }

    /// Sets the client order ID.
    #[must_use]
    pub fn client_order_id(mut self, id: impl Into<String>) -> Self {
        self.client_order_id = Some(id.into());
        self
    }

    /// Sets the trading symbol.
    #[must_use]
    pub fn symbol(mut self, symbol: Symbol) -> Self {
        self.symbol = Some(symbol);
        self
    }

    /// Sets the order side.
    #[must_use]
    pub fn side(mut self, side: OrderSide) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets the order type.
    #[must_use]
    pub fn order_type(mut self, order_type: OrderType) -> Self {
        self.order_type = Some(order_type);
        self
    }

    /// Sets the order status.
    #[must_use]
    pub fn status(mut self, status: OrderStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Sets the limit price.
    #[must_use]
    pub fn price(mut self, price: Price) -> Self {
        self.price = Some(price);
        self
    }

    /// Sets the stop price.
    #[must_use]
    pub fn stop_price(mut self, stop_price: Price) -> Self {
        self.stop_price = Some(stop_price);
        self
    }

    /// Sets the order quantity.
    #[must_use]
    pub fn quantity(mut self, quantity: Quantity) -> Self {
        self.quantity = Some(quantity);
        self
    }

    /// Sets the filled quantity.
    #[must_use]
    pub fn filled_quantity(mut self, filled_quantity: Quantity) -> Self {
        self.filled_quantity = filled_quantity;
        self
    }

    /// Sets the average fill price.
    #[must_use]
    pub fn avg_price(mut self, avg_price: Price) -> Self {
        self.avg_price = Some(avg_price);
        self
    }

    /// Sets the time in force.
    #[must_use]
    pub fn time_in_force(mut self, tif: TimeInForce) -> Self {
        self.time_in_force = tif;
        self
    }

    /// Sets the reduce only flag.
    #[must_use]
    pub fn reduce_only(mut self, reduce_only: bool) -> Self {
        self.reduce_only = reduce_only;
        self
    }

    /// Sets the post only flag.
    #[must_use]
    pub fn post_only(mut self, post_only: bool) -> Self {
        self.post_only = post_only;
        self
    }

    /// Sets the creation timestamp.
    #[must_use]
    pub fn create_time(mut self, create_time: Timestamp) -> Self {
        self.create_time = Some(create_time);
        self
    }

    /// Sets the update timestamp.
    #[must_use]
    pub fn update_time(mut self, update_time: Timestamp) -> Self {
        self.update_time = Some(update_time);
        self
    }

    /// Builds the `Order`.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing.
    pub fn build(self) -> Result<Order, OrderValidationError> {
        let create_time = self.create_time.unwrap_or_else(Timestamp::now);
        let update_time = self.update_time.unwrap_or(create_time);

        Ok(Order {
            order_id: self
                .order_id
                .ok_or(OrderValidationError::MissingField("order_id"))?,
            client_order_id: self.client_order_id,
            symbol: self
                .symbol
                .ok_or(OrderValidationError::MissingField("symbol"))?,
            side: self
                .side
                .ok_or(OrderValidationError::MissingField("side"))?,
            order_type: self
                .order_type
                .ok_or(OrderValidationError::MissingField("order_type"))?,
            status: self
                .status
                .ok_or(OrderValidationError::MissingField("status"))?,
            price: self.price,
            stop_price: self.stop_price,
            quantity: self
                .quantity
                .ok_or(OrderValidationError::MissingField("quantity"))?,
            filled_quantity: self.filled_quantity,
            avg_price: self.avg_price,
            time_in_force: self.time_in_force,
            reduce_only: self.reduce_only,
            post_only: self.post_only,
            create_time,
            update_time,
        })
    }
}

/// Order validation error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OrderValidationError {
    /// Missing required field
    #[error("missing required field: {0}")]
    MissingField(&'static str),

    /// Missing price for limit order
    #[error("price is required for limit orders")]
    MissingPrice,

    /// Missing stop price for stop order
    #[error("stop price is required for stop orders")]
    MissingStopPrice,

    /// Invalid quantity
    #[error("invalid quantity: {0}")]
    InvalidQuantity(String),

    /// Conflicting order flags
    #[error("conflicting order flags: {0}")]
    ConflictingFlags(String),

    /// Invalid status transition
    #[error("invalid status transition from {from} to {to}")]
    InvalidStatusTransition {
        /// Current status
        from: OrderStatus,
        /// Attempted new status
        to: OrderStatus,
    },

    /// Fill exceeds remaining quantity
    #[error("fill quantity {fill} exceeds remaining quantity {remaining}")]
    FillExceedsQuantity {
        /// Fill quantity
        fill: Quantity,
        /// Remaining quantity
        remaining: Quantity,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn test_symbol() -> Symbol {
        Symbol::new("BTC-USDT").unwrap()
    }

    fn test_price() -> Price {
        Price::new(dec!(50000)).unwrap()
    }

    fn test_quantity() -> Quantity {
        Quantity::new(dec!(0.1)).unwrap()
    }

    // OrderSide tests
    #[test]
    fn test_order_side_is_buy() {
        assert!(OrderSide::Buy.is_buy());
        assert!(!OrderSide::Sell.is_buy());
    }

    #[test]
    fn test_order_side_opposite() {
        assert_eq!(OrderSide::Buy.opposite(), OrderSide::Sell);
        assert_eq!(OrderSide::Sell.opposite(), OrderSide::Buy);
    }

    #[test]
    fn test_order_side_direction() {
        assert_eq!(OrderSide::Buy.direction(), Decimal::ONE);
        assert_eq!(OrderSide::Sell.direction(), -Decimal::ONE);
    }

    #[test]
    fn test_order_side_display() {
        assert_eq!(format!("{}", OrderSide::Buy), "BUY");
        assert_eq!(format!("{}", OrderSide::Sell), "SELL");
    }

    // OrderType tests
    #[test]
    fn test_order_type_requires_price() {
        assert!(OrderType::Limit.requires_price());
        assert!(!OrderType::Market.requires_price());
        assert!(OrderType::StopLimit.requires_price());
    }

    #[test]
    fn test_order_type_requires_stop_price() {
        assert!(!OrderType::Limit.requires_stop_price());
        assert!(OrderType::StopLimit.requires_stop_price());
        assert!(OrderType::StopMarket.requires_stop_price());
    }

    #[test]
    fn test_order_type_is_market() {
        assert!(!OrderType::Limit.is_market());
        assert!(OrderType::Market.is_market());
        assert!(OrderType::StopMarket.is_market());
    }

    // OrderStatus tests
    #[test]
    fn test_order_status_is_final() {
        assert!(!OrderStatus::Pending.is_final());
        assert!(!OrderStatus::New.is_final());
        assert!(OrderStatus::Filled.is_final());
        assert!(OrderStatus::Canceled.is_final());
        assert!(OrderStatus::Rejected.is_final());
    }

    #[test]
    fn test_order_status_is_active() {
        assert!(OrderStatus::Pending.is_active());
        assert!(OrderStatus::New.is_active());
        assert!(OrderStatus::PartiallyFilled.is_active());
        assert!(!OrderStatus::Filled.is_active());
    }

    #[test]
    fn test_order_status_transitions() {
        assert!(OrderStatus::Pending.can_transition_to(OrderStatus::New));
        assert!(OrderStatus::Pending.can_transition_to(OrderStatus::Rejected));
        assert!(!OrderStatus::Pending.can_transition_to(OrderStatus::Filled));

        assert!(OrderStatus::New.can_transition_to(OrderStatus::PartiallyFilled));
        assert!(OrderStatus::New.can_transition_to(OrderStatus::Filled));
        assert!(OrderStatus::New.can_transition_to(OrderStatus::Canceled));

        assert!(!OrderStatus::Filled.can_transition_to(OrderStatus::Canceled));
    }

    // TimeInForce tests
    #[test]
    fn test_time_in_force_is_immediate() {
        assert!(!TimeInForce::Gtc.is_immediate());
        assert!(TimeInForce::Ioc.is_immediate());
        assert!(TimeInForce::Fok.is_immediate());
    }

    #[test]
    fn test_time_in_force_allows_partial_fill() {
        assert!(TimeInForce::Gtc.allows_partial_fill());
        assert!(TimeInForce::Ioc.allows_partial_fill());
        assert!(!TimeInForce::Fok.allows_partial_fill());
    }

    // OrderRequest tests
    #[test]
    fn test_order_request_builder_limit() {
        let request = OrderRequest::builder()
            .symbol(test_symbol())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(test_quantity())
            .price(test_price())
            .build()
            .unwrap();

        assert_eq!(request.symbol, test_symbol());
        assert_eq!(request.side, OrderSide::Buy);
        assert_eq!(request.order_type, OrderType::Limit);
        assert_eq!(request.price, Some(test_price()));
    }

    #[test]
    fn test_order_request_builder_market() {
        let request = OrderRequest::builder()
            .symbol(test_symbol())
            .side(OrderSide::Sell)
            .order_type(OrderType::Market)
            .quantity(test_quantity())
            .build()
            .unwrap();

        assert_eq!(request.order_type, OrderType::Market);
        assert!(request.price.is_none());
    }

    #[test]
    fn test_order_request_missing_price_for_limit() {
        let result = OrderRequest::builder()
            .symbol(test_symbol())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(test_quantity())
            .build();

        assert!(matches!(result, Err(OrderValidationError::MissingPrice)));
    }

    #[test]
    fn test_order_request_conflicting_flags() {
        let result = OrderRequest::builder()
            .symbol(test_symbol())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(test_quantity())
            .price(test_price())
            .post_only(true)
            .time_in_force(TimeInForce::Ioc)
            .build();

        assert!(matches!(
            result,
            Err(OrderValidationError::ConflictingFlags(_))
        ));
    }

    // Order tests
    #[test]
    fn test_order_builder() {
        let order = Order::builder()
            .order_id(OrderId::generate())
            .symbol(test_symbol())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .status(OrderStatus::New)
            .quantity(test_quantity())
            .price(test_price())
            .create_time(Timestamp::now())
            .update_time(Timestamp::now())
            .build()
            .unwrap();

        assert_eq!(order.status, OrderStatus::New);
        assert!(order.is_active());
        assert!(!order.is_final());
    }

    #[test]
    fn test_order_from_request() {
        let request = OrderRequest::builder()
            .symbol(test_symbol())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(test_quantity())
            .price(test_price())
            .build()
            .unwrap();

        let order = Order::from_request(&request, OrderId::generate(), Timestamp::now());

        assert_eq!(order.symbol, request.symbol);
        assert_eq!(order.side, request.side);
        assert_eq!(order.status, OrderStatus::Pending);
        assert_eq!(order.filled_quantity, Quantity::ZERO);
    }

    #[test]
    fn test_order_remaining_quantity() {
        let mut order = Order::builder()
            .order_id(OrderId::generate())
            .symbol(test_symbol())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .status(OrderStatus::New)
            .quantity(Quantity::new(dec!(1.0)).unwrap())
            .price(test_price())
            .build()
            .unwrap();

        assert_eq!(order.remaining_quantity().as_decimal(), dec!(1.0));

        order.filled_quantity = Quantity::new(dec!(0.3)).unwrap();
        assert_eq!(order.remaining_quantity().as_decimal(), dec!(0.7));
    }

    #[test]
    fn test_order_fill_ratio() {
        let mut order = Order::builder()
            .order_id(OrderId::generate())
            .symbol(test_symbol())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .status(OrderStatus::New)
            .quantity(Quantity::new(dec!(1.0)).unwrap())
            .price(test_price())
            .build()
            .unwrap();

        assert_eq!(order.fill_ratio(), dec!(0));

        order.filled_quantity = Quantity::new(dec!(0.5)).unwrap();
        assert_eq!(order.fill_ratio(), dec!(0.5));
    }

    #[test]
    fn test_order_update_status() {
        let mut order = Order::builder()
            .order_id(OrderId::generate())
            .symbol(test_symbol())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .status(OrderStatus::Pending)
            .quantity(test_quantity())
            .price(test_price())
            .build()
            .unwrap();

        assert!(order.update_status(OrderStatus::New).is_ok());
        assert_eq!(order.status, OrderStatus::New);

        assert!(order.update_status(OrderStatus::Filled).is_ok());
        assert_eq!(order.status, OrderStatus::Filled);

        // Cannot transition from final state
        assert!(order.update_status(OrderStatus::Canceled).is_err());
    }

    #[test]
    fn test_order_record_fill() {
        let mut order = Order::builder()
            .order_id(OrderId::generate())
            .symbol(test_symbol())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .status(OrderStatus::New)
            .quantity(Quantity::new(dec!(1.0)).unwrap())
            .price(test_price())
            .build()
            .unwrap();

        // Partial fill
        order
            .record_fill(
                Quantity::new(dec!(0.5)).unwrap(),
                Price::new(dec!(50000)).unwrap(),
            )
            .unwrap();

        assert_eq!(order.filled_quantity.as_decimal(), dec!(0.5));
        assert_eq!(order.status, OrderStatus::PartiallyFilled);
        assert_eq!(order.avg_price.unwrap().as_decimal(), dec!(50000));

        // Complete fill
        order
            .record_fill(
                Quantity::new(dec!(0.5)).unwrap(),
                Price::new(dec!(51000)).unwrap(),
            )
            .unwrap();

        assert_eq!(order.filled_quantity.as_decimal(), dec!(1.0));
        assert_eq!(order.status, OrderStatus::Filled);
        // Average price: (50000 * 0.5 + 51000 * 0.5) / 1.0 = 50500
        assert_eq!(order.avg_price.unwrap().as_decimal(), dec!(50500));
    }

    #[test]
    fn test_order_record_fill_exceeds_quantity() {
        let mut order = Order::builder()
            .order_id(OrderId::generate())
            .symbol(test_symbol())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .status(OrderStatus::New)
            .quantity(Quantity::new(dec!(1.0)).unwrap())
            .price(test_price())
            .build()
            .unwrap();

        let result = order.record_fill(
            Quantity::new(dec!(1.5)).unwrap(),
            Price::new(dec!(50000)).unwrap(),
        );

        assert!(matches!(
            result,
            Err(OrderValidationError::FillExceedsQuantity { .. })
        ));
    }

    // Serde tests
    #[test]
    fn test_order_side_serde_roundtrip() {
        let side = OrderSide::Buy;
        let json = serde_json::to_string(&side).unwrap();
        let parsed: OrderSide = serde_json::from_str(&json).unwrap();
        assert_eq!(side, parsed);
    }

    #[test]
    fn test_order_type_serde_roundtrip() {
        let order_type = OrderType::StopLimit;
        let json = serde_json::to_string(&order_type).unwrap();
        let parsed: OrderType = serde_json::from_str(&json).unwrap();
        assert_eq!(order_type, parsed);
    }

    #[test]
    fn test_order_status_serde_roundtrip() {
        let status = OrderStatus::PartiallyFilled;
        let json = serde_json::to_string(&status).unwrap();
        let parsed: OrderStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, parsed);
    }

    #[test]
    fn test_order_request_serde_roundtrip() {
        let request = OrderRequest::builder()
            .symbol(test_symbol())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(test_quantity())
            .price(test_price())
            .client_order_id("test-123")
            .build()
            .unwrap();

        let json = serde_json::to_string(&request).unwrap();
        let parsed: OrderRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request, parsed);
    }

    #[test]
    fn test_order_serde_roundtrip() {
        let order = Order::builder()
            .order_id(OrderId::new("order-123").unwrap())
            .symbol(test_symbol())
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .status(OrderStatus::New)
            .quantity(test_quantity())
            .price(test_price())
            .create_time(Timestamp::new(1_704_067_200_000).unwrap())
            .update_time(Timestamp::new(1_704_067_200_000).unwrap())
            .build()
            .unwrap();

        let json = serde_json::to_string(&order).unwrap();
        let parsed: Order = serde_json::from_str(&json).unwrap();
        assert_eq!(order, parsed);
    }
}
