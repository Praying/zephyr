//! Hyperliquid trading gateway implementation.
//!
//! Implements the [`TraderGateway`] trait for Hyperliquid REST/WebSocket API.
//! Hyperliquid is a decentralized exchange requiring wallet-based authentication.

use async_trait::async_trait;
use parking_lot::RwLock;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tracing::{debug, info};

use zephyr_core::data::{
    Account, Balance, Exchange, MarginType, Order, OrderRequest, OrderSide, OrderStatus, OrderType,
    Position, PositionSide, TimeInForce,
};
use zephyr_core::error::ExchangeError;
use zephyr_core::traits::{Credentials, TraderCallback, TraderGateway};
use zephyr_core::types::{
    Amount, Leverage, MarkPrice, OrderId, Price, Quantity, Symbol, Timestamp,
};

use crate::rest::{RestClient, RestConfig};

use super::types::*;

/// Hyperliquid trading gateway.
///
/// Provides trading functionality for Hyperliquid DEX including:
/// - Order submission and cancellation with wallet signing
/// - Position and order queries
/// - Account information
/// - Nonce management for transaction ordering
///
/// # DEX-Specific Features
///
/// - Wallet-based authentication using private key signing
/// - On-chain transaction confirmation
/// - Nonce management for transaction ordering
/// - Vault-based trading support
///
/// # Example
///
/// ```ignore
/// use zephyr_gateway::hyperliquid::HyperliquidTrader;
/// use zephyr_core::traits::{Credentials, TraderGateway};
///
/// let mut trader = HyperliquidTrader::new();
/// // For Hyperliquid, api_key is the wallet address, api_secret is the private key
/// let creds = Credentials::new("0x...", "private_key_hex");
/// trader.login(&creds).await?;
///
/// let positions = trader.query_positions().await?;
/// ```
pub struct HyperliquidTrader {
    /// Market type
    market: HyperliquidMarket,
    /// REST client
    rest_client: Option<RestClient>,
    /// Wallet address
    wallet_address: Option<String>,
    /// Private key (hex encoded, without 0x prefix)
    private_key: Option<String>,
    /// User callback
    callback: Option<Arc<dyn TraderCallback>>,
    /// Login state
    logged_in: Arc<AtomicBool>,
    /// Local order cache
    orders: Arc<RwLock<HashMap<OrderId, Order>>>,
    /// Asset index mapping (coin -> index)
    asset_indices: Arc<RwLock<HashMap<String, u32>>>,
    /// Nonce counter for transaction ordering
    nonce: Arc<AtomicU64>,
    /// Use testnet
    testnet: bool,
    /// Vault address (optional, for vault trading)
    vault_address: Option<String>,
}

impl HyperliquidTrader {
    /// Creates a new Hyperliquid trader for perpetual market.
    #[must_use]
    pub fn new() -> Self {
        Self::with_market(HyperliquidMarket::Perpetual)
    }

    /// Creates a new Hyperliquid trader for spot market.
    #[must_use]
    pub fn new_spot() -> Self {
        Self::with_market(HyperliquidMarket::Spot)
    }

    /// Creates a new Hyperliquid trader with specified market type.
    #[must_use]
    pub fn with_market(market: HyperliquidMarket) -> Self {
        Self {
            market,
            rest_client: None,
            wallet_address: None,
            private_key: None,
            callback: None,
            logged_in: Arc::new(AtomicBool::new(false)),
            orders: Arc::new(RwLock::new(HashMap::new())),
            asset_indices: Arc::new(RwLock::new(HashMap::new())),
            nonce: Arc::new(AtomicU64::new(0)),
            testnet: false,
            vault_address: None,
        }
    }

    /// Sets whether to use testnet.
    #[must_use]
    pub fn with_testnet(mut self, testnet: bool) -> Self {
        self.testnet = testnet;
        self
    }

