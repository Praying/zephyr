//! OKX trading gateway implementation.
//!
//! Implements the [`TraderGateway`] trait for OKX REST/WebSocket API.

#![allow(clippy::too_many_lines)]
#![allow(clippy::or_fun_call)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::filter_map_next)]
#![allow(clippy::unnecessary_literal_bound)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::disallowed_types)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::unused_self)]

use async_trait::async_trait;
use base64::Engine;
use hmac::{Hmac, Mac};
use parking_lot::RwLock;
use rust_decimal::Decimal;
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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

/// OKX trading gateway.
///
/// Provides trading functionality for OKX exchange including:
/// - Order submission and cancellation
/// - Position and order queries
/// - Account information
///
/// # OKX-Specific Features
///
/// - Requires passphrase in addition to API key and secret
/// - Uses different trade modes: cash (spot), cross, isolated
/// - Supports position side for hedge mode
///
/// # Example
///
/// ```ignore
/// use zephyr_gateway::okx::OkxTrader;
/// use zephyr_core::traits::{Credentials, TraderGateway};
///
/// let mut trader = OkxTrader::new_swap();
/// let creds = Credentials::new("api_key", "api_secret")
///     .with_passphrase("passphrase");
/// trader.login(&creds).await?;
///
/// let positions = trader.query_positions().await?;
/// ```
pub struct OkxTrader {
    /// Market type
    market: OkxMarket,
    /// REST client
    rest_client: Option<RestClient>,
    /// Credentials
    credentials: Option<Credentials>,
    /// User callback
    callback: Option<Arc<dyn TraderCallback>>,
    /// Login state
    logged_in: Arc<AtomicBool>,
    /// Local order cache
    orders: Arc<RwLock<HashMap<OrderId, Order>>>,
    /// Use testnet (demo trading)
    testnet: bool,
    /// Simulated trading mode
    simulated: bool,
}

impl OkxTrader {
    /// Creates a new OKX trader for spot market.
    #[must_use]
    pub fn new() -> Self {
        Self::with_market(OkxMarket::Spot)
    }

    /// Creates a new OKX trader for perpetual swaps.
    #[must_use]
    pub fn new_swap() -> Self {
        Self::with_market(OkxMarket::Swap)
    }

    /// Creates a new OKX trader with specified market type.
    #[must_use]
    pub fn with_market(market: OkxMarket) -> Self {
        Self {
            market,
            rest_client: None,
            credentials: None,
            callback: None,
            logged_in: Arc::new(AtomicBool::new(false)),
            orders: Arc::new(RwLock::new(HashMap::new())),
            testnet: false,
            simulated: false,
        }
    }

    /// Sets whether to use testnet (demo trading).
    #[must_use]
    pub fn with_testnet(mut self, testnet: bool) -> Self {
        self.testnet = testnet;
        self
    }

    /// Sets whether to use simulated trading mode.
    /// When enabled, adds `x-simulated-trading: 1` header.
    #[must_use]
    pub fn with_simulated(mut self, simulated: bool) -> Self {
        self.simulated = simulated;
        self
    }

    /// Returns the market type.
    #[must_use]
    pub fn market(&self) -> OkxMarket {
        self.market
    }

    /// Returns the base URL for REST API.
    fn base_url(&self) -> &str {
        if self.testnet {
            self.market.testnet_rest_base_url()
        } else {
            self.market.rest_base_url()
        }
    }

    /// Converts a symbol to OKX format.
    /// OKX uses hyphenated format natively (e.g., "BTC-USDT").
    fn to_okx_symbol(symbol: &Symbol) -> String {
        symbol.as_str().to_uppercase()
    }

    /// Converts an OKX symbol to standard format.
    fn from_okx_symbol(okx_symbol: &str) -> Option<Symbol> {
        Symbol::new(okx_symbol.to_uppercase()).ok()
    }

    /// Converts order side to OKX format.
    fn to_okx_side(side: OrderSide) -> OkxOrderSide {
        match side {
            OrderSide::Buy => OkxOrderSide::Buy,
            OrderSide::Sell => OkxOrderSide::Sell,
        }
    }

    /// Converts OKX order side to standard format.
    fn from_okx_side(side: OkxOrderSide) -> OrderSide {
        match side {
            OkxOrderSide::Buy => OrderSide::Buy,
            OkxOrderSide::Sell => OrderSide::Sell,
        }
    }

