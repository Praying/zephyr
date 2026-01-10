//! Distributed executor configuration.
//!
//! This module defines configuration structures for the distributed executor
//! including node addresses and cluster settings.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Configuration for the distributed executor.
///
/// Defines cluster membership, timeouts, and synchronization intervals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedExecutorConfig {
    /// Unique identifier for this node.
    pub node_id: String,
    /// List of all nodes in the cluster (including self).
    pub cluster_nodes: Vec<NodeAddress>,
    /// Timeout for leader election.
    #[serde(with = "humantime_serde")]
    pub leader_election_timeout: Duration,
    /// Interval between heartbeat messages.
    #[serde(with = "humantime_serde")]
    pub heartbeat_interval: Duration,
    /// Interval for state synchronization.
    #[serde(with = "humantime_serde")]
    pub state_sync_interval: Duration,
    /// Maximum number of retries for failed operations.
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Timeout for acquiring distributed locks.
    #[serde(with = "humantime_serde", default = "default_lock_timeout")]
    pub lock_timeout: Duration,
    /// Enable automatic failover.
    #[serde(default = "default_auto_failover")]
    pub auto_failover: bool,
    /// Minimum number of nodes required for quorum.
    #[serde(default)]
    pub min_quorum: Option<usize>,
}

fn default_max_retries() -> u32 {
    3
}

fn default_lock_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_auto_failover() -> bool {
    true
}

impl DistributedExecutorConfig {
    /// Creates a new configuration builder.
    #[must_use]
    pub fn builder() -> DistributedExecutorConfigBuilder {
        DistributedExecutorConfigBuilder::default()
    }

    /// Returns the number of nodes in the cluster.
    #[must_use]
    pub fn cluster_size(&self) -> usize {
        self.cluster_nodes.len()
    }

    /// Returns the quorum size (majority of nodes).
    #[must_use]
    pub fn quorum_size(&self) -> usize {
        self.min_quorum
            .unwrap_or_else(|| self.cluster_size() / 2 + 1)
    }

    /// Finds a node by ID.
    #[must_use]
    pub fn find_node(&self, node_id: &str) -> Option<&NodeAddress> {
        self.cluster_nodes.iter().find(|n| n.id == node_id)
    }

    /// Returns all other nodes (excluding self).
    #[must_use]
    pub fn other_nodes(&self) -> Vec<&NodeAddress> {
        self.cluster_nodes
            .iter()
            .filter(|n| n.id != self.node_id)
            .collect()
    }

    /// Validates the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        if self.node_id.is_empty() {
            return Err(ConfigValidationError::EmptyNodeId);
        }

        if self.cluster_nodes.is_empty() {
            return Err(ConfigValidationError::EmptyCluster);
        }

        if !self.cluster_nodes.iter().any(|n| n.id == self.node_id) {
            return Err(ConfigValidationError::NodeNotInCluster);
        }

        if self.heartbeat_interval >= self.leader_election_timeout {
            return Err(ConfigValidationError::InvalidTimeouts);
        }

        if let Some(quorum) = self.min_quorum {
            if quorum > self.cluster_size() {
                return Err(ConfigValidationError::InvalidQuorum);
            }
        }

        Ok(())
    }
}

impl Default for DistributedExecutorConfig {
    fn default() -> Self {
        Self {
            node_id: String::new(),
            cluster_nodes: Vec::new(),
            leader_election_timeout: Duration::from_millis(500),
            heartbeat_interval: Duration::from_millis(150),
            state_sync_interval: Duration::from_secs(1),
            max_retries: default_max_retries(),
            lock_timeout: default_lock_timeout(),
            auto_failover: default_auto_failover(),
            min_quorum: None,
        }
    }
}

/// Builder for `DistributedExecutorConfig`.
#[derive(Debug, Default)]
pub struct DistributedExecutorConfigBuilder {
    node_id: Option<String>,
    cluster_nodes: Vec<NodeAddress>,
    leader_election_timeout: Option<Duration>,
    heartbeat_interval: Option<Duration>,
    state_sync_interval: Option<Duration>,
    max_retries: Option<u32>,
    lock_timeout: Option<Duration>,
    auto_failover: Option<bool>,
    min_quorum: Option<usize>,
}

impl DistributedExecutorConfigBuilder {
    /// Sets the node ID.
    #[must_use]
    pub fn node_id(mut self, id: impl Into<String>) -> Self {
        self.node_id = Some(id.into());
        self
    }

    /// Adds a node to the cluster.
    #[must_use]
    pub fn add_node(mut self, node: NodeAddress) -> Self {
        self.cluster_nodes.push(node);
        self
    }

    /// Sets all cluster nodes.
    #[must_use]
    pub fn cluster_nodes(mut self, nodes: Vec<NodeAddress>) -> Self {
        self.cluster_nodes = nodes;
        self
    }

    /// Sets the leader election timeout.
    #[must_use]
    pub fn leader_election_timeout(mut self, timeout: Duration) -> Self {
        self.leader_election_timeout = Some(timeout);
        self
    }

