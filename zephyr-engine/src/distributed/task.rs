//! Task distribution and load balancing.
//!
//! This module provides task management for distributed execution including
//! task IDs, priorities, and load balancing across nodes.

#![allow(clippy::map_unwrap_or)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::too_many_lines)]

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use zephyr_core::types::{Quantity, Symbol, Timestamp};

use super::config::NodeAddress;

/// Unique identifier for a task.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(String);

impl TaskId {
    /// Creates a new task ID.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generates a new unique task ID.
    #[must_use]
    pub fn generate() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        Self(format!("task-{timestamp}-{count}"))
    }

    /// Returns the ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for TaskId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for TaskId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Task priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    /// Low priority - can be delayed.
    Low = 0,
    /// Normal priority - standard execution.
    Normal = 1,
    /// High priority - execute soon.
    High = 2,
    /// Critical priority - execute immediately.
    Critical = 3,
}

impl Default for TaskPriority {
    fn default() -> Self {
        Self::Normal
    }
}

impl fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Task execution status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is pending assignment.
    Pending,
    /// Task is assigned to a node.
    Assigned,
    /// Task is currently executing.
    Running,
    /// Task completed successfully.
    Completed,
    /// Task failed.
    Failed,
    /// Task was cancelled.
    Cancelled,
    /// Task timed out.
    TimedOut,
}

impl TaskStatus {
    /// Returns true if the task is in a terminal state.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::TimedOut
        )
    }

    /// Returns true if the task is still active.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Pending | Self::Assigned | Self::Running)
    }
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Assigned => write!(f, "assigned"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::TimedOut => write!(f, "timed_out"),
        }
    }
}

/// Task assignment to a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssignment {
    /// Task ID.
    pub task_id: TaskId,
    /// Assigned node ID.
    pub node_id: String,
    /// Assignment timestamp.
    pub assigned_at: Timestamp,
    /// Current status.
    pub status: TaskStatus,
    /// Number of retry attempts.
    pub retry_count: u32,
    /// Error message if failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl TaskAssignment {
    /// Creates a new task assignment.
    #[must_use]
    pub fn new(task_id: TaskId, node_id: impl Into<String>) -> Self {
        Self {
            task_id,
            node_id: node_id.into(),
            assigned_at: Timestamp::now(),
            status: TaskStatus::Assigned,
            retry_count: 0,
            error: None,
        }
    }

    /// Marks the task as running.
    pub fn mark_running(&mut self) {
        self.status = TaskStatus::Running;
    }

    /// Marks the task as completed.
    pub fn mark_completed(&mut self) {
        self.status = TaskStatus::Completed;
    }

    /// Marks the task as failed.
    pub fn mark_failed(&mut self, error: impl Into<String>) {
        self.status = TaskStatus::Failed;
        self.error = Some(error.into());
    }

    /// Increments retry count and resets status.
    pub fn retry(&mut self) {
        self.retry_count += 1;
        self.status = TaskStatus::Pending;
        self.error = None;
    }
}

/// Configuration for load balancer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancerConfig {
    /// Maximum load per node (0.0 to 1.0).
    #[serde(default = "default_max_load")]
    pub max_load: f64,
    /// Weight for CPU load in scoring.
    #[serde(default = "default_cpu_weight")]
    pub cpu_weight: f64,
    /// Weight for memory load in scoring.
    #[serde(default = "default_memory_weight")]
    pub memory_weight: f64,
    /// Weight for task count in scoring.
    #[serde(default = "default_task_weight")]
    pub task_weight: f64,
    /// Enable sticky sessions for affinity.
    #[serde(default)]
    pub sticky_sessions: bool,
}

fn default_max_load() -> f64 {
    0.8
}

fn default_cpu_weight() -> f64 {
    0.4
}

fn default_memory_weight() -> f64 {
    0.3
}

fn default_task_weight() -> f64 {
    0.3
}

impl Default for LoadBalancerConfig {
    fn default() -> Self {
        Self {
            max_load: default_max_load(),
            cpu_weight: default_cpu_weight(),
            memory_weight: default_memory_weight(),
            task_weight: default_task_weight(),
            sticky_sessions: false,
        }
    }
}

/// Node load information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLoad {
    /// CPU utilization (0.0 to 1.0).
    pub cpu: f64,
    /// Memory utilization (0.0 to 1.0).
    pub memory: f64,
    /// Number of active tasks.
    pub active_tasks: u32,
    /// Maximum tasks this node can handle.
    pub max_tasks: u32,
    /// Last update timestamp.
    pub updated_at: Timestamp,
}

impl Default for NodeLoad {
    fn default() -> Self {
        Self {
            cpu: 0.0,
            memory: 0.0,
            active_tasks: 0,
            max_tasks: 100,
            updated_at: Timestamp::now(),
        }
    }
}

