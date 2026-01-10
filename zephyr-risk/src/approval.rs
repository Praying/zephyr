//! Approval workflow module for large order authorization.
//!
//! This module provides multi-level approval workflows for:
//! - Large order approval
//! - Risk limit override approval
//! - Configuration change approval
//!
//! # Example
//!
//! ```
//! use zephyr_risk::approval::{ApprovalWorkflow, ApprovalConfig, ApprovalRequest, ApprovalLevel};
//! use zephyr_core::types::{Amount, Symbol};
//! use rust_decimal_macros::dec;
//!
//! let config = ApprovalConfig::default();
//! let workflow = ApprovalWorkflow::new(config);
//!
//! // Check if an order requires approval
//! let symbol = Symbol::new("BTC-USDT").unwrap();
//! let amount = Amount::new(dec!(100000)).unwrap();
//! let requires = workflow.requires_approval(&symbol, amount);
//! ```

#![allow(clippy::module_name_repetitions)]

use parking_lot::RwLock;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};
use zephyr_core::types::{Amount, OrderId, Symbol, Timestamp};

/// Approval level for multi-level authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalLevel {
    /// Level 1 - Junior trader/analyst
    Level1,
    /// Level 2 - Senior trader
    Level2,
    /// Level 3 - Risk manager
    Level3,
    /// Level 4 - Head of trading
    Level4,
    /// Level 5 - Executive/CRO
    Level5,
}

impl ApprovalLevel {
    /// Returns the numeric value of the level.
    #[must_use]
    pub const fn value(&self) -> u8 {
        match self {
            Self::Level1 => 1,
            Self::Level2 => 2,
            Self::Level3 => 3,
            Self::Level4 => 4,
            Self::Level5 => 5,
        }
    }

    /// Creates a level from a numeric value.
    #[must_use]
    pub const fn from_value(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Level1),
            2 => Some(Self::Level2),
            3 => Some(Self::Level3),
            4 => Some(Self::Level4),
            5 => Some(Self::Level5),
            _ => None,
        }
    }
}

impl std::fmt::Display for ApprovalLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Level {}", self.value())
    }
}

/// Status of an approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    /// Pending approval
    Pending,
    /// Approved
    Approved,
    /// Rejected
    Rejected,
    /// Expired
    Expired,
    /// Cancelled by requester
    Cancelled,
}

impl ApprovalStatus {
    /// Returns true if the request is still pending.
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    /// Returns true if the request was approved.
    #[must_use]
    pub const fn is_approved(&self) -> bool {
        matches!(self, Self::Approved)
    }

    /// Returns true if the request is in a final state.
    #[must_use]
    pub const fn is_final(&self) -> bool {
        !matches!(self, Self::Pending)
    }
}

/// Type of approval request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalType {
    /// Large order approval
    LargeOrder {
        /// Symbol
        symbol: Symbol,
        /// Order amount
        amount: Amount,
        /// Order side (buy/sell)
        side: String,
    },
    /// Risk limit override
    RiskLimitOverride {
        /// Limit type being overridden
        limit_type: String,
        /// Current limit value
        current_limit: Decimal,
        /// Requested new limit
        requested_limit: Decimal,
    },
    /// Configuration change
    ConfigChange {
        /// Configuration key
        config_key: String,
        /// Current value
        current_value: String,
        /// New value
        new_value: String,
    },
    /// Kill switch deactivation
    KillSwitchDeactivation {
        /// Reason for deactivation
        reason: String,
    },
    /// Manual trade execution
    ManualTrade {
        /// Symbol
        symbol: Symbol,
        /// Trade details
        details: String,
    },
}

/// Approval decision record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalDecision {
    /// Approver ID
    pub approver_id: String,
    /// Approver's level
    pub level: ApprovalLevel,
    /// Decision (approved or rejected)
    pub approved: bool,
    /// Decision timestamp
    pub timestamp: Timestamp,
    /// Optional comment
    pub comment: Option<String>,
}

