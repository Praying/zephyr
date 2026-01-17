//! Binance trading gateway implementation.
//!
//! Implements the [`TraderGateway`] trait for Binance REST/WebSocket API.

#![allow(clippy::too_many_lines)]
#![allow(clippy::unnecessary_literal_bound)]
#![allow(clippy::disallowed_types)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::or_fun_call)]
#![allow(clippy::map_flatten)]
#![allow(clippy::unused_self)]
#![allow(clippy::if_not_else)]

use async_trait::async_trait;
use parking_lot::RwLock;
use rust_decimal::Decimal;
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

/// Binance trading gateway.
///
/// Provides trading functionality for Binance exchange including:
/// - Order submission and cancellation
/// - Position and order queries
/// - Account information
///
/// # Example
///
/// ```ignore
/// use zephyr_gateway::binance::BinanceTrader;
/// use zephyr_core::traits::{Credentials, TraderGateway};
///
/// let mut trader = BinanceTrader::new_futures();
/// let creds = Credentials::new("api_key", "api_secret");
/// trader.login(&creds).await?;
///
/// let positions = trader.query_positions().await?;
/// ```
pub struct BinanceTrader {
    /// Market type
    market: BinanceMarket,
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
    /// Use testnet
    testnet: bool,
}

impl BinanceTrader {
    /// Creates a new Binance trader for spot market.
    #[must_use]
    pub fn new() -> Self {
        Self::with_market(BinanceMarket::Spot)
    }

    /// Creates a new Binance trader for USDT futures.
    #[must_use]
    pub fn new_futures() -> Self {
        Self::with_market(BinanceMarket::UsdtFutures)
    }

    /// Creates a new Binance trader with specified market type.
    #[must_use]
    pub fn with_market(market: BinanceMarket) -> Self {
        Self {
            market,
            rest_client: None,
            credentials: None,
            callback: None,
            logged_in: Arc::new(AtomicBool::new(false)),
            orders: Arc::new(RwLock::new(HashMap::new())),
            testnet: false,
        }
    }

    /// Sets whether to use testnet.
    #[must_use]
    pub fn with_testnet(mut self, testnet: bool) -> Self {
        self.testnet = testnet;
        self
    }

