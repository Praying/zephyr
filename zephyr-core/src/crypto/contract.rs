//! Contract information management and symbol normalization.
//!
//! This module provides functionality for managing contract specifications
//! and normalizing symbols across different exchanges.
//!
//! # Features
//!
//! - Contract information storage and retrieval
//! - Cross-exchange symbol normalization
//! - Precision and minimum order size handling
//! - Support for linear and inverse contracts

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
#[allow(clippy::disallowed_types)]
use std::collections::HashMap;
use std::sync::Arc;

use crate::data::Exchange;
use crate::types::Symbol;

/// Contract type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContractType {
    /// Spot trading
    Spot,
    /// Linear perpetual (USDT-margined)
    LinearPerpetual,
    /// Inverse perpetual (coin-margined)
    InversePerpetual,
    /// Linear futures (USDT-margined with expiry)
    LinearFutures,
    /// Inverse futures (coin-margined with expiry)
    InverseFutures,
}

impl ContractType {
    /// Returns true if this is a perpetual contract.
    #[must_use]
    pub const fn is_perpetual(&self) -> bool {
        matches!(self, Self::LinearPerpetual | Self::InversePerpetual)
    }

    /// Returns true if this is a futures contract.
    #[must_use]
    pub const fn is_futures(&self) -> bool {
        matches!(self, Self::LinearFutures | Self::InverseFutures)
    }

    /// Returns true if this is a linear contract (USDT-margined).
    #[must_use]
    pub const fn is_linear(&self) -> bool {
        matches!(self, Self::LinearPerpetual | Self::LinearFutures)
    }

    /// Returns true if this is an inverse contract (coin-margined).
    #[must_use]
    pub const fn is_inverse(&self) -> bool {
        matches!(self, Self::InversePerpetual | Self::InverseFutures)
    }

    /// Returns true if this is a spot market.
    #[must_use]
    pub const fn is_spot(&self) -> bool {
        matches!(self, Self::Spot)
    }
}

impl std::fmt::Display for ContractType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spot => write!(f, "SPOT"),
            Self::LinearPerpetual => write!(f, "LINEAR_PERPETUAL"),
            Self::InversePerpetual => write!(f, "INVERSE_PERPETUAL"),
            Self::LinearFutures => write!(f, "LINEAR_FUTURES"),
            Self::InverseFutures => write!(f, "INVERSE_FUTURES"),
        }
    }
}

/// Margin mode for the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MarginMode {
    /// Cross margin mode
    #[default]
    Cross,
    /// Isolated margin mode
    Isolated,
}

/// Contract information.
///
/// Contains all specifications for a trading contract including
/// precision, limits, and margin requirements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractInfo {
    /// Normalized symbol (e.g., "BTC-USDT")
    pub symbol: Symbol,
    /// Exchange-specific symbol (e.g., "BTCUSDT" for Binance)
    pub exchange_symbol: String,
    /// Exchange identifier
    pub exchange: Exchange,
    /// Contract type
    pub contract_type: ContractType,
    /// Base asset (e.g., "BTC")
    pub base_asset: String,
    /// Quote asset (e.g., "USDT")
    pub quote_asset: String,
    /// Price tick size (minimum price increment)
    pub tick_size: Decimal,
    /// Lot size (minimum quantity increment)
    pub lot_size: Decimal,
    /// Minimum order quantity
    pub min_quantity: Decimal,
    /// Maximum order quantity
    pub max_quantity: Decimal,
    /// Minimum notional value
    pub min_notional: Decimal,
    /// Price precision (decimal places)
    pub price_precision: u8,
    /// Quantity precision (decimal places)
    pub quantity_precision: u8,
    /// Maximum leverage allowed
    pub max_leverage: u8,
    /// Maintenance margin rate
    pub maintenance_margin_rate: Decimal,
    /// Taker fee rate
    pub taker_fee_rate: Decimal,
    /// Maker fee rate
    pub maker_fee_rate: Decimal,
    /// Whether the contract is currently tradeable
    pub is_trading: bool,
}

impl ContractInfo {
    /// Creates a new contract info builder.
    #[must_use]
    pub fn builder() -> ContractInfoBuilder {
        ContractInfoBuilder::default()
    }

