//! Trade history handlers.

use axum::extract::{Query, State};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::{ApiError, ApiResult};
use crate::middleware::Auth;
use crate::response::PaginatedResponse;
use crate::state::AppState;

/// Trade record.
#[derive(Debug, Serialize)]
pub struct TradeRecord {
    /// Trade ID
    pub id: String,
    /// Order ID
    pub order_id: String,
    /// Symbol
    pub symbol: String,
    /// Trade side (BUY/SELL)
    pub side: String,
    /// Execution price
    pub price: Decimal,
    /// Execution quantity
    pub quantity: Decimal,
    /// Trade value (price * quantity)
    pub value: Decimal,
    /// Trading fee
    pub fee: Decimal,
    /// Fee currency
    pub fee_currency: String,
    /// Execution timestamp
    pub executed_at: DateTime<Utc>,
}

/// Trade query parameters.
#[derive(Debug, Deserialize)]
pub struct TradeQuery {
    /// Filter by symbol
    #[serde(default)]
    pub symbol: Option<String>,
    /// Filter by order ID
    #[serde(default)]
    pub order_id: Option<String>,
    /// Start time filter
    #[serde(default)]
    pub start_time: Option<DateTime<Utc>>,
    /// End time filter
    #[serde(default)]
    pub end_time: Option<DateTime<Utc>>,
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
    50
}

/// List trades.
///
/// GET /api/v1/trades
pub async fn list_trades(
    State(_state): State<Arc<AppState>>,
    Auth(user): Auth,
    Query(query): Query<TradeQuery>,
) -> ApiResult<PaginatedResponse<TradeRecord>> {
    // Check permission
    if !user.role.can_view_positions() {
        return Err(ApiError::Forbidden("Insufficient permissions".to_string()));
    }

    // In a real implementation, this would fetch trades from storage
    // For now, return an empty paginated response
    let trades: Vec<TradeRecord> = vec![];

    Ok(PaginatedResponse::new(
        trades,
        query.page,
        query.per_page,
        0,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trade_query_defaults() {
        assert_eq!(default_page(), 1);
        assert_eq!(default_per_page(), 50);
    }
}
