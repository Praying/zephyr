//! Application state for the API server.

use dashmap::DashMap;
use std::sync::Arc;

use crate::auth::JwtManager;
use crate::config::ApiConfig;
use crate::middleware::RateLimiter;

use zephyr_security::tenant::TenantManager;

// Re-export StrategyType for use in handlers
pub use zephyr_core::types::StrategyType;

/// Shared application state.
#[derive(Debug)]
pub struct AppState {
    /// API configuration
    pub config: ApiConfig,
    /// JWT manager for authentication
    pub jwt_manager: Arc<JwtManager>,
    /// Rate limiter
    pub rate_limiter: Arc<RateLimiter>,
    /// Tenant manager for multi-tenant support
    tenant_manager: Arc<TenantManager>,
    /// Strategy manager (type-erased to avoid circular dependency)
    pub strategy_manager: Option<Arc<dyn std::any::Any + Send + Sync>>,
    /// Active strategies (strategy_id -> StrategyState)
    pub strategies: DashMap<String, StrategyState>,
    /// Active orders (order_id -> OrderState)
    pub orders: DashMap<String, OrderState>,
}

impl AppState {
    /// Creates a new application state.
    #[must_use]
    pub fn new(config: ApiConfig) -> Self {
        let jwt_manager = Arc::new(JwtManager::new(&config.jwt));
        let rate_limiter = Arc::new(RateLimiter::new(config.rate_limit.clone()));
        let tenant_manager = Arc::new(TenantManager::new());

        Self {
            config,
            jwt_manager,
            rate_limiter,
            tenant_manager,
            strategy_manager: None,
            strategies: DashMap::new(),
            orders: DashMap::new(),
        }
    }

    /// Sets the strategy manager.
    pub fn set_strategy_manager(&mut self, manager: Arc<dyn std::any::Any + Send + Sync>) {
        self.strategy_manager = Some(manager);
    }

    /// Returns a reference to the JWT manager.
    #[must_use]
    pub fn jwt_manager(&self) -> &Arc<JwtManager> {
        &self.jwt_manager
    }

    /// Returns a reference to the rate limiter.
    #[must_use]
    pub fn rate_limiter(&self) -> &Arc<RateLimiter> {
        &self.rate_limiter
    }

    /// Returns a reference to the tenant manager.
    #[must_use]
    pub fn tenant_manager(&self) -> &Arc<TenantManager> {
        &self.tenant_manager
    }
}

/// Strategy state for tracking.
#[derive(Debug, Clone)]
pub struct StrategyState {
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

/// Strategy status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StrategyStatus {
    /// Strategy is stopped
    Stopped,
    /// Strategy is starting
    Starting,
    /// Strategy is running
    Running,
    /// Strategy is paused
    Paused,
    /// Strategy is stopping
    Stopping,
    /// Strategy encountered an error
    Error,
}

impl StrategyStatus {
    /// Returns true if the strategy is active.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Running | Self::Paused)
    }

    /// Returns true if the strategy can be started.
    #[must_use]
    pub const fn can_start(&self) -> bool {
        matches!(self, Self::Stopped | Self::Error)
    }

    /// Returns true if the strategy can be stopped.
    #[must_use]
    pub const fn can_stop(&self) -> bool {
        matches!(self, Self::Running | Self::Paused | Self::Starting)
    }
}

/// Order state for tracking.
#[derive(Debug, Clone)]
pub struct OrderState {
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
    pub quantity: rust_decimal::Decimal,
    /// Price (for limit orders)
    pub price: Option<rust_decimal::Decimal>,
    /// Filled quantity
    pub filled_quantity: rust_decimal::Decimal,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_new() {
        let config = ApiConfig::default();
        let state = AppState::new(config);

        assert!(state.strategies.is_empty());
        assert!(state.orders.is_empty());
    }

    #[test]
    fn test_strategy_status_is_active() {
        assert!(StrategyStatus::Running.is_active());
        assert!(StrategyStatus::Paused.is_active());
        assert!(!StrategyStatus::Stopped.is_active());
        assert!(!StrategyStatus::Starting.is_active());
    }

    #[test]
    fn test_strategy_status_can_start() {
        assert!(StrategyStatus::Stopped.can_start());
        assert!(StrategyStatus::Error.can_start());
        assert!(!StrategyStatus::Running.can_start());
        assert!(!StrategyStatus::Starting.can_start());
    }

    #[test]
    fn test_strategy_status_can_stop() {
        assert!(StrategyStatus::Running.can_stop());
        assert!(StrategyStatus::Paused.can_stop());
        assert!(StrategyStatus::Starting.can_stop());
        assert!(!StrategyStatus::Stopped.can_stop());
    }
}
