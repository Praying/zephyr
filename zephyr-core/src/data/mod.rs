//! Market data structures.
//!
//! This module provides data structures for market data including
//! tick data, K-line data, order book data, and order/position models.
//!
//! # Structures
//!
//! - TickData - Real-time tick/trade data
//! - KlineData - Candlestick/OHLCV data
//! - OrderBook - Order book with bid/ask levels
//! - KlinePeriod - K-line time periods
//! - Order - Order with current state
//! - OrderRequest - Request to create a new order
//! - Position - Trading position
//! - Account - Trading account

mod kline;
mod order;
mod orderbook;
mod position;
mod tick;

pub use kline::{KlineData, KlineDataBuilder, KlinePeriod};
pub use order::{
    Order, OrderBuilder, OrderRequest, OrderRequestBuilder, OrderSide, OrderStatus, OrderType,
    OrderValidationError, TimeInForce,
};
pub use orderbook::{OrderBook, OrderBookBuilder, OrderBookLevel};
pub use position::{
    Account, Balance, Exchange, MarginType, Position, PositionBuilder, PositionSide,
};
pub use tick::{TickData, TickDataBuilder};

/// Validation error for data structures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DataValidationError {
    /// Missing required field
    #[error("missing required field: {0}")]
    MissingField(&'static str),

    /// Invalid price (high < low, etc.)
    #[error("invalid price relationship: {0}")]
    InvalidPriceRelation(String),

    /// Invalid timestamp
    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(String),

    /// Invalid volume
    #[error("invalid volume: {0}")]
    InvalidVolume(String),

    /// Empty order book
    #[error("order book cannot be empty on both sides")]
    EmptyOrderBook,

    /// Invalid order book (crossed)
    #[error("order book is crossed: best bid {bid} >= best ask {ask}")]
    CrossedOrderBook {
        /// Best bid price
        bid: String,
        /// Best ask price
        ask: String,
    },
}