impl NodeLoad {
    /// Calculates the overall load score.
    #[must_use]
    pub fn score(&self, config: &LoadBalancerConfig) -> f64 {
        let task_ratio = if self.max_tasks > 0 {
            f64::from(self.active_tasks) / f64::from(self.max_tasks)
        } else {
            1.0
        };

        self.cpu * config.cpu_weight
            + self.memory * config.memory_weight
            + task_ratio * config.task_weight
    }

    /// Returns true if the node is overloaded.
    #[must_use]
    pub fn is_overloaded(&self, config: &LoadBalancerConfig) -> bool {
        self.score(config) > config.max_load
    }
}

/// Load balancer for distributing tasks across nodes.
pub struct LoadBalancer {
    config: LoadBalancerConfig,
    node_loads: HashMap<String, NodeLoad>,
    affinity_map: HashMap<String, String>, // key -> node_id
}

impl LoadBalancer {
    /// Creates a new load balancer.
    #[must_use]
    pub fn new(config: LoadBalancerConfig) -> Self {
        Self {
            config,
            node_loads: HashMap::new(),
            affinity_map: HashMap::new(),
        }
    }

    /// Updates load information for a node.
    pub fn update_load(&mut self, node_id: impl Into<String>, load: NodeLoad) {
        self.node_loads.insert(node_id.into(), load);
    }

    /// Removes a node from the load balancer.
    pub fn remove_node(&mut self, node_id: &str) {
        self.node_loads.remove(node_id);
        self.affinity_map.retain(|_, v| v != node_id);
    }

    /// Selects the best node for a task.
    ///
    /// Returns `None` if no suitable node is available.
    #[must_use]
    pub fn select_node(&self, affinity_key: Option<&str>, nodes: &[NodeAddress]) -> Option<String> {
        // Check affinity first
        if self.config.sticky_sessions {
            if let Some(key) = affinity_key {
                if let Some(node_id) = self.affinity_map.get(key) {
                    if self.is_node_available(node_id, nodes) {
                        return Some(node_id.clone());
                    }
                }
            }
        }

        // Find node with lowest load
        let mut best_node: Option<(&str, f64)> = None;

        for node in nodes {
            let load = self.node_loads.get(&node.id);
            let score = load.map_or(0.0, |l| l.score(&self.config));

            // Skip overloaded nodes
            if let Some(l) = load {
                if l.is_overloaded(&self.config) {
                    continue;
                }
            }

            match best_node {
                None => best_node = Some((&node.id, score)),
                Some((_, best_score)) if score < best_score => {
                    best_node = Some((&node.id, score));
                }
                _ => {}
            }
        }

        best_node.map(|(id, _)| id.to_string())
    }

    /// Sets affinity for a key to a specific node.
    pub fn set_affinity(&mut self, key: impl Into<String>, node_id: impl Into<String>) {
        self.affinity_map.insert(key.into(), node_id.into());
    }

    /// Clears affinity for a key.
    pub fn clear_affinity(&mut self, key: &str) {
        self.affinity_map.remove(key);
    }

    /// Returns the current load for a node.
    #[must_use]
    pub fn get_load(&self, node_id: &str) -> Option<&NodeLoad> {
        self.node_loads.get(node_id)
    }

    fn is_node_available(&self, node_id: &str, nodes: &[NodeAddress]) -> bool {
        if !nodes.iter().any(|n| n.id == node_id) {
            return false;
        }

        self.node_loads
            .get(node_id)
            .map_or(true, |l| !l.is_overloaded(&self.config))
    }
}

/// Task distributor for managing task lifecycle.
pub struct TaskDistributor {
    load_balancer: LoadBalancer,
    assignments: HashMap<TaskId, TaskAssignment>,
    max_retries: u32,
}

impl TaskDistributor {
    /// Creates a new task distributor.
    #[must_use]
    pub fn new(config: LoadBalancerConfig, max_retries: u32) -> Self {
        Self {
            load_balancer: LoadBalancer::new(config),
            assignments: HashMap::new(),
            max_retries,
        }
    }

    /// Assigns a task to a node.
    ///
    /// Returns the assignment if successful.
    pub fn assign_task(
        &mut self,
        task_id: TaskId,
        affinity_key: Option<&str>,
        nodes: &[NodeAddress],
    ) -> Option<TaskAssignment> {
        let node_id = self.load_balancer.select_node(affinity_key, nodes)?;

        let assignment = TaskAssignment::new(task_id.clone(), &node_id);
        self.assignments.insert(task_id, assignment.clone());

        // Update affinity if key provided
        if let Some(key) = affinity_key {
            self.load_balancer.set_affinity(key, &node_id);
        }

        Some(assignment)
    }