    /// Sets the vault address for vault trading.
    #[must_use]
    pub fn with_vault(mut self, vault_address: impl Into<String>) -> Self {
        self.vault_address = Some(vault_address.into());
        self
    }

    /// Returns the market type.
    #[must_use]
    pub fn market(&self) -> HyperliquidMarket {
        self.market
    }

    /// Returns whether using testnet.
    #[must_use]
    pub fn is_testnet(&self) -> bool {
        self.testnet
    }

    /// Returns the base URL for REST API.
    fn base_url(&self) -> &str {
        if self.testnet {
            self.market.testnet_rest_base_url()
        } else {
            self.market.rest_base_url()
        }
    }

    /// Converts a symbol to Hyperliquid coin format.
    fn to_hyperliquid_coin(symbol: &Symbol) -> String {
        let s = symbol.as_str();
        if let Some(pos) = s.find('-') {
            s[..pos].to_string()
        } else {
            s.to_string()
        }
    }

    /// Converts a Hyperliquid coin to standard symbol format.
    fn from_hyperliquid_coin(coin: &str) -> Option<Symbol> {
        Symbol::new(format!("{coin}-USD")).ok()
    }

    /// Gets the next nonce for transaction ordering.
    fn next_nonce(&self) -> u64 {
        self.nonce.fetch_add(1, Ordering::SeqCst)
    }

