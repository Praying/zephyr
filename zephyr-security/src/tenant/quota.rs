//! Resource quota definitions for tenants.

#![allow(clippy::cast_precision_loss)]

use serde::{Deserialize, Serialize};

/// Resource quota limits for a tenant.
///
/// Defines the maximum resources a tenant can use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuota {
    /// Maximum number of strategies.
    pub max_strategies: u64,
    /// Maximum number of open positions.
    pub max_positions: u64,
    /// Maximum orders per second.
    pub max_orders_per_second: u64,
    /// Maximum API requests per minute.
    pub max_api_requests_per_minute: u64,
    /// Maximum WebSocket connections.
    pub max_websocket_connections: u64,
    /// Maximum data storage in bytes.
    pub max_storage_bytes: u64,
    /// Maximum number of exchange connections.
    pub max_exchange_connections: u64,
    /// Maximum number of users.
    pub max_users: u64,
}

impl ResourceQuota {
    /// Creates a new resource quota with the given limits.
    #[must_use]
    pub const fn new(
        max_strategies: u64,
        max_positions: u64,
        max_orders_per_second: u64,
        max_api_requests_per_minute: u64,
    ) -> Self {
        Self {
            max_strategies,
            max_positions,
            max_orders_per_second,
            max_api_requests_per_minute,
            max_websocket_connections: 10,
            max_storage_bytes: 10 * 1024 * 1024 * 1024, // 10 GB
            max_exchange_connections: 5,
            max_users: 10,
        }
    }

    /// Creates a free tier quota.
    #[must_use]
    pub const fn free_tier() -> Self {
        Self {
            max_strategies: 3,
            max_positions: 10,
            max_orders_per_second: 5,
            max_api_requests_per_minute: 60,
            max_websocket_connections: 2,
            max_storage_bytes: 1024 * 1024 * 1024, // 1 GB
            max_exchange_connections: 1,
            max_users: 1,
        }
    }

    /// Creates a professional tier quota.
    #[must_use]
    pub const fn professional_tier() -> Self {
        Self {
            max_strategies: 20,
            max_positions: 100,
            max_orders_per_second: 50,
            max_api_requests_per_minute: 600,
            max_websocket_connections: 10,
            max_storage_bytes: 50 * 1024 * 1024 * 1024, // 50 GB
            max_exchange_connections: 5,
            max_users: 10,
        }
    }

    /// Creates an enterprise tier quota.
    #[must_use]
    pub const fn enterprise_tier() -> Self {
        Self {
            max_strategies: 100,
            max_positions: 1000,
            max_orders_per_second: 500,
            max_api_requests_per_minute: 6000,
            max_websocket_connections: 100,
            max_storage_bytes: 500 * 1024 * 1024 * 1024, // 500 GB
            max_exchange_connections: 20,
            max_users: 100,
        }
    }

    /// Creates an unlimited quota (for platform operators).
    #[must_use]
    pub const fn unlimited() -> Self {
        Self {
            max_strategies: u64::MAX,
            max_positions: u64::MAX,
            max_orders_per_second: u64::MAX,
            max_api_requests_per_minute: u64::MAX,
            max_websocket_connections: u64::MAX,
            max_storage_bytes: u64::MAX,
            max_exchange_connections: u64::MAX,
            max_users: u64::MAX,
        }
    }
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self::free_tier()
    }
}

/// Current resource usage for a tenant.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QuotaUsage {
    /// Current number of strategies.
    pub strategies: u64,
    /// Current number of open positions.
    pub positions: u64,
    /// Current orders per second (rolling average).
    pub orders_per_second: u64,
    /// Current API requests per minute (rolling count).
    pub api_requests_per_minute: u64,
    /// Current WebSocket connections.
    pub websocket_connections: u64,
    /// Current storage usage in bytes.
    pub storage_bytes: u64,
    /// Current number of exchange connections.
    pub exchange_connections: u64,
    /// Current number of users.
    pub users: u64,
}