    /// Sets the heartbeat interval.
    #[must_use]
    pub fn heartbeat_interval(mut self, interval: Duration) -> Self {
        self.heartbeat_interval = Some(interval);
        self
    }

    /// Sets the state sync interval.
    #[must_use]
    pub fn state_sync_interval(mut self, interval: Duration) -> Self {
        self.state_sync_interval = Some(interval);
        self
    }

    /// Sets the maximum retries.
    #[must_use]
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = Some(retries);
        self
    }

    /// Sets the lock timeout.
    #[must_use]
    pub fn lock_timeout(mut self, timeout: Duration) -> Self {
        self.lock_timeout = Some(timeout);
        self
    }

    /// Sets auto failover.
    #[must_use]
    pub fn auto_failover(mut self, enabled: bool) -> Self {
        self.auto_failover = Some(enabled);
        self
    }

    /// Sets the minimum quorum.
    #[must_use]
    pub fn min_quorum(mut self, quorum: usize) -> Self {
        self.min_quorum = Some(quorum);
        self
    }

    /// Builds the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing.
    pub fn build(self) -> Result<DistributedExecutorConfig, ConfigBuildError> {
        let defaults = DistributedExecutorConfig::default();

        Ok(DistributedExecutorConfig {
            node_id: self
                .node_id
                .ok_or(ConfigBuildError::MissingField("node_id"))?,
            cluster_nodes: self.cluster_nodes,
            leader_election_timeout: self
                .leader_election_timeout
                .unwrap_or(defaults.leader_election_timeout),
            heartbeat_interval: self
                .heartbeat_interval
                .unwrap_or(defaults.heartbeat_interval),
            state_sync_interval: self
                .state_sync_interval
                .unwrap_or(defaults.state_sync_interval),
            max_retries: self.max_retries.unwrap_or(defaults.max_retries),
            lock_timeout: self.lock_timeout.unwrap_or(defaults.lock_timeout),
            auto_failover: self.auto_failover.unwrap_or(defaults.auto_failover),
            min_quorum: self.min_quorum,
        })
    }
}

/// Node address in the cluster.
///
/// Represents a single node's network location and metadata.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeAddress {
    /// Unique node identifier.
    pub id: String,
    /// Hostname or IP address.
    pub host: String,
    /// Port number.
    pub port: u16,
    /// Optional region for geographic distribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// Optional tags for node affinity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Node priority for leader election (higher = more preferred).
    #[serde(default)]
    pub priority: u32,
}

impl NodeAddress {
    /// Creates a new node address.
    #[must_use]
    pub fn new(id: impl Into<String>, host: impl Into<String>, port: u16) -> Self {
        Self {
            id: id.into(),
            host: host.into(),
            port,
            region: None,
            tags: Vec::new(),
            priority: 0,
        }
    }

    /// Sets the region.
    #[must_use]
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Adds a tag.
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Sets the priority.
    #[must_use]
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Returns the full address as "host:port".
    #[must_use]
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Returns the HTTP URL for this node.
    #[must_use]
    pub fn http_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }

    /// Checks if this node has a specific tag.
    #[must_use]
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

impl std::fmt::Display for NodeAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}:{}", self.id, self.host, self.port)
    }
}

/// Error validating configuration.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigValidationError {
    /// Node ID is empty.
    #[error("node ID cannot be empty")]
    EmptyNodeId,
    /// Cluster has no nodes.
    #[error("cluster must have at least one node")]
    EmptyCluster,
    /// This node is not in the cluster.
    #[error("this node is not in the cluster node list")]
    NodeNotInCluster,
    /// Invalid timeout configuration.
    #[error("heartbeat interval must be less than election timeout")]
    InvalidTimeouts,
    /// Invalid quorum configuration.
    #[error("quorum size cannot exceed cluster size")]
    InvalidQuorum,
}

