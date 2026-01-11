//! Tick (real-time trade) data structures.

use serde::{Deserialize, Serialize};

use crate::types::{FundingRate, MarkPrice, Price, Quantity, Symbol, Timestamp};

use super::DataValidationError;

/// Tick data structure representing real-time market data.
///
/// Contains the latest price, volume, and order book snapshot.
///
/// # Examples
///
/// ```
/// use zephyr_core::data::TickData;
/// use zephyr_core::types::{Symbol, Timestamp, Price, Quantity};
/// use rust_decimal_macros::dec;
///
/// let tick = TickData::builder()
///     .symbol(Symbol::new("BTC-USDT").unwrap())
///     .timestamp(Timestamp::now())
///     .price(Price::new(dec!(42000)).unwrap())
///     .volume(Quantity::new(dec!(0.5)).unwrap())
///     .bid_price(Price::new(dec!(41999)).unwrap())
///     .bid_quantity(Quantity::new(dec!(10)).unwrap())
///     .ask_price(Price::new(dec!(42001)).unwrap())
///     .ask_quantity(Quantity::new(dec!(8)).unwrap())
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TickData {
    /// Trading pair symbol
    pub symbol: Symbol,
    /// Tick timestamp
    pub timestamp: Timestamp,
    /// Last traded price
    pub price: Price,
    /// Last traded volume
    pub volume: Quantity,
    /// Best bid prices (up to 5 levels)
    pub bid_prices: Vec<Price>,
    /// Best bid quantities (up to 5 levels)
    pub bid_quantities: Vec<Quantity>,
    /// Best ask prices (up to 5 levels)
    pub ask_prices: Vec<Price>,
    /// Best ask quantities (up to 5 levels)
    pub ask_quantities: Vec<Quantity>,
    /// Open interest (for futures/perpetuals)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_interest: Option<Quantity>,
    /// Funding rate (for perpetuals)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funding_rate: Option<FundingRate>,
    /// Mark price (for futures/perpetuals)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mark_price: Option<MarkPrice>,
}

impl TickData {
    /// Creates a new builder for `TickData`.
    #[must_use]
    pub fn builder() -> TickDataBuilder {
        TickDataBuilder::default()
    }

    /// Validates the tick data.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Timestamp is zero
    /// - Bid/ask arrays have mismatched lengths
    pub fn validate(&self) -> Result<(), DataValidationError> {
        if self.timestamp.is_zero() {
            return Err(DataValidationError::InvalidTimestamp(
                "timestamp cannot be zero".to_string(),
            ));
        }

        if self.bid_prices.len() != self.bid_quantities.len() {
            return Err(DataValidationError::InvalidPriceRelation(
                "bid prices and quantities must have same length".to_string(),
            ));
        }

        if self.ask_prices.len() != self.ask_quantities.len() {
            return Err(DataValidationError::InvalidPriceRelation(
                "ask prices and quantities must have same length".to_string(),
            ));
        }

        Ok(())
    }

    /// Returns the best bid price (highest bid).
    #[must_use]
    pub fn best_bid(&self) -> Option<Price> {
        self.bid_prices.first().copied()
    }

    /// Returns the best ask price (lowest ask).
    #[must_use]
    pub fn best_ask(&self) -> Option<Price> {
        self.ask_prices.first().copied()
    }

    /// Returns the spread (best ask - best bid).
    #[must_use]
    pub fn spread(&self) -> Option<rust_decimal::Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        }
    }

    /// Returns the mid price ((best bid + best ask) / 2).
    #[must_use]
    pub fn mid_price(&self) -> Option<Price> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => {
                let mid = (bid.as_decimal() + ask.as_decimal()) / rust_decimal::Decimal::from(2);
                Price::new(mid).ok()
            }
            _ => None,
        }
    }
}

/// Builder for `TickData`.
#[derive(Debug, Default)]
pub struct TickDataBuilder {
    symbol: Option<Symbol>,
    timestamp: Option<Timestamp>,
    price: Option<Price>,
    volume: Option<Quantity>,
    bid_prices: Vec<Price>,
    bid_quantities: Vec<Quantity>,
    ask_prices: Vec<Price>,
    ask_quantities: Vec<Quantity>,
    open_interest: Option<Quantity>,
    funding_rate: Option<FundingRate>,
    mark_price: Option<MarkPrice>,
}

impl TickDataBuilder {
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

    /// Sets the last traded price.
    #[must_use]
    pub fn price(mut self, price: Price) -> Self {
        self.price = Some(price);
        self
    }

    /// Sets the last traded volume.
    #[must_use]
    pub fn volume(mut self, volume: Quantity) -> Self {
        self.volume = Some(volume);
        self
    }

    /// Adds a single bid level (price and quantity).
    #[must_use]
    pub fn bid_price(mut self, price: Price) -> Self {
        self.bid_prices.push(price);
        self
    }

    /// Adds a single bid quantity.
    #[must_use]
    pub fn bid_quantity(mut self, quantity: Quantity) -> Self {
        self.bid_quantities.push(quantity);
        self
    }

    /// Adds a single ask level (price and quantity).
    #[must_use]
    pub fn ask_price(mut self, price: Price) -> Self {
        self.ask_prices.push(price);
        self
    }

    /// Adds a single ask quantity.
    #[must_use]
    pub fn ask_quantity(mut self, quantity: Quantity) -> Self {
        self.ask_quantities.push(quantity);
        self
    }