    /// Gets the assignment for a task.
    #[must_use]
    pub fn get_assignment(&self, task_id: &TaskId) -> Option<&TaskAssignment> {
        self.assignments.get(task_id)
    }

    /// Gets a mutable assignment for a task.
    pub fn get_assignment_mut(&mut self, task_id: &TaskId) -> Option<&mut TaskAssignment> {
        self.assignments.get_mut(task_id)
    }

    /// Marks a task as completed.
    pub fn complete_task(&mut self, task_id: &TaskId) {
        if let Some(assignment) = self.assignments.get_mut(task_id) {
            assignment.mark_completed();
        }
    }

    /// Marks a task as failed and optionally retries.
    ///
    /// Returns true if the task will be retried.
    pub fn fail_task(&mut self, task_id: &TaskId, error: impl Into<String>) -> bool {
        if let Some(assignment) = self.assignments.get_mut(task_id) {
            if assignment.retry_count < self.max_retries {
                assignment.retry();
                return true;
            }
            assignment.mark_failed(error);
        }
        false
    }

    /// Updates node load information.
    pub fn update_node_load(&mut self, node_id: impl Into<String>, load: NodeLoad) {
        self.load_balancer.update_load(node_id, load);
    }

    /// Removes a node and reassigns its tasks.
    pub fn remove_node(&mut self, node_id: &str) -> Vec<TaskId> {
        self.load_balancer.remove_node(node_id);

        // Find tasks assigned to this node that need reassignment
        self.assignments
            .iter()
            .filter(|(_, a)| a.node_id == node_id && a.status.is_active())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Returns all active assignments.
    #[must_use]
    pub fn active_assignments(&self) -> Vec<&TaskAssignment> {
        self.assignments
            .values()
            .filter(|a| a.status.is_active())
            .collect()
    }

    /// Cleans up completed tasks older than the given age.
    pub fn cleanup_old_tasks(&mut self, max_age: std::time::Duration) {
        let cutoff = Timestamp::now().as_millis() - max_age.as_millis() as i64;
        self.assignments
            .retain(|_, a| !a.status.is_terminal() || a.assigned_at.as_millis() > cutoff);
    }
}

/// Execution task signal - simplified for distributed execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSignal {
    /// Symbol for the position change.
    pub symbol: Symbol,
    /// Target quantity.
    pub target_quantity: Quantity,
    /// Source strategy name.
    pub source_strategy: String,
    /// Timestamp.
    pub timestamp: Timestamp,
}

/// Execution task for distributed processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTask {
    /// Unique task identifier.
    pub id: TaskId,
    /// Execution signal to process.
    pub signal: TaskSignal,
    /// Task priority.
    pub priority: TaskPriority,
    /// Optional deadline for execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<Timestamp>,
    /// Optional node affinity (prefer specific node).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affinity: Option<String>,
    /// Creation timestamp.
    pub created_at: Timestamp,
}

impl ExecutionTask {
    /// Creates a new execution task.
    #[must_use]
    pub fn new(signal: TaskSignal) -> Self {
        Self {
            id: TaskId::generate(),
            signal,
            priority: TaskPriority::Normal,
            deadline: None,
            affinity: None,
            created_at: Timestamp::now(),
        }
    }

    /// Sets the task priority.
    #[must_use]
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Sets the deadline.
    #[must_use]
    pub fn with_deadline(mut self, deadline: Timestamp) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Sets the node affinity.
    #[must_use]
    pub fn with_affinity(mut self, affinity: impl Into<String>) -> Self {
        self.affinity = Some(affinity.into());
        self
    }