/// Approval request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Unique request ID
    pub id: String,
    /// Type of approval
    pub approval_type: ApprovalType,
    /// Requester ID
    pub requester_id: String,
    /// Required approval level
    pub required_level: ApprovalLevel,
    /// Current status
    pub status: ApprovalStatus,
    /// Decisions made on this request
    pub decisions: Vec<ApprovalDecision>,
    /// Creation timestamp
    pub created_at: Timestamp,
    /// Expiration timestamp
    pub expires_at: Timestamp,
    /// Associated order ID (if applicable)
    pub order_id: Option<OrderId>,
    /// Priority (lower = higher priority)
    pub priority: u32,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl ApprovalRequest {
    /// Creates a new approval request.
    #[must_use]
    pub fn new(
        approval_type: ApprovalType,
        requester_id: impl Into<String>,
        required_level: ApprovalLevel,
        expiry_secs: u64,
    ) -> Self {
        let now = Timestamp::now();
        let expires_at =
            Timestamp::new(now.as_millis() + (expiry_secs * 1000) as i64).unwrap_or(now);

        Self {
            id: generate_request_id(),
            approval_type,
            requester_id: requester_id.into(),
            required_level,
            status: ApprovalStatus::Pending,
            decisions: Vec::new(),
            created_at: now,
            expires_at,
            order_id: None,
            priority: 100,
            metadata: HashMap::new(),
        }
    }

    /// Sets the associated order ID.
    #[must_use]
    pub fn with_order_id(mut self, order_id: OrderId) -> Self {
        self.order_id = Some(order_id);
        self
    }

    /// Sets the priority.
    #[must_use]
    pub const fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Adds metadata.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Returns true if the request has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Timestamp::now().as_millis() > self.expires_at.as_millis()
    }

    /// Returns the highest approval level received.
    #[must_use]
    pub fn highest_approval_level(&self) -> Option<ApprovalLevel> {
        self.decisions
            .iter()
            .filter(|d| d.approved)
            .map(|d| d.level)
            .max()
    }

    /// Returns true if the request has sufficient approvals.
    #[must_use]
    pub fn has_sufficient_approvals(&self) -> bool {
        self.highest_approval_level()
            .map(|level| level >= self.required_level)
            .unwrap_or(false)
    }
}

/// Threshold configuration for automatic approval level determination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalThreshold {
    /// Minimum amount requiring this level
    pub min_amount: Amount,
    /// Required approval level
    pub level: ApprovalLevel,
}

/// Approval workflow configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalConfig {
    /// Whether approval workflow is enabled
    pub enabled: bool,
    /// Default expiry time for requests (seconds)
    pub default_expiry_secs: u64,
    /// Amount thresholds for automatic level determination
    pub amount_thresholds: Vec<ApprovalThreshold>,
    /// Symbol-specific thresholds
    pub symbol_thresholds: HashMap<Symbol, Vec<ApprovalThreshold>>,
    /// Whether to allow self-approval
    pub allow_self_approval: bool,
    /// Minimum approvers required (for multi-sig style approval)
    pub min_approvers: u32,
    /// Whether to require sequential approval (Level1 before Level2, etc.)
    pub require_sequential: bool,
    /// Auto-reject after expiry
    pub auto_reject_expired: bool,
}

impl Default for ApprovalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_expiry_secs: 3600, // 1 hour
            amount_thresholds: vec![
                ApprovalThreshold {
                    min_amount: Amount::new_unchecked(Decimal::new(10000, 0)),
                    level: ApprovalLevel::Level1,
                },
                ApprovalThreshold {
                    min_amount: Amount::new_unchecked(Decimal::new(50000, 0)),
                    level: ApprovalLevel::Level2,
                },
                ApprovalThreshold {
                    min_amount: Amount::new_unchecked(Decimal::new(100000, 0)),
                    level: ApprovalLevel::Level3,
                },
                ApprovalThreshold {
                    min_amount: Amount::new_unchecked(Decimal::new(500000, 0)),
                    level: ApprovalLevel::Level4,
                },
                ApprovalThreshold {
                    min_amount: Amount::new_unchecked(Decimal::new(1000000, 0)),
                    level: ApprovalLevel::Level5,
                },
            ],
            symbol_thresholds: HashMap::new(),
            allow_self_approval: false,
            min_approvers: 1,
            require_sequential: false,
            auto_reject_expired: true,
        }
    }
}

impl ApprovalConfig {
    /// Creates a new config with custom thresholds.
    #[must_use]
    pub fn with_thresholds(thresholds: Vec<ApprovalThreshold>) -> Self {
        Self {
            amount_thresholds: thresholds,
            ..Default::default()
        }
    }