    /// Rounds a price to the contract's tick size.
    #[must_use]
    pub fn round_price(&self, price: Decimal) -> Decimal {
        if self.tick_size.is_zero() {
            return price;
        }
        (price / self.tick_size).round() * self.tick_size
    }

    /// Rounds a quantity to the contract's lot size.
    #[must_use]
    pub fn round_quantity(&self, quantity: Decimal) -> Decimal {
        if self.lot_size.is_zero() {
            return quantity;
        }
        (quantity / self.lot_size).floor() * self.lot_size
    }

    /// Validates if a price is valid for this contract.
    #[must_use]
    pub fn is_valid_price(&self, price: Decimal) -> bool {
        price > Decimal::ZERO && (price % self.tick_size).is_zero()
    }

    /// Validates if a quantity is valid for this contract.
    #[must_use]
    pub fn is_valid_quantity(&self, quantity: Decimal) -> bool {
        quantity >= self.min_quantity
            && quantity <= self.max_quantity
            && (quantity % self.lot_size).is_zero()
    }

    /// Validates if a notional value meets the minimum requirement.
    #[must_use]
    pub fn is_valid_notional(&self, price: Decimal, quantity: Decimal) -> bool {
        price * quantity >= self.min_notional
    }
}

/// Builder for `ContractInfo`.
#[derive(Debug, Default)]
pub struct ContractInfoBuilder {
    symbol: Option<Symbol>,
    exchange_symbol: Option<String>,
    exchange: Option<Exchange>,
    contract_type: Option<ContractType>,
    base_asset: Option<String>,
    quote_asset: Option<String>,
    tick_size: Decimal,
    lot_size: Decimal,
    min_quantity: Decimal,
    max_quantity: Decimal,
    min_notional: Decimal,
    price_precision: u8,
    quantity_precision: u8,
    max_leverage: u8,
    maintenance_margin_rate: Decimal,
    taker_fee_rate: Decimal,
    maker_fee_rate: Decimal,
    is_trading: bool,
}

impl ContractInfoBuilder {
    /// Sets the normalized symbol.
    #[must_use]
    pub fn symbol(mut self, symbol: Symbol) -> Self {
        self.symbol = Some(symbol);
        self
    }

