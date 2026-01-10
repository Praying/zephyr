//! Account information handlers.

use axum::extract::State;
use rust_decimal::Decimal;
use serde::Serialize;
use std::sync::Arc;

use crate::error::{ApiError, ApiResult};
use crate::middleware::Auth;
use crate::response::ApiResponse;
use crate::state::AppState;

/// Account summary.
#[derive(Debug, Serialize)]
pub struct AccountSummary {
    /// Total equity
    pub total_equity: Decimal,
    /// Available balance
    pub available_balance: Decimal,
    /// Margin used
    pub margin_used: Decimal,
    /// Unrealized PnL
    pub unrealized_pnl: Decimal,
    /// Realized PnL (today)
    pub realized_pnl_today: Decimal,
    /// Margin ratio
    pub margin_ratio: Decimal,
    /// Currency balances
    pub balances: Vec<BalanceSummary>,
}

/// Balance summary for a currency.
#[derive(Debug, Serialize)]
pub struct BalanceSummary {
    /// Currency code
    pub currency: String,
    /// Total balance
    pub total: Decimal,
    /// Available balance
    pub available: Decimal,
    /// Frozen balance
    pub frozen: Decimal,
}

/// Get account summary.
///
/// GET /api/v1/account
pub async fn get_account(
    State(_state): State<Arc<AppState>>,
    Auth(user): Auth,
) -> ApiResult<ApiResponse<AccountSummary>> {
    // Check permission
    if !user.role.can_view_positions() {
        return Err(ApiError::Forbidden("Insufficient permissions".to_string()));
    }

    // In a real implementation, this would fetch account data from the engine
    let account = AccountSummary {
        total_equity: Decimal::ZERO,
        available_balance: Decimal::ZERO,
        margin_used: Decimal::ZERO,
        unrealized_pnl: Decimal::ZERO,
        realized_pnl_today: Decimal::ZERO,
        margin_ratio: Decimal::ZERO,
        balances: vec![],
    };

    Ok(ApiResponse::success(account))
}

/// Account risk metrics.
#[derive(Debug, Serialize)]
pub struct RiskMetrics {
    /// Current margin ratio
    pub margin_ratio: Decimal,
    /// Maximum allowed margin ratio
    pub max_margin_ratio: Decimal,
    /// Current drawdown
    pub current_drawdown: Decimal,
    /// Maximum allowed drawdown
    pub max_drawdown: Decimal,
    /// Daily loss
    pub daily_loss: Decimal,
    /// Daily loss limit
    pub daily_loss_limit: Decimal,
    /// Position count
    pub position_count: u32,
    /// Maximum positions allowed
    pub max_positions: u32,
}

/// Get account risk metrics.
///
/// GET /api/v1/account/risk
pub async fn get_risk_metrics(
    State(_state): State<Arc<AppState>>,
    Auth(user): Auth,
) -> ApiResult<ApiResponse<RiskMetrics>> {
    // Check permission
    if !user.role.can_view_positions() {
        return Err(ApiError::Forbidden("Insufficient permissions".to_string()));
    }

    // In a real implementation, this would fetch risk metrics from the risk controller
    let metrics = RiskMetrics {
        margin_ratio: Decimal::ZERO,
        max_margin_ratio: Decimal::new(80, 2), // 0.80
        current_drawdown: Decimal::ZERO,
        max_drawdown: Decimal::new(20, 2), // 0.20
        daily_loss: Decimal::ZERO,
        daily_loss_limit: Decimal::new(10000, 0),
        position_count: 0,
        max_positions: 100,
    };

    Ok(ApiResponse::success(metrics))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_summary_default() {
        let account = AccountSummary {
            total_equity: Decimal::ZERO,
            available_balance: Decimal::ZERO,
            margin_used: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            realized_pnl_today: Decimal::ZERO,
            margin_ratio: Decimal::ZERO,
            balances: vec![],
        };

        assert!(account.balances.is_empty());
    }
}
