//! Position and account types for trading operations.
//!
//! This module provides position and account-related types including:
//! - [`Position`] - Represents a trading position
//! - [`PositionSide`] - Long, Short, or Both (hedge mode)
//! - [`MarginType`] - Cross or Isolated margin
//! - [`Account`] - Trading account information
//! - [`Balance`] - Currency balance

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::types::{Amount, Leverage, MarkPrice, Price, Quantity, Symbol, Timestamp};

/// Position side - Long, Short, or Both (for one-way mode).
///
/// # Examples
///
/// ```
/// use zephyr_core::data::PositionSide;
///
/// let side = PositionSide::Long;
/// assert!(side.is_long());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PositionSide {
    /// Long position (profit when price increases)
    Long,
    /// Short position (profit when price decreases)
    Short,
    /// Both sides (one-way position mode)
    Both,
}

impl PositionSide {
    /// Returns true if this is a long position.
    #[must_use]
    pub const fn is_long(&self) -> bool {
        matches!(self, Self::Long)
    }

    /// Returns true if this is a short position.
    #[must_use]
    pub const fn is_short(&self) -> bool {
        matches!(self, Self::Short)
    }

    /// Returns true if this is a one-way position mode.
    #[must_use]
    pub const fn is_both(&self) -> bool {
        matches!(self, Self::Both)
    }

    /// Returns the direction multiplier (1 for Long, -1 for Short, 0 for Both).
    #[must_use]
    pub fn direction(&self) -> Decimal {
        match self {
            Self::Long => Decimal::ONE,
            Self::Short => -Decimal::ONE,
            Self::Both => Decimal::ZERO,
        }
    }
}

impl fmt::Display for PositionSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Long => write!(f, "LONG"),
            Self::Short => write!(f, "SHORT"),
            Self::Both => write!(f, "BOTH"),
        }
    }
}

/// Margin type - Cross or Isolated margin mode.
///
/// # Examples
///
/// ```
/// use zephyr_core::data::MarginType;
///
/// let margin = MarginType::Cross;
/// assert!(margin.is_cross());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MarginType {
    /// Cross margin - shares margin across all positions
    #[default]
    Cross,
    /// Isolated margin - margin is isolated per position
    Isolated,
}

impl fmt::Display for MarginType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cross => write!(f, "CROSS"),
            Self::Isolated => write!(f, "ISOLATED"),
        }
    }
}

impl MarginType {
    /// Returns true if this is cross margin mode.
    #[must_use]
    pub const fn is_cross(&self) -> bool {
        matches!(self, Self::Cross)
    }

    /// Returns true if this is isolated margin mode.
    #[must_use]
    pub const fn is_isolated(&self) -> bool {
        matches!(self, Self::Isolated)
    }
}

/// Position - represents a trading position.
///
/// # Examples
///
/// ```
/// use zephyr_core::data::{Position, PositionSide, MarginType};
/// use zephyr_core::types::{Symbol, Price, Quantity, Leverage, MarkPrice, Amount, Timestamp};
/// use rust_decimal_macros::dec;
///
/// let position = Position::builder()
///     .symbol(Symbol::new("BTC-USDT").unwrap())
///     .side(PositionSide::Long)
///     .quantity(Quantity::new(dec!(0.1)).unwrap())
///     .entry_price(Price::new(dec!(50000)).unwrap())
///     .mark_price(MarkPrice::new(dec!(51000)).unwrap())
///     .leverage(Leverage::new(10).unwrap())
///     .margin_type(MarginType::Cross)
///     .update_time(Timestamp::now())
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// Trading symbol
    pub symbol: Symbol,
    /// Position side
    pub side: PositionSide,
    /// Position quantity (absolute value)
    pub quantity: Quantity,
    /// Average entry price
    pub entry_price: Price,
    /// Current mark price
    pub mark_price: MarkPrice,
    /// Liquidation price (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidation_price: Option<Price>,
    /// Unrealized profit/loss
    pub unrealized_pnl: Amount,
    /// Realized profit/loss
    pub realized_pnl: Amount,
    /// Position leverage
    pub leverage: Leverage,
    /// Margin type
    pub margin_type: MarginType,
    /// Initial margin required
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_margin: Option<Amount>,
    /// Maintenance margin required
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maintenance_margin: Option<Amount>,
    /// Last update timestamp
    pub update_time: Timestamp,
}