impl QuotaUsage {
    /// Creates a new empty usage tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            strategies: 0,
            positions: 0,
            orders_per_second: 0,
            api_requests_per_minute: 0,
            websocket_connections: 0,
            storage_bytes: 0,
            exchange_connections: 0,
            users: 0,
        }
    }

    /// Checks if the usage exceeds the given quota.
    #[must_use]
    pub fn exceeds_quota(&self, quota: &ResourceQuota) -> bool {
        self.strategies > quota.max_strategies
            || self.positions > quota.max_positions
            || self.orders_per_second > quota.max_orders_per_second
            || self.api_requests_per_minute > quota.max_api_requests_per_minute
            || self.websocket_connections > quota.max_websocket_connections
            || self.storage_bytes > quota.max_storage_bytes
            || self.exchange_connections > quota.max_exchange_connections
            || self.users > quota.max_users
    }

    /// Returns the usage percentage for each resource.
    #[must_use]
    pub fn usage_percentages(&self, quota: &ResourceQuota) -> UsagePercentages {
        UsagePercentages {
            strategies: percentage(self.strategies, quota.max_strategies),
            positions: percentage(self.positions, quota.max_positions),
            orders_per_second: percentage(self.orders_per_second, quota.max_orders_per_second),
            api_requests_per_minute: percentage(
                self.api_requests_per_minute,
                quota.max_api_requests_per_minute,
            ),
            websocket_connections: percentage(
                self.websocket_connections,
                quota.max_websocket_connections,
            ),
            storage_bytes: percentage(self.storage_bytes, quota.max_storage_bytes),
            exchange_connections: percentage(
                self.exchange_connections,
                quota.max_exchange_connections,
            ),
            users: percentage(self.users, quota.max_users),
        }
    }
}

/// Usage percentages for each resource type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsagePercentages {
    /// Strategies usage percentage (0-100).
    pub strategies: f64,
    /// Positions usage percentage (0-100).
    pub positions: f64,
    /// Orders per second usage percentage (0-100).
    pub orders_per_second: f64,
    /// API requests per minute usage percentage (0-100).
    pub api_requests_per_minute: f64,
    /// WebSocket connections usage percentage (0-100).
    pub websocket_connections: f64,
    /// Storage usage percentage (0-100).
    pub storage_bytes: f64,
    /// Exchange connections usage percentage (0-100).
    pub exchange_connections: f64,
    /// Users usage percentage (0-100).
    pub users: f64,
}

impl UsagePercentages {
    /// Returns the highest usage percentage.
    #[must_use]
    pub fn max_usage(&self) -> f64 {
        [
            self.strategies,
            self.positions,
            self.orders_per_second,
            self.api_requests_per_minute,
            self.websocket_connections,
            self.storage_bytes,
            self.exchange_connections,
            self.users,
        ]
        .into_iter()
        .fold(0.0, f64::max)
    }

    /// Returns true if any resource is above the warning threshold (80%).
    #[must_use]
    pub fn has_warning(&self) -> bool {
        self.max_usage() >= 80.0
    }

    /// Returns true if any resource is above the critical threshold (95%).
    #[must_use]
    pub fn has_critical(&self) -> bool {
        self.max_usage() >= 95.0
    }
}

fn percentage(current: u64, max: u64) -> f64 {
    if max == 0 || max == u64::MAX {
        0.0
    } else {
        (current as f64 / max as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quota_tiers() {
        let free = ResourceQuota::free_tier();
        let pro = ResourceQuota::professional_tier();
        let enterprise = ResourceQuota::enterprise_tier();

        assert!(free.max_strategies < pro.max_strategies);
        assert!(pro.max_strategies < enterprise.max_strategies);
    }

    #[test]
    fn test_usage_exceeds_quota() {
        let quota = ResourceQuota::free_tier();
        let mut usage = QuotaUsage::new();

        assert!(!usage.exceeds_quota(&quota));

        usage.strategies = quota.max_strategies + 1;
        assert!(usage.exceeds_quota(&quota));
    }

    #[test]
    fn test_usage_percentages() {
        let quota = ResourceQuota::new(100, 100, 100, 100);
        let mut usage = QuotaUsage::new();
        usage.strategies = 50;
        usage.positions = 80;

        let percentages = usage.usage_percentages(&quota);
        assert!((percentages.strategies - 50.0).abs() < 0.01);
        assert!((percentages.positions - 80.0).abs() < 0.01);
    }

    #[test]
    fn test_warning_thresholds() {
        let quota = ResourceQuota::new(100, 100, 100, 100);
        let mut usage = QuotaUsage::new();

        usage.strategies = 79;
        let percentages = usage.usage_percentages(&quota);
        assert!(!percentages.has_warning());

        usage.strategies = 80;
        let percentages = usage.usage_percentages(&quota);
        assert!(percentages.has_warning());

        usage.strategies = 95;
        let percentages = usage.usage_percentages(&quota);
        assert!(percentages.has_critical());
    }

    #[test]
    fn test_quota_serde() {
        let quota = ResourceQuota::professional_tier();
        let json = serde_json::to_string(&quota).unwrap();
        let parsed: ResourceQuota = serde_json::from_str(&json).unwrap();

        assert_eq!(quota.max_strategies, parsed.max_strategies);
        assert_eq!(quota.max_positions, parsed.max_positions);
    }
}