    /// Converts order type to OKX format.
    fn to_okx_order_type(order_type: OrderType, tif: TimeInForce) -> OkxOrderType {
        match (order_type, tif) {
            (OrderType::Market, _) => OkxOrderType::Market,
            (OrderType::Limit, TimeInForce::PostOnly) => OkxOrderType::PostOnly,
            (OrderType::Limit, TimeInForce::Fok) => OkxOrderType::Fok,
            (OrderType::Limit, TimeInForce::Ioc) => OkxOrderType::Ioc,
            (OrderType::Limit, _) => OkxOrderType::Limit,
            _ => OkxOrderType::Limit,
        }
    }

    /// Converts OKX order type to standard format.
    fn from_okx_order_type(order_type: OkxOrderType) -> (OrderType, TimeInForce) {
        match order_type {
            OkxOrderType::Market => (OrderType::Market, TimeInForce::Gtc),
            OkxOrderType::Limit => (OrderType::Limit, TimeInForce::Gtc),
            OkxOrderType::PostOnly => (OrderType::Limit, TimeInForce::PostOnly),
            OkxOrderType::Fok => (OrderType::Limit, TimeInForce::Fok),
            OkxOrderType::Ioc => (OrderType::Limit, TimeInForce::Ioc),
            OkxOrderType::OptimalLimitIoc => (OrderType::Market, TimeInForce::Ioc),
        }
    }

    /// Converts OKX order status to standard format.
    fn from_okx_status(status: OkxOrderStatus) -> OrderStatus {
        match status {
            OkxOrderStatus::Live => OrderStatus::New,
            OkxOrderStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
            OkxOrderStatus::Filled => OrderStatus::Filled,
            OkxOrderStatus::Canceled | OkxOrderStatus::MmpCanceled => OrderStatus::Canceled,
        }
    }

    /// Returns the trade mode for the current market.
    fn trade_mode(&self) -> OkxTradeMode {
        match self.market {
            OkxMarket::Spot => OkxTradeMode::Cash,
            _ => OkxTradeMode::Cross, // Default to cross margin for derivatives
        }
    }

    /// Returns the current timestamp in ISO 8601 format.
    fn timestamp_iso() -> String {
        chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string()
    }