    /// Sets all bid prices.
    #[must_use]
    pub fn bid_prices(mut self, prices: Vec<Price>) -> Self {
        self.bid_prices = prices;
        self
    }

    /// Sets all bid quantities.
    #[must_use]
    pub fn bid_quantities(mut self, quantities: Vec<Quantity>) -> Self {
        self.bid_quantities = quantities;
        self
    }

    /// Sets all ask prices.
    #[must_use]
    pub fn ask_prices(mut self, prices: Vec<Price>) -> Self {
        self.ask_prices = prices;
        self
    }

    /// Sets all ask quantities.
    #[must_use]
    pub fn ask_quantities(mut self, quantities: Vec<Quantity>) -> Self {
        self.ask_quantities = quantities;
        self
    }

    /// Sets the open interest.
    #[must_use]
    pub fn open_interest(mut self, open_interest: Quantity) -> Self {
        self.open_interest = Some(open_interest);
        self
    }

    /// Sets the funding rate.
    #[must_use]
    pub fn funding_rate(mut self, funding_rate: FundingRate) -> Self {
        self.funding_rate = Some(funding_rate);
        self
    }

    /// Sets the mark price.
    #[must_use]
    pub fn mark_price(mut self, mark_price: MarkPrice) -> Self {
        self.mark_price = Some(mark_price);
        self
    }

    /// Builds the `TickData`.
    ///
    /// # Errors
    ///
    /// Returns an error if any required field is missing or validation fails.
    pub fn build(self) -> Result<TickData, DataValidationError> {
        let tick = TickData {
            symbol: self
                .symbol
                .ok_or(DataValidationError::MissingField("symbol"))?,
            timestamp: self
                .timestamp
                .ok_or(DataValidationError::MissingField("timestamp"))?,
            price: self
                .price
                .ok_or(DataValidationError::MissingField("price"))?,
            volume: self
                .volume
                .ok_or(DataValidationError::MissingField("volume"))?,
            bid_prices: self.bid_prices,
            bid_quantities: self.bid_quantities,
            ask_prices: self.ask_prices,
            ask_quantities: self.ask_quantities,
            open_interest: self.open_interest,
            funding_rate: self.funding_rate,
            mark_price: self.mark_price,
        };
        tick.validate()?;
        Ok(tick)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_valid_tick() -> TickData {
        TickData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1_704_067_200_000).unwrap())
            .price(Price::new(dec!(42000)).unwrap())
            .volume(Quantity::new(dec!(0.5)).unwrap())
            .bid_price(Price::new(dec!(41999)).unwrap())
            .bid_quantity(Quantity::new(dec!(10)).unwrap())
            .ask_price(Price::new(dec!(42001)).unwrap())
            .ask_quantity(Quantity::new(dec!(8)).unwrap())
            .build()
            .unwrap()
    }

    #[test]
    fn test_tick_builder_valid() {
        let tick = create_valid_tick();
        assert_eq!(tick.symbol.as_str(), "BTC-USDT");
        assert_eq!(tick.price.as_decimal(), dec!(42000));
    }

    #[test]
    fn test_tick_builder_missing_field() {
        let result = TickData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .build();
        assert!(matches!(result, Err(DataValidationError::MissingField(_))));
    }

    #[test]
    fn test_tick_best_bid_ask() {
        let tick = create_valid_tick();
        assert_eq!(tick.best_bid().unwrap().as_decimal(), dec!(41999));
        assert_eq!(tick.best_ask().unwrap().as_decimal(), dec!(42001));
    }

    #[test]
    fn test_tick_spread() {
        let tick = create_valid_tick();
        assert_eq!(tick.spread().unwrap(), dec!(2));
    }

    #[test]
    fn test_tick_mid_price() {
        let tick = create_valid_tick();
        assert_eq!(tick.mid_price().unwrap().as_decimal(), dec!(42000));
    }

    #[test]
    fn test_tick_with_perpetual_fields() {
        let tick = TickData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1_704_067_200_000).unwrap())
            .price(Price::new(dec!(42000)).unwrap())
            .volume(Quantity::new(dec!(0.5)).unwrap())
            .open_interest(Quantity::new(dec!(1000000)).unwrap())
            .funding_rate(FundingRate::new(dec!(0.0001)).unwrap())
            .mark_price(MarkPrice::new(dec!(42005)).unwrap())
            .build()
            .unwrap();

        assert!(tick.open_interest.is_some());
        assert!(tick.funding_rate.is_some());
        assert!(tick.mark_price.is_some());
    }

    #[test]
    fn test_tick_serde_roundtrip() {
        let tick = create_valid_tick();
        let json = serde_json::to_string(&tick).unwrap();
        let parsed: TickData = serde_json::from_str(&json).unwrap();
        assert_eq!(tick, parsed);
    }

    #[test]
    fn test_tick_mismatched_bid_lengths() {
        let result = TickData::builder()
            .symbol(Symbol::new("BTC-USDT").unwrap())
            .timestamp(Timestamp::new(1_704_067_200_000).unwrap())
            .price(Price::new(dec!(42000)).unwrap())
            .volume(Quantity::new(dec!(0.5)).unwrap())
            .bid_price(Price::new(dec!(41999)).unwrap())
            // Missing bid_quantity
            .build();
        assert!(matches!(
            result,
            Err(DataValidationError::InvalidPriceRelation(_))
        ));
    }
}
