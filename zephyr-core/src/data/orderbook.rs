//! Order book data structures.

use serde::{Deserialize, Serialize};

use crate::types::{Price, Quantity, Symbol, Timestamp};

use super::DataValidationError;

/// A single level in the order book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderBookLevel {
    /// Price at this level
    pub price: Price,
    /// Quantity at this level
    pub quantity: Quantity,
}

impl OrderBookLevel {
    /// Creates a new order book level.
    #[must_use]
    pub const fn new(price: Price, quantity: Quantity) -> Self {
        Self { price, quantity }
    }
}

/// Order book data structure.
///
/// Contains bid and ask levels sorted by price.
///
/// # Examples
///
/// ```
/// use zephyr_core::data::{OrderBook, OrderBookLevel};
/// use zephyr_core::types::{Symbol, Timestamp, Price, Quantity};
/// use rust_decimal_macros::dec;
///
/// let orderbook = OrderBook::builder()
///     .symbol(Symbol::new("BTC-USDT").unwrap())
///     .timestamp(Timestamp::now())
///     .bid(Price::new(dec!(41999)).unwrap(), Quantity::new(dec!(10)).unwrap())
///     .bid(Price::new(dec!(41998)).unwrap(), Quantity::new(dec!(20)).unwrap())
///     .ask(Price::new(dec!(42001)).unwrap(), Quantity::new(dec!(8)).unwrap())
///     .ask(Price::new(dec!(42002)).unwrap(), Quantity::new(dec!(15)).unwrap())
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderBook {
    /// Trading pair symbol
    pub symbol: Symbol,
    /// Order book timestamp
    pub timestamp: Timestamp,
    /// Bid levels (sorted by price descending - highest first)
    pub bids: Vec<OrderBookLevel>,
    /// Ask levels (sorted by price ascending - lowest first)
    pub asks: Vec<OrderBookLevel>,
}

impl OrderBook {
    /// Creates a new builder for `OrderBook`.
    #[must_use]
    pub fn builder() -> OrderBookBuilder {
        OrderBookBuilder::default()
    }

    /// Validates the order book.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Timestamp is zero
    /// - Order book is crossed (best bid >= best ask)
    #[allow(clippy::collapsible_if)]
    pub fn validate(&self) -> Result<(), DataValidationError> {
        if self.timestamp.is_zero() {
            return Err(DataValidationError::InvalidTimestamp(
                "timestamp cannot be zero".to_string(),
            ));
        }

        // Check for crossed order book
        if let (Some(best_bid), Some(best_ask)) = (self.best_bid(), self.best_ask()) {
            if best_bid.price >= best_ask.price {
                return Err(DataValidationError::CrossedOrderBook {
                    bid: best_bid.price.to_string(),
                    ask: best_ask.price.to_string(),
                });
            }
        }

        Ok(())
    }

    /// Returns the best bid (highest bid price).
    #[must_use]
    pub fn best_bid(&self) -> Option<&OrderBookLevel> {
        self.bids.first()
    }

    /// Returns the best ask (lowest ask price).
    #[must_use]
    pub fn best_ask(&self) -> Option<&OrderBookLevel> {
        self.asks.first()
    }

    /// Returns the spread (best ask - best bid).
    #[must_use]
    pub fn spread(&self) -> Option<rust_decimal::Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(ask.price - bid.price),
            _ => None,
        }
    }

    /// Returns the mid price ((best bid + best ask) / 2).
    #[must_use]
    pub fn mid_price(&self) -> Option<Price> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => {
                let mid = (bid.price.as_decimal() + ask.price.as_decimal())
                    / rust_decimal::Decimal::from(2);
                Price::new(mid).ok()
            }
            _ => None,
        }
    }

    /// Returns the total bid volume.
    #[must_use]
    pub fn total_bid_volume(&self) -> Quantity {
        self.bids.iter().map(|l| l.quantity).sum()
    }

    /// Returns the total ask volume.
    #[must_use]
    pub fn total_ask_volume(&self) -> Quantity {
        self.asks.iter().map(|l| l.quantity).sum()
    }

    /// Returns the number of bid levels.
    #[must_use]
    pub fn bid_depth(&self) -> usize {
        self.bids.len()
    }

    /// Returns the number of ask levels.
    #[must_use]
    pub fn ask_depth(&self) -> usize {
        self.asks.len()
    }

    /// Returns true if the order book is empty on both sides.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bids.is_empty() && self.asks.is_empty()
    }
}

/// Builder for `OrderBook`.
#[derive(Debug, Default)]
pub struct OrderBookBuilder {
    symbol: Option<Symbol>,
    timestamp: Option<Timestamp>,
    bids: Vec<OrderBookLevel>,
    asks: Vec<OrderBookLevel>,
}

impl OrderBookBuilder {
    /// Sets the symbol.
    #[must_use]
    pub fn symbol(mut self, symbol: Symbol) -> Self {
        self.symbol = Some(symbol);
        self
    }

    /// Sets the timestamp.
    #[must_use]
    pub fn timestamp(mut self, timestamp: Timestamp) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Adds a bid level.
    #[must_use]
    pub fn bid(mut self, price: Price, quantity: Quantity) -> Self {
        self.bids.push(OrderBookLevel::new(price, quantity));
        self
    }