    /// Sets the exchange-specific symbol.
    #[must_use]
    pub fn exchange_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.exchange_symbol = Some(symbol.into());
        self
    }

    /// Sets the exchange.
    #[must_use]
    pub fn exchange(mut self, exchange: Exchange) -> Self {
        self.exchange = Some(exchange);
        self
    }

    /// Sets the contract type.
    #[must_use]
    pub fn contract_type(mut self, contract_type: ContractType) -> Self {
        self.contract_type = Some(contract_type);
        self
    }

    /// Sets the base asset.
    #[must_use]
    pub fn base_asset(mut self, asset: impl Into<String>) -> Self {
        self.base_asset = Some(asset.into());
        self
    }

    /// Sets the quote asset.
    #[must_use]
    pub fn quote_asset(mut self, asset: impl Into<String>) -> Self {
        self.quote_asset = Some(asset.into());
        self
    }

    /// Sets the tick size.
    #[must_use]
    pub fn tick_size(mut self, tick_size: Decimal) -> Self {
        self.tick_size = tick_size;
        self
    }

    /// Sets the lot size.
    #[must_use]
    pub fn lot_size(mut self, lot_size: Decimal) -> Self {
        self.lot_size = lot_size;
        self
    }

    /// Sets the minimum quantity.
    #[must_use]
    pub fn min_quantity(mut self, min_quantity: Decimal) -> Self {
        self.min_quantity = min_quantity;
        self
    }

    /// Sets the maximum quantity.
    #[must_use]
    pub fn max_quantity(mut self, max_quantity: Decimal) -> Self {
        self.max_quantity = max_quantity;
        self
    }

    /// Sets the minimum notional value.
    #[must_use]
    pub fn min_notional(mut self, min_notional: Decimal) -> Self {
        self.min_notional = min_notional;
        self
    }

    /// Sets the price precision.
    #[must_use]
    pub fn price_precision(mut self, precision: u8) -> Self {
        self.price_precision = precision;
        self
    }

    /// Sets the quantity precision.
    #[must_use]
    pub fn quantity_precision(mut self, precision: u8) -> Self {
        self.quantity_precision = precision;
        self
    }

    /// Sets the maximum leverage.
    #[must_use]
    pub fn max_leverage(mut self, leverage: u8) -> Self {
        self.max_leverage = leverage;
        self
    }

    /// Sets the maintenance margin rate.
    #[must_use]
    pub fn maintenance_margin_rate(mut self, rate: Decimal) -> Self {
        self.maintenance_margin_rate = rate;
        self
    }

    /// Sets the taker fee rate.
    #[must_use]
    pub fn taker_fee_rate(mut self, rate: Decimal) -> Self {
        self.taker_fee_rate = rate;
        self
    }

    /// Sets the maker fee rate.
    #[must_use]
    pub fn maker_fee_rate(mut self, rate: Decimal) -> Self {
        self.maker_fee_rate = rate;
        self
    }

    /// Sets whether the contract is trading.
    #[must_use]
    pub fn is_trading(mut self, is_trading: bool) -> Self {
        self.is_trading = is_trading;
        self
    }

    /// Builds the `ContractInfo`.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing.
    pub fn build(self) -> Result<ContractInfo, ContractValidationError> {
        Ok(ContractInfo {
            symbol: self
                .symbol
                .ok_or(ContractValidationError::MissingField("symbol"))?,
            exchange_symbol: self
                .exchange_symbol
                .ok_or(ContractValidationError::MissingField("exchange_symbol"))?,
            exchange: self
                .exchange
                .ok_or(ContractValidationError::MissingField("exchange"))?,
            contract_type: self
                .contract_type
                .ok_or(ContractValidationError::MissingField("contract_type"))?,
            base_asset: self
                .base_asset
                .ok_or(ContractValidationError::MissingField("base_asset"))?,
            quote_asset: self
                .quote_asset
                .ok_or(ContractValidationError::MissingField("quote_asset"))?,
            tick_size: self.tick_size,
            lot_size: self.lot_size,
            min_quantity: self.min_quantity,
            max_quantity: self.max_quantity,
            min_notional: self.min_notional,
            price_precision: self.price_precision,
            quantity_precision: self.quantity_precision,
            max_leverage: self.max_leverage,
            maintenance_margin_rate: self.maintenance_margin_rate,
            taker_fee_rate: self.taker_fee_rate,
            maker_fee_rate: self.maker_fee_rate,
            is_trading: self.is_trading,
        })
    }
}

/// Contract validation error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ContractValidationError {
    /// Missing required field
    #[error("missing required field: {0}")]
    MissingField(&'static str),
    /// Invalid tick size
    #[error("invalid tick size: {0}")]
    InvalidTickSize(String),
    /// Invalid lot size
    #[error("invalid lot size: {0}")]
    InvalidLotSize(String),
}

/// Symbol normalizer for cross-exchange symbol standardization.
///
/// Converts exchange-specific symbols to a normalized format and vice versa.
///
/// # Examples
///
/// ```
/// use zephyr_core::crypto::SymbolNormalizer;
/// use zephyr_core::data::Exchange;
///
/// let normalizer = SymbolNormalizer::new();
///
/// // Normalize Binance symbol
/// let normalized = normalizer.normalize("BTCUSDT", &Exchange::Binance);
/// assert_eq!(normalized, "BTC-USDT");
///
/// // Convert back to exchange format
/// let exchange_symbol = normalizer.to_exchange_format("BTC-USDT", &Exchange::Binance);
/// assert_eq!(exchange_symbol, "BTCUSDT");
/// ```
#[derive(Debug, Clone, Default)]
#[allow(clippy::disallowed_types)]
pub struct SymbolNormalizer {
    /// Custom mappings for special cases
    custom_mappings: HashMap<(String, Exchange), String>,
}

impl SymbolNormalizer {
    /// Creates a new symbol normalizer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a custom mapping for a specific exchange symbol.
    pub fn add_mapping(&mut self, exchange_symbol: &str, exchange: Exchange, normalized: &str) {
        self.custom_mappings.insert(
            (exchange_symbol.to_string(), exchange),
            normalized.to_string(),
        );
    }

