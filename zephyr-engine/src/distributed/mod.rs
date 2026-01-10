//! Distributed execution framework for Zephyr trading system.
//!
//! This module provides distributed execution capabilities including:
//! - Cluster configuration and node management
//! - Leader election using Raft consensus
//! - Distributed locking for preventing duplicate orders
//! - State synchronization across nodes
//! - Load balancing and task distribution
//!
//! # Architecture
//!
//! The distributed executor follows a leader-follower model:
//! - One leader node coordinates task distribution
//! - Follower nodes execute assigned tasks
//! - Automatic failover when leader fails
//!
//! # Example
//!
//! ```ignore
//! use zephyr_engine::distributed::{DistributedExecutorConfig, NodeAddress};
//!
//! let config = DistributedExecutorConfig::builder()
//!     .node_id("node-1")
//!     .add_node(NodeAddress::new("node-2", "192.168.1.2", 8080))
//!     .build();
//! ```

mod config;
mod election;
mod executor;
mod lock;
mod state;
mod task;

pub use config::{DistributedExecutorConfig, NodeAddress};
pub use election::{
    ElectionConfig, ElectionEvent, ElectionState, HeartbeatMessage, LeaderElection, RaftState,
    VoteRequest, VoteResponse,
};
pub use executor::{
    ClusterHealth, ClusterStatus, DistributedExecutor, DistributedExecutorImpl, ExecutorError,
    NodeRole, NodeStatus,
};
pub use lock::{DistributedLock, LockError, LockGuard, LockManager};
pub use state::{
    OrderState, PositionState, StateSync, StateSyncConfig, StateSyncError, SyncMessage,
};
pub use task::{
    ExecutionTask, LoadBalancer, LoadBalancerConfig, NodeLoad, TaskAssignment, TaskDistributor,
    TaskId, TaskPriority, TaskSignal, TaskStatus,
};