    /// Adds an ask level.
    #[must_use]
    pub fn ask(mut self, price: Price, quantity: Quantity) -> Self {
        self.asks.push(OrderBookLevel::new(price, quantity));
        self
    }

    /// Sets all bid levels.
    #[must_use]
    pub fn bids(mut self, bids: Vec<OrderBookLevel>) -> Self {
        self.bids = bids;
        self
    }

    /// Sets all ask levels.
    #[must_use]
    pub fn asks(mut self, asks: Vec<OrderBookLevel>) -> Self {
        self.asks = asks;
        self
    }

    /// Builds the `OrderBook`.
    ///
    /// # Errors
    ///
    /// Returns an error if any required field is missing or validation fails.
    pub fn build(mut self) -> Result<OrderBook, DataValidationError> {
        // Sort bids descending by price
        self.bids.sort_by(|a, b| b.price.cmp(&a.price));
        // Sort asks ascending by price
        self.asks.sort_by(|a, b| a.price.cmp(&b.price));

        let orderbook = OrderBook {
            symbol: self
                .symbol
                .ok_or(DataValidationError::MissingField("symbol"))?,
            timestamp: self
                .timestamp
                .ok_or(DataValidationError::MissingField("timestamp"))?,
            bids: self.bids,
            asks: self.asks,
        };
        orderbook.validate()?;
        Ok(orderbook)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_valid_orderbook() -> OrderBook {
        OrderBook::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1_704_067_200_000).unwrap())
            .bid(
                Price::new(dec!(41999)).unwrap(),
                Quantity::new(dec!(10)).unwrap(),
            )
            .bid(
                Price::new(dec!(41998)).unwrap(),
                Quantity::new(dec!(20)).unwrap(),
            )
            .ask(
                Price::new(dec!(42001)).unwrap(),
                Quantity::new(dec!(8)).unwrap(),
            )
            .ask(
                Price::new(dec!(42002)).unwrap(),
                Quantity::new(dec!(15)).unwrap(),
            )
            .build()
            .unwrap()
    }

    #[test]
    fn test_orderbook_builder_valid() {
        let ob = create_valid_orderbook();
        assert_eq!(ob.symbol.as_str(), "BTC-USDT");
        assert_eq!(ob.bid_depth(), 2);
        assert_eq!(ob.ask_depth(), 2);
    }

    #[test]
    fn test_orderbook_builder_missing_field() {
        let result = OrderBook::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .build();
        assert!(matches!(result, Err(DataValidationError::MissingField(_))));
    }

    #[test]
    fn test_orderbook_best_bid_ask() {
        let ob = create_valid_orderbook();
        assert_eq!(ob.best_bid().unwrap().price.as_decimal(), dec!(41999));
        assert_eq!(ob.best_ask().unwrap().price.as_decimal(), dec!(42001));
    }

    #[test]
    fn test_orderbook_spread() {
        let ob = create_valid_orderbook();
        assert_eq!(ob.spread().unwrap(), dec!(2));
    }

    #[test]
    fn test_orderbook_mid_price() {
        let ob = create_valid_orderbook();
        assert_eq!(ob.mid_price().unwrap().as_decimal(), dec!(42000));
    }

    #[test]
    fn test_orderbook_total_volume() {
        let ob = create_valid_orderbook();
        assert_eq!(ob.total_bid_volume().as_decimal(), dec!(30));
        assert_eq!(ob.total_ask_volume().as_decimal(), dec!(23));
    }

    #[test]
    fn test_orderbook_sorting() {
        // Add bids in wrong order
        let ob = OrderBook::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1_704_067_200_000).unwrap())
            .bid(
                Price::new(dec!(41998)).unwrap(),
                Quantity::new(dec!(20)).unwrap(),
            )
            .bid(
                Price::new(dec!(41999)).unwrap(),
                Quantity::new(dec!(10)).unwrap(),
            )
            .ask(
                Price::new(dec!(42002)).unwrap(),
                Quantity::new(dec!(15)).unwrap(),
            )
            .ask(
                Price::new(dec!(42001)).unwrap(),
                Quantity::new(dec!(8)).unwrap(),
            )
            .build()
            .unwrap();

        // Should be sorted correctly
        assert_eq!(ob.best_bid().unwrap().price.as_decimal(), dec!(41999));
        assert_eq!(ob.best_ask().unwrap().price.as_decimal(), dec!(42001));
    }

    #[test]
    fn test_orderbook_crossed() {
        let result = OrderBook::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1_704_067_200_000).unwrap())
            .bid(
                Price::new(dec!(42001)).unwrap(),
                Quantity::new(dec!(10)).unwrap(),
            )
            .ask(
                Price::new(dec!(42000)).unwrap(),
                Quantity::new(dec!(8)).unwrap(),
            )
            .build();
        assert!(matches!(
            result,
            Err(DataValidationError::CrossedOrderBook { .. })
        ));
    }

    #[test]
    fn test_orderbook_empty() {
        let ob = OrderBook::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1_704_067_200_000).unwrap())
            .build()
            .unwrap();
        assert!(ob.is_empty());
    }

    #[test]
    fn test_orderbook_serde_roundtrip() {
        let ob = create_valid_orderbook();
        let json = serde_json::to_string(&ob).unwrap();
        let parsed: OrderBook = serde_json::from_str(&json).unwrap();
        assert_eq!(ob, parsed);
    }
}