impl Position {
    /// Creates a new builder for `Position`.
    #[must_use]
    pub fn builder() -> PositionBuilder {
        PositionBuilder::default()
    }

    /// Returns the notional value of the position.
    #[must_use]
    pub fn notional_value(&self) -> Amount {
        Amount::new_unchecked(self.mark_price.as_decimal() * self.quantity.as_decimal())
    }

    /// Returns the entry notional value of the position.
    #[must_use]
    pub fn entry_value(&self) -> Amount {
        Amount::new_unchecked(self.entry_price.as_decimal() * self.quantity.as_decimal())
    }

    /// Returns true if the position is empty (zero quantity).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.quantity.is_zero()
    }

    /// Returns true if this is a long position.
    #[must_use]
    pub fn is_long(&self) -> bool {
        self.side.is_long() || (self.side.is_both() && self.quantity.is_positive())
    }

    /// Returns true if this is a short position.
    #[must_use]
    pub fn is_short(&self) -> bool {
        self.side.is_short() || (self.side.is_both() && self.quantity.is_negative())
    }

    /// Calculates unrealized `PnL` based on current mark price.
    ///
    /// For long positions: (`mark_price` - `entry_price`) * quantity
    /// For short positions: (`entry_price` - `mark_price`) * quantity
    #[must_use]
    pub fn calculate_unrealized_pnl(&self) -> Amount {
        let price_diff = self.mark_price.as_decimal() - self.entry_price.as_decimal();
        let direction = self.side.direction();
        let pnl = price_diff * self.quantity.as_decimal() * direction;
        Amount::new_unchecked(pnl)
    }

    /// Returns the margin ratio (`maintenance_margin` / `notional_value`).
    #[must_use]
    pub fn margin_ratio(&self) -> Option<Decimal> {
        let notional = self.notional_value();
        if notional.is_zero() {
            return None;
        }
        self.maintenance_margin
            .map(|mm| mm.as_decimal() / notional.as_decimal())
    }

    /// Returns the ROE (Return on Equity) percentage.
    #[must_use]
    pub fn roe(&self) -> Option<Decimal> {
        self.initial_margin.map(|im| {
            if im.is_zero() {
                Decimal::ZERO
            } else {
                (self.unrealized_pnl.as_decimal() / im.as_decimal()) * Decimal::ONE_HUNDRED
            }
        })
    }
}

/// Builder for `Position`.
#[derive(Debug, Default)]
pub struct PositionBuilder {
    symbol: Option<Symbol>,
    side: Option<PositionSide>,
    quantity: Option<Quantity>,
    entry_price: Option<Price>,
    mark_price: Option<MarkPrice>,
    liquidation_price: Option<Price>,
    unrealized_pnl: Amount,
    realized_pnl: Amount,
    leverage: Option<Leverage>,
    margin_type: MarginType,
    initial_margin: Option<Amount>,
    maintenance_margin: Option<Amount>,
    update_time: Option<Timestamp>,
}

impl PositionBuilder {
    /// Sets the trading symbol.
    #[must_use]
    pub fn symbol(mut self, symbol: Symbol) -> Self {
        self.symbol = Some(symbol);
        self
    }

    /// Sets the position side.
    #[must_use]
    pub fn side(mut self, side: PositionSide) -> Self {
        self.side = Some(side);
        self
    }

    /// Sets the position quantity.
    #[must_use]
    pub fn quantity(mut self, quantity: Quantity) -> Self {
        self.quantity = Some(quantity);
        self
    }

    /// Sets the entry price.
    #[must_use]
    pub fn entry_price(mut self, entry_price: Price) -> Self {
        self.entry_price = Some(entry_price);
        self
    }

    /// Sets the mark price.
    #[must_use]
    pub fn mark_price(mut self, mark_price: MarkPrice) -> Self {
        self.mark_price = Some(mark_price);
        self
    }

    /// Sets the liquidation price.
    #[must_use]
    pub fn liquidation_price(mut self, liquidation_price: Price) -> Self {
        self.liquidation_price = Some(liquidation_price);
        self
    }

    /// Sets the unrealized `PnL`.
    #[must_use]
    pub fn unrealized_pnl(mut self, unrealized_pnl: Amount) -> Self {
        self.unrealized_pnl = unrealized_pnl;
        self
    }

    /// Sets the realized `PnL`.
    #[must_use]
    pub fn realized_pnl(mut self, realized_pnl: Amount) -> Self {
        self.realized_pnl = realized_pnl;
        self
    }