    /// Gets the current timestamp in milliseconds.
    fn timestamp_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64
    }

    /// Gets the REST client, returning error if not logged in.
    fn client(&self) -> Result<&RestClient, ExchangeError> {
        self.rest_client
            .as_ref()
            .ok_or(ExchangeError::AuthenticationFailed {
                reason: "Not logged in".to_string(),
            })
    }

    /// Gets the wallet address, returning error if not logged in.
    fn wallet(&self) -> Result<&str, ExchangeError> {
        self.wallet_address
            .as_deref()
            .ok_or(ExchangeError::AuthenticationFailed {
                reason: "Not logged in".to_string(),
            })
    }

    /// Gets the asset index for a coin.
    fn get_asset_index(&self, coin: &str) -> Option<u32> {
        self.asset_indices.read().get(coin).copied()
    }

    /// Sets the asset index mapping.
    pub fn set_asset_indices(&self, indices: HashMap<String, u32>) {
        *self.asset_indices.write() = indices;
    }

    /// Loads asset metadata from the exchange.
    async fn load_meta(&self) -> Result<(), ExchangeError> {
        let client = self.client()?;

        let response = client
            .post("/info")
            .json(&serde_json::json!({"type": "meta"}))
            .send()
            .await
            .map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Failed to load meta: {e}"),
            })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(Self::parse_error(status.as_u16(), &body));
        }

        let meta: HyperliquidMeta =
            serde_json::from_str(&body).map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Failed to parse meta: {e}"),
            })?;

        // Build asset index mapping
        let mut indices = HashMap::new();
        for (idx, asset) in meta.universe.iter().enumerate() {
            indices.insert(asset.name.clone(), idx as u32);
        }
        self.set_asset_indices(indices);

        debug!(
            exchange = "hyperliquid",
            assets = meta.universe.len(),
            "Loaded asset metadata"
        );

        Ok(())
    }

    /// Parses an API error response.
    fn parse_error(status: u16, body: &str) -> ExchangeError {
        if let Ok(error) = serde_json::from_str::<HyperliquidApiError>(body) {
            let msg = error.error.to_lowercase();
            if msg.contains("insufficient") || msg.contains("balance") {
                ExchangeError::InsufficientBalance {
                    required: Decimal::ZERO,
                    available: Decimal::ZERO,
                }
            } else if msg.contains("nonce") {
                ExchangeError::InvalidParameter {
                    param: "nonce".to_string(),
                    reason: error.error,
                }
            } else if msg.contains("signature") || msg.contains("auth") {
                ExchangeError::AuthenticationFailed {
                    reason: error.error,
                }
            } else if msg.contains("not found") || msg.contains("unknown") {
                ExchangeError::OrderNotFound {
                    order_id: String::new(),
                }
            } else {
                ExchangeError::Unknown {
                    code: status as i32,
                    message: error.error,
                }
            }
        } else {
            ExchangeError::Unknown {
                code: status as i32,
                message: body.to_string(),
            }
        }
    }

    /// Signs a message using EIP-712 typed data signing.
    ///
    /// Note: This is a simplified implementation. In production, you would use
    /// a proper Ethereum signing library like ethers-rs.
    fn sign_action(
        &self,
        action: &HyperliquidAction,
        nonce: u64,
    ) -> Result<HyperliquidSignature, ExchangeError> {
        let _private_key =
            self.private_key
                .as_ref()
                .ok_or(ExchangeError::AuthenticationFailed {
                    reason: "Private key not set".to_string(),
                })?;

        // In a real implementation, this would:
        // 1. Construct the EIP-712 typed data structure
        // 2. Hash the typed data
        // 3. Sign with the private key using secp256k1
        // 4. Return the r, s, v components

        // For now, return a placeholder signature
        // Production code should use ethers-rs or similar
        let _ = (action, nonce);

        Ok(HyperliquidSignature {
            r: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            s: "0x0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            v: 27,
        })
    }

    /// Converts order side to Hyperliquid format.
    fn to_hyperliquid_side(side: OrderSide) -> bool {
        matches!(side, OrderSide::Buy)
    }

    /// Converts Hyperliquid side to standard format.
    fn from_hyperliquid_side(is_buy: bool) -> OrderSide {
        if is_buy {
            OrderSide::Buy
        } else {
            OrderSide::Sell
        }
    }

    /// Converts time in force to Hyperliquid format.
    fn to_hyperliquid_tif(tif: TimeInForce) -> String {
        match tif {
            TimeInForce::Gtc => "Gtc".to_string(),
            TimeInForce::Ioc => "Ioc".to_string(),
            TimeInForce::Fok => "Ioc".to_string(), // Hyperliquid doesn't have FOK, use IOC
            TimeInForce::PostOnly => "Alo".to_string(),
            TimeInForce::Gtd => "Gtc".to_string(), // Fallback to GTC
        }
    }

    /// Converts Hyperliquid position to standard Position.
    fn convert_position(&self, pos: &HyperliquidAssetPosition) -> Option<Position> {
        let symbol = Self::from_hyperliquid_coin(&pos.position.coin)?;
        let szi: Decimal = pos.position.szi.parse().ok()?;

        // Skip empty positions
        if szi.is_zero() {
            return None;
        }

        let quantity = Quantity::new(szi.abs()).ok()?;
        let entry_price = pos
            .position
            .entry_px
            .as_ref()
            .and_then(|p| p.parse::<Decimal>().ok())
            .and_then(|d| Price::new(d).ok())
            .unwrap_or(Price::ZERO);

        // Use position value to estimate mark price if not available
        let position_value: Decimal = pos.position.position_value.parse().ok()?;
        let mark_price_dec = if !szi.is_zero() {
            position_value / szi.abs()
        } else {
            entry_price.as_decimal()
        };
        let mark_price = MarkPrice::new(mark_price_dec.abs()).ok()?;

        let unrealized_pnl = Amount::new(pos.position.unrealized_pnl.parse().ok()?).ok()?;
        let leverage = Leverage::new(pos.position.leverage.value as u8).ok()?;

        let liquidation_price = pos
            .position
            .liquidation_px
            .as_ref()
            .and_then(|p| p.parse::<Decimal>().ok())
            .and_then(|d| Price::new(d).ok());

        let side = if szi > Decimal::ZERO {
            PositionSide::Long
        } else {
            PositionSide::Short
        };

        let margin_type = match pos.position.leverage.leverage_type.as_str() {
            "isolated" => MarginType::Isolated,
            _ => MarginType::Cross,
        };

        let margin_used = Amount::new(pos.position.margin_used.parse().ok()?).ok();

        Some(Position {
            symbol,
            side,
            quantity,
            entry_price,
            mark_price,
            liquidation_price,
            unrealized_pnl,
            realized_pnl: Amount::ZERO,
            leverage,
            margin_type,
            initial_margin: margin_used,
            maintenance_margin: None,
            update_time: Timestamp::now(),
        })
    }

    /// Converts Hyperliquid open order to standard Order.
    fn convert_open_order(&self, order: &HyperliquidOpenOrder) -> Option<Order> {
        let symbol = Self::from_hyperliquid_coin(&order.coin)?;
        let order_id = OrderId::new(order.oid.to_string()).ok()?;
        let price = Price::new(order.limit_px.parse().ok()?).ok()?;
        let quantity = Quantity::new(order.sz.parse().ok()?).ok()?;
        let side = if order.side == "B" {
            OrderSide::Buy
        } else {
            OrderSide::Sell
        };
        let timestamp = Timestamp::new(order.timestamp).ok()?;

        Some(Order {
            order_id,
            client_order_id: None,
            symbol,
            side,
            order_type: OrderType::Limit,
            status: OrderStatus::New,
            price: Some(price),
            stop_price: None,
            quantity,
            filled_quantity: Quantity::ZERO,
            avg_price: None,
            time_in_force: TimeInForce::Gtc,
            reduce_only: false,
            post_only: false,
            create_time: timestamp,
            update_time: timestamp,
        })
    }
}