    /// Returns the market type.
    #[must_use]
    pub fn market(&self) -> BinanceMarket {
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

    /// Converts a symbol to Binance format.
    fn to_binance_symbol(symbol: &Symbol) -> String {
        symbol.as_str().replace('-', "").to_uppercase()
    }

    /// Converts a Binance symbol to standard format.
    fn from_binance_symbol(binance_symbol: &str) -> Option<Symbol> {
        let quotes = ["USDT", "BUSD", "USDC", "BTC", "ETH", "BNB"];
        let upper = binance_symbol.to_uppercase();

        for quote in quotes {
            if upper.ends_with(quote) {
                let base = &upper[..upper.len() - quote.len()];
                if !base.is_empty() {
                    return Symbol::new(format!("{base}-{quote}")).ok();
                }
            }
        }
        Symbol::new(binance_symbol).ok()
    }

    /// Converts order side to Binance format.
    fn to_binance_side(side: OrderSide) -> BinanceOrderSide {
        match side {
            OrderSide::Buy => BinanceOrderSide::Buy,
            OrderSide::Sell => BinanceOrderSide::Sell,
        }
    }

    /// Converts Binance order side to standard format.
    fn from_binance_side(side: BinanceOrderSide) -> OrderSide {
        match side {
            BinanceOrderSide::Buy => OrderSide::Buy,
            BinanceOrderSide::Sell => OrderSide::Sell,
        }
    }

    /// Converts order type to Binance format.
    fn to_binance_order_type(order_type: OrderType) -> BinanceOrderType {
        match order_type {
            OrderType::Limit => BinanceOrderType::Limit,
            OrderType::Market => BinanceOrderType::Market,
            OrderType::StopLimit => BinanceOrderType::StopLossLimit,
            OrderType::StopMarket => BinanceOrderType::StopLoss,
            OrderType::TakeProfit => BinanceOrderType::TakeProfitLimit,
            OrderType::TakeProfitMarket => BinanceOrderType::TakeProfit,
        }
    }

    /// Converts Binance order type to standard format.
    fn from_binance_order_type(order_type: BinanceOrderType) -> OrderType {
        match order_type {
            BinanceOrderType::Limit | BinanceOrderType::LimitMaker => OrderType::Limit,
            BinanceOrderType::Market => OrderType::Market,
            BinanceOrderType::StopLoss => OrderType::StopMarket,
            BinanceOrderType::StopLossLimit => OrderType::StopLimit,
            BinanceOrderType::TakeProfit => OrderType::TakeProfitMarket,
            BinanceOrderType::TakeProfitLimit => OrderType::TakeProfit,
        }
    }

    /// Converts Binance order status to standard format.
    fn from_binance_status(status: BinanceOrderStatus) -> OrderStatus {
        match status {
            BinanceOrderStatus::New => OrderStatus::New,
            BinanceOrderStatus::PartiallyFilled => OrderStatus::PartiallyFilled,
            BinanceOrderStatus::Filled => OrderStatus::Filled,
            BinanceOrderStatus::Canceled | BinanceOrderStatus::PendingCancel => {
                OrderStatus::Canceled
            }
            BinanceOrderStatus::Rejected => OrderStatus::Rejected,
            BinanceOrderStatus::Expired | BinanceOrderStatus::ExpiredInMatch => {
                OrderStatus::Expired
            }
        }
    }

    /// Converts time in force to Binance format.
    fn to_binance_tif(tif: TimeInForce) -> BinanceTimeInForce {
        match tif {
            TimeInForce::Gtc => BinanceTimeInForce::Gtc,
            TimeInForce::Ioc => BinanceTimeInForce::Ioc,
            TimeInForce::Fok => BinanceTimeInForce::Fok,
            TimeInForce::Gtd => BinanceTimeInForce::Gtd,
            TimeInForce::PostOnly => BinanceTimeInForce::Gtx,
        }
    }

    /// Converts Binance time in force to standard format.
    fn from_binance_tif(tif: BinanceTimeInForce) -> TimeInForce {
        match tif {
            BinanceTimeInForce::Gtc => TimeInForce::Gtc,
            BinanceTimeInForce::Ioc => TimeInForce::Ioc,
            BinanceTimeInForce::Fok => TimeInForce::Fok,
            BinanceTimeInForce::Gtd => TimeInForce::Gtd,
            BinanceTimeInForce::Gtx => TimeInForce::PostOnly,
        }
    }

    /// Returns the current timestamp in milliseconds.
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

    /// Parses a Binance API error response.
    fn parse_error(status: u16, body: &str) -> ExchangeError {
        if let Ok(error) = serde_json::from_str::<BinanceApiError>(body) {
            match error.code {
                -1000 => ExchangeError::Unknown {
                    code: error.code,
                    message: error.msg,
                },
                -1002 | -2015 => ExchangeError::AuthenticationFailed { reason: error.msg },
                -1003 | -1015 => ExchangeError::RateLimited {
                    retry_after_ms: 60_000,
                },
                -2010 => ExchangeError::InsufficientBalance {
                    required: Decimal::ZERO,
                    available: Decimal::ZERO,
                },
                -2011 => ExchangeError::OrderNotFound {
                    order_id: String::new(),
                },
                -1021 => ExchangeError::InvalidParameter {
                    param: "timestamp".to_string(),
                    reason: error.msg,
                },
                _ => ExchangeError::Unknown {
                    code: error.code,
                    message: error.msg,
                },
            }
        } else {
            ExchangeError::Unknown {
                code: status as i32,
                message: body.to_string(),
            }
        }
    }

    /// Converts Binance order response to standard Order.
    fn convert_order_response(&self, resp: &BinanceOrderResponse) -> Option<Order> {
        let symbol = Self::from_binance_symbol(&resp.symbol)?;
        let order_id = OrderId::new(resp.order_id.to_string()).ok()?;
        let status = Self::from_binance_status(resp.status);
        let side = Self::from_binance_side(resp.side);
        let order_type = Self::from_binance_order_type(resp.order_type);
        let tif = resp
            .time_in_force
            .map(Self::from_binance_tif)
            .unwrap_or_default();

        let price = resp
            .price
            .as_ref()
            .and_then(|p| p.parse::<Decimal>().ok().and_then(|d| Price::new(d).ok()));
        let quantity = resp.orig_qty.as_ref().and_then(|q| {
            q.parse::<Decimal>()
                .ok()
                .and_then(|d| Quantity::new(d).ok())
        })?;
        let filled_quantity = resp
            .executed_qty
            .as_ref()
            .and_then(|q| {
                q.parse::<Decimal>()
                    .ok()
                    .and_then(|d| Quantity::new(d).ok())
            })
            .unwrap_or(Quantity::ZERO);
        let avg_price = resp
            .avg_price
            .as_ref()
            .and_then(|p| p.parse::<Decimal>().ok().and_then(|d| Price::new(d).ok()));

        let create_time = resp
            .transact_time
            .map(|t| Timestamp::new(t).ok())
            .flatten()
            .unwrap_or_else(Timestamp::now);
        let update_time = resp
            .update_time
            .map(|t| Timestamp::new(t).ok())
            .flatten()
            .unwrap_or(create_time);

        Some(Order {
            order_id,
            client_order_id: Some(resp.client_order_id.clone()),
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
            reduce_only: resp.reduce_only.unwrap_or(false),
            post_only: matches!(tif, TimeInForce::PostOnly),
            create_time,
            update_time,
        })
    }

    /// Converts Binance position to standard Position.
    fn convert_position(&self, pos: &BinancePosition) -> Option<Position> {
        let symbol = Self::from_binance_symbol(&pos.symbol)?;
        let quantity = Quantity::new(pos.position_amt.parse().ok()?).ok()?;

        // Skip empty positions
        if quantity.is_zero() {
            return None;
        }

        let entry_price = Price::new(pos.entry_price.parse().ok()?).ok()?;
        let mark_price = MarkPrice::new(pos.mark_price.parse().ok()?).ok()?;
        let unrealized_pnl = pos.un_realized_profit.parse().ok()?;
        let realized_pnl = pos.realized_profit.parse().ok()?;
        let leverage = Some(pos.leverage.parse().ok()?);

        let liquidation_price = if pos.liquidation_price != "0" {
            Some(Price::new(pos.liquidation_price.parse().ok()?).ok()?)
        } else {
            None
        };

        let side = match pos.position_side {
            BinancePositionSide::Long => PositionSide::Long,
            BinancePositionSide::Short => PositionSide::Short,
            BinancePositionSide::Both => PositionSide::Both,
        };

        let margin_type = match pos.margin_type {
            BinanceMarginType::Cross => Some(MarginType::Cross),
            BinanceMarginType::Isolated => Some(MarginType::Isolated),
        };

        let update_time = Some(Timestamp::new(pos.update_time).ok()?);

        Some(Position {
            symbol,
            side,
            quantity: Quantity::new(quantity.as_decimal().abs()).ok()?,
            entry_price,
            mark_price,
            liquidation_price,
            unrealized_pnl: Some(unrealized_pnl.as_decimal()),
            realized_pnl: Some(realized_pnl.as_decimal()),
            leverage,
            margin_type,
            initial_margin: None,
            maintenance_margin: None,
            update_time,
        })
    }
}

impl Default for BinanceTrader {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TraderGateway for BinanceTrader {
    async fn login(&mut self, credentials: &Credentials) -> Result<(), ExchangeError> {
        let base_url = self.base_url();

        let rest_config = RestConfig::builder()
            .base_url(base_url)
            .exchange("binance")
            .api_key(&credentials.api_key)
            .api_secret(credentials.api_secret())
            .build();

        let rest_client =
            RestClient::new(rest_config).map_err(|e| ExchangeError::AuthenticationFailed {
                reason: format!("Failed to create REST client: {e}"),
            })?;

        // Test authentication by querying account
        let timestamp = Self::timestamp_ms();
        let response = rest_client
            .get(match self.market {
                BinanceMarket::Spot => "/api/v3/account",
                BinanceMarket::UsdtFutures => "/fapi/v2/account",
                BinanceMarket::CoinFutures => "/dapi/v1/account",
            })
            .query("timestamp", timestamp.to_string())
            .query("recvWindow", "5000")
            .signed()
            .send()
            .await
            .map_err(|e| ExchangeError::AuthenticationFailed {
                reason: format!("Failed to authenticate: {e}"),
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Self::parse_error(status.as_u16(), &body));
        }

        self.rest_client = Some(rest_client);
        self.credentials = Some(credentials.clone());
        self.logged_in.store(true, Ordering::SeqCst);

        if let Some(callback) = &self.callback {
            callback.on_connected().await;
        }

        info!(
            exchange = "binance",
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

        info!(exchange = "binance", "Trader logged out");
        Ok(())
    }

    fn is_logged_in(&self) -> bool {
        self.logged_in.load(Ordering::SeqCst)
    }

    async fn order_insert(&self, order: &OrderRequest) -> Result<OrderId, ExchangeError> {
        let client = self.client()?;
        let timestamp = Self::timestamp_ms();

        let binance_symbol = Self::to_binance_symbol(&order.symbol);
        let side = Self::to_binance_side(order.side);
        let order_type = Self::to_binance_order_type(order.order_type);
        let tif = Self::to_binance_tif(order.time_in_force);

        let endpoint = match self.market {
            BinanceMarket::Spot => "/api/v3/order",
            BinanceMarket::UsdtFutures => "/fapi/v1/order",
            BinanceMarket::CoinFutures => "/dapi/v1/order",
        };

        let mut request = client
            .post(endpoint)
            .query("symbol", &binance_symbol)
            .query("side", format!("{side:?}").to_uppercase())
            .query("type", format!("{order_type:?}").to_uppercase())
            .query("quantity", order.quantity.to_string())
            .query("timestamp", timestamp.to_string())
            .query("recvWindow", "5000");

        // Add price for limit orders
        if order.order_type.requires_price() {
            request = request.query("price", order.price.to_string());
        }

        // Add time in force for limit orders
        if matches!(order.order_type, OrderType::Limit) {
            request = request.query("timeInForce", format!("{tif:?}").to_uppercase());
        }

        // Add stop price for stop orders
        if order.order_type.requires_stop_price() {
            request = request.query("stopPrice", order.stop_price.to_string());
        }

        // Add reduce only for futures
        if matches!(
            self.market,
            BinanceMarket::UsdtFutures | BinanceMarket::CoinFutures
        ) {
            if order.reduce_only {
                request = request.query("reduceOnly", "true");
            }
        }

        // Add client order ID if provided
        if let Some(ref client_order_id) = order.client_order_id {
            request = request.query("newClientOrderId", client_order_id);
        }

        let response = request
            .signed()
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

        let order_response: BinanceOrderResponse =
            serde_json::from_str(&body).map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Failed to parse response: {e}"),
            })?;