    /// Sets the leverage.
    #[must_use]
    pub fn leverage(mut self, leverage: Leverage) -> Self {
        self.leverage = Some(leverage);
        self
    }

    /// Sets the margin type.
    #[must_use]
    pub fn margin_type(mut self, margin_type: MarginType) -> Self {
        self.margin_type = margin_type;
        self
    }

    /// Sets the initial margin.
    #[must_use]
    pub fn initial_margin(mut self, initial_margin: Amount) -> Self {
        self.initial_margin = Some(initial_margin);
        self
    }

    /// Sets the maintenance margin.
    #[must_use]
    pub fn maintenance_margin(mut self, maintenance_margin: Amount) -> Self {
        self.maintenance_margin = Some(maintenance_margin);
        self
    }

    /// Sets the update timestamp.
    #[must_use]
    pub fn update_time(mut self, update_time: Timestamp) -> Self {
        self.update_time = Some(update_time);
        self
    }

    /// Builds the `Position`.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing.
    pub fn build(self) -> Result<Position, PositionValidationError> {
        Ok(Position {
            symbol: self
                .symbol
                .ok_or(PositionValidationError::MissingField("symbol"))?,
            side: self
                .side
                .ok_or(PositionValidationError::MissingField("side"))?,
            quantity: self
                .quantity
                .ok_or(PositionValidationError::MissingField("quantity"))?,
            entry_price: self
                .entry_price
                .ok_or(PositionValidationError::MissingField("entry_price"))?,
            mark_price: self
                .mark_price
                .ok_or(PositionValidationError::MissingField("mark_price"))?,
            liquidation_price: self.liquidation_price,
            unrealized_pnl: self.unrealized_pnl,
            realized_pnl: self.realized_pnl,
            leverage: self
                .leverage
                .ok_or(PositionValidationError::MissingField("leverage"))?,
            margin_type: self.margin_type,
            initial_margin: self.initial_margin,
            maintenance_margin: self.maintenance_margin,
            update_time: self
                .update_time
                .ok_or(PositionValidationError::MissingField("update_time"))?,
        })
    }
}

/// Position validation error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PositionValidationError {
    /// Missing required field
    #[error("missing required field: {0}")]
    MissingField(&'static str),
}

/// Balance - represents a currency balance.
///
/// # Examples
///
/// ```
/// use zephyr_core::data::Balance;
/// use zephyr_core::types::Amount;
/// use rust_decimal_macros::dec;
///
/// let balance = Balance {
///     currency: "USDT".to_string(),
///     total: Amount::new(dec!(10000)).unwrap(),
///     available: Amount::new(dec!(8000)).unwrap(),
///     frozen: Amount::new(dec!(2000)).unwrap(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Balance {
    /// Currency code (e.g., "USDT", "BTC")
    pub currency: String,
    /// Total balance
    pub total: Amount,
    /// Available balance (can be used for trading)
    pub available: Amount,
    /// Frozen balance (locked in orders or positions)
    pub frozen: Amount,
}

impl Balance {
    /// Creates a new `Balance`.
    #[must_use]
    pub fn new(
        currency: impl Into<String>,
        total: Amount,
        available: Amount,
        frozen: Amount,
    ) -> Self {
        Self {
            currency: currency.into(),
            total,
            available,
            frozen,
        }
    }

    /// Returns true if the balance is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.total.is_zero()
    }

    /// Returns the utilization ratio (frozen / total).
    #[must_use]
    pub fn utilization(&self) -> Decimal {
        if self.total.is_zero() {
            return Decimal::ZERO;
        }
        self.frozen.as_decimal() / self.total.as_decimal()
    }
}

/// Exchange identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Exchange {
    /// Binance exchange
    Binance,
    /// OKX exchange
    Okx,
    /// Bitget exchange
    Bitget,
    /// Hyperliquid DEX
    Hyperliquid,
    /// Other exchange
    Other(String),
}

impl fmt::Display for Exchange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binance => write!(f, "binance"),
            Self::Okx => write!(f, "okx"),
            Self::Bitget => write!(f, "bitget"),
            Self::Hyperliquid => write!(f, "hyperliquid"),
            Self::Other(name) => write!(f, "{name}"),
        }
    }
}