    /// Normalizes an exchange-specific symbol to the standard format.
    ///
    /// Standard format: `BASE-QUOTE` (e.g., "BTC-USDT")
    #[must_use]
    pub fn normalize(&self, exchange_symbol: &str, exchange: &Exchange) -> String {
        // Check custom mappings first
        if let Some(normalized) = self
            .custom_mappings
            .get(&(exchange_symbol.to_string(), exchange.clone()))
        {
            return normalized.clone();
        }

        // Apply exchange-specific normalization rules
        match exchange {
            Exchange::Binance => Self::normalize_binance(exchange_symbol),
            Exchange::Okx => Self::normalize_okx(exchange_symbol),
            Exchange::Bitget => Self::normalize_bitget(exchange_symbol),
            Exchange::Hyperliquid => Self::normalize_hyperliquid(exchange_symbol),
            Exchange::Other(_) => exchange_symbol.to_string(),
        }
    }

    /// Converts a normalized symbol to exchange-specific format.
    #[must_use]
    pub fn to_exchange_format(&self, normalized: &str, exchange: &Exchange) -> String {
        match exchange {
            Exchange::Binance => Self::to_binance_format(normalized),
            Exchange::Okx => Self::to_okx_format(normalized),
            Exchange::Bitget => Self::to_bitget_format(normalized),
            Exchange::Hyperliquid => Self::to_hyperliquid_format(normalized),
            Exchange::Other(_) => normalized.to_string(),
        }
    }

    fn normalize_binance(symbol: &str) -> String {
        // Binance uses concatenated format: BTCUSDT
        // Common quote assets: USDT, BUSD, USDC, BTC, ETH, BNB
        let quote_assets = ["USDT", "BUSD", "USDC", "BTC", "ETH", "BNB", "TUSD", "PAX"];

        for quote in quote_assets {
            if let Some(base) = symbol.strip_suffix(quote) {
                return format!("{base}-{quote}");
            }
        }
        symbol.to_string()
    }

    fn normalize_okx(symbol: &str) -> String {
        // OKX uses hyphenated format: BTC-USDT, BTC-USDT-SWAP
        // Remove contract suffix if present
        let parts: Vec<&str> = symbol.split('-').collect();
        if parts.len() >= 2 {
            // Check if last part is a contract type suffix
            let last = parts.last().unwrap_or(&"");
            if *last == "SWAP" || *last == "PERP" || last.chars().all(char::is_numeric) {
                // It's a derivative, use first two parts
                return format!("{}-{}", parts[0], parts[1]);
            }
            return format!("{}-{}", parts[0], parts[1]);
        }
        symbol.to_string()
    }

    fn normalize_bitget(symbol: &str) -> String {
        // Bitget uses formats like: BTCUSDT_UMCBL (perpetual), BTCUSDT (spot)
        let symbol = symbol.split('_').next().unwrap_or(symbol);
        Self::normalize_binance(symbol)
    }

    fn normalize_hyperliquid(symbol: &str) -> String {
        // Hyperliquid uses simple format: BTC, ETH (implied USDC quote)
        if !symbol.contains('-') && !symbol.contains('/') {
            return format!("{symbol}-USDC");
        }
        symbol.replace('/', "-")
    }

    fn to_binance_format(normalized: &str) -> String {
        // Convert BTC-USDT to BTCUSDT
        normalized.replace('-', "")
    }

    fn to_okx_format(normalized: &str) -> String {
        // OKX already uses hyphenated format
        normalized.to_string()
    }

    fn to_bitget_format(normalized: &str) -> String {
        // Convert BTC-USDT to BTCUSDT
        normalized.replace('-', "")
    }

    fn to_hyperliquid_format(normalized: &str) -> String {
        // Hyperliquid uses just the base asset for USDC pairs
        if let Some(base) = normalized.strip_suffix("-USDC") {
            return base.to_string();
        }
        normalized.to_string()
    }
}

/// Contract manager for storing and retrieving contract information.
///
/// Provides a centralized registry for contract specifications across exchanges.
#[derive(Debug, Clone, Default)]
#[allow(clippy::disallowed_types)]
pub struct ContractManager {
    /// Contracts indexed by normalized symbol and exchange
    contracts: HashMap<(Symbol, Exchange), Arc<ContractInfo>>,
    /// Symbol normalizer
    normalizer: SymbolNormalizer,
}

