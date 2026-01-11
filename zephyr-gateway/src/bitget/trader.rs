#![allow(clippy::disallowed_types)]

//! Bitget trading gateway implementation.
//!
//! Implements the [`TraderGateway`] trait for Bitget REST/WebSocket API.

use async_trait::async_trait;
use base64::Engine;
use hmac::{Hmac, Mac};
use parking_lot::RwLock;
use rust_decimal::Decimal;
use sha2::Sha256;
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

/// Bitget trading gateway.
///
/// Provides trading functionality for Bitget exchange including:
/// - Order submission and cancellation
/// - Position and order queries
/// - Account information
///
/// # Bitget-Specific Features
///
/// - Requires passphrase in addition to API key and secret
/// - Uses product types: SPOT, USDT-FUTURES, COIN-FUTURES
/// - Supports margin modes: crossed, isolated
///
/// # Example
///
/// ```ignore
/// use zephyr_gateway::bitget::BitgetTrader;
/// use zephyr_core::traits::{Credentials, TraderGateway};
///
/// let mut trader = BitgetTrader::new_futures();
/// let creds = Credentials::new("api_key", "api_secret")
///     .with_passphrase("passphrase");
/// trader.login(&creds).await?;
///
/// let positions = trader.query_positions().await?;
/// ```
pub struct BitgetTrader {
    /// Market type
    market: BitgetMarket,
    /// REST client
    rest_client: Option<RestClient>,
    /// Credentials
    credentials: Option<Credentials>,
    /// User callback
    callback: Option<Arc<dyn TraderCallback>>,
    /// Login state
    logged_in: Arc<AtomicBool>,
    /// Local order cache
    orders: Arc<RwLock<std::collections::HashMap<OrderId, Order>>>,
    /// Use testnet (demo trading)
    testnet: bool,
}

impl BitgetTrader {
    /// Creates a new Bitget trader for spot market.
    #[must_use]
    pub fn new() -> Self {
        Self::with_market(BitgetMarket::Spot)
    }

    /// Creates a new Bitget trader for USDT futures.
    #[must_use]
    pub fn new_futures() -> Self {
        Self::with_market(BitgetMarket::UsdtFutures)
    }

    /// Creates a new Bitget trader with specified market type.
    #[must_use]
    pub fn with_market(market: BitgetMarket) -> Self {
        Self {
            market,
            rest_client: None,
            credentials: None,
            callback: None,
            logged_in: Arc::new(AtomicBool::new(false)),
            orders: Arc::new(RwLock::new(std::collections::HashMap::new())),
            testnet: false,
        }
    }

    /// Sets whether to use testnet (demo trading).
    #[must_use]
    pub fn with_testnet(mut self, testnet: bool) -> Self {
        self.testnet = testnet;
        self
    }

