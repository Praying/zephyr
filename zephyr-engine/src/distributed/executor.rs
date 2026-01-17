//! Distributed executor implementation.
//!
//! This module provides the main distributed executor that coordinates
//! task distribution, leader election, and state synchronization.

#![allow(unused_imports)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use zephyr_core::types::Timestamp;

use super::config::DistributedExecutorConfig;
use super::election::{ElectionConfig, ElectionEvent, HeartbeatMessage, LeaderElection};
use super::lock::{LockError, LockGuard, LockManager};
use super::state::{StateSync, StateSyncConfig, StateSyncError, SyncMessage};
use super::task::{
    ExecutionTask, LoadBalancer, LoadBalancerConfig, NodeLoad, TaskAssignment, TaskDistributor,
    TaskId, TaskStatus,
};

/// Error type for distributed executor operations.
#[derive(Debug, Clone, Error)]
pub enum ExecutorError {
    /// Not the leader.
    #[error("not the leader, current leader: {0:?}")]
    NotLeader(Option<String>),

    /// No leader available.
    #[error("no leader available")]
    NoLeader,

    /// Task not found.
    #[error("task not found: {0}")]
    TaskNotFound(String),

    /// Lock error.
    #[error("lock error: {0}")]
    Lock(#[from] LockError),

    /// State sync error.
    #[error("state sync error: {0}")]
    StateSync(#[from] StateSyncError),

    /// Node not found.
    #[error("node not found: {0}")]
    NodeNotFound(String),

    /// Cluster not healthy.
    #[error("cluster not healthy: {0}")]
    ClusterUnhealthy(String),

    /// Task submission failed.
    #[error("task submission failed: {0}")]
    SubmissionFailed(String),

    /// Network error.
    #[error("network error: {0}")]
    NetworkError(String),
}

/// Node role in the cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeRole {
    /// Leader node - coordinates task distribution.
    Leader,
    /// Follower node - executes assigned tasks.
    #[default]
    Follower,
    /// Candidate node - participating in election.
    Candidate,
}

impl std::fmt::Display for NodeRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Leader => write!(f, "leader"),
            Self::Follower => write!(f, "follower"),
            Self::Candidate => write!(f, "candidate"),
        }
    }
}

/// Status of a node in the cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStatus {
    /// Node ID.
    pub id: String,
    /// Current role.
    pub role: NodeRole,
    /// Last heartbeat timestamp.
    pub last_heartbeat: Timestamp,
    /// Current load (0.0 to 1.0).
    pub load: f64,
    /// Whether the node is healthy.
    pub healthy: bool,
    /// Number of active tasks.
    pub active_tasks: u32,
    /// Node region.
    pub region: Option<String>,
}

impl NodeStatus {
    /// Creates a new node status.
    #[must_use]
    pub fn new(id: impl Into<String>, role: NodeRole) -> Self {
        Self {
            id: id.into(),
            role,
            last_heartbeat: Timestamp::now(),
            load: 0.0,
            healthy: true,
            active_tasks: 0,
            region: None,
        }
    }

    /// Updates the heartbeat timestamp.
    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = Timestamp::now();
    }

    /// Updates the load information.
    pub fn update_load(&mut self, load: f64, active_tasks: u32) {
        self.load = load;
        self.active_tasks = active_tasks;
        self.update_heartbeat();
    }
}

/// Cluster health status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterHealth {
    /// All nodes healthy.
    Healthy,
    /// Some nodes unhealthy but quorum maintained.
    Degraded,
    /// Quorum lost.
    Critical,
    /// Unknown state.
    Unknown,
}

impl std::fmt::Display for ClusterHealth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Degraded => write!(f, "degraded"),
            Self::Critical => write!(f, "critical"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Overall cluster status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStatus {
    /// Current leader ID.
    pub leader_id: Option<String>,
    /// All node statuses.
    pub nodes: Vec<NodeStatus>,
    /// Cluster health.
    pub health: ClusterHealth,
    /// Number of healthy nodes.
    pub healthy_nodes: usize,
    /// Total nodes in cluster.
    pub total_nodes: usize,
    /// Quorum size.
    pub quorum_size: usize,
}

impl ClusterStatus {
    /// Returns true if the cluster has quorum.
    #[must_use]
    pub fn has_quorum(&self) -> bool {
        self.healthy_nodes >= self.quorum_size
    }

    /// Returns true if the cluster is healthy.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.health == ClusterHealth::Healthy
    }
}

/// Distributed executor trait.
#[async_trait]
pub trait DistributedExecutor: Send + Sync {
    /// Gets the current node role.
    fn get_role(&self) -> NodeRole;

    /// Gets the cluster status.
    fn get_cluster_status(&self) -> ClusterStatus;

