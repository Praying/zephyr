//! Order management handlers.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::{ApiError, ApiResult};
use crate::middleware::Auth;
use crate::response::{ApiResponse, CreatedResponse, EmptyResponse, PaginatedResponse};
use crate::state::{AppState, OrderState};

/// Order list item.
#[derive(Debug, Clone, Serialize)]
pub struct OrderListItem {
    /// Order ID
    pub id: String,
    /// Symbol
    pub symbol: String,
    /// Order side
    pub side: String,
    /// Order type
    pub order_type: String,
    /// Order status
    pub status: String,
    /// Quantity
    pub quantity: Decimal,
    /// Price (for limit orders)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<Decimal>,
    /// Filled quantity
    pub filled_quantity: Decimal,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<&OrderState> for OrderListItem {
    fn from(state: &OrderState) -> Self {
        Self {
            id: state.id.clone(),
            symbol: state.symbol.clone(),
            side: state.side.clone(),
            order_type: state.order_type.clone(),
            status: state.status.clone(),
            quantity: state.quantity,
            price: state.price,
            filled_quantity: state.filled_quantity,
            created_at: state.created_at,
        }
    }
}

/// Create order request.
#[derive(Debug, Deserialize)]
pub struct CreateOrderRequest {
    /// Symbol
    pub symbol: String,
    /// Order side (BUY/SELL)
    pub side: String,
    /// Order type (LIMIT/MARKET)
    pub order_type: String,
    /// Quantity
    pub quantity: Decimal,
    /// Price (required for limit orders)
    #[serde(default)]
    pub price: Option<Decimal>,
    /// Time in force
    #[serde(default = "default_time_in_force")]
    pub time_in_force: String,
    /// Reduce only flag
    #[serde(default)]
    pub reduce_only: bool,
    /// Post only flag
    #[serde(default)]
    pub post_only: bool,
}

fn default_time_in_force() -> String {
    "GTC".to_string()
}

/// Order query parameters.
#[derive(Debug, Deserialize)]
pub struct OrderQuery {
    /// Filter by symbol
    #[serde(default)]
    pub symbol: Option<String>,
    /// Filter by status
    #[serde(default)]
    pub status: Option<String>,
    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page: u32,
    /// Items per page
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    20
}

/// List orders.
///
/// GET /api/v1/orders
pub async fn list_orders(
    State(state): State<Arc<AppState>>,
    Auth(user): Auth,
    Query(query): Query<OrderQuery>,
) -> ApiResult<PaginatedResponse<OrderListItem>> {
    // Check permission
    if !user.role.can_view_positions() {
        return Err(ApiError::Forbidden("Insufficient permissions".to_string()));
    }

    // Filter orders
    let mut orders: Vec<OrderListItem> = state
        .orders
        .iter()
        .filter(|entry| {
            let order = entry.value();
            let symbol_match = query.symbol.as_ref().is_none_or(|s| &order.symbol == s);
            let status_match = query.status.as_ref().is_none_or(|s| &order.status == s);
            symbol_match && status_match
        })
        .map(|entry| OrderListItem::from(entry.value()))
        .collect();

    // Sort by creation time (newest first)
    orders.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let total = orders.len() as u64;

    // Paginate
    let start = ((query.page - 1) * query.per_page) as usize;
    let end = (start + query.per_page as usize).min(orders.len());
    let page_orders = if start < orders.len() {
        orders[start..end].to_vec()
    } else {
        vec![]
    };

    Ok(PaginatedResponse::new(
        page_orders,
        query.page,
        query.per_page,
        total,
    ))
}

/// Create a new order.
///
/// POST /api/v1/orders
pub async fn create_order(
    State(state): State<Arc<AppState>>,
    Auth(user): Auth,
    Json(request): Json<CreateOrderRequest>,
) -> ApiResult<CreatedResponse<OrderListItem>> {
    // Check permission
    if !user.role.can_submit_orders() {
        return Err(ApiError::Forbidden("Insufficient permissions".to_string()));
    }

    // Validate request
    if request.symbol.is_empty() {
        return Err(ApiError::BadRequest("Symbol is required".to_string()));
    }

    let side = request.side.to_uppercase();
    if side != "BUY" && side != "SELL" {
        return Err(ApiError::BadRequest("Side must be BUY or SELL".to_string()));
    }

    let order_type = request.order_type.to_uppercase();
    if order_type != "LIMIT" && order_type != "MARKET" {
        return Err(ApiError::BadRequest(
            "Order type must be LIMIT or MARKET".to_string(),
        ));
    }

    if request.quantity <= Decimal::ZERO {
        return Err(ApiError::BadRequest(
            "Quantity must be positive".to_string(),
        ));
    }

    if order_type == "LIMIT" && request.price.is_none() {
        return Err(ApiError::BadRequest(
            "Price is required for limit orders".to_string(),
        ));
    }

    // Generate order ID
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();

    let order_state = OrderState {
        id: id.clone(),
        symbol: request.symbol.clone(),
        side: side.clone(),
        order_type: order_type.clone(),
        status: "NEW".to_string(),
        quantity: request.quantity,
        price: request.price,
        filled_quantity: Decimal::ZERO,
        created_at: now,
    };

    // Store order
    state.orders.insert(id.clone(), order_state.clone());

    let item = OrderListItem::from(&order_state);

    Ok(CreatedResponse::with_message(
        item,
        "Order created successfully",
    ))
}

/// Get order by ID.
///
/// GET /api/v1/orders/{id}
pub async fn get_order(
    State(state): State<Arc<AppState>>,
    Auth(user): Auth,
    Path(id): Path<String>,
) -> ApiResult<ApiResponse<OrderListItem>> {
    // Check permission
    if !user.role.can_view_positions() {
        return Err(ApiError::Forbidden("Insufficient permissions".to_string()));
    }

    let order = state
        .orders
        .get(&id)
        .ok_or_else(|| ApiError::NotFound(format!("Order not found: {id}")))?;

    Ok(ApiResponse::success(OrderListItem::from(order.value())))
}

/// Cancel an order.
///
/// DELETE /api/v1/orders/{id}
pub async fn cancel_order(
    State(state): State<Arc<AppState>>,
    Auth(user): Auth,
    Path(id): Path<String>,
) -> ApiResult<EmptyResponse> {
    // Check permission
    if !user.role.can_submit_orders() {
        return Err(ApiError::Forbidden("Insufficient permissions".to_string()));
    }

    let mut order = state
        .orders
        .get_mut(&id)
        .ok_or_else(|| ApiError::NotFound(format!("Order not found: {id}")))?;

    // Check if order can be canceled
    if order.status != "NEW" && order.status != "PARTIALLY_FILLED" {
        return Err(ApiError::Conflict(format!(
            "Order cannot be canceled from status: {}",
            order.status
        )));
    }

    order.status = "CANCELED".to_string();
    drop(order);

    Ok(EmptyResponse::success_with_message("Order canceled"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_list_item_from_state() {
        let state = OrderState {
            id: "order-123".to_string(),
            symbol: "BTC-USDT".to_string(),
            side: "BUY".to_string(),
            order_type: "LIMIT".to_string(),
            status: "NEW".to_string(),
            quantity: Decimal::new(1, 1), // 0.1
            price: Some(Decimal::new(50000, 0)),
            filled_quantity: Decimal::ZERO,
            created_at: Utc::now(),
        };

        let item = OrderListItem::from(&state);
        assert_eq!(item.id, "order-123");
        assert_eq!(item.symbol, "BTC-USDT");
        assert_eq!(item.side, "BUY");
    }

    #[test]
    fn test_default_time_in_force() {
        assert_eq!(default_time_in_force(), "GTC");
    }
}