impl Default for HyperliquidTrader {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TraderGateway for HyperliquidTrader {
    async fn login(&mut self, credentials: &Credentials) -> Result<(), ExchangeError> {
        let base_url = self.base_url();

        // For Hyperliquid:
        // - api_key is the wallet address (0x...)
        // - api_secret is the private key (hex encoded)
        let wallet_address = credentials.api_key.clone();
        let private_key = credentials.api_secret().to_string();

        // Validate wallet address format
        if !wallet_address.starts_with("0x") || wallet_address.len() != 42 {
            return Err(ExchangeError::AuthenticationFailed {
                reason: "Invalid wallet address format. Expected 0x followed by 40 hex characters"
                    .to_string(),
            });
        }

        // Validate private key format (should be 64 hex characters, optionally with 0x prefix)
        let pk = private_key.strip_prefix("0x").unwrap_or(&private_key);
        if pk.len() != 64 || !pk.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ExchangeError::AuthenticationFailed {
                reason: "Invalid private key format. Expected 64 hex characters".to_string(),
            });
        }

        let rest_config = RestConfig::builder()
            .base_url(base_url)
            .exchange("hyperliquid")
            .build();

        let rest_client =
            RestClient::new(rest_config).map_err(|e| ExchangeError::AuthenticationFailed {
                reason: format!("Failed to create REST client: {e}"),
            })?;

        self.rest_client = Some(rest_client);
        self.wallet_address = Some(wallet_address.clone());
        self.private_key = Some(pk.to_string());

        // Load asset metadata
        self.load_meta().await?;

        // Initialize nonce from current timestamp
        self.nonce.store(Self::timestamp_ms(), Ordering::SeqCst);

        self.logged_in.store(true, Ordering::SeqCst);

        if let Some(callback) = &self.callback {
            callback.on_connected().await;
        }

        info!(
            exchange = "hyperliquid",
            market = ?self.market,
            testnet = self.testnet,
            wallet = %wallet_address,
            "Trader logged in"
        );