    /// Submits a task for execution.
    async fn submit_task(&self, task: ExecutionTask) -> Result<TaskId, ExecutorError>;

    /// Gets the status of a task.
    fn get_task_status(&self, id: &TaskId) -> Option<TaskStatus>;

    /// Synchronizes state with the cluster.
    async fn sync_state(&self) -> Result<(), ExecutorError>;

    /// Acquires a distributed lock.
    async fn acquire_lock(&self, key: &str, ttl: Duration) -> Result<LockGuard, ExecutorError>;
}

/// Distributed executor implementation.
pub struct DistributedExecutorImpl {
    config: DistributedExecutorConfig,
    election: Arc<LeaderElection>,
    lock_manager: Arc<LockManager>,
    state_sync: Arc<StateSync>,
    task_distributor: RwLock<TaskDistributor>,
    node_statuses: RwLock<HashMap<String, NodeStatus>>,
    event_rx: RwLock<Option<mpsc::Receiver<ElectionEvent>>>,
}

impl DistributedExecutorImpl {
    /// Creates a new distributed executor.
    #[must_use]
    pub fn new(config: DistributedExecutorConfig) -> Self {
        let (event_tx, event_rx) = mpsc::channel(100);

        // Find node index
        let node_index = config
            .cluster_nodes
            .iter()
            .position(|n| n.id == config.node_id)
            .unwrap_or(0) as u64;

        // Get node priority
        let node_priority = config
            .find_node(&config.node_id)
            .map(|n| n.priority)
            .unwrap_or(0);

        let election_config = ElectionConfig::from(&config);
        let election = Arc::new(LeaderElection::new(
            election_config,
            config.node_id.clone(),
            node_index,
            node_priority,
            config.cluster_size(),
            event_tx,
        ));

        let lock_manager = LockManager::new(&config.node_id, config.lock_timeout);

        let state_sync = Arc::new(StateSync::new(
            StateSyncConfig {
                sync_interval: config.state_sync_interval,
                ..Default::default()
            },
            &config.node_id,
        ));

        let task_distributor =
            TaskDistributor::new(LoadBalancerConfig::default(), config.max_retries);

        // Initialize node statuses
        let mut node_statuses = HashMap::new();
        for node in &config.cluster_nodes {
            node_statuses.insert(
                node.id.clone(),
                NodeStatus {
                    id: node.id.clone(),
                    role: NodeRole::Follower,
                    last_heartbeat: Timestamp::now(),
                    load: 0.0,
                    healthy: true,
                    active_tasks: 0,
                    region: node.region.clone(),
                },
            );
        }

        Self {
            config,
            election,
            lock_manager,
            state_sync,
            task_distributor: RwLock::new(task_distributor),
            node_statuses: RwLock::new(node_statuses),
            event_rx: RwLock::new(Some(event_rx)),
        }
    }

    /// Returns the node ID.
    #[must_use]
    pub fn node_id(&self) -> &str {
        &self.config.node_id
    }

    /// Returns the configuration.
    #[must_use]
    pub fn config(&self) -> &DistributedExecutorConfig {
        &self.config
    }

    /// Returns the election manager.
    #[must_use]
    pub fn election(&self) -> &Arc<LeaderElection> {
        &self.election
    }

    /// Returns the lock manager.
    #[must_use]
    pub fn lock_manager(&self) -> &Arc<LockManager> {
        &self.lock_manager
    }

    /// Returns the state sync manager.
    #[must_use]
    pub fn state_sync(&self) -> &Arc<StateSync> {
        &self.state_sync
    }

    /// Takes the event receiver (can only be called once).
    pub fn take_event_receiver(&self) -> Option<mpsc::Receiver<ElectionEvent>> {
        self.event_rx.write().take()
    }

    /// Updates node status.
    pub fn update_node_status(&self, node_id: &str, load: f64, active_tasks: u32) {
        if let Some(status) = self.node_statuses.write().get_mut(node_id) {
            status.update_load(load, active_tasks);
        }

        // Also update task distributor
        self.task_distributor.write().update_node_load(
            node_id,
            NodeLoad {
                cpu: load,
                memory: 0.0,
                active_tasks,
                max_tasks: 100,
                updated_at: Timestamp::now(),
            },
        );
    }

    /// Marks a node as unhealthy.
    pub fn mark_node_unhealthy(&self, node_id: &str) {
        if let Some(status) = self.node_statuses.write().get_mut(node_id) {
            status.healthy = false;
        }
    }

    /// Marks a node as healthy.
    pub fn mark_node_healthy(&self, node_id: &str) {
        if let Some(status) = self.node_statuses.write().get_mut(node_id) {
            status.healthy = true;
            status.update_heartbeat();
        }
    }