/// Account - represents a trading account.
///
/// # Examples
///
/// ```
/// use zephyr_core::data::{Account, Balance, Exchange};
/// use zephyr_core::types::{Amount, Timestamp};
/// use rust_decimal_macros::dec;
///
/// let account = Account {
///     exchange: Exchange::Binance,
///     balances: vec![
///         Balance::new("USDT", Amount::new(dec!(10000)).unwrap(), Amount::new(dec!(8000)).unwrap(), Amount::new(dec!(2000)).unwrap()),
///     ],
///     total_equity: Amount::new(dec!(10000)).unwrap(),
///     available_balance: Amount::new(dec!(8000)).unwrap(),
///     margin_used: Amount::new(dec!(2000)).unwrap(),
///     unrealized_pnl: Amount::new(dec!(500)).unwrap(),
///     update_time: Timestamp::now(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    /// Exchange identifier
    pub exchange: Exchange,
    /// Currency balances
    pub balances: Vec<Balance>,
    /// Total account equity
    pub total_equity: Amount,
    /// Available balance for trading
    pub available_balance: Amount,
    /// Margin currently in use
    pub margin_used: Amount,
    /// Total unrealized `PnL` across all positions
    pub unrealized_pnl: Amount,
    /// Last update timestamp
    pub update_time: Timestamp,
}

impl Account {
    /// Returns the balance for a specific currency.
    #[must_use]
    pub fn get_balance(&self, currency: &str) -> Option<&Balance> {
        self.balances.iter().find(|b| b.currency == currency)
    }

    /// Returns the margin utilization ratio.
    #[must_use]
    pub fn margin_utilization(&self) -> Decimal {
        if self.total_equity.is_zero() {
            return Decimal::ZERO;
        }
        self.margin_used.as_decimal() / self.total_equity.as_decimal()
    }