        let order_id = OrderId::new(order_response.order_id.to_string()).map_err(|_| {
            ExchangeError::Unknown {
                code: 0,
                message: "Invalid order ID returned from exchange".to_string(),
            }
        })?;

        // Update local cache
        if let Some(converted) = self.convert_order_response(&order_response) {
            self.orders
                .write()
                .insert(order_id.clone(), converted.clone());

            if let Some(callback) = &self.callback {
                callback.on_order(&converted).await;
            }
        }

        info!(
            exchange = "binance",
            order_id = %order_id,
            symbol = %order.symbol,
            side = ?order.side,
            "Order submitted"
        );

        Ok(order_id)
    }

    async fn order_cancel(&self, order_id: &OrderId) -> Result<bool, ExchangeError> {
        let client = self.client()?;
        let timestamp = Self::timestamp_ms();

        // Get symbol from local cache
        let symbol = {
            let orders = self.orders.read();
            orders.get(order_id).map(|o| o.symbol.clone())
        };

        let symbol = symbol.ok_or(ExchangeError::OrderNotFound {
            order_id: order_id.to_string(),
        })?;

        let binance_symbol = Self::to_binance_symbol(&symbol);

        let endpoint = match self.market {
            BinanceMarket::Spot => "/api/v3/order",
            BinanceMarket::UsdtFutures => "/fapi/v1/order",
            BinanceMarket::CoinFutures => "/dapi/v1/order",
        };

        let response = client
            .delete(endpoint)
            .query("symbol", &binance_symbol)
            .query("orderId", order_id.as_str())
            .query("timestamp", timestamp.to_string())
            .query("recvWindow", "5000")
            .signed()
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

        let cancel_response: BinanceCancelResponse =
            serde_json::from_str(&body).map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Failed to parse response: {e}"),
            })?;