    /// Returns true if the task has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.deadline.map_or(false, |d| Timestamp::now() > d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_signal() -> TaskSignal {
        TaskSignal {
            symbol: Symbol::new("BTC-USDT").unwrap(),
            target_quantity: Quantity::new(dec!(1.0)).unwrap(),
            source_strategy: "test".to_string(),
            timestamp: Timestamp::now(),
        }
    }

    #[test]
    fn test_task_id_generate() {
        let id1 = TaskId::generate();
        let id2 = TaskId::generate();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_task_priority_ordering() {
        assert!(TaskPriority::Low < TaskPriority::Normal);
        assert!(TaskPriority::Normal < TaskPriority::High);
        assert!(TaskPriority::High < TaskPriority::Critical);
    }

    #[test]
    fn test_task_status_terminal() {
        assert!(!TaskStatus::Pending.is_terminal());
        assert!(!TaskStatus::Running.is_terminal());
        assert!(TaskStatus::Completed.is_terminal());
        assert!(TaskStatus::Failed.is_terminal());
    }

    #[test]
    fn test_task_assignment() {
        let mut assignment = TaskAssignment::new(TaskId::new("task-1"), "node-1");
        assert_eq!(assignment.status, TaskStatus::Assigned);

        assignment.mark_running();
        assert_eq!(assignment.status, TaskStatus::Running);

        assignment.mark_completed();
        assert_eq!(assignment.status, TaskStatus::Completed);
    }

    #[test]
    fn test_task_assignment_retry() {
        let mut assignment = TaskAssignment::new(TaskId::new("task-1"), "node-1");
        assignment.mark_failed("test error");
        assert_eq!(assignment.retry_count, 0);

        assignment.retry();
        assert_eq!(assignment.retry_count, 1);
        assert_eq!(assignment.status, TaskStatus::Pending);
        assert!(assignment.error.is_none());
    }

    #[test]
    fn test_node_load_score() {
        let config = LoadBalancerConfig::default();
        let load = NodeLoad {
            cpu: 0.5,
            memory: 0.3,
            active_tasks: 5,
            max_tasks: 10,
            updated_at: Timestamp::now(),
        };

        let score = load.score(&config);
        // 0.5 * 0.4 + 0.3 * 0.3 + 0.5 * 0.3 = 0.2 + 0.09 + 0.15 = 0.44
        assert!((score - 0.44).abs() < 0.01);
    }

    #[test]
    fn test_load_balancer_select_node() {
        let config = LoadBalancerConfig::default();
        let mut balancer = LoadBalancer::new(config);

        let nodes = vec![
            NodeAddress::new("node-1", "192.168.1.1", 8080),
            NodeAddress::new("node-2", "192.168.1.2", 8080),
        ];

        // Node 1 has higher load
        balancer.update_load(
            "node-1",
            NodeLoad {
                cpu: 0.8,
                memory: 0.7,
                active_tasks: 8,
                max_tasks: 10,
                updated_at: Timestamp::now(),
            },
        );

        // Node 2 has lower load
        balancer.update_load(
            "node-2",
            NodeLoad {
                cpu: 0.2,
                memory: 0.3,
                active_tasks: 2,
                max_tasks: 10,
                updated_at: Timestamp::now(),
            },
        );

        let selected = balancer.select_node(None, &nodes);
        assert_eq!(selected, Some("node-2".to_string()));
    }

    #[test]
    fn test_load_balancer_affinity() {
        let mut config = LoadBalancerConfig::default();
        config.sticky_sessions = true;
        let mut balancer = LoadBalancer::new(config);

        let nodes = vec![
            NodeAddress::new("node-1", "192.168.1.1", 8080),
            NodeAddress::new("node-2", "192.168.1.2", 8080),
        ];

        balancer.set_affinity("btc-orders", "node-1");

        let selected = balancer.select_node(Some("btc-orders"), &nodes);
        assert_eq!(selected, Some("node-1".to_string()));
    }

    #[test]
    fn test_task_distributor() {
        let config = LoadBalancerConfig::default();
        let mut distributor = TaskDistributor::new(config, 3);

        let nodes = vec![
            NodeAddress::new("node-1", "192.168.1.1", 8080),
            NodeAddress::new("node-2", "192.168.1.2", 8080),
        ];

        let task_id = TaskId::new("task-1");
        let assignment = distributor.assign_task(task_id.clone(), None, &nodes);
        assert!(assignment.is_some());

        let retrieved = distributor.get_assignment(&task_id);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_task_distributor_retry() {
        let config = LoadBalancerConfig::default();
        let mut distributor = TaskDistributor::new(config, 3);

        let nodes = vec![NodeAddress::new("node-1", "192.168.1.1", 8080)];

        let task_id = TaskId::new("task-1");
        distributor.assign_task(task_id.clone(), None, &nodes);

        // First failure - should retry
        assert!(distributor.fail_task(&task_id, "error 1"));

        // Second failure - should retry
        assert!(distributor.fail_task(&task_id, "error 2"));

        // Third failure - should retry
        assert!(distributor.fail_task(&task_id, "error 3"));

        // Fourth failure - max retries exceeded
        assert!(!distributor.fail_task(&task_id, "error 4"));
    }

    #[test]
    fn test_execution_task() {
        let signal = create_test_signal();
        let task = ExecutionTask::new(signal)
            .with_priority(TaskPriority::High)
            .with_affinity("binance");

        assert_eq!(task.priority, TaskPriority::High);
        assert_eq!(task.affinity, Some("binance".to_string()));
        assert!(!task.is_expired());
    }

    #[test]
    fn test_execution_task_expired() {
        let signal = create_test_signal();
        let task = ExecutionTask::new(signal).with_deadline(Timestamp::new_unchecked(0)); // Past deadline

        assert!(task.is_expired());
    }
}