    /// Returns the current timestamp in milliseconds.
    fn timestamp_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64
    }

    /// Signs a request using HMAC-SHA256.
    fn sign_request(timestamp: &str, method: &str, path: &str, body: &str, secret: &str) -> String {
        let prehash = format!("{timestamp}{method}{path}{body}");

        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(prehash.as_bytes());

        base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
    }

    /// Gets the REST client, returning error if not logged in.
    fn client(&self) -> Result<&RestClient, ExchangeError> {
        self.rest_client
            .as_ref()
            .ok_or(ExchangeError::AuthenticationFailed {
                reason: "Not logged in".to_string(),
            })
    }

    /// Parses an OKX API error response.
    fn parse_error(code: &str, msg: &str) -> ExchangeError {
        match code {
            "50000" | "50001" | "50002" => ExchangeError::Unknown {
                code: code.parse().unwrap_or(0),
                message: msg.to_string(),
            },
            "50100" | "50101" | "50102" | "50103" | "50104" | "50105" => {
                ExchangeError::AuthenticationFailed {
                    reason: msg.to_string(),
                }
            }
            "50011" => ExchangeError::RateLimited {
                retry_after_ms: 1000,
            },
            "51000" | "51001" | "51002" | "51003" | "51004" | "51005" | "51006" => {
                ExchangeError::InvalidParameter {
                    param: "order".to_string(),
                    reason: msg.to_string(),
                }
            }
            "51008" => ExchangeError::InsufficientBalance {
                required: Decimal::ZERO,
                available: Decimal::ZERO,
            },
            "51400" | "51401" | "51402" | "51403" => ExchangeError::OrderNotFound {
                order_id: String::new(),
            },
            "51509" => ExchangeError::OrderRejected {
                reason: msg.to_string(),
                code: Some(code.parse().unwrap_or(0)),
            },
            _ => ExchangeError::Unknown {
                code: code.parse().unwrap_or(0),
                message: msg.to_string(),
            },
        }
    }

    /// Converts OKX order details to standard Order.
    fn convert_order_details(&self, details: &OkxOrderDetails) -> Option<Order> {
        let symbol = Self::from_okx_symbol(&details.inst_id)?;
        let order_id = OrderId::new(&details.ord_id).ok()?;
        let status = Self::from_okx_status(details.state);
        let side = Self::from_okx_side(details.side);
        let (order_type, tif) = Self::from_okx_order_type(details.ord_type);

        let price = if details.px.is_empty() || details.px == "0" {
            None
        } else {
            details
                .px
                .parse::<Decimal>()
                .ok()
                .and_then(|d| Price::new(d).ok())
        };

        let quantity = Quantity::new(details.sz.parse().ok()?).ok()?;
        let filled_quantity = Quantity::new(details.acc_fill_sz.parse().ok()?).ok()?;

        let avg_price = details.avg_px.as_ref().and_then(|p| {
            if p.is_empty() || p == "0" {
                None
            } else {
                p.parse::<Decimal>().ok().and_then(|d| Price::new(d).ok())
            }
        });

        let create_time = Timestamp::new(details.c_time.parse().ok()?).ok()?;
        let update_time = Timestamp::new(details.u_time.parse().ok()?).ok()?;

        let reduce_only = details.reduce_only.as_deref() == Some("true");

        Some(Order {
            order_id,
            client_order_id: Some(details.cl_ord_id.clone()),
            symbol,
            side,
            order_type,
            status,
            price,
            stop_price: None,
            quantity,
            filled_quantity,
            avg_price,
            time_in_force: tif,
            reduce_only,
            post_only: matches!(details.ord_type, OkxOrderType::PostOnly),
            create_time,
            update_time,
        })
    }

    /// Converts OKX position to standard Position.
    fn convert_position(&self, pos: &OkxPosition) -> Option<Position> {
        let symbol = Self::from_okx_symbol(&pos.inst_id)?;
        let quantity_val: Decimal = pos.pos.parse().ok()?;

        // Skip empty positions
        if quantity_val == Decimal::ZERO {
            return None;
        }

        let quantity = Quantity::new(quantity_val.abs()).ok()?;
        let entry_price = Price::new(pos.avg_px.parse().ok()?).ok()?;

        let mark_price = pos
            .mark_px
            .as_ref()
            .and_then(|p| p.parse::<Decimal>().ok())
            .and_then(|d| MarkPrice::new(d).ok())
            .unwrap_or(MarkPrice::ZERO);

        let unrealized_pnl = Amount::new(pos.upl.parse().ok()?).ok()?;

        let leverage = pos
            .lever
            .as_ref()
            .and_then(|l| l.parse::<u8>().ok())
            .and_then(|l| Leverage::new(l).ok())
            .unwrap_or(Leverage::ONE);

        let liquidation_price = pos
            .liq_px
            .as_ref()
            .filter(|p| !p.is_empty() && *p != "0")
            .and_then(|p| p.parse::<Decimal>().ok())
            .and_then(|d| Price::new(d).ok());

        let side = match pos.pos_side {
            OkxPositionSide::Long => PositionSide::Long,
            OkxPositionSide::Short => PositionSide::Short,
            OkxPositionSide::Net => {
                if quantity_val > Decimal::ZERO {
                    PositionSide::Long
                } else {
                    PositionSide::Short
                }
            }
        };

        let margin_type = match pos.mgn_mode {
            OkxMarginMode::Cross => MarginType::Cross,
            OkxMarginMode::Isolated => MarginType::Isolated,
        };

        let update_time = Timestamp::new(pos.u_time.parse().ok()?).ok()?;

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
            initial_margin: pos
                .imr
                .as_ref()
                .and_then(|m| m.parse::<Decimal>().ok())
                .and_then(|d| Amount::new(d).ok()),
            maintenance_margin: pos
                .mmr
                .as_ref()
                .and_then(|m| m.parse::<Decimal>().ok())
                .and_then(|d| Amount::new(d).ok()),
            update_time,
        })
    }
}

impl Default for OkxTrader {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TraderGateway for OkxTrader {
    async fn login(&mut self, credentials: &Credentials) -> Result<(), ExchangeError> {
        // OKX requires passphrase
        let passphrase =
            credentials
                .passphrase
                .as_ref()
                .ok_or(ExchangeError::AuthenticationFailed {
                    reason: "OKX requires passphrase".to_string(),
                })?;

        let base_url = self.base_url();

        let rest_config = RestConfig::builder()
            .base_url(base_url)
            .exchange("okx")
            .api_key(&credentials.api_key)
            .api_secret(credentials.api_secret())
            .build();

        let rest_client =
            RestClient::new(rest_config).map_err(|e| ExchangeError::AuthenticationFailed {
                reason: format!("Failed to create REST client: {e}"),
            })?;

        // Test authentication by querying account balance
        let timestamp = Self::timestamp_iso();
        let method = "GET";
        let path = "/api/v5/account/balance";
        let body = "";

        let sign = Self::sign_request(&timestamp, method, path, body, credentials.api_secret());

        let mut request = rest_client.get(path);

        // Add OKX-specific headers
        request = request
            .header("OK-ACCESS-KEY", &credentials.api_key)
            .header("OK-ACCESS-SIGN", &sign)
            .header("OK-ACCESS-TIMESTAMP", &timestamp)
            .header("OK-ACCESS-PASSPHRASE", passphrase);

        if self.simulated {
            request = request.header("x-simulated-trading", "1");
        }

        let response = request
            .send()
            .await
            .map_err(|e| ExchangeError::AuthenticationFailed {
                reason: format!("Failed to authenticate: {e}"),
            })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(ExchangeError::AuthenticationFailed {
                reason: format!("HTTP {}: {}", status, body),
            });
        }

