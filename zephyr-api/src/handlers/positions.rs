//! Position query handlers.

use axum::extract::State;
use rust_decimal::Decimal;
use serde::Serialize;
use std::sync::Arc;

use crate::error::{ApiError, ApiResult};
use crate::middleware::Auth;
use crate::response::ApiResponse;
use crate::state::AppState;

/// Position summary.
#[derive(Debug, Serialize)]
pub struct PositionSummary {
    /// Symbol
    pub symbol: String,
    /// Position side (LONG/SHORT)
    pub side: String,
    /// Position quantity
    pub quantity: Decimal,
    /// Entry price
    pub entry_price: Decimal,
    /// Mark price
    pub mark_price: Decimal,
    /// Unrealized PnL
    pub unrealized_pnl: Decimal,
    /// Realized PnL
    pub realized_pnl: Decimal,
    /// Leverage
    pub leverage: u8,
    /// Margin type (CROSS/ISOLATED)
    pub margin_type: String,
    /// Liquidation price
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidation_price: Option<Decimal>,
}

/// List all positions.
///
/// GET /api/v1/positions
pub async fn list_positions(
    State(_state): State<Arc<AppState>>,
    Auth(user): Auth,
) -> ApiResult<ApiResponse<Vec<PositionSummary>>> {
    // Check permission
    if !user.role.can_view_positions() {
        return Err(ApiError::Forbidden("Insufficient permissions".to_string()));
    }

    // In a real implementation, this would fetch positions from the engine
    // For now, return an empty list
    let positions: Vec<PositionSummary> = vec![];

    Ok(ApiResponse::success(positions))
}

/// Position statistics.
#[derive(Debug, Serialize)]
pub struct PositionStats {
    /// Total position count
    pub total_positions: u32,
    /// Long position count
    pub long_positions: u32,
    /// Short position count
    pub short_positions: u32,
    /// Total unrealized PnL
    pub total_unrealized_pnl: Decimal,
    /// Total realized PnL
    pub total_realized_pnl: Decimal,
    /// Total notional value
    pub total_notional: Decimal,
}

/// Get position statistics.
///
/// GET /api/v1/positions/stats
pub async fn position_stats(
    State(_state): State<Arc<AppState>>,
    Auth(user): Auth,
) -> ApiResult<ApiResponse<PositionStats>> {
    // Check permission
    if !user.role.can_view_positions() {
        return Err(ApiError::Forbidden("Insufficient permissions".to_string()));
    }

    // In a real implementation, this would calculate stats from actual positions
    let stats = PositionStats {
        total_positions: 0,
        long_positions: 0,
        short_positions: 0,
        total_unrealized_pnl: Decimal::ZERO,
        total_realized_pnl: Decimal::ZERO,
        total_notional: Decimal::ZERO,
    };

    Ok(ApiResponse::success(stats))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_stats_default() {
        let stats = PositionStats {
            total_positions: 0,
            long_positions: 0,
            short_positions: 0,
            total_unrealized_pnl: Decimal::ZERO,
            total_realized_pnl: Decimal::ZERO,
            total_notional: Decimal::ZERO,
        };

        assert_eq!(stats.total_positions, 0);
    }
}
