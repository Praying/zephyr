//! Trader gateway trait definitions.
//!
//! This module provides trait definitions for trading gateways
//! that connect to exchange trading APIs.
//!
//! # Architecture
//!
//! The trader system uses a callback-based architecture:
//! - [`TraderGateway`] - Main trait for trader implementations
//! - [`TraderCallback`] - Callback trait for receiving trade events
//! - [`Credentials`] - Authentication credentials for exchange APIs
//!
//! # Example
//!
//! ```ignore
//! use zephyr_core::traits::{TraderGateway, TraderCallback, Credentials};
//!
//! struct MyTrader { /* ... */ }
//!
//! #[async_trait::async_trait]
//! impl TraderGateway for MyTrader {
//!     async fn login(&mut self, credentials: &Credentials) -> Result<(), ExchangeError> {
//!         // Authenticate with exchange
//!         Ok(())
//!     }
//!     // ... other methods
//! }
//! ```

use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::data::{Account, Order, OrderRequest, OrderSide, Position};
use crate::error::ExchangeError;
use crate::types::{Amount, OrderId, Price, Quantity, Symbol, Timestamp};

/// Authentication credentials for exchange APIs.
///
/// Contains API key, secret, and optional additional authentication data.
///
/// # Security
///
/// The `api_secret` field is marked with `#[serde(skip)]` to prevent
/// accidental serialization of sensitive data.
///
/// # Examples
///
/// ```
/// use zephyr_core::traits::Credentials;
///
/// let creds = Credentials::new("my_api_key", "my_api_secret")
///     .with_passphrase("my_passphrase");
/// ```
#[derive(Debug, Clone)]
pub struct Credentials {
    /// API key for authentication.
    pub api_key: String,

    /// API secret for signing requests.
    /// This field is not serialized for security.
    api_secret: String,

    /// Optional passphrase (required by some exchanges like OKX).
    pub passphrase: Option<String>,

    /// Optional subaccount identifier.
    pub subaccount: Option<String>,

    /// Whether this is a testnet/sandbox account.
    pub testnet: bool,
}

impl Credentials {
    /// Creates new credentials with API key and secret.
    #[must_use]
    pub fn new(api_key: impl Into<String>, api_secret: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            api_secret: api_secret.into(),
            passphrase: None,
            subaccount: None,
            testnet: false,
        }
    }

    /// Sets the passphrase.
    #[must_use]
    pub fn with_passphrase(mut self, passphrase: impl Into<String>) -> Self {
        self.passphrase = Some(passphrase.into());
        self
    }

    /// Sets the subaccount.
    #[must_use]
    pub fn with_subaccount(mut self, subaccount: impl Into<String>) -> Self {
        self.subaccount = Some(subaccount.into());
        self
    }

    /// Sets whether this is a testnet account.
    #[must_use]
    pub fn with_testnet(mut self, testnet: bool) -> Self {
        self.testnet = testnet;
        self
    }

    /// Returns the API secret.
    ///
    /// # Security
    ///
    /// Use this method carefully. The secret should only be used
    /// for signing requests and should never be logged or serialized.
    #[must_use]
    pub fn api_secret(&self) -> &str {
        &self.api_secret
    }
}

/// Trade execution information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    /// Trade ID.
    pub trade_id: String,
    /// Order ID.
    pub order_id: OrderId,
    /// Trading pair symbol.
    pub symbol: Symbol,
    /// Trade side.
    pub side: OrderSide,
    /// Execution price.
    pub price: Price,
    /// Execution quantity.
    pub quantity: Quantity,
    /// Trade value (price × quantity).
    pub value: Amount,
    /// Trading fee.
    pub fee: Decimal,
    /// Fee currency.
    pub fee_currency: String,
    /// Trade timestamp.
    pub timestamp: Timestamp,
    /// Whether this trade was as maker.
    pub is_maker: bool,
}

/// Callback trait for receiving trade events.
///
/// Implementations receive callbacks when trading events occur.
/// All methods are async to support non-blocking processing.
#[async_trait]
pub trait TraderCallback: Send + Sync {
    /// Called when an order update is received.
    async fn on_order(&self, order: &Order);

    /// Called when a trade execution is received.
    async fn on_trade(&self, trade: &Trade);