    /// Returns the market type.
    #[must_use]
    pub fn market(&self) -> BitgetMarket {
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

    /// Converts a symbol to Bitget format (e.g., "BTC-USDT" -> "BTCUSDT").
    fn to_bitget_symbol(symbol: &Symbol) -> String {
        symbol.as_str().replace('-', "").to_uppercase()
    }

    /// Converts a Bitget symbol to standard format (e.g., "BTCUSDT" -> "BTC-USDT").
    fn from_bitget_symbol(bitget_symbol: &str) -> Option<Symbol> {
        let quotes = ["USDT", "USDC", "BTC", "ETH"];
        let upper = bitget_symbol.to_uppercase();

        for quote in quotes {
            if upper.ends_with(quote) {
                let base = &upper[..upper.len() - quote.len()];
                if !base.is_empty() {
                    return Symbol::new(format!("{base}-{quote}")).ok();
                }
            }
        }
        Symbol::new(bitget_symbol).ok()
    }

    /// Converts order side to Bitget format.
    fn to_bitget_side(side: OrderSide) -> BitgetOrderSide {
        match side {
            OrderSide::Buy => BitgetOrderSide::Buy,
            OrderSide::Sell => BitgetOrderSide::Sell,
        }
    }

    /// Converts Bitget order side to standard format.
    fn from_bitget_side(side: BitgetOrderSide) -> OrderSide {
        match side {
            BitgetOrderSide::Buy => OrderSide::Buy,
            BitgetOrderSide::Sell => OrderSide::Sell,
        }
    }

    /// Converts order type to Bitget format.
    fn to_bitget_order_type(order_type: OrderType) -> BitgetOrderType {
        match order_type {
            OrderType::Market => BitgetOrderType::Market,
            OrderType::Limit | _ => BitgetOrderType::Limit,
        }
    }

    /// Converts Bitget order type to standard format.
    fn from_bitget_order_type(order_type: BitgetOrderType) -> OrderType {
        match order_type {
            BitgetOrderType::Limit => OrderType::Limit,
            BitgetOrderType::Market => OrderType::Market,
        }
    }

    /// Converts Bitget order status to standard format.
    fn from_bitget_status(status: BitgetOrderStatus) -> OrderStatus {
        match status {
            BitgetOrderStatus::Init | BitgetOrderStatus::New | BitgetOrderStatus::Live => {
                OrderStatus::New
            }
            BitgetOrderStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
            BitgetOrderStatus::Filled => OrderStatus::Filled,
            BitgetOrderStatus::Cancelled => OrderStatus::Canceled,
        }
    }

    /// Converts time in force to Bitget format.
    fn to_bitget_tif(tif: TimeInForce) -> BitgetTimeInForce {
        match tif {
            TimeInForce::Gtc | TimeInForce::Gtd => BitgetTimeInForce::Gtc,
            TimeInForce::Ioc => BitgetTimeInForce::Ioc,
            TimeInForce::Fok => BitgetTimeInForce::Fok,
            TimeInForce::PostOnly => BitgetTimeInForce::PostOnly,
        }
    }

    /// Returns the current timestamp in milliseconds.
    #[allow(clippy::cast_possible_truncation)]
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
            .ok_or_else(|| ExchangeError::AuthenticationFailed {
                reason: "Not logged in".to_string(),
            })
    }

    /// Parses a Bitget API error response.
    fn parse_error(code: &str, msg: &str) -> ExchangeError {
        match code {
            "00000" => ExchangeError::Unknown {
                code: 0,
                message: msg.to_string(),
            },
            "40001" | "40002" | "40003" | "40004" | "40005" | "40006" | "40007" => {
                ExchangeError::AuthenticationFailed {
                    reason: msg.to_string(),
                }
            }
            "40014" | "40015" | "40016" | "40017" | "40018" => ExchangeError::InvalidParameter {
                param: "request".to_string(),
                reason: msg.to_string(),
            },
            "40700" | "40701" | "40702" | "40703" | "40704" => ExchangeError::RateLimited {
                retry_after_ms: 1000,
            },
            "43001" | "43002" | "43003" | "43004" | "43005" | "43006" | "43007" | "43008" => {
                ExchangeError::InsufficientBalance {
                    required: Decimal::ZERO,
                    available: Decimal::ZERO,
                }
            }
            "45001" | "45002" | "45003" | "45004" | "45005" | "45006" | "45007" | "45008" => {
                ExchangeError::OrderRejected {
                    reason: msg.to_string(),
                    code: Some(code.parse().unwrap_or(0)),
                }
            }
            "45110" => ExchangeError::OrderNotFound {
                order_id: String::new(),
            },
            _ => ExchangeError::Unknown {
                code: code.parse().unwrap_or(0),
                message: msg.to_string(),
            },
        }
    }