    /// Handles a heartbeat from another node.
    pub async fn handle_heartbeat(&self, heartbeat: &HeartbeatMessage) {
        self.election.handle_heartbeat(heartbeat).await;

        // Update node status
        if let Some(status) = self.node_statuses.write().get_mut(&heartbeat.leader_id) {
            status.role = NodeRole::Leader;
            status.load = heartbeat.load;
            status.update_heartbeat();
        }
    }

    /// Completes a task.
    pub fn complete_task(&self, task_id: &TaskId) {
        self.task_distributor.write().complete_task(task_id);
    }

    /// Fails a task.
    pub fn fail_task(&self, task_id: &TaskId, error: impl Into<String>) -> bool {
        self.task_distributor.write().fail_task(task_id, error)
    }

    /// Gets all active task assignments.
    #[must_use]
    pub fn active_assignments(&self) -> Vec<TaskAssignment> {
        self.task_distributor
            .read()
            .active_assignments()
            .into_iter()
            .cloned()
            .collect()
    }

    /// Handles node failure.
    pub fn handle_node_failure(&self, node_id: &str) -> Vec<TaskId> {
        warn!(node_id = %node_id, "Handling node failure");

        self.mark_node_unhealthy(node_id);

        // Get tasks that need reassignment
        let tasks = self.task_distributor.write().remove_node(node_id);

        // Transfer locks from failed node
        let transferred = self.lock_manager.transfer_locks(node_id);
        if !transferred.is_empty() {
            info!(
                node_id = %node_id,
                locks = ?transferred,
                "Transferred locks from failed node"
            );
        }

        tasks
    }

    fn calculate_cluster_health(&self) -> (ClusterHealth, usize) {
        let statuses = self.node_statuses.read();
        let healthy_count = statuses.values().filter(|s| s.healthy).count();
        let total = statuses.len();
        let quorum = total / 2 + 1;

        let health = if healthy_count == total {
            ClusterHealth::Healthy
        } else if healthy_count >= quorum {
            ClusterHealth::Degraded
        } else if healthy_count > 0 {
            ClusterHealth::Critical
        } else {
            ClusterHealth::Unknown
        };

        (health, healthy_count)
    }
}

#[async_trait]
impl DistributedExecutor for DistributedExecutorImpl {
    fn get_role(&self) -> NodeRole {
        self.election.role()
    }

    fn get_cluster_status(&self) -> ClusterStatus {
        let (health, healthy_nodes) = self.calculate_cluster_health();
        let statuses = self.node_statuses.read();

        ClusterStatus {
            leader_id: self.election.leader_id(),
            nodes: statuses.values().cloned().collect(),
            health,
            healthy_nodes,
            total_nodes: statuses.len(),
            quorum_size: self.config.quorum_size(),
        }
    }

    async fn submit_task(&self, task: ExecutionTask) -> Result<TaskId, ExecutorError> {
        // Only leader can submit tasks
        if !self.election.is_leader() {
            return Err(ExecutorError::NotLeader(self.election.leader_id()));
        }

        let task_id = task.id.clone();
        let affinity = task.affinity.as_deref();

        let assignment = self
            .task_distributor
            .write()
            .assign_task(task_id.clone(), affinity, &self.config.cluster_nodes)
            .ok_or_else(|| ExecutorError::SubmissionFailed("no available nodes".to_string()))?;

        debug!(
            task_id = %task_id,
            node_id = %assignment.node_id,
            "Task assigned"
        );

        Ok(task_id)
    }

    fn get_task_status(&self, id: &TaskId) -> Option<TaskStatus> {
        self.task_distributor
            .read()
            .get_assignment(id)
            .map(|a| a.status)
    }

    async fn sync_state(&self) -> Result<(), ExecutorError> {
        // Create snapshot and apply (in real implementation, this would
        // communicate with other nodes)
        let _snapshot = self.state_sync.create_snapshot();

        // For now, just log that sync was requested
        debug!(
            node_id = %self.config.node_id,
            version = self.state_sync.global_version(),
            "State sync requested"
        );

        Ok(())
    }