    /// Called when a position update is received.
    async fn on_position(&self, position: &Position);

    /// Called when an account update is received.
    async fn on_account(&self, account: &Account);

    /// Called when the trader connects.
    async fn on_connected(&self);

    /// Called when the trader disconnects.
    async fn on_disconnected(&self, reason: Option<String>);

    /// Called when an error occurs.
    async fn on_error(&self, error: ExchangeError);
}

/// Trader gateway trait.
///
/// Defines the interface for trading gateways that connect to
/// exchange APIs for order management and account queries.
///
/// # Lifecycle
///
/// 1. Create trader instance
/// 2. Call `login()` with credentials
/// 3. Set callback with `set_callback()`
/// 4. Use `order_insert()`, `order_cancel()`, etc.
/// 5. Receive updates via callbacks
/// 6. Call `logout()` when done
#[async_trait]
pub trait TraderGateway: Send + Sync {
    /// Authenticates with the exchange.
    ///
    /// # Errors
    ///
    /// Returns `ExchangeError` if authentication fails.
    async fn login(&mut self, credentials: &Credentials) -> Result<(), ExchangeError>;

    /// Logs out from the exchange.
    ///
    /// # Errors
    ///
    /// Returns `ExchangeError` if logout fails.
    async fn logout(&mut self) -> Result<(), ExchangeError>;

    /// Returns whether the trader is currently logged in.
    fn is_logged_in(&self) -> bool;

    /// Submits a new order.
    ///
    /// # Returns
    ///
    /// Returns the exchange-assigned order ID on success.
    ///
    /// # Errors
    ///
    /// Returns `ExchangeError` if order submission fails.
    async fn order_insert(&self, order: &OrderRequest) -> Result<OrderId, ExchangeError>;

    /// Cancels an existing order.
    ///
    /// # Returns
    ///
    /// Returns `true` if the order was successfully canceled.
    ///
    /// # Errors
    ///
    /// Returns `ExchangeError` if cancellation fails.
    async fn order_cancel(&self, order_id: &OrderId) -> Result<bool, ExchangeError>;

    /// Cancels all orders for a symbol.
    ///
    /// # Returns
    ///
    /// Returns the number of orders canceled.
    ///
    /// # Errors
    ///
    /// Returns `ExchangeError` if cancellation fails.
    async fn cancel_all_orders(&self, symbol: Option<&Symbol>) -> Result<usize, ExchangeError>;

    /// Queries current positions.
    ///
    /// # Errors
    ///
    /// Returns `ExchangeError` if query fails.
    async fn query_positions(&self) -> Result<Vec<Position>, ExchangeError>;

    /// Queries open orders.
    ///
    /// # Arguments
    ///
    /// * `symbol` - Optional symbol filter
    ///
    /// # Errors
    ///
    /// Returns `ExchangeError` if query fails.
    async fn query_orders(&self, symbol: Option<&Symbol>) -> Result<Vec<Order>, ExchangeError>;

    /// Queries a specific order by ID.
    ///
    /// # Errors
    ///
    /// Returns `ExchangeError` if query fails.
    async fn query_order(&self, order_id: &OrderId) -> Result<Order, ExchangeError>;

    /// Queries account information.
    ///
    /// # Errors
    ///
    /// Returns `ExchangeError` if query fails.
    async fn query_account(&self) -> Result<Account, ExchangeError>;

    /// Sets the callback for receiving trade events.
    fn set_callback(&mut self, callback: Box<dyn TraderCallback>);

    /// Returns the exchange identifier.
    fn exchange(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_new() {
        let creds = Credentials::new("api_key", "api_secret");
        assert_eq!(creds.api_key, "api_key");
        assert_eq!(creds.api_secret(), "api_secret");
        assert!(creds.passphrase.is_none());
        assert!(!creds.testnet);
    }

    #[test]
    fn test_credentials_with_options() {
        let creds = Credentials::new("key", "secret")
            .with_passphrase("pass")
            .with_subaccount("sub1")
            .with_testnet(true);

        assert_eq!(creds.passphrase, Some("pass".to_string()));
        assert_eq!(creds.subaccount, Some("sub1".to_string()));
        assert!(creds.testnet);
    }
}