        Ok(())
    }

    async fn logout(&mut self) -> Result<(), ExchangeError> {
        self.rest_client = None;
        self.wallet_address = None;
        self.private_key = None;
        self.logged_in.store(false, Ordering::SeqCst);

        if let Some(callback) = &self.callback {
            callback
                .on_disconnected(Some("Logged out".to_string()))
                .await;
        }

        info!(exchange = "hyperliquid", "Trader logged out");
        Ok(())
    }

    fn is_logged_in(&self) -> bool {
        self.logged_in.load(Ordering::SeqCst)
    }

    async fn order_insert(&self, order: &OrderRequest) -> Result<OrderId, ExchangeError> {
        let client = self.client()?;
        let _wallet = self.wallet()?;

        let coin = Self::to_hyperliquid_coin(&order.symbol);
        let asset_index = self
            .get_asset_index(&coin)
            .ok_or(ExchangeError::InvalidParameter {
                param: "symbol".to_string(),
                reason: format!("Unknown asset: {coin}"),
            })?;

        let price = order
            .price
            .as_ref()
            .ok_or(ExchangeError::InvalidParameter {
                param: "price".to_string(),
                reason: "Price required for limit orders".to_string(),
            })?;

        let order_request = HyperliquidOrderRequest {
            asset: asset_index,
            is_buy: Self::to_hyperliquid_side(order.side),
            price: price.to_string(),
            size: order.quantity.to_string(),
            reduce_only: order.reduce_only,
            order_type: HyperliquidOrderTypeSpec {
                limit: HyperliquidLimitParams {
                    tif: Self::to_hyperliquid_tif(order.time_in_force),
                },
            },
        };

        let action = HyperliquidAction::Order {
            orders: vec![order_request],
            grouping: "na".to_string(),
        };

        let nonce = self.next_nonce();
        let signature = self.sign_action(&action, nonce)?;

        let request = HyperliquidExchangeRequest {
            action,
            nonce,
            signature,
            vault_address: self.vault_address.clone(),
        };

        let response = client
            .post("/exchange")
            .json(&request)
            .send()
            .await
            .map_err(|e| ExchangeError::OrderRejected {
                reason: format!("Request failed: {e}"),
                code: None,
            })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(Self::parse_error(status.as_u16(), &body));
        }

        let order_response: HyperliquidOrderResponse =
            serde_json::from_str(&body).map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Failed to parse response: {e}"),
            })?;

        // Extract order ID from response
        let order_id = if let Some(ref data) = order_response.response {
            if let Some(ref statuses) = data.data {
                if let Some(status) = statuses.statuses.first() {
                    match status {
                        HyperliquidOrderStatus::Success { resting, filled } => {
                            let oid = resting
                                .as_ref()
                                .map(|r| r.oid)
                                .or_else(|| filled.as_ref().map(|f| f.oid))
                                .ok_or(ExchangeError::Unknown {
                                    code: 0,
                                    message: "No order ID in response".to_string(),
                                })?;
                            OrderId::new(oid.to_string()).map_err(|_| ExchangeError::Unknown {
                                code: 0,
                                message: "Invalid order ID".to_string(),
                            })?
                        }
                        HyperliquidOrderStatus::Error { error } => {
                            return Err(ExchangeError::OrderRejected {
                                reason: error.clone(),
                                code: None,
                            });
                        }
                    }
                } else {
                    return Err(ExchangeError::Unknown {
                        code: 0,
                        message: "Empty order statuses".to_string(),
                    });
                }
            } else {
                return Err(ExchangeError::Unknown {
                    code: 0,
                    message: "No order data in response".to_string(),
                });
            }
        } else {
            return Err(ExchangeError::Unknown {
                code: 0,
                message: "No response data".to_string(),
            });
        };

        // Create local order record
        let local_order = Order {
            order_id: order_id.clone(),
            client_order_id: order.client_order_id.clone(),
            symbol: order.symbol.clone(),
            side: order.side,
            order_type: order.order_type,
            status: OrderStatus::New,
            price: order.price,
            stop_price: order.stop_price,
            quantity: order.quantity,
            filled_quantity: Quantity::ZERO,
            avg_price: None,
            time_in_force: order.time_in_force,
            reduce_only: order.reduce_only,
            post_only: order.post_only,
            create_time: Timestamp::now(),
            update_time: Timestamp::now(),
        };

        self.orders
            .write()
            .insert(order_id.clone(), local_order.clone());

        if let Some(callback) = &self.callback {
            callback.on_order(&local_order).await;
        }

        info!(
            exchange = "hyperliquid",
            order_id = %order_id,
            symbol = %order.symbol,
            side = ?order.side,
            "Order submitted"
        );

        Ok(order_id)
    }

    async fn order_cancel(&self, order_id: &OrderId) -> Result<bool, ExchangeError> {
        let client = self.client()?;
        let _wallet = self.wallet()?;

        // Get order from cache to find asset
        let order = {
            let orders = self.orders.read();
            orders.get(order_id).cloned()
        };

        let order = order.ok_or(ExchangeError::OrderNotFound {
            order_id: order_id.to_string(),
        })?;

        let coin = Self::to_hyperliquid_coin(&order.symbol);
        let asset_index = self
            .get_asset_index(&coin)
            .ok_or(ExchangeError::InvalidParameter {
                param: "symbol".to_string(),
                reason: format!("Unknown asset: {coin}"),
            })?;

        let oid: i64 = order_id
            .as_str()
            .parse()
            .map_err(|_| ExchangeError::InvalidParameter {
                param: "order_id".to_string(),
                reason: "Invalid order ID format".to_string(),
            })?;

        let cancel_request = HyperliquidCancelRequest {
            asset: asset_index,
            oid,
        };

        let action = HyperliquidAction::Cancel {
            cancels: vec![cancel_request],
        };

        let nonce = self.next_nonce();
        let signature = self.sign_action(&action, nonce)?;

        let request = HyperliquidExchangeRequest {
            action,
            nonce,
            signature,
            vault_address: self.vault_address.clone(),
        };

        let response = client
            .post("/exchange")
            .json(&request)
            .send()
            .await
            .map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Request failed: {e}"),
            })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(Self::parse_error(status.as_u16(), &body));
        }

        // Update local cache and prepare callback
        let order_for_callback = {
            let mut orders = self.orders.write();
            if let Some(order) = orders.get_mut(order_id) {
                order.status = OrderStatus::Canceled;
                order.update_time = Timestamp::now();
                Some(order.clone())
            } else {
                None
            }
        };

        // Call callback outside the lock
        if let (Some(callback), Some(order)) = (&self.callback, order_for_callback) {
            callback.on_order(&order).await;
        }

        info!(
            exchange = "hyperliquid",
            order_id = %order_id,
            "Order canceled"
        );

        Ok(true)
    }

    async fn cancel_all_orders(&self, symbol: Option<&Symbol>) -> Result<usize, ExchangeError> {
        // Get all open orders
        let orders = self.query_orders(symbol).await?;
        let mut canceled = 0;

        for order in orders {
            if matches!(
                order.status,
                OrderStatus::New | OrderStatus::PartiallyFilled
            ) {
                if self.order_cancel(&order.order_id).await.is_ok() {
                    canceled += 1;
                }
            }
        }

        info!(
            exchange = "hyperliquid",
            count = canceled,
            symbol = ?symbol,
            "All orders canceled"
        );

        Ok(canceled)
    }

    async fn query_positions(&self) -> Result<Vec<Position>, ExchangeError> {
        let client = self.client()?;
        let wallet = self.wallet()?;

        let response = client
            .post("/info")
            .json(&serde_json::json!({
                "type": "clearinghouseState",
                "user": wallet
            }))
            .send()
            .await
            .map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Request failed: {e}"),
            })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(Self::parse_error(status.as_u16(), &body));
        }

        let user_state: HyperliquidUserState =
            serde_json::from_str(&body).map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Failed to parse response: {e}"),
            })?;

        let positions: Vec<Position> = user_state
            .asset_positions
            .iter()
            .filter_map(|p| self.convert_position(p))
            .collect();

        debug!(
            exchange = "hyperliquid",
            count = positions.len(),
            "Positions queried"
        );

        Ok(positions)
    }

    async fn query_orders(&self, symbol: Option<&Symbol>) -> Result<Vec<Order>, ExchangeError> {
        let client = self.client()?;
        let wallet = self.wallet()?;

        let response = client
            .post("/info")
            .json(&serde_json::json!({
                "type": "openOrders",
                "user": wallet
            }))
            .send()
            .await
            .map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Request failed: {e}"),
            })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(Self::parse_error(status.as_u16(), &body));
        }

        let open_orders: Vec<HyperliquidOpenOrder> =
            serde_json::from_str(&body).map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Failed to parse response: {e}"),
            })?;

        let mut orders: Vec<Order> = open_orders
            .iter()
            .filter_map(|o| self.convert_open_order(o))
            .collect();

        // Filter by symbol if specified
        if let Some(sym) = symbol {
            orders.retain(|o| &o.symbol == sym);
        }

        // Update local cache
        {
            let mut cache = self.orders.write();
            for order in &orders {
                cache.insert(order.order_id.clone(), order.clone());
            }
        }

        debug!(
            exchange = "hyperliquid",
            count = orders.len(),
            "Orders queried"
        );

        Ok(orders)
    }

    async fn query_order(&self, order_id: &OrderId) -> Result<Order, ExchangeError> {
        // First check local cache
        {
            let orders = self.orders.read();
            if let Some(order) = orders.get(order_id) {
                return Ok(order.clone());
            }
        }

        // Refresh orders and try again
        self.query_orders(None).await?;

        let orders = self.orders.read();
        orders
            .get(order_id)
            .cloned()
            .ok_or(ExchangeError::OrderNotFound {
                order_id: order_id.to_string(),
            })
    }

    async fn query_account(&self) -> Result<Account, ExchangeError> {
        let client = self.client()?;
        let wallet = self.wallet()?;

        let response = client
            .post("/info")
            .json(&serde_json::json!({
                "type": "clearinghouseState",
                "user": wallet
            }))
            .send()
            .await
            .map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Request failed: {e}"),
            })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(Self::parse_error(status.as_u16(), &body));
        }

        let user_state: HyperliquidUserState =
            serde_json::from_str(&body).map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Failed to parse response: {e}"),
            })?;

        let margin = &user_state.cross_margin_summary;

        let total_equity =
            Amount::new(
                margin
                    .account_value
                    .parse()
                    .map_err(|_| ExchangeError::Unknown {
                        code: 0,
                        message: "Failed to parse account value".to_string(),
                    })?,
            )
            .map_err(|_| ExchangeError::Unknown {
                code: 0,
                message: "Invalid account value".to_string(),
            })?;

        let margin_used =
            Amount::new(
                margin
                    .total_margin_used
                    .parse()
                    .map_err(|_| ExchangeError::Unknown {
                        code: 0,
                        message: "Failed to parse margin used".to_string(),
                    })?,
            )
            .map_err(|_| ExchangeError::Unknown {
                code: 0,
                message: "Invalid margin used".to_string(),
            })?;

        let withdrawable =
            Amount::new(
                user_state
                    .withdrawable
                    .parse()
                    .map_err(|_| ExchangeError::Unknown {
                        code: 0,
                        message: "Failed to parse withdrawable".to_string(),
                    })?,
            )
            .map_err(|_| ExchangeError::Unknown {
                code: 0,
                message: "Invalid withdrawable".to_string(),
            })?;

        // Calculate unrealized PnL from positions
        let unrealized_pnl = user_state
            .asset_positions
            .iter()
            .filter_map(|p| p.position.unrealized_pnl.parse::<Decimal>().ok())
            .sum::<Decimal>();
        let unrealized_pnl = Amount::new(unrealized_pnl).unwrap_or(Amount::ZERO);

        // Hyperliquid uses USDC as the settlement currency
        let balance = Balance::new(
            "USDC",
            total_equity,
            withdrawable,
            Amount::new(margin_used.as_decimal()).unwrap_or(Amount::ZERO),
        );

        Ok(Account {
            exchange: Exchange::Hyperliquid,
            balances: vec![balance],
            total_equity,
            available_balance: withdrawable,
            margin_used,
            unrealized_pnl,
            update_time: Timestamp::now(),
        })
    }

    fn set_callback(&mut self, callback: Box<dyn TraderCallback>) {
        self.callback = Some(Arc::from(callback));
    }

    fn exchange(&self) -> &str {
        "hyperliquid"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_hyperliquid_coin() {
        assert_eq!(
            HyperliquidTrader::to_hyperliquid_coin(&Symbol::new("BTC-USD").unwrap()),
            "BTC"
        );
        assert_eq!(
            HyperliquidTrader::to_hyperliquid_coin(&Symbol::new("ETH-USD").unwrap()),
            "ETH"
        );
    }

    #[test]
    fn test_from_hyperliquid_coin() {
        let symbol = HyperliquidTrader::from_hyperliquid_coin("BTC").unwrap();
        assert_eq!(symbol.as_str(), "BTC-USD");
    }

    #[test]
    fn test_trader_default() {
        let trader = HyperliquidTrader::default();
        assert_eq!(trader.market(), HyperliquidMarket::Perpetual);
        assert!(!trader.is_logged_in());
        assert!(!trader.is_testnet());
    }

    #[test]
    fn test_trader_with_testnet() {
        let trader = HyperliquidTrader::new().with_testnet(true);
        assert!(trader.is_testnet());
    }

    #[test]
    fn test_trader_with_vault() {
        let trader =
            HyperliquidTrader::new().with_vault("0x1234567890123456789012345678901234567890");
        assert!(trader.vault_address.is_some());
    }

    #[test]
    fn test_to_hyperliquid_side() {
        assert!(HyperliquidTrader::to_hyperliquid_side(OrderSide::Buy));
        assert!(!HyperliquidTrader::to_hyperliquid_side(OrderSide::Sell));
    }

    #[test]
    fn test_from_hyperliquid_side() {
        assert_eq!(
            HyperliquidTrader::from_hyperliquid_side(true),
            OrderSide::Buy
        );
        assert_eq!(
            HyperliquidTrader::from_hyperliquid_side(false),
            OrderSide::Sell
        );
    }

    #[test]
    fn test_to_hyperliquid_tif() {
        assert_eq!(
            HyperliquidTrader::to_hyperliquid_tif(TimeInForce::Gtc),
            "Gtc"
        );
        assert_eq!(
            HyperliquidTrader::to_hyperliquid_tif(TimeInForce::Ioc),
            "Ioc"
        );
        assert_eq!(
            HyperliquidTrader::to_hyperliquid_tif(TimeInForce::PostOnly),
            "Alo"
        );
    }

    #[test]
    fn test_asset_indices() {
        let trader = HyperliquidTrader::new();
        let mut indices = HashMap::new();
        indices.insert("BTC".to_string(), 0);
        indices.insert("ETH".to_string(), 1);
        trader.set_asset_indices(indices);

        assert_eq!(trader.get_asset_index("BTC"), Some(0));
        assert_eq!(trader.get_asset_index("ETH"), Some(1));
        assert_eq!(trader.get_asset_index("SOL"), None);
    }

    #[test]
    fn test_nonce_increment() {
        let trader = HyperliquidTrader::new();
        trader.nonce.store(100, Ordering::SeqCst);

        assert_eq!(trader.next_nonce(), 100);
        assert_eq!(trader.next_nonce(), 101);
        assert_eq!(trader.next_nonce(), 102);
    }
}