    async fn acquire_lock(&self, key: &str, ttl: Duration) -> Result<LockGuard, ExecutorError> {
        self.lock_manager
            .acquire_lock(key, ttl, self.config.lock_timeout)
            .await
            .map_err(ExecutorError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributed::config::NodeAddress;

    fn create_test_config() -> DistributedExecutorConfig {
        DistributedExecutorConfig::builder()
            .node_id("node-1")
            .add_node(NodeAddress::new("node-1", "192.168.1.1", 8080))
            .add_node(NodeAddress::new("node-2", "192.168.1.2", 8080))
            .add_node(NodeAddress::new("node-3", "192.168.1.3", 8080))
            .build()
            .unwrap()
    }

    #[test]
    fn test_node_role_display() {
        assert_eq!(format!("{}", NodeRole::Leader), "leader");
        assert_eq!(format!("{}", NodeRole::Follower), "follower");
        assert_eq!(format!("{}", NodeRole::Candidate), "candidate");
    }

    #[test]
    fn test_node_status_new() {
        let status = NodeStatus::new("node-1", NodeRole::Follower);
        assert_eq!(status.id, "node-1");
        assert_eq!(status.role, NodeRole::Follower);
        assert!(status.healthy);
    }

    #[test]
    fn test_node_status_update_load() {
        let mut status = NodeStatus::new("node-1", NodeRole::Follower);
        status.update_load(0.5, 10);

        assert_eq!(status.load, 0.5);
        assert_eq!(status.active_tasks, 10);
    }

    #[test]
    fn test_cluster_health_display() {
        assert_eq!(format!("{}", ClusterHealth::Healthy), "healthy");
        assert_eq!(format!("{}", ClusterHealth::Degraded), "degraded");
        assert_eq!(format!("{}", ClusterHealth::Critical), "critical");
    }

    #[test]
    fn test_cluster_status_has_quorum() {
        let status = ClusterStatus {
            leader_id: Some("node-1".to_string()),
            nodes: vec![],
            health: ClusterHealth::Healthy,
            healthy_nodes: 2,
            total_nodes: 3,
            quorum_size: 2,
        };

        assert!(status.has_quorum());
    }

    #[test]
    fn test_executor_new() {
        let config = create_test_config();
        let executor = DistributedExecutorImpl::new(config);

        assert_eq!(executor.node_id(), "node-1");
        assert_eq!(executor.get_role(), NodeRole::Follower);
    }

    #[test]
    fn test_executor_cluster_status() {
        let config = create_test_config();
        let executor = DistributedExecutorImpl::new(config);

        let status = executor.get_cluster_status();
        assert_eq!(status.total_nodes, 3);
        assert_eq!(status.quorum_size, 2);
    }

    #[test]
    fn test_executor_update_node_status() {
        let config = create_test_config();
        let executor = DistributedExecutorImpl::new(config);

        executor.update_node_status("node-1", 0.5, 10);

        let status = executor.get_cluster_status();
        let node = status.nodes.iter().find(|n| n.id == "node-1").unwrap();
        assert_eq!(node.load, 0.5);
        assert_eq!(node.active_tasks, 10);
    }

    #[test]
    fn test_executor_mark_node_unhealthy() {
        let config = create_test_config();
        let executor = DistributedExecutorImpl::new(config);

        executor.mark_node_unhealthy("node-2");

        let status = executor.get_cluster_status();
        let node = status.nodes.iter().find(|n| n.id == "node-2").unwrap();
        assert!(!node.healthy);
    }

    #[test]
    fn test_executor_calculate_health() {
        let config = create_test_config();
        let executor = DistributedExecutorImpl::new(config);

        // All healthy
        let status = executor.get_cluster_status();
        assert_eq!(status.health, ClusterHealth::Healthy);

        // One unhealthy
        executor.mark_node_unhealthy("node-2");
        let status = executor.get_cluster_status();
        assert_eq!(status.health, ClusterHealth::Degraded);

        // Two unhealthy (below quorum)
        executor.mark_node_unhealthy("node-3");
        let status = executor.get_cluster_status();
        assert_eq!(status.health, ClusterHealth::Critical);
    }

    #[tokio::test]
    async fn test_executor_acquire_lock() {
        let config = create_test_config();
        let executor = DistributedExecutorImpl::new(config);

        let guard = executor
            .acquire_lock("test-key", Duration::from_secs(30))
            .await;
        assert!(guard.is_ok());
    }

    #[tokio::test]
    async fn test_executor_submit_task_not_leader() {
        let config = create_test_config();
        let executor = DistributedExecutorImpl::new(config);

        use crate::distributed::task::TaskSignal;
        use rust_decimal_macros::dec;
        use zephyr_core::types::{Quantity, Symbol};

        let task = ExecutionTask::new(TaskSignal {
            symbol: Symbol::new("BTC-USDT").unwrap(),
            target_quantity: Quantity::new(dec!(1.0)).unwrap(),
            source_strategy: "test".to_string(),
            timestamp: Timestamp::now(),
        });

        let result = executor.submit_task(task).await;
        assert!(matches!(result, Err(ExecutorError::NotLeader(_))));
    }

    #[test]
    fn test_executor_handle_node_failure() {
        let config = create_test_config();
        let executor = DistributedExecutorImpl::new(config);

        let tasks = executor.handle_node_failure("node-2");
        assert!(tasks.is_empty()); // No tasks assigned yet

        let status = executor.get_cluster_status();
        let node = status.nodes.iter().find(|n| n.id == "node-2").unwrap();
        assert!(!node.healthy);
    }
}