/// Error building configuration.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigBuildError {
    /// Missing required field.
    #[error("missing required field: {0}")]
    MissingField(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_address_new() {
        let node = NodeAddress::new("node-1", "192.168.1.1", 8080);
        assert_eq!(node.id, "node-1");
        assert_eq!(node.host, "192.168.1.1");
        assert_eq!(node.port, 8080);
        assert!(node.region.is_none());
    }

    #[test]
    fn test_node_address_with_region() {
        let node = NodeAddress::new("node-1", "192.168.1.1", 8080).with_region("us-east-1");
        assert_eq!(node.region, Some("us-east-1".to_string()));
    }

    #[test]
    fn test_node_address_with_tags() {
        let node = NodeAddress::new("node-1", "192.168.1.1", 8080)
            .with_tag("binance")
            .with_tag("primary");
        assert!(node.has_tag("binance"));
        assert!(node.has_tag("primary"));
        assert!(!node.has_tag("okx"));
    }

    #[test]
    fn test_node_address_display() {
        let node = NodeAddress::new("node-1", "192.168.1.1", 8080);
        assert_eq!(format!("{node}"), "node-1@192.168.1.1:8080");
    }

    #[test]
    fn test_node_address_http_url() {
        let node = NodeAddress::new("node-1", "192.168.1.1", 8080);
        assert_eq!(node.http_url(), "http://192.168.1.1:8080");
    }

    #[test]
    fn test_config_builder() {
        let config = DistributedExecutorConfig::builder()
            .node_id("node-1")
            .add_node(NodeAddress::new("node-1", "192.168.1.1", 8080))
            .add_node(NodeAddress::new("node-2", "192.168.1.2", 8080))
            .add_node(NodeAddress::new("node-3", "192.168.1.3", 8080))
            .leader_election_timeout(Duration::from_millis(1000))
            .heartbeat_interval(Duration::from_millis(300))
            .build()
            .unwrap();

        assert_eq!(config.node_id, "node-1");
        assert_eq!(config.cluster_size(), 3);
        assert_eq!(config.quorum_size(), 2);
    }

    #[test]
    fn test_config_builder_missing_node_id() {
        let result = DistributedExecutorConfig::builder()
            .add_node(NodeAddress::new("node-1", "192.168.1.1", 8080))
            .build();
        assert!(matches!(
            result,
            Err(ConfigBuildError::MissingField("node_id"))
        ));
    }

    #[test]
    fn test_config_validate_empty_node_id() {
        let config = DistributedExecutorConfig {
            node_id: String::new(),
            cluster_nodes: vec![NodeAddress::new("node-1", "192.168.1.1", 8080)],
            ..Default::default()
        };
        assert!(matches!(
            config.validate(),
            Err(ConfigValidationError::EmptyNodeId)
        ));
    }

    #[test]
    fn test_config_validate_empty_cluster() {
        let config = DistributedExecutorConfig {
            node_id: "node-1".to_string(),
            cluster_nodes: vec![],
            ..Default::default()
        };
        assert!(matches!(
            config.validate(),
            Err(ConfigValidationError::EmptyCluster)
        ));
    }

    #[test]
    fn test_config_validate_node_not_in_cluster() {
        let config = DistributedExecutorConfig {
            node_id: "node-1".to_string(),
            cluster_nodes: vec![NodeAddress::new("node-2", "192.168.1.2", 8080)],
            ..Default::default()
        };
        assert!(matches!(
            config.validate(),
            Err(ConfigValidationError::NodeNotInCluster)
        ));
    }

    #[test]
    fn test_config_validate_invalid_timeouts() {
        let config = DistributedExecutorConfig {
            node_id: "node-1".to_string(),
            cluster_nodes: vec![NodeAddress::new("node-1", "192.168.1.1", 8080)],
            leader_election_timeout: Duration::from_millis(100),
            heartbeat_interval: Duration::from_millis(200),
            ..Default::default()
        };
        assert!(matches!(
            config.validate(),
            Err(ConfigValidationError::InvalidTimeouts)
        ));
    }

    #[test]
    fn test_config_validate_invalid_quorum() {
        let config = DistributedExecutorConfig {
            node_id: "node-1".to_string(),
            cluster_nodes: vec![NodeAddress::new("node-1", "192.168.1.1", 8080)],
            min_quorum: Some(5),
            ..Default::default()
        };
        assert!(matches!(
            config.validate(),
            Err(ConfigValidationError::InvalidQuorum)
        ));
    }

    #[test]
    fn test_config_other_nodes() {
        let config = DistributedExecutorConfig::builder()
            .node_id("node-1")
            .add_node(NodeAddress::new("node-1", "192.168.1.1", 8080))
            .add_node(NodeAddress::new("node-2", "192.168.1.2", 8080))
            .add_node(NodeAddress::new("node-3", "192.168.1.3", 8080))
            .build()
            .unwrap();

        let others = config.other_nodes();
        assert_eq!(others.len(), 2);
        assert!(others.iter().all(|n| n.id != "node-1"));
    }

    #[test]
    fn test_config_find_node() {
        let config = DistributedExecutorConfig::builder()
            .node_id("node-1")
            .add_node(NodeAddress::new("node-1", "192.168.1.1", 8080))
            .add_node(NodeAddress::new("node-2", "192.168.1.2", 8080))
            .build()
            .unwrap();

        assert!(config.find_node("node-1").is_some());
        assert!(config.find_node("node-2").is_some());
        assert!(config.find_node("node-3").is_none());
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let config = DistributedExecutorConfig::builder()
            .node_id("node-1")
            .add_node(NodeAddress::new("node-1", "192.168.1.1", 8080).with_region("us-east-1"))
            .leader_election_timeout(Duration::from_millis(500))
            .heartbeat_interval(Duration::from_millis(150))
            .build()
            .unwrap();

        let json = serde_json::to_string(&config).unwrap();
        let parsed: DistributedExecutorConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.node_id, parsed.node_id);
        assert_eq!(config.cluster_nodes.len(), parsed.cluster_nodes.len());
    }
}