        // Parse response to check for API errors
        if let Ok(resp) = serde_json::from_str::<OkxApiResponse<serde_json::Value>>(&body) {
            if !resp.is_success() {
                return Err(Self::parse_error(&resp.code, &resp.msg));
            }
        }

        self.rest_client = Some(rest_client);
        self.credentials = Some(credentials.clone());
        self.logged_in.store(true, Ordering::SeqCst);

        if let Some(callback) = &self.callback {
            callback.on_connected().await;
        }

        info!(
            exchange = "okx",
            market = ?self.market,
            testnet = self.testnet,
            simulated = self.simulated,
            "Trader logged in"
        );

        Ok(())
    }

    async fn logout(&mut self) -> Result<(), ExchangeError> {
        self.rest_client = None;
        self.credentials = None;
        self.logged_in.store(false, Ordering::SeqCst);

        if let Some(callback) = &self.callback {
            callback
                .on_disconnected(Some("Logged out".to_string()))
                .await;
        }

        info!(exchange = "okx", "Trader logged out");
        Ok(())
    }

    fn is_logged_in(&self) -> bool {
        self.logged_in.load(Ordering::SeqCst)
    }

    async fn order_insert(&self, order: &OrderRequest) -> Result<OrderId, ExchangeError> {
        let client = self.client()?;
        let credentials = self
            .credentials
            .as_ref()
            .ok_or(ExchangeError::AuthenticationFailed {
                reason: "Not logged in".to_string(),
            })?;
        let passphrase =
            credentials
                .passphrase
                .as_ref()
                .ok_or(ExchangeError::AuthenticationFailed {
                    reason: "Missing passphrase".to_string(),
                })?;

        let okx_symbol = Self::to_okx_symbol(&order.symbol);
        let side = Self::to_okx_side(order.side);
        let ord_type = Self::to_okx_order_type(order.order_type, order.time_in_force);
        let td_mode = self.trade_mode();

        // Build order request body
        let mut order_body = serde_json::json!({
            "instId": okx_symbol,
            "tdMode": format!("{:?}", td_mode).to_lowercase(),
            "side": format!("{:?}", side).to_lowercase(),
            "ordType": match ord_type {
                OkxOrderType::Market => "market",
                OkxOrderType::Limit => "limit",
                OkxOrderType::PostOnly => "post_only",
                OkxOrderType::Fok => "fok",
                OkxOrderType::Ioc => "ioc",
                OkxOrderType::OptimalLimitIoc => "optimal_limit_ioc",
            },
            "sz": order.quantity.to_string(),
        });

        // Add price for limit orders
        if order.order_type.requires_price() {
            if let Some(price) = &order.price {
                order_body["px"] = serde_json::json!(price.to_string());
            }
        }

        // Add client order ID if provided
        if let Some(client_order_id) = &order.client_order_id {
            order_body["clOrdId"] = serde_json::json!(client_order_id);
        }

        // Add reduce only for derivatives
        if order.reduce_only && !matches!(self.market, OkxMarket::Spot) {
            order_body["reduceOnly"] = serde_json::json!(true);
        }

        let body_str = serde_json::to_string(&order_body).unwrap_or_default();
        let timestamp = Self::timestamp_iso();
        let method = "POST";
        let path = "/api/v5/trade/order";

        let sign = Self::sign_request(
            &timestamp,
            method,
            path,
            &body_str,
            credentials.api_secret(),
        );

        let mut request = client.post(path);

        request = request
            .header("OK-ACCESS-KEY", &credentials.api_key)
            .header("OK-ACCESS-SIGN", &sign)
            .header("OK-ACCESS-TIMESTAMP", &timestamp)
            .header("OK-ACCESS-PASSPHRASE", passphrase)
            .header("Content-Type", "application/json")
            .body(&body_str);

        if self.simulated {
            request = request.header("x-simulated-trading", "1");
        }

        let response = request
            .send()
            .await
            .map_err(|e| ExchangeError::OrderRejected {
                reason: format!("Request failed: {e}"),
                code: None,
            })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(ExchangeError::OrderRejected {
                reason: format!("HTTP {}: {}", status, body),
                code: None,
            });
        }