    /// Returns true if the account has sufficient available balance.
    #[must_use]
    pub fn has_sufficient_balance(&self, required: Amount) -> bool {
        self.available_balance.as_decimal() >= required.as_decimal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn test_symbol() -> Symbol {
        Symbol::new("BTC-USDT").unwrap()
    }

    // PositionSide tests
    #[test]
    fn test_position_side_is_long() {
        assert!(PositionSide::Long.is_long());
        assert!(!PositionSide::Short.is_long());
        assert!(!PositionSide::Both.is_long());
    }

    #[test]
    fn test_position_side_direction() {
        assert_eq!(PositionSide::Long.direction(), Decimal::ONE);
        assert_eq!(PositionSide::Short.direction(), -Decimal::ONE);
        assert_eq!(PositionSide::Both.direction(), Decimal::ZERO);
    }

    #[test]
    fn test_position_side_display() {
        assert_eq!(format!("{}", PositionSide::Long), "LONG");
        assert_eq!(format!("{}", PositionSide::Short), "SHORT");
        assert_eq!(format!("{}", PositionSide::Both), "BOTH");
    }

    // MarginType tests
    #[test]
    fn test_margin_type_is_cross() {
        assert!(MarginType::Cross.is_cross());
        assert!(!MarginType::Isolated.is_cross());
    }

    #[test]
    fn test_margin_type_default() {
        assert_eq!(MarginType::default(), MarginType::Cross);
    }

    // Position tests
    #[test]
    fn test_position_builder() {
        let position = Position::builder()
            .symbol(test_symbol())
            .side(PositionSide::Long)
            .quantity(Quantity::new(dec!(0.1)).unwrap())
            .entry_price(Price::new(dec!(50000)).unwrap())
            .mark_price(MarkPrice::new(dec!(51000)).unwrap())
            .leverage(Leverage::new(10).unwrap())
            .margin_type(MarginType::Cross)
            .update_time(Timestamp::now())
            .build()
            .unwrap();

        assert_eq!(position.symbol, test_symbol());
        assert_eq!(position.side, PositionSide::Long);
        assert!(!position.is_empty());
    }

    #[test]
    fn test_position_notional_value() {
        let position = Position::builder()
            .symbol(test_symbol())
            .side(PositionSide::Long)
            .quantity(Quantity::new(dec!(0.1)).unwrap())
            .entry_price(Price::new(dec!(50000)).unwrap())
            .mark_price(MarkPrice::new(dec!(51000)).unwrap())
            .leverage(Leverage::new(10).unwrap())
            .update_time(Timestamp::now())
            .build()
            .unwrap();

        // 51000 * 0.1 = 5100
        assert_eq!(position.notional_value().as_decimal(), dec!(5100));
    }

    #[test]
    fn test_position_entry_value() {
        let position = Position::builder()
            .symbol(test_symbol())
            .side(PositionSide::Long)
            .quantity(Quantity::new(dec!(0.1)).unwrap())
            .entry_price(Price::new(dec!(50000)).unwrap())
            .mark_price(MarkPrice::new(dec!(51000)).unwrap())
            .leverage(Leverage::new(10).unwrap())
            .update_time(Timestamp::now())
            .build()
            .unwrap();

        // 50000 * 0.1 = 5000
        assert_eq!(position.entry_value().as_decimal(), dec!(5000));
    }

    #[test]
    fn test_position_calculate_unrealized_pnl_long() {
        let position = Position::builder()
            .symbol(test_symbol())
            .side(PositionSide::Long)
            .quantity(Quantity::new(dec!(0.1)).unwrap())
            .entry_price(Price::new(dec!(50000)).unwrap())
            .mark_price(MarkPrice::new(dec!(51000)).unwrap())
            .leverage(Leverage::new(10).unwrap())
            .update_time(Timestamp::now())
            .build()
            .unwrap();

        // (51000 - 50000) * 0.1 * 1 = 100
        assert_eq!(position.calculate_unrealized_pnl().as_decimal(), dec!(100));
    }

    #[test]
    fn test_position_calculate_unrealized_pnl_short() {
        let position = Position::builder()
            .symbol(test_symbol())
            .side(PositionSide::Short)
            .quantity(Quantity::new(dec!(0.1)).unwrap())
            .entry_price(Price::new(dec!(50000)).unwrap())
            .mark_price(MarkPrice::new(dec!(49000)).unwrap())
            .leverage(Leverage::new(10).unwrap())
            .update_time(Timestamp::now())
            .build()
            .unwrap();

        // (49000 - 50000) * 0.1 * -1 = 100
        assert_eq!(position.calculate_unrealized_pnl().as_decimal(), dec!(100));
    }

    #[test]
    fn test_position_roe() {
        let position = Position::builder()
            .symbol(test_symbol())
            .side(PositionSide::Long)
            .quantity(Quantity::new(dec!(0.1)).unwrap())
            .entry_price(Price::new(dec!(50000)).unwrap())
            .mark_price(MarkPrice::new(dec!(51000)).unwrap())
            .leverage(Leverage::new(10).unwrap())
            .unrealized_pnl(Amount::new(dec!(100)).unwrap())
            .initial_margin(Amount::new(dec!(500)).unwrap())
            .update_time(Timestamp::now())
            .build()
            .unwrap();

        // (100 / 500) * 100 = 20%
        assert_eq!(position.roe(), Some(dec!(20)));
    }

    // Balance tests
    #[test]
    fn test_balance_new() {
        let balance = Balance::new(
            "USDT",
            Amount::new(dec!(10000)).unwrap(),
            Amount::new(dec!(8000)).unwrap(),
            Amount::new(dec!(2000)).unwrap(),
        );

        assert_eq!(balance.currency, "USDT");
        assert!(!balance.is_empty());
    }

    #[test]
    fn test_balance_utilization() {
        let balance = Balance::new(
            "USDT",
            Amount::new(dec!(10000)).unwrap(),
            Amount::new(dec!(8000)).unwrap(),
            Amount::new(dec!(2000)).unwrap(),
        );

        // 2000 / 10000 = 0.2
        assert_eq!(balance.utilization(), dec!(0.2));
    }

    // Account tests
    #[test]
    fn test_account_get_balance() {
        let account = Account {
            exchange: Exchange::Binance,
            balances: vec![
                Balance::new(
                    "USDT",
                    Amount::new(dec!(10000)).unwrap(),
                    Amount::new(dec!(8000)).unwrap(),
                    Amount::new(dec!(2000)).unwrap(),
                ),
                Balance::new(
                    "BTC",
                    Amount::new(dec!(1)).unwrap(),
                    Amount::new(dec!(1)).unwrap(),
                    Amount::ZERO,
                ),
            ],
            total_equity: Amount::new(dec!(60000)).unwrap(),
            available_balance: Amount::new(dec!(50000)).unwrap(),
            margin_used: Amount::new(dec!(10000)).unwrap(),
            unrealized_pnl: Amount::new(dec!(500)).unwrap(),
            update_time: Timestamp::now(),
        };

        let usdt = account.get_balance("USDT").unwrap();
        assert_eq!(usdt.currency, "USDT");

        let btc = account.get_balance("BTC").unwrap();
        assert_eq!(btc.currency, "BTC");

        assert!(account.get_balance("ETH").is_none());
    }

    #[test]
    fn test_account_margin_utilization() {
        let account = Account {
            exchange: Exchange::Binance,
            balances: vec![],
            total_equity: Amount::new(dec!(10000)).unwrap(),
            available_balance: Amount::new(dec!(8000)).unwrap(),
            margin_used: Amount::new(dec!(2000)).unwrap(),
            unrealized_pnl: Amount::ZERO,
            update_time: Timestamp::now(),
        };

        // 2000 / 10000 = 0.2
        assert_eq!(account.margin_utilization(), dec!(0.2));
    }

    #[test]
    fn test_account_has_sufficient_balance() {
        let account = Account {
            exchange: Exchange::Binance,
            balances: vec![],
            total_equity: Amount::new(dec!(10000)).unwrap(),
            available_balance: Amount::new(dec!(8000)).unwrap(),
            margin_used: Amount::new(dec!(2000)).unwrap(),
            unrealized_pnl: Amount::ZERO,
            update_time: Timestamp::now(),
        };

        assert!(account.has_sufficient_balance(Amount::new(dec!(5000)).unwrap()));
        assert!(account.has_sufficient_balance(Amount::new(dec!(8000)).unwrap()));
        assert!(!account.has_sufficient_balance(Amount::new(dec!(9000)).unwrap()));
    }

    // Serde tests
    #[test]
    fn test_position_side_serde_roundtrip() {
        let side = PositionSide::Long;
        let json = serde_json::to_string(&side).unwrap();
        let parsed: PositionSide = serde_json::from_str(&json).unwrap();
        assert_eq!(side, parsed);
    }

    #[test]
    fn test_margin_type_serde_roundtrip() {
        let margin = MarginType::Isolated;
        let json = serde_json::to_string(&margin).unwrap();
        let parsed: MarginType = serde_json::from_str(&json).unwrap();
        assert_eq!(margin, parsed);
    }

    #[test]
    fn test_position_serde_roundtrip() {
        let position = Position::builder()
            .symbol(test_symbol())
            .side(PositionSide::Long)
            .quantity(Quantity::new(dec!(0.1)).unwrap())
            .entry_price(Price::new(dec!(50000)).unwrap())
            .mark_price(MarkPrice::new(dec!(51000)).unwrap())
            .leverage(Leverage::new(10).unwrap())
            .margin_type(MarginType::Cross)
            .update_time(Timestamp::new(1_704_067_200_000).unwrap())
            .build()
            .unwrap();

        let json = serde_json::to_string(&position).unwrap();
        let parsed: Position = serde_json::from_str(&json).unwrap();
        assert_eq!(position, parsed);
    }

    #[test]
    fn test_balance_serde_roundtrip() {
        let balance = Balance::new(
            "USDT",
            Amount::new(dec!(10000)).unwrap(),
            Amount::new(dec!(8000)).unwrap(),
            Amount::new(dec!(2000)).unwrap(),
        );

        let json = serde_json::to_string(&balance).unwrap();
        let parsed: Balance = serde_json::from_str(&json).unwrap();
        assert_eq!(balance, parsed);
    }

    #[test]
    fn test_account_serde_roundtrip() {
        let account = Account {
            exchange: Exchange::Binance,
            balances: vec![Balance::new(
                "USDT",
                Amount::new(dec!(10000)).unwrap(),
                Amount::new(dec!(8000)).unwrap(),
                Amount::new(dec!(2000)).unwrap(),
            )],
            total_equity: Amount::new(dec!(10000)).unwrap(),
            available_balance: Amount::new(dec!(8000)).unwrap(),
            margin_used: Amount::new(dec!(2000)).unwrap(),
            unrealized_pnl: Amount::new(dec!(500)).unwrap(),
            update_time: Timestamp::new(1_704_067_200_000).unwrap(),
        };

        let json = serde_json::to_string(&account).unwrap();
        let parsed: Account = serde_json::from_str(&json).unwrap();
        assert_eq!(account, parsed);
    }

    #[test]
    fn test_exchange_display() {
        assert_eq!(format!("{}", Exchange::Binance), "binance");
        assert_eq!(format!("{}", Exchange::Okx), "okx");
        assert_eq!(
            format!("{}", Exchange::Other("custom".to_string())),
            "custom"
        );
    }
}
