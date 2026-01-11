//! Strategy management handlers.

use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::{ApiError, ApiResult};
use crate::middleware::Auth;
use crate::response::{ApiResponse, CreatedResponse, EmptyResponse};
use crate::state::{AppState, StrategyState, StrategyStatus, StrategyType};

/// Strategy list item.
#[derive(Debug, Serialize)]
pub struct StrategyListItem {
    /// Strategy ID
    pub id: String,
    /// Strategy name
    pub name: String,
    /// Strategy type
    pub strategy_type: StrategyType,
    /// Current status
    pub status: StrategyStatus,
    /// Associated symbols
    pub symbols: Vec<String>,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last update timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<&StrategyState> for StrategyListItem {
    fn from(state: &StrategyState) -> Self {
        Self {
            id: state.id.clone(),
            name: state.name.clone(),
            strategy_type: state.strategy_type,
            status: state.status,
            symbols: state.symbols.clone(),
            created_at: state.created_at,
            updated_at: state.updated_at,
        }
    }
}

/// Create strategy request.
#[derive(Debug, Deserialize)]
pub struct CreateStrategyRequest {
    /// Strategy name
    pub name: String,
    /// Strategy type
    pub strategy_type: StrategyType,
    /// Associated symbols
    pub symbols: Vec<String>,
    /// Strategy configuration (JSON)
    #[serde(default)]
    pub config: serde_json::Value,
}

/// Strategy detail response.
#[derive(Debug, Serialize)]
pub struct StrategyDetail {
    /// Strategy ID
    pub id: String,
    /// Strategy name
    pub name: String,
    /// Strategy type
    pub strategy_type: StrategyType,
    /// Current status
    pub status: StrategyStatus,
    /// Associated symbols
    pub symbols: Vec<String>,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last update timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// List all strategies.
///
/// GET /api/v1/strategies
pub async fn list_strategies(
    State(state): State<Arc<AppState>>,
    Auth(user): Auth,
) -> ApiResult<ApiResponse<Vec<StrategyListItem>>> {
    // Check permission
    if !user.role.can_view_positions() {
        return Err(ApiError::Forbidden("Insufficient permissions".to_string()));
    }

    let strategies: Vec<StrategyListItem> = state
        .strategies
        .iter()
        .map(|entry| StrategyListItem::from(entry.value()))
        .collect();

    Ok(ApiResponse::success(strategies))
}

/// Create a new strategy.
///
/// POST /api/v1/strategies
pub async fn create_strategy(
    State(state): State<Arc<AppState>>,
    Auth(user): Auth,
    Json(request): Json<CreateStrategyRequest>,
) -> ApiResult<CreatedResponse<StrategyDetail>> {
    // Check permission
    if !user.role.can_manage_strategies() {
        return Err(ApiError::Forbidden("Insufficient permissions".to_string()));
    }

    // Validate request
    if request.name.is_empty() {
        return Err(ApiError::BadRequest(
            "Strategy name is required".to_string(),
        ));
    }

    if request.symbols.is_empty() {
        return Err(ApiError::BadRequest(
            "At least one symbol is required".to_string(),
        ));
    }

    // Generate ID
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();

    let strategy_state = StrategyState {
        id: id.clone(),
        name: request.name.clone(),
        strategy_type: request.strategy_type,
        status: StrategyStatus::Stopped,
        symbols: request.symbols.clone(),
        created_at: now,
        updated_at: now,
    };

    // Store strategy
    state.strategies.insert(id.clone(), strategy_state);

    let detail = StrategyDetail {
        id,
        name: request.name,
        strategy_type: request.strategy_type,
        status: StrategyStatus::Stopped,
        symbols: request.symbols,
        created_at: now,
        updated_at: now,
    };

    Ok(CreatedResponse::with_message(
        detail,
        "Strategy created successfully",
    ))
}

/// Get strategy by ID.
///
/// GET /api/v1/strategies/{id}
pub async fn get_strategy(
    State(state): State<Arc<AppState>>,
    Auth(user): Auth,
    Path(id): Path<String>,
) -> ApiResult<ApiResponse<StrategyDetail>> {
    // Check permission
    if !user.role.can_view_positions() {
        return Err(ApiError::Forbidden("Insufficient permissions".to_string()));
    }

    let strategy = state
        .strategies
        .get(&id)
        .ok_or_else(|| ApiError::NotFound(format!("Strategy not found: {id}")))?;

    let detail = StrategyDetail {
        id: strategy.id.clone(),
        name: strategy.name.clone(),
        strategy_type: strategy.strategy_type,
        status: strategy.status,
        symbols: strategy.symbols.clone(),
        created_at: strategy.created_at,
        updated_at: strategy.updated_at,
    };
    drop(strategy);

    Ok(ApiResponse::success(detail))
}

/// Start a strategy.
///
/// POST /api/v1/strategies/{id}/start
pub async fn start_strategy(
    State(state): State<Arc<AppState>>,
    Auth(user): Auth,
    Path(id): Path<String>,
) -> ApiResult<EmptyResponse> {
    // Check permission
    if !user.role.can_manage_strategies() {
        return Err(ApiError::Forbidden("Insufficient permissions".to_string()));
    }

    let mut strategy = state
        .strategies
        .get_mut(&id)
        .ok_or_else(|| ApiError::NotFound(format!("Strategy not found: {id}")))?;

    if !strategy.status.can_start() {
        return Err(ApiError::Conflict(format!(
            "Strategy cannot be started from status: {:?}",
            strategy.status
        )));
    }

    strategy.status = StrategyStatus::Running;
    strategy.updated_at = Utc::now();
    drop(strategy);

    Ok(EmptyResponse::success_with_message("Strategy started"))
}

/// Stop a strategy.
///
/// POST /api/v1/strategies/{id}/stop
pub async fn stop_strategy(
    State(state): State<Arc<AppState>>,
    Auth(user): Auth,
    Path(id): Path<String>,
) -> ApiResult<EmptyResponse> {
    // Check permission
    if !user.role.can_manage_strategies() {
        return Err(ApiError::Forbidden("Insufficient permissions".to_string()));
    }

    let mut strategy = state
        .strategies
        .get_mut(&id)
        .ok_or_else(|| ApiError::NotFound(format!("Strategy not found: {id}")))?;

    if !strategy.status.can_stop() {
        return Err(ApiError::Conflict(format!(
            "Strategy cannot be stopped from status: {:?}",
            strategy.status
        )));
    }

    strategy.status = StrategyStatus::Stopped;
    strategy.updated_at = Utc::now();
    drop(strategy);

    Ok(EmptyResponse::success_with_message("Strategy stopped"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strategy_list_item_from_state() {
        let state = StrategyState {
            id: "test-id".to_string(),
            name: "Test Strategy".to_string(),
            strategy_type: StrategyType::Cta,
            status: StrategyStatus::Running,
            symbols: vec!["BTC-USDT".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let item = StrategyListItem::from(&state);
        assert_eq!(item.id, "test-id");
        assert_eq!(item.name, "Test Strategy");
        assert_eq!(item.strategy_type, StrategyType::Cta);
    }
}