        let api_response: OkxApiResponse<OkxOrderResponse> =
            serde_json::from_str(&body).map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Failed to parse response: {e}"),
            })?;

        if !api_response.is_success() {
            return Err(Self::parse_error(&api_response.code, &api_response.msg));
        }

        let order_response = api_response.data.first().ok_or(ExchangeError::Unknown {
            code: 0,
            message: "Empty response data".to_string(),
        })?;

        // Check individual order response
        if order_response.s_code != "0" {
            return Err(Self::parse_error(
                &order_response.s_code,
                &order_response.s_msg,
            ));
        }

        let order_id =
            OrderId::new(&order_response.ord_id).map_err(|_| ExchangeError::Unknown {
                code: 0,
                message: "Invalid order ID returned from exchange".to_string(),
            })?;

        info!(
            exchange = "okx",
            order_id = %order_id,
            symbol = %order.symbol,
            side = ?order.side,
            "Order submitted"
        );

        Ok(order_id)
    }

    async fn order_cancel(&self, order_id: &OrderId) -> Result<bool, ExchangeError> {
        let client = self.client()?;
        let credentials = self
            .credentials
            .as_ref()
            .ok_or(ExchangeError::AuthenticationFailed {
                reason: "Not logged in".to_string(),
            })?;
        let passphrase =
            credentials
                .passphrase
                .as_ref()
                .ok_or(ExchangeError::AuthenticationFailed {
                    reason: "Missing passphrase".to_string(),
                })?;

        // Get symbol from local cache
        let symbol = {
            let orders = self.orders.read();
            orders.get(order_id).map(|o| o.symbol.clone())
        };

        let symbol = symbol.ok_or(ExchangeError::OrderNotFound {
            order_id: order_id.to_string(),
        })?;

        let okx_symbol = Self::to_okx_symbol(&symbol);

        let cancel_body = serde_json::json!({
            "instId": okx_symbol,
            "ordId": order_id.as_str(),
        });

        let body_str = serde_json::to_string(&cancel_body).unwrap_or_default();
        let timestamp = Self::timestamp_iso();
        let method = "POST";
        let path = "/api/v5/trade/cancel-order";

        let sign = Self::sign_request(
            &timestamp,
            method,
            path,
            &body_str,
            credentials.api_secret(),
        );

        let mut request = client.post(path);

        request = request
            .header("OK-ACCESS-KEY", &credentials.api_key)
            .header("OK-ACCESS-SIGN", &sign)
            .header("OK-ACCESS-TIMESTAMP", &timestamp)
            .header("OK-ACCESS-PASSPHRASE", passphrase)
            .header("Content-Type", "application/json")
            .body(&body_str);

        if self.simulated {
            request = request.header("x-simulated-trading", "1");
        }

        let response = request.send().await.map_err(|e| ExchangeError::Unknown {
            code: 0,
            message: format!("Request failed: {e}"),
        })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(ExchangeError::Unknown {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        let api_response: OkxApiResponse<OkxCancelResponse> =
            serde_json::from_str(&body).map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Failed to parse response: {e}"),
            })?;

        if !api_response.is_success() {
            return Err(Self::parse_error(&api_response.code, &api_response.msg));
        }

        // Update local cache
        {
            let mut orders = self.orders.write();
            if let Some(order) = orders.get_mut(order_id) {
                order.status = OrderStatus::Canceled;
                order.update_time = Timestamp::now();
            }
        }

        info!(
            exchange = "okx",
            order_id = %order_id,
            "Order canceled"
        );

        Ok(true)
    }

    async fn cancel_all_orders(&self, symbol: Option<&Symbol>) -> Result<usize, ExchangeError> {
        let _client = self.client()?;
        let credentials = self
            .credentials
            .as_ref()
            .ok_or(ExchangeError::AuthenticationFailed {
                reason: "Not logged in".to_string(),
            })?;
        let _passphrase =
            credentials
                .passphrase
                .as_ref()
                .ok_or(ExchangeError::AuthenticationFailed {
                    reason: "Missing passphrase".to_string(),
                })?;

        // First query open orders
        let orders = self.query_orders(symbol).await?;

        if orders.is_empty() {
            return Ok(0);
        }

        // Cancel each order
        let mut canceled = 0;
        for order in &orders {
            if self.order_cancel(&order.order_id).await.is_ok() {
                canceled += 1;
            }
        }

        info!(
            exchange = "okx",
            count = canceled,
            symbol = ?symbol,
            "All orders canceled"
        );

        Ok(canceled)
    }

    async fn query_positions(&self) -> Result<Vec<Position>, ExchangeError> {
        let client = self.client()?;
        let credentials = self
            .credentials
            .as_ref()
            .ok_or(ExchangeError::AuthenticationFailed {
                reason: "Not logged in".to_string(),
            })?;
        let passphrase =
            credentials
                .passphrase
                .as_ref()
                .ok_or(ExchangeError::AuthenticationFailed {
                    reason: "Missing passphrase".to_string(),
                })?;

        // Spot doesn't have positions
        if matches!(self.market, OkxMarket::Spot) {
            return Ok(Vec::new());
        }

        let timestamp = Self::timestamp_iso();
        let method = "GET";
        let path = "/api/v5/account/positions";

        let sign = Self::sign_request(&timestamp, method, path, "", credentials.api_secret());

        let mut request = client.get(path);

        request = request
            .header("OK-ACCESS-KEY", &credentials.api_key)
            .header("OK-ACCESS-SIGN", &sign)
            .header("OK-ACCESS-TIMESTAMP", &timestamp)
            .header("OK-ACCESS-PASSPHRASE", passphrase);

        if self.simulated {
            request = request.header("x-simulated-trading", "1");
        }

        let response = request.send().await.map_err(|e| ExchangeError::Unknown {
            code: 0,
            message: format!("Request failed: {e}"),
        })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(ExchangeError::Unknown {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        let api_response: OkxApiResponse<OkxPosition> =
            serde_json::from_str(&body).map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Failed to parse response: {e}"),
            })?;

        if !api_response.is_success() {
            return Err(Self::parse_error(&api_response.code, &api_response.msg));
        }

        let positions: Vec<Position> = api_response
            .data
            .iter()
            .filter_map(|p| self.convert_position(p))
            .collect();

        debug!(
            exchange = "okx",
            count = positions.len(),
            "Positions queried"
        );

        Ok(positions)
    }

    async fn query_orders(&self, symbol: Option<&Symbol>) -> Result<Vec<Order>, ExchangeError> {
        let client = self.client()?;
        let credentials = self
            .credentials
            .as_ref()
            .ok_or(ExchangeError::AuthenticationFailed {
                reason: "Not logged in".to_string(),
            })?;
        let passphrase =
            credentials
                .passphrase
                .as_ref()
                .ok_or(ExchangeError::AuthenticationFailed {
                    reason: "Missing passphrase".to_string(),
                })?;

        let timestamp = Self::timestamp_iso();
        let method = "GET";

        let path = if let Some(sym) = symbol {
            format!(
                "/api/v5/trade/orders-pending?instId={}",
                Self::to_okx_symbol(sym)
            )
        } else {
            format!(
                "/api/v5/trade/orders-pending?instType={}",
                self.market.inst_type()
            )
        };

        let sign = Self::sign_request(&timestamp, method, &path, "", credentials.api_secret());

        let mut request = client.get(&path);

        request = request
            .header("OK-ACCESS-KEY", &credentials.api_key)
            .header("OK-ACCESS-SIGN", &sign)
            .header("OK-ACCESS-TIMESTAMP", &timestamp)
            .header("OK-ACCESS-PASSPHRASE", passphrase);

        if self.simulated {
            request = request.header("x-simulated-trading", "1");
        }

        let response = request.send().await.map_err(|e| ExchangeError::Unknown {
            code: 0,
            message: format!("Request failed: {e}"),
        })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(ExchangeError::Unknown {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        let api_response: OkxApiResponse<OkxOrderDetails> =
            serde_json::from_str(&body).map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Failed to parse response: {e}"),
            })?;

        if !api_response.is_success() {
            return Err(Self::parse_error(&api_response.code, &api_response.msg));
        }

        let orders: Vec<Order> = api_response
            .data
            .iter()
            .filter_map(|o| self.convert_order_details(o))
            .collect();

        // Update local cache
        {
            let mut cache = self.orders.write();
            for order in &orders {
                cache.insert(order.order_id.clone(), order.clone());
            }
        }

        debug!(exchange = "okx", count = orders.len(), "Orders queried");

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

        // Order not in cache
        Err(ExchangeError::OrderNotFound {
            order_id: order_id.to_string(),
        })
    }

    async fn query_account(&self) -> Result<Account, ExchangeError> {
        let client = self.client()?;
        let credentials = self
            .credentials
            .as_ref()
            .ok_or(ExchangeError::AuthenticationFailed {
                reason: "Not logged in".to_string(),
            })?;
        let passphrase =
            credentials
                .passphrase
                .as_ref()
                .ok_or(ExchangeError::AuthenticationFailed {
                    reason: "Missing passphrase".to_string(),
                })?;

        let timestamp = Self::timestamp_iso();
        let method = "GET";
        let path = "/api/v5/account/balance";

        let sign = Self::sign_request(&timestamp, method, path, "", credentials.api_secret());

        let mut request = client.get(path);

        request = request
            .header("OK-ACCESS-KEY", &credentials.api_key)
            .header("OK-ACCESS-SIGN", &sign)
            .header("OK-ACCESS-TIMESTAMP", &timestamp)
            .header("OK-ACCESS-PASSPHRASE", passphrase);

        if self.simulated {
            request = request.header("x-simulated-trading", "1");
        }

        let response = request.send().await.map_err(|e| ExchangeError::Unknown {
            code: 0,
            message: format!("Request failed: {e}"),
        })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(ExchangeError::Unknown {
                code: status.as_u16() as i32,
                message: body,
            });
        }

        let api_response: OkxApiResponse<OkxAccount> =
            serde_json::from_str(&body).map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Failed to parse response: {e}"),
            })?;

        if !api_response.is_success() {
            return Err(Self::parse_error(&api_response.code, &api_response.msg));
        }

        let okx_account = api_response.data.first().ok_or(ExchangeError::Unknown {
            code: 0,
            message: "Empty account data".to_string(),
        })?;

        let balances: Vec<Balance> = okx_account
            .details
            .iter()
            .filter_map(|b| {
                let total = Amount::new(b.eq.parse().ok()?).ok()?;
                let available = Amount::new(b.avail_eq.parse().ok()?).ok()?;
                let frozen = b
                    .frozen_bal
                    .as_ref()
                    .and_then(|f| f.parse::<Decimal>().ok())
                    .and_then(|d| Amount::new(d).ok())
                    .unwrap_or(Amount::ZERO);

                Some(Balance::new(&b.ccy, total, available, frozen))
            })
            .collect();

        let total_equity = okx_account
            .total_eq
            .parse::<Decimal>()
            .map_err(|_| ExchangeError::Unknown {
                code: 0,
                message: "Failed to parse total equity".to_string(),
            })
            .and_then(|d| {
                Amount::new(d).map_err(|_| ExchangeError::Unknown {
                    code: 0,
                    message: "Invalid total equity value".to_string(),
                })
            })?;

        // Calculate available balance from details
        let available_balance = balances
            .iter()
            .map(|b| b.available.as_decimal())
            .sum::<Decimal>();
        let available_balance = Amount::new(available_balance).unwrap_or(Amount::ZERO);

        let margin_used = okx_account
            .imr
            .as_ref()
            .and_then(|m| m.parse::<Decimal>().ok())
            .and_then(|d| Amount::new(d).ok())
            .unwrap_or(Amount::ZERO);

        let unrealized_pnl = balances
            .iter()
            .filter_map(|_| {
                okx_account
                    .details
                    .iter()
                    .filter_map(|d| d.upl.as_ref()?.parse::<Decimal>().ok())
                    .next()
            })
            .sum::<Decimal>();
        let unrealized_pnl = Amount::new(unrealized_pnl).unwrap_or(Amount::ZERO);

        let update_time = Timestamp::new(okx_account.u_time.parse().unwrap_or(0))
            .unwrap_or_else(|_| Timestamp::now());

        Ok(Account {
            exchange: Exchange::Okx,
            balances,
            total_equity,
            available_balance,
            margin_used,
            unrealized_pnl,
            update_time,
        })
    }

    fn set_callback(&mut self, callback: Box<dyn TraderCallback>) {
        self.callback = Some(Arc::from(callback));
    }

    fn exchange(&self) -> &str {
        "okx"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_okx_symbol() {
        assert_eq!(
            OkxTrader::to_okx_symbol(&Symbol::new("BTC-USDT").unwrap()),
            "BTC-USDT"
        );
        assert_eq!(
            OkxTrader::to_okx_symbol(&Symbol::new("eth-btc").unwrap()),
            "ETH-BTC"
        );
    }

    #[test]
    fn test_from_okx_symbol() {
        let symbol = OkxTrader::from_okx_symbol("BTC-USDT").unwrap();
        assert_eq!(symbol.as_str(), "BTC-USDT");

        let symbol = OkxTrader::from_okx_symbol("ETH-BTC").unwrap();
        assert_eq!(symbol.as_str(), "ETH-BTC");
    }

    #[test]
    fn test_side_conversion() {
        assert!(matches!(
            OkxTrader::to_okx_side(OrderSide::Buy),
            OkxOrderSide::Buy
        ));
        assert!(matches!(
            OkxTrader::to_okx_side(OrderSide::Sell),
            OkxOrderSide::Sell
        ));

        assert!(matches!(
            OkxTrader::from_okx_side(OkxOrderSide::Buy),
            OrderSide::Buy
        ));
        assert!(matches!(
            OkxTrader::from_okx_side(OkxOrderSide::Sell),
            OrderSide::Sell
        ));
    }

    #[test]
    fn test_order_type_conversion() {
        assert!(matches!(
            OkxTrader::to_okx_order_type(OrderType::Limit, TimeInForce::Gtc),
            OkxOrderType::Limit
        ));
        assert!(matches!(
            OkxTrader::to_okx_order_type(OrderType::Market, TimeInForce::Gtc),
            OkxOrderType::Market
        ));
        assert!(matches!(
            OkxTrader::to_okx_order_type(OrderType::Limit, TimeInForce::PostOnly),
            OkxOrderType::PostOnly
        ));
        assert!(matches!(
            OkxTrader::to_okx_order_type(OrderType::Limit, TimeInForce::Fok),
            OkxOrderType::Fok
        ));
        assert!(matches!(
            OkxTrader::to_okx_order_type(OrderType::Limit, TimeInForce::Ioc),
            OkxOrderType::Ioc
        ));
    }

    #[test]
    fn test_status_conversion() {
        assert!(matches!(
            OkxTrader::from_okx_status(OkxOrderStatus::Live),
            OrderStatus::New
        ));
        assert!(matches!(
            OkxTrader::from_okx_status(OkxOrderStatus::Filled),
            OrderStatus::Filled
        ));
        assert!(matches!(
            OkxTrader::from_okx_status(OkxOrderStatus::Canceled),
            OrderStatus::Canceled
        ));
        assert!(matches!(
            OkxTrader::from_okx_status(OkxOrderStatus::PartiallyFilled),
            OrderStatus::PartiallyFilled
        ));
    }

    #[test]
    fn test_trade_mode() {
        let spot_trader = OkxTrader::new();
        assert!(matches!(spot_trader.trade_mode(), OkxTradeMode::Cash));

        let swap_trader = OkxTrader::new_swap();
        assert!(matches!(swap_trader.trade_mode(), OkxTradeMode::Cross));
    }

    #[test]
    fn test_sign_request() {
        let timestamp = "2023-01-01T00:00:00.000Z";
        let method = "GET";
        let path = "/api/v5/account/balance";
        let body = "";
        let secret = "test_secret";

        let sign = OkxTrader::sign_request(timestamp, method, path, body, secret);

        // Verify signature is base64 encoded
        assert!(
            base64::engine::general_purpose::STANDARD
                .decode(&sign)
                .is_ok()
        );
    }

    #[test]
    fn test_parse_error() {
        let error = OkxTrader::parse_error("50100", "API key invalid");
        assert!(matches!(error, ExchangeError::AuthenticationFailed { .. }));

        let error = OkxTrader::parse_error("50011", "Rate limit exceeded");
        assert!(matches!(error, ExchangeError::RateLimited { .. }));

        let error = OkxTrader::parse_error("51008", "Insufficient balance");
        assert!(matches!(error, ExchangeError::InsufficientBalance { .. }));

        let error = OkxTrader::parse_error("51400", "Order not found");
        assert!(matches!(error, ExchangeError::OrderNotFound { .. }));
    }

    #[test]
    fn test_trader_default() {
        let trader = OkxTrader::default();
        assert_eq!(trader.market(), OkxMarket::Spot);
        assert!(!trader.is_logged_in());
    }

    #[test]
    fn test_trader_swap() {
        let trader = OkxTrader::new_swap();
        assert_eq!(trader.market(), OkxMarket::Swap);
    }

    #[test]
    fn test_trader_testnet() {
        let trader = OkxTrader::new().with_testnet(true);
        assert_eq!(trader.base_url(), "https://www.okx.com");
    }

    #[test]
    fn test_trader_simulated() {
        let trader = OkxTrader::new().with_simulated(true);
        assert!(trader.simulated);
    }

    #[test]
    fn test_timestamp_iso() {
        let ts = OkxTrader::timestamp_iso();
        // Should be in ISO 8601 format
        assert!(ts.contains('T'));
        assert!(ts.ends_with('Z'));
    }
}