impl ContractManager {
    /// Creates a new contract manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a contract manager with a custom normalizer.
    #[must_use]
    #[allow(clippy::disallowed_types)]
    pub fn with_normalizer(normalizer: SymbolNormalizer) -> Self {
        Self {
            contracts: HashMap::new(),
            normalizer,
        }
    }

    /// Registers a contract.
    pub fn register(&mut self, contract: ContractInfo) {
        let key = (contract.symbol.clone(), contract.exchange.clone());
        self.contracts.insert(key, Arc::new(contract));
    }

    /// Gets a contract by normalized symbol and exchange.
    #[must_use]
    pub fn get(&self, symbol: &Symbol, exchange: &Exchange) -> Option<Arc<ContractInfo>> {
        self.contracts
            .get(&(symbol.clone(), exchange.clone()))
            .cloned()
    }

    /// Gets a contract by exchange-specific symbol.
    #[must_use]
    pub fn get_by_exchange_symbol(
        &self,
        exchange_symbol: &str,
        exchange: &Exchange,
    ) -> Option<Arc<ContractInfo>> {
        let normalized = self.normalizer.normalize(exchange_symbol, exchange);
        if let Ok(symbol) = Symbol::new(&normalized) {
            return self.get(&symbol, exchange);
        }
        None
    }

    /// Lists all contracts for an exchange.
    #[must_use]
    pub fn list_by_exchange(&self, exchange: &Exchange) -> Vec<Arc<ContractInfo>> {
        self.contracts
            .iter()
            .filter(|((_, ex), _)| ex == exchange)
            .map(|(_, contract)| contract.clone())
            .collect()
    }

    /// Lists all contracts.
    #[must_use]
    pub fn list_all(&self) -> Vec<Arc<ContractInfo>> {
        self.contracts.values().cloned().collect()
    }

    /// Removes a contract.
    pub fn remove(&mut self, symbol: &Symbol, exchange: &Exchange) -> Option<Arc<ContractInfo>> {
        self.contracts.remove(&(symbol.clone(), exchange.clone()))
    }

    /// Returns the number of registered contracts.
    #[must_use]
    pub fn len(&self) -> usize {
        self.contracts.len()
    }