        // Update local cache and get order for callback
        let updated_order = {
            let mut orders = self.orders.write();
            if let Some(order) = orders.get_mut(order_id) {
                order.status = Self::from_binance_status(cancel_response.status);
                order.update_time = Timestamp::now();
                Some(order.clone())
            } else {
                None
            }
        };

        // Call callback outside the lock
        if let (Some(callback), Some(order)) = (&self.callback, updated_order) {
            callback.on_order(&order).await;
        }

        info!(
            exchange = "binance",
            order_id = %order_id,
            "Order canceled"
        );

        Ok(true)
    }

    async fn cancel_all_orders(&self, symbol: Option<&Symbol>) -> Result<usize, ExchangeError> {
        let client = self.client()?;
        let timestamp = Self::timestamp_ms();

        let endpoint = match self.market {
            BinanceMarket::Spot => "/api/v3/openOrders",
            BinanceMarket::UsdtFutures => "/fapi/v1/allOpenOrders",
            BinanceMarket::CoinFutures => "/dapi/v1/allOpenOrders",
        };

        let mut request = client
            .delete(endpoint)
            .query("timestamp", timestamp.to_string())
            .query("recvWindow", "5000");

        if let Some(sym) = symbol {
            request = request.query("symbol", Self::to_binance_symbol(sym));
        }

        let response = request
            .signed()
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

        // Parse response to count canceled orders
        let canceled: Vec<BinanceCancelResponse> = serde_json::from_str(&body).unwrap_or_default();
        let count = canceled.len();

        info!(
            exchange = "binance",
            count = count,
            symbol = ?symbol,
            "All orders canceled"
        );

        Ok(count)
    }

    async fn query_positions(&self) -> Result<Vec<Position>, ExchangeError> {
        let client = self.client()?;
        let timestamp = Self::timestamp_ms();

        // Spot doesn't have positions in the same sense
        if matches!(self.market, BinanceMarket::Spot) {
            return Ok(Vec::new());
        }

        let endpoint = match self.market {
            BinanceMarket::UsdtFutures => "/fapi/v2/positionRisk",
            BinanceMarket::CoinFutures => "/dapi/v1/positionRisk",
            BinanceMarket::Spot => return Ok(Vec::new()),
        };

        let response = client
            .get(endpoint)
            .query("timestamp", timestamp.to_string())
            .query("recvWindow", "5000")
            .signed()
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

        let binance_positions: Vec<BinancePosition> =
            serde_json::from_str(&body).map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Failed to parse response: {e}"),
            })?;

        let positions: Vec<Position> = binance_positions
            .iter()
            .filter_map(|p| self.convert_position(p))
            .collect();

        debug!(
            exchange = "binance",
            count = positions.len(),
            "Positions queried"
        );

        Ok(positions)
    }

    async fn query_orders(&self, symbol: Option<&Symbol>) -> Result<Vec<Order>, ExchangeError> {
        let client = self.client()?;
        let timestamp = Self::timestamp_ms();

        let endpoint = match self.market {
            BinanceMarket::Spot => "/api/v3/openOrders",
            BinanceMarket::UsdtFutures => "/fapi/v1/openOrders",
            BinanceMarket::CoinFutures => "/dapi/v1/openOrders",
        };

        let mut request = client
            .get(endpoint)
            .query("timestamp", timestamp.to_string())
            .query("recvWindow", "5000");

        if let Some(sym) = symbol {
            request = request.query("symbol", Self::to_binance_symbol(sym));
        }

        let response = request
            .signed()
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

        let binance_orders: Vec<BinanceOrderResponse> =
            serde_json::from_str(&body).map_err(|e| ExchangeError::Unknown {
                code: 0,
                message: format!("Failed to parse response: {e}"),
            })?;

        let orders: Vec<Order> = binance_orders
            .iter()
            .filter_map(|o| self.convert_order_response(o))
            .collect();

        // Update local cache
        {
            let mut cache = self.orders.write();
            for order in &orders {
                cache.insert(order.order_id.clone(), order.clone());
            }
        }

        debug!(exchange = "binance", count = orders.len(), "Orders queried");

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
        let timestamp = Self::timestamp_ms();

        let endpoint = match self.market {
            BinanceMarket::Spot => "/api/v3/account",
            BinanceMarket::UsdtFutures => "/fapi/v2/account",
            BinanceMarket::CoinFutures => "/dapi/v1/account",
        };

        let response = client
            .get(endpoint)
            .query("timestamp", timestamp.to_string())
            .query("recvWindow", "5000")
            .signed()
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

        // Parse based on market type
        if matches!(
            self.market,
            BinanceMarket::UsdtFutures | BinanceMarket::CoinFutures
        ) {
            let futures_account: BinanceFuturesAccount =
                serde_json::from_str(&body).map_err(|e| ExchangeError::Unknown {
                    code: 0,
                    message: format!("Failed to parse response: {e}"),
                })?;

            let balances: Vec<Balance> = futures_account
                .assets
                .iter()
                .filter_map(|b| {
                    let total = Amount::new(b.balance.parse().ok()?).ok()?;
                    let available = Amount::new(b.available_balance.parse().ok()?).ok()?;
                    let frozen = Amount::new(
                        (total.as_decimal() - available.as_decimal()).max(Decimal::ZERO),
                    )
                    .ok()?;

                    Some(Balance::new_with_frozen(&b.asset, total, available, frozen))
                })
                .collect();

            let total_equity = futures_account
                .total_margin_balance
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

            let available_balance = futures_account
                .available_balance
                .parse::<Decimal>()
                .map_err(|_| ExchangeError::Unknown {
                    code: 0,
                    message: "Failed to parse available balance".to_string(),
                })
                .and_then(|d| {
                    Amount::new(d).map_err(|_| ExchangeError::Unknown {
                        code: 0,
                        message: "Invalid available balance value".to_string(),
                    })
                })?;

            let margin_used = futures_account
                .total_initial_margin
                .parse::<Decimal>()
                .map_err(|_| ExchangeError::Unknown {
                    code: 0,
                    message: "Failed to parse margin used".to_string(),
                })
                .and_then(|d| {
                    Amount::new(d).map_err(|_| ExchangeError::Unknown {
                        code: 0,
                        message: "Invalid margin used value".to_string(),
                    })
                })?;

            let unrealized_pnl = futures_account
                .total_unrealized_profit
                .parse::<Decimal>()
                .map_err(|_| ExchangeError::Unknown {
                    code: 0,
                    message: "Failed to parse unrealized PnL".to_string(),
                })
                .and_then(|d| {
                    Amount::new(d).map_err(|_| ExchangeError::Unknown {
                        code: 0,
                        message: "Invalid unrealized PnL value".to_string(),
                    })
                })?;

            Ok(Account {
                exchange: Exchange::Binance,
                balance: balances,
                total_equity: Some(total_equity.into()),
                available_balance: Some(available_balance.into()),
                margin_used: Some(margin_used.into()),
                unrealized_pnl: Some(unrealized_pnl.into()),
                update_time: Timestamp::new(futures_account.update_time)
                    .unwrap_or_else(|_| Timestamp::now()),
            })
        } else {
            // Spot account - simplified
            Ok(Account {
                exchange: Exchange::Binance,
                balance: Vec::new(),
                total_equity: Decimal::ZERO,
                available_balance: Decimal::ZERO,
                margin_used: Decimal::ZERO,
                unrealized_pnl: Decimal::ZERO,
                update_time: Timestamp::now(),
            })
        }
    }

    fn set_callback(&mut self, callback: Box<dyn TraderCallback>) {
        self.callback = Some(Arc::from(callback));
    }

    fn exchange(&self) -> &str {
        "binance"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_binance_symbol() {
        assert_eq!(
            BinanceTrader::to_binance_symbol(&Symbol::new("BTC-USDT").unwrap()),
            "BTCUSDT"
        );
        assert_eq!(
            BinanceTrader::to_binance_symbol(&Symbol::new("ETH-BTC").unwrap()),
            "ETHBTC"
        );
    }

    #[test]
    fn test_from_binance_symbol() {
        let symbol = BinanceTrader::from_binance_symbol("BTCUSDT").unwrap();
        assert_eq!(symbol.as_str(), "BTC-USDT");

        let symbol = BinanceTrader::from_binance_symbol("ETHBTC").unwrap();
        assert_eq!(symbol.as_str(), "ETH-BTC");
    }

    #[test]
    fn test_side_conversion() {
        assert!(matches!(
            BinanceTrader::to_binance_side(OrderSide::Buy),
            BinanceOrderSide::Buy
        ));
        assert!(matches!(
            BinanceTrader::to_binance_side(OrderSide::Sell),
            BinanceOrderSide::Sell
        ));

        assert!(matches!(
            BinanceTrader::from_binance_side(BinanceOrderSide::Buy),
            OrderSide::Buy
        ));
        assert!(matches!(
            BinanceTrader::from_binance_side(BinanceOrderSide::Sell),
            OrderSide::Sell
        ));
    }

    #[test]
    fn test_order_type_conversion() {
        assert!(matches!(
            BinanceTrader::to_binance_order_type(OrderType::Limit),
            BinanceOrderType::Limit
        ));
        assert!(matches!(
            BinanceTrader::to_binance_order_type(OrderType::Market),
            BinanceOrderType::Market
        ));

        assert!(matches!(
            BinanceTrader::from_binance_order_type(BinanceOrderType::Limit),
            OrderType::Limit
        ));
        assert!(matches!(
            BinanceTrader::from_binance_order_type(BinanceOrderType::Market),
            OrderType::Market
        ));
    }

    #[test]
    fn test_status_conversion() {
        assert!(matches!(
            BinanceTrader::from_binance_status(BinanceOrderStatus::New),
            OrderStatus::New
        ));
        assert!(matches!(
            BinanceTrader::from_binance_status(BinanceOrderStatus::Filled),
            OrderStatus::Filled
        ));
        assert!(matches!(
            BinanceTrader::from_binance_status(BinanceOrderStatus::Canceled),
            OrderStatus::Canceled
        ));
    }

    #[test]
    fn test_tif_conversion() {
        assert!(matches!(
            BinanceTrader::to_binance_tif(TimeInForce::Gtc),
            BinanceTimeInForce::Gtc
        ));
        assert!(matches!(
            BinanceTrader::to_binance_tif(TimeInForce::Ioc),
            BinanceTimeInForce::Ioc
        ));
        assert!(matches!(
            BinanceTrader::to_binance_tif(TimeInForce::PostOnly),
            BinanceTimeInForce::Gtx
        ));

        assert!(matches!(
            BinanceTrader::from_binance_tif(BinanceTimeInForce::Gtc),
            TimeInForce::Gtc
        ));
        assert!(matches!(
            BinanceTrader::from_binance_tif(BinanceTimeInForce::Gtx),
            TimeInForce::PostOnly
        ));
    }

    #[test]
    fn test_parse_error() {
        let body = r#"{"code": -1003, "msg": "Too many requests"}"#;
        let error = BinanceTrader::parse_error(429, body);
        assert!(matches!(error, ExchangeError::RateLimited { .. }));

        let body = r#"{"code": -2015, "msg": "Invalid API-key"}"#;
        let error = BinanceTrader::parse_error(401, body);
        assert!(matches!(error, ExchangeError::AuthenticationFailed { .. }));

        let body = r#"{"code": -2010, "msg": "Insufficient balance"}"#;
        let error = BinanceTrader::parse_error(400, body);
        assert!(matches!(error, ExchangeError::InsufficientBalance { .. }));
    }

    #[test]
    fn test_trader_default() {
        let trader = BinanceTrader::default();
        assert_eq!(trader.market(), BinanceMarket::Spot);
        assert!(!trader.is_logged_in());
    }

    #[test]
    fn test_trader_futures() {
        let trader = BinanceTrader::new_futures();
        assert_eq!(trader.market(), BinanceMarket::UsdtFutures);
    }

    #[test]
    fn test_trader_testnet() {
        let trader = BinanceTrader::new().with_testnet(true);
        assert_eq!(trader.base_url(), "https://testnet.binance.vision");

        let trader = BinanceTrader::new_futures().with_testnet(true);
        assert_eq!(trader.base_url(), "https://testnet.binancefuture.com");
    }

    #[test]
    fn test_convert_order_response() {
        let trader = BinanceTrader::new();
        let response = BinanceOrderResponse {
            symbol: "BTCUSDT".to_string(),
            order_id: 12345,
            order_list_id: None,
            client_order_id: "test123".to_string(),
            transact_time: Some(1672515782136),
            price: Some("42000.00".to_string()),
            orig_qty: Some("0.1".to_string()),
            executed_qty: Some("0.05".to_string()),
            cummulative_quote_qty: Some("2100.00".to_string()),
            status: BinanceOrderStatus::PartiallyFilled,
            time_in_force: Some(BinanceTimeInForce::Gtc),
            order_type: BinanceOrderType::Limit,
            side: BinanceOrderSide::Buy,
            update_time: None,
            avg_price: Some("42000.00".to_string()),
            position_side: None,
            reduce_only: None,
        };

        let order = trader.convert_order_response(&response).unwrap();
        assert_eq!(order.symbol.as_str(), "BTC-USDT");
        assert_eq!(order.order_id.as_str(), "12345");
        assert_eq!(order.status, OrderStatus::PartiallyFilled);
        assert_eq!(order.side, OrderSide::Buy);
    }
}