    /// Adds a symbol-specific threshold.
    #[must_use]
    pub fn with_symbol_threshold(
        mut self,
        symbol: Symbol,
        thresholds: Vec<ApprovalThreshold>,
    ) -> Self {
        self.symbol_thresholds.insert(symbol, thresholds);
        self
    }

    /// Sets the minimum approvers required.
    #[must_use]
    pub const fn with_min_approvers(mut self, min: u32) -> Self {
        self.min_approvers = min;
        self
    }
}

/// Approver information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approver {
    /// Approver ID
    pub id: String,
    /// Approver name
    pub name: String,
    /// Approval level
    pub level: ApprovalLevel,
    /// Whether the approver is active
    pub active: bool,
}

/// Approval workflow manager.
pub struct ApprovalWorkflow {
    /// Configuration
    config: RwLock<ApprovalConfig>,
    /// Pending requests
    pending_requests: RwLock<HashMap<String, ApprovalRequest>>,
    /// Completed requests (for history)
    completed_requests: RwLock<Vec<ApprovalRequest>>,
    /// Registered approvers
    approvers: RwLock<HashMap<String, Approver>>,
}

impl ApprovalWorkflow {
    /// Creates a new approval workflow with the given configuration.
    #[must_use]
    pub fn new(config: ApprovalConfig) -> Self {
        Self {
            config: RwLock::new(config),
            pending_requests: RwLock::new(HashMap::new()),
            completed_requests: RwLock::new(Vec::new()),
            approvers: RwLock::new(HashMap::new()),
        }
    }

    /// Creates a new approval workflow wrapped in an Arc.
    #[must_use]
    pub fn new_shared(config: ApprovalConfig) -> Arc<Self> {
        Arc::new(Self::new(config))
    }