    /// Returns true if no contracts are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.contracts.is_empty()
    }

    /// Returns a reference to the symbol normalizer.
    #[must_use]
    pub fn normalizer(&self) -> &SymbolNormalizer {
        &self.normalizer
    }

    /// Rounds a price according to contract specifications.
    #[must_use]
    pub fn round_price(
        &self,
        symbol: &Symbol,
        exchange: &Exchange,
        price: Decimal,
    ) -> Option<Decimal> {
        self.get(symbol, exchange).map(|c| c.round_price(price))
    }

    /// Rounds a quantity according to contract specifications.
    #[must_use]
    pub fn round_quantity(
        &self,
        symbol: &Symbol,
        exchange: &Exchange,
        quantity: Decimal,
    ) -> Option<Decimal> {
        self.get(symbol, exchange)
            .map(|c| c.round_quantity(quantity))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn test_symbol() -> Symbol {
        Symbol::new("BTC-USDT").unwrap()
    }

    fn test_contract() -> ContractInfo {
        ContractInfo::builder()
            .symbol(test_symbol())
            .exchange_symbol("BTCUSDT")
            .exchange(Exchange::Binance)
            .contract_type(ContractType::LinearPerpetual)
            .base_asset("BTC")
            .quote_asset("USDT")
            .tick_size(dec!(0.1))
            .lot_size(dec!(0.001))
            .min_quantity(dec!(0.001))
            .max_quantity(dec!(1000))
            .min_notional(dec!(5))
            .price_precision(1)
            .quantity_precision(3)
            .max_leverage(125)
            .maintenance_margin_rate(dec!(0.004))
            .taker_fee_rate(dec!(0.0004))
            .maker_fee_rate(dec!(0.0002))
            .is_trading(true)
            .build()
            .unwrap()
    }

    // ContractType tests
    #[test]
    fn test_contract_type_is_perpetual() {
        assert!(ContractType::LinearPerpetual.is_perpetual());
        assert!(ContractType::InversePerpetual.is_perpetual());
        assert!(!ContractType::Spot.is_perpetual());
        assert!(!ContractType::LinearFutures.is_perpetual());
    }

    #[test]
    fn test_contract_type_is_linear() {
        assert!(ContractType::LinearPerpetual.is_linear());
        assert!(ContractType::LinearFutures.is_linear());
        assert!(!ContractType::InversePerpetual.is_linear());
    }

    #[test]
    fn test_contract_type_display() {
        assert_eq!(
            format!("{}", ContractType::LinearPerpetual),
            "LINEAR_PERPETUAL"
        );
        assert_eq!(format!("{}", ContractType::Spot), "SPOT");
    }

    // ContractInfo tests
    #[test]
    fn test_contract_info_builder() {
        let contract = test_contract();
        assert_eq!(contract.symbol, test_symbol());
        assert_eq!(contract.exchange_symbol, "BTCUSDT");
        assert_eq!(contract.exchange, Exchange::Binance);
        assert!(contract.is_trading);
    }

    #[test]
    fn test_contract_round_price() {
        let contract = test_contract();

        // 50000.15 should round to 50000.2 with tick_size 0.1
        assert_eq!(contract.round_price(dec!(50000.15)), dec!(50000.2));
        // 50000.14 should round to 50000.1
        assert_eq!(contract.round_price(dec!(50000.14)), dec!(50000.1));
    }

    #[test]
    fn test_contract_round_quantity() {
        let contract = test_contract();

        // 1.2345 should floor to 1.234 with lot_size 0.001
        assert_eq!(contract.round_quantity(dec!(1.2345)), dec!(1.234));
        // 1.2349 should still floor to 1.234
        assert_eq!(contract.round_quantity(dec!(1.2349)), dec!(1.234));
    }

    #[test]
    fn test_contract_is_valid_price() {
        let contract = test_contract();

        assert!(contract.is_valid_price(dec!(50000.1)));
        assert!(contract.is_valid_price(dec!(50000.0)));
        assert!(!contract.is_valid_price(dec!(50000.15))); // Not on tick
        assert!(!contract.is_valid_price(dec!(0))); // Zero
        assert!(!contract.is_valid_price(dec!(-100))); // Negative
    }

    #[test]
    fn test_contract_is_valid_quantity() {
        let contract = test_contract();

        assert!(contract.is_valid_quantity(dec!(0.001))); // Min quantity
        assert!(contract.is_valid_quantity(dec!(1.0)));
        assert!(contract.is_valid_quantity(dec!(1000))); // Max quantity
        assert!(!contract.is_valid_quantity(dec!(0.0001))); // Below min
        assert!(!contract.is_valid_quantity(dec!(1001))); // Above max
        assert!(!contract.is_valid_quantity(dec!(1.0001))); // Not on lot size
    }

    #[test]
    fn test_contract_is_valid_notional() {
        let contract = test_contract();

        assert!(contract.is_valid_notional(dec!(50000), dec!(0.001))); // 50 USDT
        assert!(contract.is_valid_notional(dec!(5000), dec!(0.001))); // 5 USDT (min)
        assert!(!contract.is_valid_notional(dec!(4000), dec!(0.001))); // 4 USDT (below min)
    }

    // SymbolNormalizer tests
    #[test]
    fn test_normalize_binance() {
        let normalizer = SymbolNormalizer::new();

        assert_eq!(
            normalizer.normalize("BTCUSDT", &Exchange::Binance),
            "BTC-USDT"
        );
        assert_eq!(
            normalizer.normalize("ETHUSDT", &Exchange::Binance),
            "ETH-USDT"
        );
        assert_eq!(
            normalizer.normalize("BTCBUSD", &Exchange::Binance),
            "BTC-BUSD"
        );
        assert_eq!(
            normalizer.normalize("ETHBTC", &Exchange::Binance),
            "ETH-BTC"
        );
    }

    #[test]
    fn test_normalize_okx() {
        let normalizer = SymbolNormalizer::new();

        assert_eq!(normalizer.normalize("BTC-USDT", &Exchange::Okx), "BTC-USDT");
        assert_eq!(
            normalizer.normalize("BTC-USDT-SWAP", &Exchange::Okx),
            "BTC-USDT"
        );
        assert_eq!(
            normalizer.normalize("ETH-USDT-PERP", &Exchange::Okx),
            "ETH-USDT"
        );
    }

    #[test]
    fn test_normalize_bitget() {
        let normalizer = SymbolNormalizer::new();

        assert_eq!(
            normalizer.normalize("BTCUSDT", &Exchange::Bitget),
            "BTC-USDT"
        );
        assert_eq!(
            normalizer.normalize("BTCUSDT_UMCBL", &Exchange::Bitget),
            "BTC-USDT"
        );
    }

    #[test]
    fn test_normalize_hyperliquid() {
        let normalizer = SymbolNormalizer::new();

        assert_eq!(
            normalizer.normalize("BTC", &Exchange::Hyperliquid),
            "BTC-USDC"
        );
        assert_eq!(
            normalizer.normalize("ETH", &Exchange::Hyperliquid),
            "ETH-USDC"
        );
    }

    #[test]
    fn test_to_exchange_format_binance() {
        let normalizer = SymbolNormalizer::new();

        assert_eq!(
            normalizer.to_exchange_format("BTC-USDT", &Exchange::Binance),
            "BTCUSDT"
        );
        assert_eq!(
            normalizer.to_exchange_format("ETH-BTC", &Exchange::Binance),
            "ETHBTC"
        );
    }

    #[test]
    fn test_to_exchange_format_hyperliquid() {
        let normalizer = SymbolNormalizer::new();

        assert_eq!(
            normalizer.to_exchange_format("BTC-USDC", &Exchange::Hyperliquid),
            "BTC"
        );
        assert_eq!(
            normalizer.to_exchange_format("ETH-USDC", &Exchange::Hyperliquid),
            "ETH"
        );
    }

    #[test]
    fn test_custom_mapping() {
        let mut normalizer = SymbolNormalizer::new();
        normalizer.add_mapping("1000SHIBUSDT", Exchange::Binance, "SHIB-USDT");

        assert_eq!(
            normalizer.normalize("1000SHIBUSDT", &Exchange::Binance),
            "SHIB-USDT"
        );
    }

    // ContractManager tests
    #[test]
    fn test_contract_manager_register_and_get() {
        let mut manager = ContractManager::new();
        let contract = test_contract();

        manager.register(contract);

        let retrieved = manager.get(&test_symbol(), &Exchange::Binance).unwrap();
        assert_eq!(retrieved.symbol, test_symbol());
        assert_eq!(retrieved.exchange_symbol, "BTCUSDT");
    }

    #[test]
    fn test_contract_manager_get_by_exchange_symbol() {
        let mut manager = ContractManager::new();
        manager.register(test_contract());

        let retrieved = manager
            .get_by_exchange_symbol("BTCUSDT", &Exchange::Binance)
            .unwrap();
        assert_eq!(retrieved.symbol, test_symbol());
    }

    #[test]
    fn test_contract_manager_list_by_exchange() {
        let mut manager = ContractManager::new();
        manager.register(test_contract());

        let eth_contract = ContractInfo::builder()
            .symbol(Symbol::new("ETH-USDT").unwrap())
            .exchange_symbol("ETHUSDT")
            .exchange(Exchange::Binance)
            .contract_type(ContractType::LinearPerpetual)
            .base_asset("ETH")
            .quote_asset("USDT")
            .is_trading(true)
            .build()
            .unwrap();
        manager.register(eth_contract);

        let binance_contracts = manager.list_by_exchange(&Exchange::Binance);
        assert_eq!(binance_contracts.len(), 2);

        let okx_contracts = manager.list_by_exchange(&Exchange::Okx);
        assert!(okx_contracts.is_empty());
    }

    #[test]
    fn test_contract_manager_remove() {
        let mut manager = ContractManager::new();
        manager.register(test_contract());

        assert_eq!(manager.len(), 1);

        let removed = manager.remove(&test_symbol(), &Exchange::Binance);
        assert!(removed.is_some());
        assert!(manager.is_empty());
    }

    // Serde tests
    #[test]
    fn test_contract_type_serde_roundtrip() {
        let ct = ContractType::LinearPerpetual;
        let json = serde_json::to_string(&ct).unwrap();
        let parsed: ContractType = serde_json::from_str(&json).unwrap();
        assert_eq!(ct, parsed);
    }

    #[test]
    fn test_contract_info_serde_roundtrip() {
        let contract = test_contract();
        let json = serde_json::to_string(&contract).unwrap();
        let parsed: ContractInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(contract, parsed);
    }
}