    /// Converts Bitget order details to standard Order.
    #[allow(clippy::too_many_lines)]
    fn convert_order_details(details: &BitgetOrderDetails) -> Option<Order> {
        let symbol = Self::from_bitget_symbol(&details.inst_id)?;
        let order_id = OrderId::new(&details.order_id).ok()?;
        let status = Self::from_bitget_status(details.status);
        let side = Self::from_bitget_side(details.side);
        let order_type = Self::from_bitget_order_type(details.order_type);

        let price = details.price.as_ref().and_then(|p| {
            if p.is_empty() || p == "0" {
                None
            } else {
                p.parse::<Decimal>().ok().and_then(|d| Price::new(d).ok())
            }
        });

        let quantity = Quantity::new(details.size.parse().ok()?).ok()?;
        let filled_quantity = details
            .filled_qty
            .as_ref()
            .and_then(|q| q.parse::<Decimal>().ok())
            .and_then(|d| Quantity::new(d).ok())
            .unwrap_or(Quantity::ZERO);

        let avg_price = details.price_avg.as_ref().and_then(|p| {
            if p.is_empty() || p == "0" {
                None
            } else {
                p.parse::<Decimal>().ok().and_then(|d| Price::new(d).ok())
            }
        });

        let create_time = details
            .c_time
            .as_ref()
            .and_then(|t| t.parse().ok())
            .and_then(|t| Timestamp::new(t).ok())
            .unwrap_or_else(Timestamp::now);
        let update_time = details
            .u_time
            .as_ref()
            .and_then(|t| t.parse().ok())
            .and_then(|t| Timestamp::new(t).ok())
            .unwrap_or(create_time);

        let reduce_only = details.reduce_only.as_deref() == Some("true");
        let tif = details.force.unwrap_or(BitgetTimeInForce::Gtc);

        Some(Order {
            order_id,
            client_order_id: details.client_oid.clone(),
            symbol,
            side,
            order_type,
            status,
            price,
            stop_price: None,
            quantity,
            filled_quantity,
            avg_price,
            time_in_force: match tif {
                BitgetTimeInForce::Gtc => TimeInForce::Gtc,
                BitgetTimeInForce::Ioc => TimeInForce::Ioc,
                BitgetTimeInForce::Fok => TimeInForce::Fok,
                BitgetTimeInForce::PostOnly => TimeInForce::PostOnly,
            },
            reduce_only,
            post_only: matches!(tif, BitgetTimeInForce::PostOnly),
            create_time,
            update_time,
        })
    }

    /// Converts Bitget position to standard Position.
    #[allow(clippy::too_many_lines)]
    fn convert_position(pos: &BitgetPosition) -> Option<Position> {
        let symbol = Self::from_bitget_symbol(&pos.inst_id)?;
        let quantity_val: Decimal = pos.total.parse().ok()?;

        // Skip empty positions
        if quantity_val == Decimal::ZERO {
            return None;
        }

        let quantity = Quantity::new(quantity_val.abs()).ok()?;
        let entry_price = Price::new(pos.average_open_price.parse().ok()?).ok()?;

        let mark_price = pos
            .mark_price
            .as_ref()
            .and_then(|p| p.parse::<Decimal>().ok())
            .and_then(|d| MarkPrice::new(d).ok())
            .unwrap_or(MarkPrice::ZERO);

        let unrealized_pnl = Amount::new(pos.unrealized_pl.parse().ok()?).ok()?;
        let realized_pnl = pos
            .realized_pl
            .as_ref()
            .and_then(|p| p.parse::<Decimal>().ok())
            .and_then(|d| Amount::new(d).ok())
            .unwrap_or(Amount::ZERO);

        let leverage = pos
            .leverage
            .parse::<u8>()
            .ok()
            .and_then(|l| Leverage::new(l).ok())
            .unwrap_or(Leverage::ONE);

        let liquidation_price = pos
            .liquidation_price
            .as_ref()
            .filter(|p| !p.is_empty() && *p != "0")
            .and_then(|p| p.parse::<Decimal>().ok())
            .and_then(|d| Price::new(d).ok());

        let side = match pos.hold_side {
            BitgetPositionSide::Long => PositionSide::Long,
            BitgetPositionSide::Short => PositionSide::Short,
        };

        let margin_type = match pos.margin_mode {
            BitgetMarginMode::Crossed => MarginType::Cross,
            BitgetMarginMode::Isolated => MarginType::Isolated,
        };

        let update_time = pos
            .u_time
            .as_ref()
            .and_then(|t| t.parse().ok())
            .and_then(|t| Timestamp::new(t).ok())
            .unwrap_or_else(Timestamp::now);

        Some(Position {
            symbol,
            side,
            quantity,
            entry_price,
            mark_price,
            liquidation_price,
            unrealized_pnl,
            realized_pnl,
            leverage,
            margin_type,
            initial_margin: pos
                .margin
                .as_ref()
                .and_then(|m| m.parse::<Decimal>().ok())
                .and_then(|d| Amount::new(d).ok()),
            maintenance_margin: None,
            update_time,
        })
    }
}