    /// Returns true if approval workflow is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.read().enabled
    }

    /// Checks if an order requires approval.
    #[must_use]
    pub fn requires_approval(&self, symbol: &Symbol, amount: Amount) -> bool {
        if !self.is_enabled() {
            return false;
        }

        self.get_required_level(symbol, amount).is_some()
    }

    /// Gets the required approval level for an order.
    #[must_use]
    pub fn get_required_level(&self, symbol: &Symbol, amount: Amount) -> Option<ApprovalLevel> {
        let config = self.config.read();
        let amount_value = amount.as_decimal().abs();

        // Check symbol-specific thresholds first
        if let Some(thresholds) = config.symbol_thresholds.get(symbol) {
            for threshold in thresholds.iter().rev() {
                if amount_value >= threshold.min_amount.as_decimal() {
                    return Some(threshold.level);
                }
            }
        }

        // Fall back to global thresholds
        for threshold in config.amount_thresholds.iter().rev() {
            if amount_value >= threshold.min_amount.as_decimal() {
                return Some(threshold.level);
            }
        }

        None
    }

    /// Submits a new approval request.
    pub fn submit_request(&self, request: ApprovalRequest) -> String {
        let id = request.id.clone();
        info!(
            request_id = %id,
            approval_type = ?request.approval_type,
            required_level = %request.required_level,
            "Approval request submitted"
        );
        self.pending_requests.write().insert(id.clone(), request);
        id
    }

    /// Creates and submits a large order approval request.
    pub fn request_large_order_approval(
        &self,
        symbol: Symbol,
        amount: Amount,
        side: impl Into<String>,
        requester_id: impl Into<String>,
    ) -> Option<String> {
        let required_level = self.get_required_level(&symbol, amount)?;
        let config = self.config.read();

        let request = ApprovalRequest::new(
            ApprovalType::LargeOrder {
                symbol,
                amount,
                side: side.into(),
            },
            requester_id,
            required_level,
            config.default_expiry_secs,
        );

        drop(config);
        Some(self.submit_request(request))
    }

    /// Registers an approver.
    pub fn register_approver(&self, approver: Approver) {
        info!(
            approver_id = %approver.id,
            level = %approver.level,
            "Approver registered"
        );
        self.approvers.write().insert(approver.id.clone(), approver);
    }

    /// Gets an approver by ID.
    #[must_use]
    pub fn get_approver(&self, id: &str) -> Option<Approver> {
        self.approvers.read().get(id).cloned()
    }

    /// Processes an approval decision.
    pub fn process_decision(
        &self,
        request_id: &str,
        approver_id: &str,
        approved: bool,
        comment: Option<String>,
    ) -> Result<ApprovalStatus, ApprovalError> {
        // Get approver
        let approver = self
            .get_approver(approver_id)
            .ok_or(ApprovalError::ApproverNotFound(approver_id.to_string()))?;

        if !approver.active {
            return Err(ApprovalError::ApproverInactive(approver_id.to_string()));
        }

        let mut pending = self.pending_requests.write();
        let request = pending
            .get_mut(request_id)
            .ok_or(ApprovalError::RequestNotFound(request_id.to_string()))?;

        // Check if request is still pending
        if !request.status.is_pending() {
            return Err(ApprovalError::RequestNotPending(request_id.to_string()));
        }

        // Check if expired
        if request.is_expired() {
            request.status = ApprovalStatus::Expired;
            let completed = request.clone();
            pending.remove(request_id);
            self.completed_requests.write().push(completed);
            return Err(ApprovalError::RequestExpired(request_id.to_string()));
        }

        // Check self-approval
        let config = self.config.read();
        if !config.allow_self_approval && request.requester_id == approver_id {
            return Err(ApprovalError::SelfApprovalNotAllowed);
        }

        // Check sequential approval requirement
        if config.require_sequential {
            let current_max = request
                .highest_approval_level()
                .map(|l| l.value())
                .unwrap_or(0);
            if approver.level.value() > current_max + 1 {
                return Err(ApprovalError::SequentialApprovalRequired);
            }
        }
        drop(config);

        // Record decision
        let decision = ApprovalDecision {
            approver_id: approver_id.to_string(),
            level: approver.level,
            approved,
            timestamp: Timestamp::now(),
            comment,
        };
        request.decisions.push(decision);

        // Determine final status
        if !approved {
            request.status = ApprovalStatus::Rejected;
            warn!(
                request_id = %request_id,
                approver_id = %approver_id,
                "Approval request rejected"
            );
        } else if request.has_sufficient_approvals() {
            let config = self.config.read();
            let approval_count = request.decisions.iter().filter(|d| d.approved).count() as u32;
            if approval_count >= config.min_approvers {
                request.status = ApprovalStatus::Approved;
                info!(
                    request_id = %request_id,
                    "Approval request approved"
                );
            }
        }

        let status = request.status;

        // Move to completed if final
        if status.is_final() {
            let completed = request.clone();
            pending.remove(request_id);
            self.completed_requests.write().push(completed);
        }

        Ok(status)
    }

    /// Cancels a pending request.
    pub fn cancel_request(
        &self,
        request_id: &str,
        requester_id: &str,
    ) -> Result<(), ApprovalError> {
        let mut pending = self.pending_requests.write();
        let request = pending
            .get_mut(request_id)
            .ok_or(ApprovalError::RequestNotFound(request_id.to_string()))?;

        if request.requester_id != requester_id {
            return Err(ApprovalError::NotAuthorized);
        }

        if !request.status.is_pending() {
            return Err(ApprovalError::RequestNotPending(request_id.to_string()));
        }

        request.status = ApprovalStatus::Cancelled;
        let completed = request.clone();
        pending.remove(request_id);
        self.completed_requests.write().push(completed);

        info!(request_id = %request_id, "Approval request cancelled");
        Ok(())
    }

    /// Gets a pending request by ID.
    #[must_use]
    pub fn get_request(&self, request_id: &str) -> Option<ApprovalRequest> {
        self.pending_requests.read().get(request_id).cloned()
    }

    /// Gets all pending requests.
    #[must_use]
    pub fn pending_requests(&self) -> Vec<ApprovalRequest> {
        self.pending_requests.read().values().cloned().collect()
    }

    /// Gets pending requests for a specific approver level.
    #[must_use]
    pub fn pending_for_level(&self, level: ApprovalLevel) -> Vec<ApprovalRequest> {
        self.pending_requests
            .read()
            .values()
            .filter(|r| r.required_level <= level)
            .cloned()
            .collect()
    }

    /// Cleans up expired requests.
    pub fn cleanup_expired(&self) -> usize {
        let config = self.config.read();
        if !config.auto_reject_expired {
            return 0;
        }
        drop(config);

        let mut pending = self.pending_requests.write();
        let mut completed = self.completed_requests.write();

        let expired: Vec<String> = pending
            .iter()
            .filter(|(_, r)| r.is_expired())
            .map(|(id, _)| id.clone())
            .collect();

        let count = expired.len();
        for id in expired {
            if let Some(mut request) = pending.remove(&id) {
                request.status = ApprovalStatus::Expired;
                completed.push(request);
            }
        }

        if count > 0 {
            info!(count = count, "Expired approval requests cleaned up");
        }

        count
    }

    /// Gets the approval history.
    #[must_use]
    pub fn history(&self) -> Vec<ApprovalRequest> {
        self.completed_requests.read().clone()
    }

    /// Updates the configuration.
    pub fn update_config(&self, config: ApprovalConfig) {
        *self.config.write() = config;
    }
}