impl Default for BitgetTrader {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TraderGateway for BitgetTrader {
    #[allow(clippy::too_many_lines)]
    async fn login(&mut self, credentials: &Credentials) -> Result<(), ExchangeError> {
        // Bitget requires passphrase
        let passphrase =
            credentials
                .passphrase
                .as_ref()
                .ok_or_else(|| ExchangeError::AuthenticationFailed {
                    reason: "Bitget requires passphrase".to_string(),
                })?;

        let base_url = self.base_url();

        let rest_config = RestConfig::builder()
            .base_url(base_url)
            .exchange("bitget")
            .api_key(&credentials.api_key)
            .api_secret(credentials.api_secret())
            .build();

        let rest_client =
            RestClient::new(rest_config).map_err(|e| ExchangeError::AuthenticationFailed {
                reason: format!("Failed to create REST client: {e}"),
            })?;

        // Test authentication by querying account
        let timestamp = Self::timestamp_ms().to_string();
        let method = "GET";
        let path = match self.market {
            BitgetMarket::Spot => "/api/v2/spot/account/assets",
            BitgetMarket::UsdtFutures => "/api/v2/mix/account/accounts?productType=USDT-FUTURES",
            BitgetMarket::CoinFutures => "/api/v2/mix/account/accounts?productType=COIN-FUTURES",
        };

        let sign = Self::sign_request(&timestamp, method, path, "", credentials.api_secret());

        let mut request = rest_client.get(path);

        // Add Bitget-specific headers
        request = request
            .header("ACCESS-KEY", &credentials.api_key)
            .header("ACCESS-SIGN", &sign)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", passphrase)
            .header("Content-Type", "application/json")
            .header("locale", "en-US");

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
                reason: format!("HTTP {status}: {body}"),
            });
        }

        // Parse response to check for API errors
        if let Ok(resp) = serde_json::from_str::<BitgetApiResponse<serde_json::Value>>(&body)
            && !resp.is_success()
        {
            return Err(Self::parse_error(&resp.code, &resp.msg));
        }

        self.rest_client = Some(rest_client);
        self.credentials = Some(credentials.clone());
        self.logged_in.store(true, Ordering::SeqCst);

        if let Some(callback) = &self.callback {
            callback.on_connected().await;
        }

        info!(
            exchange = "bitget",
            market = ?self.market,
            testnet = self.testnet,
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

        info!(exchange = "bitget", "Trader logged out");
        Ok(())
    }

    fn is_logged_in(&self) -> bool {
        self.logged_in.load(Ordering::SeqCst)
    }

    async fn order_insert(&self, order: &OrderRequest) -> Result<OrderId, ExchangeError> {
        let client = self.client()?;
        let credentials =
            self.credentials
                .as_ref()
                .ok_or_else(|| ExchangeError::AuthenticationFailed {
                    reason: "Not logged in".to_string(),
                })?;
        let passphrase =
            credentials
                .passphrase
                .as_ref()
                .ok_or_else(|| ExchangeError::AuthenticationFailed {
                    reason: "Missing passphrase".to_string(),
                })?;

        let bitget_symbol = Self::to_bitget_symbol(&order.symbol);
        let side = Self::to_bitget_side(order.side);
        let ord_type = Self::to_bitget_order_type(order.order_type);
        let tif = Self::to_bitget_tif(order.time_in_force);

        // Build order request body based on market type
        let (path, order_body) = match self.market {
            BitgetMarket::Spot => {
                let mut body = serde_json::json!({
                    "symbol": bitget_symbol,
                    "side": format!("{side:?}").to_lowercase(),
                    "orderType": format!("{ord_type:?}").to_lowercase(),
                    "size": order.quantity.to_string(),
                    "force": format!("{tif:?}").to_lowercase(),
                });

                if order.order_type.requires_price() && order.price.is_some() {
                    body["price"] = serde_json::json!(order.price.as_ref().unwrap().to_string());
                }

                if let Some(client_order_id) = &order.client_order_id {
                    body["clientOid"] = serde_json::json!(client_order_id);
                }

                ("/api/v2/spot/trade/place-order", body)
            }
            BitgetMarket::UsdtFutures | BitgetMarket::CoinFutures => {
                let product_type = self.market.product_type();
                let margin_coin = if matches!(self.market, BitgetMarket::UsdtFutures) {
                    "USDT"
                } else {
                    // For coin futures, extract base currency
                    order.symbol.as_str().split('-').next().unwrap_or("BTC")
                };

                let mut body = serde_json::json!({
                    "symbol": bitget_symbol,
                    "productType": product_type,
                    "marginMode": "crossed",
                    "marginCoin": margin_coin,
                    "side": format!("{side:?}").to_lowercase(),
                    "orderType": format!("{ord_type:?}").to_lowercase(),
                    "size": order.quantity.to_string(),
                    "force": format!("{tif:?}").to_lowercase(),
                });

                if order.order_type.requires_price() && order.price.is_some() {
                    body["price"] = serde_json::json!(order.price.as_ref().unwrap().to_string());
                }

                if let Some(client_order_id) = &order.client_order_id {
                    body["clientOid"] = serde_json::json!(client_order_id);
                }

                if order.reduce_only {
                    body["reduceOnly"] = serde_json::json!("YES");
                }

                ("/api/v2/mix/order/place-order", body)
            }
        };

        let body_str = serde_json::to_string(&order_body).unwrap_or_default();
        let timestamp = Self::timestamp_ms().to_string();
        let method = "POST";

        let sign = Self::sign_request(
            &timestamp,
            method,
            path,
            &body_str,
            credentials.api_secret(),
        );

        let mut request = client.post(path);

        request = request
            .header("ACCESS-KEY", &credentials.api_key)
            .header("ACCESS-SIGN", &sign)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", passphrase)
            .header("Content-Type", "application/json")
            .header("locale", "en-US")
            .body(&body_str);

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
                reason: format!("HTTP {status}: {body}"),
                code: None,
            });
        }

        let api_response: BitgetApiResponse<BitgetOrderResponse> = serde_json::from_str(&body)
            .map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Failed to parse response: {e}"),
            })?;

        if !api_response.is_success() {
            return Err(Self::parse_error(&api_response.code, &api_response.msg));
        }

        let order_response = api_response.data.ok_or_else(|| ExchangeError::Unknown {
            code: 0,
            message: "Empty response data".to_string(),
        })?;

        let order_id =
            OrderId::new(&order_response.order_id).map_err(|_| ExchangeError::Unknown {
                code: 0,
                message: "Invalid order ID returned from exchange".to_string(),
            })?;

        info!(
            exchange = "bitget",
            order_id = %order_id,
            symbol = %order.symbol,
            side = ?order.side,
            "Order submitted"
        );

        Ok(order_id)
    }

    async fn order_cancel(&self, order_id: &OrderId) -> Result<bool, ExchangeError> {
        let client = self.client()?;
        let credentials =
            self.credentials
                .as_ref()
                .ok_or_else(|| ExchangeError::AuthenticationFailed {
                    reason: "Not logged in".to_string(),
                })?;
        let passphrase =
            credentials
                .passphrase
                .as_ref()
                .ok_or_else(|| ExchangeError::AuthenticationFailed {
                    reason: "Missing passphrase".to_string(),
                })?;

        // Get symbol from local cache
        let symbol = {
            let orders = self.orders.read();
            orders.get(order_id).map(|o| o.symbol.clone())
        };

        let symbol = symbol.ok_or_else(|| ExchangeError::OrderNotFound {
            order_id: order_id.to_string(),
        })?;

        let bitget_symbol = Self::to_bitget_symbol(&symbol);

        let (path, cancel_body) = match self.market {
            BitgetMarket::Spot => {
                let body = serde_json::json!({
                    "symbol": bitget_symbol,
                    "orderId": order_id.as_str(),
                });
                ("/api/v2/spot/trade/cancel-order", body)
            }
            BitgetMarket::UsdtFutures | BitgetMarket::CoinFutures => {
                let product_type = self.market.product_type();
                let body = serde_json::json!({
                    "symbol": bitget_symbol,
                    "productType": product_type,
                    "orderId": order_id.as_str(),
                });
                ("/api/v2/mix/order/cancel-order", body)
            }
        };

        let body_str = serde_json::to_string(&cancel_body).unwrap_or_default();
        let timestamp = Self::timestamp_ms().to_string();
        let method = "POST";

        let sign = Self::sign_request(
            &timestamp,
            method,
            path,
            &body_str,
            credentials.api_secret(),
        );

        let mut request = client.post(path);

        request = request
            .header("ACCESS-KEY", &credentials.api_key)
            .header("ACCESS-SIGN", &sign)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", passphrase)
            .header("Content-Type", "application/json")
            .header("locale", "en-US")
            .body(&body_str);

        let response = request.send().await.map_err(|e| ExchangeError::Unknown {
            code: 0,
            message: format!("Request failed: {e}"),
        })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(ExchangeError::Unknown {
                code: i32::from(status.as_u16()),
                message: body,
            });
        }

        let api_response: BitgetApiResponse<BitgetCancelResponse> = serde_json::from_str(&body)
            .map_err(|e| ExchangeError::Unknown {
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
            exchange = "bitget",
            order_id = %order_id,
            "Order canceled"
        );

        Ok(true)
    }

    async fn cancel_all_orders(&self, symbol: Option<&Symbol>) -> Result<usize, ExchangeError> {
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
            exchange = "bitget",
            count = canceled,
            symbol = ?symbol,
            "All orders canceled"
        );

        Ok(canceled)
    }

    async fn query_positions(&self) -> Result<Vec<Position>, ExchangeError> {
        let client = self.client()?;
        let credentials =
            self.credentials
                .as_ref()
                .ok_or_else(|| ExchangeError::AuthenticationFailed {
                    reason: "Not logged in".to_string(),
                })?;
        let passphrase =
            credentials
                .passphrase
                .as_ref()
                .ok_or_else(|| ExchangeError::AuthenticationFailed {
                    reason: "Missing passphrase".to_string(),
                })?;

        // Spot doesn't have positions
        if matches!(self.market, BitgetMarket::Spot) {
            return Ok(Vec::new());
        }

        let product_type = self.market.product_type();
        let path = format!("/api/v2/mix/position/all-position?productType={product_type}");

        let timestamp = Self::timestamp_ms().to_string();
        let method = "GET";

        let sign = Self::sign_request(&timestamp, method, &path, "", credentials.api_secret());

        let mut request = client.get(&path);

        request = request
            .header("ACCESS-KEY", &credentials.api_key)
            .header("ACCESS-SIGN", &sign)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", passphrase)
            .header("Content-Type", "application/json")
            .header("locale", "en-US");

        let response = request.send().await.map_err(|e| ExchangeError::Unknown {
            code: 0,
            message: format!("Request failed: {e}"),
        })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(ExchangeError::Unknown {
                code: i32::from(status.as_u16()),
                message: body,
            });
        }

        let api_response: BitgetApiResponse<Vec<BitgetPosition>> = serde_json::from_str(&body)
            .map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Failed to parse response: {e}"),
            })?;

        if !api_response.is_success() {
            return Err(Self::parse_error(&api_response.code, &api_response.msg));
        }

        let positions: Vec<Position> = api_response
            .data
            .unwrap_or_default()
            .iter()
            .filter_map(Self::convert_position)
            .collect();

        debug!(
            exchange = "bitget",
            count = positions.len(),
            "Positions queried"
        );

        Ok(positions)
    }

    async fn query_orders(&self, symbol: Option<&Symbol>) -> Result<Vec<Order>, ExchangeError> {
        let client = self.client()?;
        let credentials =
            self.credentials
                .as_ref()
                .ok_or_else(|| ExchangeError::AuthenticationFailed {
                    reason: "Not logged in".to_string(),
                })?;
        let passphrase =
            credentials
                .passphrase
                .as_ref()
                .ok_or_else(|| ExchangeError::AuthenticationFailed {
                    reason: "Missing passphrase".to_string(),
                })?;

        let path = match self.market {
            BitgetMarket::Spot => symbol.map_or_else(
                || "/api/v2/spot/trade/unfilled-orders".to_string(),
                |sym| {
                    format!(
                        "/api/v2/spot/trade/unfilled-orders?symbol={}",
                        Self::to_bitget_symbol(sym)
                    )
                },
            ),
            BitgetMarket::UsdtFutures | BitgetMarket::CoinFutures => {
                let product_type = self.market.product_type();
                symbol.map_or_else(
                    || format!("/api/v2/mix/order/orders-pending?productType={product_type}"),
                    |sym| {
                        format!(
                            "/api/v2/mix/order/orders-pending?productType={product_type}&symbol={}",
                            Self::to_bitget_symbol(sym)
                        )
                    },
                )
            }
        };

        let timestamp = Self::timestamp_ms().to_string();
        let method = "GET";

        let sign = Self::sign_request(&timestamp, method, &path, "", credentials.api_secret());

        let mut request = client.get(&path);

        request = request
            .header("ACCESS-KEY", &credentials.api_key)
            .header("ACCESS-SIGN", &sign)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", passphrase)
            .header("Content-Type", "application/json")
            .header("locale", "en-US");

        let response = request.send().await.map_err(|e| ExchangeError::Unknown {
            code: 0,
            message: format!("Request failed: {e}"),
        })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(ExchangeError::Unknown {
                code: i32::from(status.as_u16()),
                message: body,
            });
        }

        let api_response: BitgetApiResponse<Vec<BitgetOrderDetails>> = serde_json::from_str(&body)
            .map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Failed to parse response: {e}"),
            })?;

        if !api_response.is_success() {
            return Err(Self::parse_error(&api_response.code, &api_response.msg));
        }

        let orders: Vec<Order> = api_response
            .data
            .unwrap_or_default()
            .iter()
            .filter_map(Self::convert_order_details)
            .collect();

        // Update local cache
        {
            let mut cache = self.orders.write();
            for order in &orders {
                cache.insert(order.order_id.clone(), order.clone());
            }
        }

        debug!(exchange = "bitget", count = orders.len(), "Orders queried");

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

        // Order not in cache, need symbol to query
        Err(ExchangeError::OrderNotFound {
            order_id: order_id.to_string(),
        })
    }

    async fn query_account(&self) -> Result<Account, ExchangeError> {
        let client = self.client()?;
        let credentials =
            self.credentials
                .as_ref()
                .ok_or_else(|| ExchangeError::AuthenticationFailed {
                    reason: "Not logged in".to_string(),
                })?;
        let passphrase =
            credentials
                .passphrase
                .as_ref()
                .ok_or_else(|| ExchangeError::AuthenticationFailed {
                    reason: "Missing passphrase".to_string(),
                })?;

        let path = match self.market {
            BitgetMarket::Spot => "/api/v2/spot/account/assets".to_string(),
            BitgetMarket::UsdtFutures => {
                "/api/v2/mix/account/accounts?productType=USDT-FUTURES".to_string()
            }
            BitgetMarket::CoinFutures => {
                "/api/v2/mix/account/accounts?productType=COIN-FUTURES".to_string()
            }
        };

        let timestamp = Self::timestamp_ms().to_string();
        let method = "GET";

        let sign = Self::sign_request(&timestamp, method, &path, "", credentials.api_secret());

        let mut request = client.get(&path);

        request = request
            .header("ACCESS-KEY", &credentials.api_key)
            .header("ACCESS-SIGN", &sign)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", passphrase)
            .header("Content-Type", "application/json")
            .header("locale", "en-US");

        let response = request.send().await.map_err(|e| ExchangeError::Unknown {
            code: 0,
            message: format!("Request failed: {e}"),
        })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(ExchangeError::Unknown {
                code: i32::from(status.as_u16()),
                message: body,
            });
        }

        // Parse based on market type
        match self.market {
            BitgetMarket::Spot => {
                let api_response: BitgetApiResponse<Vec<BitgetBalance>> =
                    serde_json::from_str(&body).map_err(|e| ExchangeError::Unknown {
                        code: 0,
                        message: format!("Failed to parse response: {e}"),
                    })?;

                if !api_response.is_success() {
                    return Err(Self::parse_error(&api_response.code, &api_response.msg));
                }

                let balances: Vec<Balance> = api_response
                    .data
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|b| {
                        let available = Amount::new(b.available.parse().ok()?).ok()?;
                        let frozen = b
                            .frozen
                            .as_ref()
                            .and_then(|f| f.parse::<Decimal>().ok())
                            .and_then(|d| Amount::new(d).ok())
                            .unwrap_or(Amount::ZERO);
                        let total =
                            Amount::new(available.as_decimal() + frozen.as_decimal()).ok()?;
                        Some(Balance::new(&b.coin, total, available, frozen))
                    })
                    .collect();

                let total_equity = balances
                    .iter()
                    .map(|b| b.total.as_decimal())
                    .sum::<Decimal>();

                let available_balance = balances
                    .iter()
                    .map(|b| b.available.as_decimal())
                    .sum::<Decimal>();

                Ok(Account {
                    exchange: Exchange::Bitget,
                    balances,
                    total_equity: Amount::new(total_equity).unwrap_or(Amount::ZERO),
                    available_balance: Amount::new(available_balance).unwrap_or(Amount::ZERO),
                    margin_used: Amount::ZERO,
                    unrealized_pnl: Amount::ZERO,
                    update_time: Timestamp::now(),
                })
            }
            BitgetMarket::UsdtFutures | BitgetMarket::CoinFutures => {
                let api_response: BitgetApiResponse<Vec<BitgetAccount>> =
                    serde_json::from_str(&body).map_err(|e| ExchangeError::Unknown {
                        code: 0,
                        message: format!("Failed to parse response: {e}"),
                    })?;

                if !api_response.is_success() {
                    return Err(Self::parse_error(&api_response.code, &api_response.msg));
                }

                let accounts = api_response.data.unwrap_or_default();

                let balances: Vec<Balance> = accounts
                    .iter()
                    .filter_map(|a| {
                        let available = Amount::new(a.available.parse().ok()?).ok()?;
                        let locked = a
                            .locked
                            .as_ref()
                            .and_then(|l| l.parse::<Decimal>().ok())
                            .and_then(|d| Amount::new(d).ok())
                            .unwrap_or(Amount::ZERO);
                        let equity = a
                            .equity
                            .as_ref()
                            .and_then(|e| e.parse::<Decimal>().ok())
                            .and_then(|d| Amount::new(d).ok())
                            .unwrap_or(available);
                        Some(Balance::new(&a.margin_coin, equity, available, locked))
                    })
                    .collect();

                let total_equity = accounts
                    .first()
                    .and_then(|a| a.usd_equity.as_ref())
                    .and_then(|e| e.parse::<Decimal>().ok())
                    .and_then(|d| Amount::new(d).ok())
                    .unwrap_or(Amount::ZERO);

                let available_balance = accounts
                    .first()
                    .and_then(|a| a.available.parse::<Decimal>().ok())
                    .and_then(|d| Amount::new(d).ok())
                    .unwrap_or(Amount::ZERO);

                let unrealized_pnl = accounts
                    .first()
                    .and_then(|a| a.unrealized_pl.as_ref())
                    .and_then(|p| p.parse::<Decimal>().ok())
                    .and_then(|d| Amount::new(d).ok())
                    .unwrap_or(Amount::ZERO);

                Ok(Account {
                    exchange: Exchange::Bitget,
                    balances,
                    total_equity,
                    available_balance,
                    margin_used: Amount::ZERO,
                    unrealized_pnl,
                    update_time: Timestamp::now(),
                })
            }
        }
    }

    fn set_callback(&mut self, callback: Box<dyn TraderCallback>) {
        self.callback = Some(Arc::from(callback));
    }

    fn exchange(&self) -> &'static str {
        "bitget"
    }
}