impl Default for ApprovalWorkflow {
    fn default() -> Self {
        Self::new(ApprovalConfig::default())
    }
}

impl std::fmt::Debug for ApprovalWorkflow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApprovalWorkflow")
            .field("enabled", &self.is_enabled())
            .field("pending_count", &self.pending_requests.read().len())
            .finish()
    }
}

/// Approval workflow errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum ApprovalError {
    /// Request not found
    #[error("approval request not found: {0}")]
    RequestNotFound(String),

    /// Approver not found
    #[error("approver not found: {0}")]
    ApproverNotFound(String),

    /// Approver is inactive
    #[error("approver is inactive: {0}")]
    ApproverInactive(String),

    /// Request is not pending
    #[error("request is not pending: {0}")]
    RequestNotPending(String),

    /// Request has expired
    #[error("request has expired: {0}")]
    RequestExpired(String),

    /// Self-approval not allowed
    #[error("self-approval is not allowed")]
    SelfApprovalNotAllowed,

    /// Sequential approval required
    #[error("sequential approval is required")]
    SequentialApprovalRequired,

    /// Not authorized
    #[error("not authorized to perform this action")]
    NotAuthorized,

    /// Insufficient approval level
    #[error("insufficient approval level")]
    InsufficientLevel,
}

/// Generates a unique request ID.
fn generate_request_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("apr-{:016x}", timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn test_symbol() -> Symbol {
        Symbol::new("BTC-USDT").unwrap()
    }

    fn setup_workflow() -> ApprovalWorkflow {
        let workflow = ApprovalWorkflow::default();

        // Register approvers
        workflow.register_approver(Approver {
            id: "approver1".to_string(),
            name: "Junior Trader".to_string(),
            level: ApprovalLevel::Level1,
            active: true,
        });
        workflow.register_approver(Approver {
            id: "approver2".to_string(),
            name: "Senior Trader".to_string(),
            level: ApprovalLevel::Level2,
            active: true,
        });
        workflow.register_approver(Approver {
            id: "approver3".to_string(),
            name: "Risk Manager".to_string(),
            level: ApprovalLevel::Level3,
            active: true,
        });

        workflow
    }

    #[test]
    fn test_approval_level_ordering() {
        assert!(ApprovalLevel::Level1 < ApprovalLevel::Level2);
        assert!(ApprovalLevel::Level2 < ApprovalLevel::Level3);
        assert!(ApprovalLevel::Level5 > ApprovalLevel::Level1);
    }

    #[test]
    fn test_approval_level_value() {
        assert_eq!(ApprovalLevel::Level1.value(), 1);
        assert_eq!(ApprovalLevel::Level5.value(), 5);
        assert_eq!(ApprovalLevel::from_value(3), Some(ApprovalLevel::Level3));
        assert_eq!(ApprovalLevel::from_value(10), None);
    }

    #[test]
    fn test_requires_approval() {
        let workflow = setup_workflow();

        // Small amount - no approval needed
        assert!(!workflow.requires_approval(&test_symbol(), Amount::new(dec!(1000)).unwrap()));

        // Large amount - approval needed
        assert!(workflow.requires_approval(&test_symbol(), Amount::new(dec!(50000)).unwrap()));
    }

    #[test]
    fn test_get_required_level() {
        let workflow = setup_workflow();

        // Below threshold
        assert_eq!(
            workflow.get_required_level(&test_symbol(), Amount::new(dec!(5000)).unwrap()),
            None
        );

        // Level 1 threshold
        assert_eq!(
            workflow.get_required_level(&test_symbol(), Amount::new(dec!(15000)).unwrap()),
            Some(ApprovalLevel::Level1)
        );

        // Level 2 threshold
        assert_eq!(
            workflow.get_required_level(&test_symbol(), Amount::new(dec!(75000)).unwrap()),
            Some(ApprovalLevel::Level2)
        );

        // Level 3 threshold
        assert_eq!(
            workflow.get_required_level(&test_symbol(), Amount::new(dec!(200000)).unwrap()),
            Some(ApprovalLevel::Level3)
        );
    }

    #[test]
    fn test_submit_and_approve_request() {
        let workflow = setup_workflow();

        let request_id = workflow
            .request_large_order_approval(
                test_symbol(),
                Amount::new(dec!(50000)).unwrap(),
                "buy",
                "trader1",
            )
            .unwrap();

        // Check pending
        let request = workflow.get_request(&request_id).unwrap();
        assert_eq!(request.status, ApprovalStatus::Pending);

        // Approve
        let status = workflow
            .process_decision(&request_id, "approver2", true, Some("Approved".to_string()))
            .unwrap();

        assert_eq!(status, ApprovalStatus::Approved);
    }

    #[test]
    fn test_reject_request() {
        let workflow = setup_workflow();

        let request_id = workflow
            .request_large_order_approval(
                test_symbol(),
                Amount::new(dec!(50000)).unwrap(),
                "buy",
                "trader1",
            )
            .unwrap();

        let status = workflow
            .process_decision(
                &request_id,
                "approver2",
                false,
                Some("Too risky".to_string()),
            )
            .unwrap();

        assert_eq!(status, ApprovalStatus::Rejected);
    }

    #[test]
    fn test_self_approval_blocked() {
        let workflow = setup_workflow();

        // Register trader as approver
        workflow.register_approver(Approver {
            id: "trader1".to_string(),
            name: "Trader".to_string(),
            level: ApprovalLevel::Level2,
            active: true,
        });

        let request_id = workflow
            .request_large_order_approval(
                test_symbol(),
                Amount::new(dec!(50000)).unwrap(),
                "buy",
                "trader1",
            )
            .unwrap();

        let result = workflow.process_decision(&request_id, "trader1", true, None);
        assert!(matches!(result, Err(ApprovalError::SelfApprovalNotAllowed)));
    }

    #[test]
    fn test_cancel_request() {
        let workflow = setup_workflow();

        let request_id = workflow
            .request_large_order_approval(
                test_symbol(),
                Amount::new(dec!(50000)).unwrap(),
                "buy",
                "trader1",
            )
            .unwrap();

        workflow.cancel_request(&request_id, "trader1").unwrap();

        // Should be in history now
        let history = workflow.history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].status, ApprovalStatus::Cancelled);
    }

    #[test]
    fn test_pending_for_level() {
        let workflow = setup_workflow();

        // Submit requests at different levels
        workflow.request_large_order_approval(
            test_symbol(),
            Amount::new(dec!(15000)).unwrap(), // Level 1
            "buy",
            "trader1",
        );
        workflow.request_large_order_approval(
            test_symbol(),
            Amount::new(dec!(75000)).unwrap(), // Level 2
            "buy",
            "trader2",
        );

        // Level 1 approver should see 1 request
        let level1_requests = workflow.pending_for_level(ApprovalLevel::Level1);
        assert_eq!(level1_requests.len(), 1);

        // Level 2 approver should see 2 requests
        let level2_requests = workflow.pending_for_level(ApprovalLevel::Level2);
        assert_eq!(level2_requests.len(), 2);
    }

    #[test]
    fn test_approval_config_serde() {
        let config = ApprovalConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ApprovalConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.enabled, parsed.enabled);
        assert_eq!(config.default_expiry_secs, parsed.default_expiry_secs);
    }

    #[test]
    fn test_approval_request_serde() {
        let request = ApprovalRequest::new(
            ApprovalType::LargeOrder {
                symbol: test_symbol(),
                amount: Amount::new(dec!(100000)).unwrap(),
                side: "buy".to_string(),
            },
            "trader1",
            ApprovalLevel::Level3,
            3600,
        );

        let json = serde_json::to_string(&request).unwrap();
        let parsed: ApprovalRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(request.id, parsed.id);
        assert_eq!(request.required_level, parsed.required_level);
    }
}
