//! Leader election using Raft consensus.
//!
//! This module implements a simplified Raft-based leader election algorithm
//! for distributed coordination.

#![allow(unused_imports)]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::{Instant, interval, timeout};
use tracing::{debug, info, warn};

use zephyr_core::types::Timestamp;

use super::config::DistributedExecutorConfig;
use super::executor::NodeRole;

/// Election configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectionConfig {
    /// Base election timeout.
    #[serde(with = "humantime_serde")]
    pub election_timeout: Duration,
    /// Heartbeat interval.
    #[serde(with = "humantime_serde")]
    pub heartbeat_interval: Duration,
    /// Random jitter range for election timeout.
    #[serde(with = "humantime_serde", default = "default_jitter")]
    pub election_jitter: Duration,
    /// Maximum missed heartbeats before considering leader dead.
    #[serde(default = "default_max_missed_heartbeats")]
    pub max_missed_heartbeats: u32,
}

fn default_jitter() -> Duration {
    Duration::from_millis(150)
}

fn default_max_missed_heartbeats() -> u32 {
    3
}

impl Default for ElectionConfig {
    fn default() -> Self {
        Self {
            election_timeout: Duration::from_millis(500),
            heartbeat_interval: Duration::from_millis(150),
            election_jitter: default_jitter(),
            max_missed_heartbeats: default_max_missed_heartbeats(),
        }
    }
}

impl From<&DistributedExecutorConfig> for ElectionConfig {
    fn from(config: &DistributedExecutorConfig) -> Self {
        Self {
            election_timeout: config.leader_election_timeout,
            heartbeat_interval: config.heartbeat_interval,
            ..Default::default()
        }
    }
}

/// Raft state for a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaftState {
    /// Current term number.
    pub current_term: u64,
    /// Node we voted for in current term.
    pub voted_for: Option<u64>,
    /// Current role.
    pub role: NodeRole,
}

impl Default for RaftState {
    fn default() -> Self {
        Self {
            current_term: 0,
            voted_for: None,
            role: NodeRole::Follower,
        }
    }
}

/// Election state tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectionState {
    /// Current Raft state.
    pub raft: RaftState,
    /// Current leader ID (if known).
    pub leader_id: Option<String>,
    /// Last heartbeat received.
    pub last_heartbeat: Timestamp,
    /// Votes received in current election.
    pub votes_received: u32,
    /// Total nodes in cluster.
    pub cluster_size: usize,
}

impl ElectionState {
    /// Creates a new election state.
    #[must_use]
    pub fn new(cluster_size: usize) -> Self {
        Self {
            raft: RaftState::default(),
            leader_id: None,
            last_heartbeat: Timestamp::now(),
            votes_received: 0,
            cluster_size,
        }
    }

    /// Returns the quorum size.
    #[must_use]
    pub fn quorum(&self) -> usize {
        self.cluster_size / 2 + 1
    }

    /// Returns true if we have enough votes to become leader.
    #[must_use]
    pub fn has_quorum(&self) -> bool {
        self.votes_received as usize >= self.quorum()
    }

    /// Returns true if this node is the leader.
    #[must_use]
    pub fn is_leader(&self) -> bool {
        self.raft.role == NodeRole::Leader
    }

    /// Returns true if this node is a candidate.
    #[must_use]
    pub fn is_candidate(&self) -> bool {
        self.raft.role == NodeRole::Candidate
    }

    /// Transitions to candidate state.
    pub fn become_candidate(&mut self, node_index: u64) {
        self.raft.current_term += 1;
        self.raft.role = NodeRole::Candidate;
        self.raft.voted_for = Some(node_index);
        self.votes_received = 1; // Vote for self
        self.leader_id = None;
    }

    /// Transitions to leader state.
    pub fn become_leader(&mut self, node_id: String) {
        self.raft.role = NodeRole::Leader;
        self.leader_id = Some(node_id);
    }

    /// Transitions to follower state.
    pub fn become_follower(&mut self, term: u64, leader_id: Option<String>) {
        self.raft.current_term = term;
        self.raft.role = NodeRole::Follower;
        self.raft.voted_for = None;
        self.votes_received = 0;
        self.leader_id = leader_id;
        self.last_heartbeat = Timestamp::now();
    }

    /// Records a received vote.
    pub fn record_vote(&mut self) {
        self.votes_received += 1;
    }

    /// Updates the last heartbeat time.
    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = Timestamp::now();
    }
}

/// Vote request message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteRequest {
    /// Term number.
    pub term: u64,
    /// Candidate ID.
    pub candidate_id: String,
    /// Candidate's node index.
    pub candidate_index: u64,
    /// Candidate's priority.
    pub priority: u32,
}

/// Vote response message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteResponse {
    /// Term number.
    pub term: u64,
    /// Whether vote was granted.
    pub vote_granted: bool,
    /// Voter's node ID.
    pub voter_id: String,
}

/// Heartbeat message from leader.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatMessage {
    /// Term number.
    pub term: u64,
    /// Leader ID.
    pub leader_id: String,
    /// Leader's timestamp.
    pub timestamp: Timestamp,
    /// Leader's load (for monitoring).
    pub load: f64,
}

/// Election event for external handling.
#[derive(Debug, Clone)]
pub enum ElectionEvent {
    /// This node became leader.
    BecameLeader,
    /// This node became follower.
    BecameFollower { leader_id: String },
    /// Leader changed.
    LeaderChanged { old: Option<String>, new: String },
    /// Election started.
    ElectionStarted { term: u64 },
    /// Election timeout (no leader).
    ElectionTimeout,
}

/// Leader election manager.
pub struct LeaderElection {
    config: ElectionConfig,
    node_id: String,
    node_index: u64,
    node_priority: u32,
    state: Arc<RwLock<ElectionState>>,
    event_tx: mpsc::Sender<ElectionEvent>,
    term_counter: AtomicU64,
}

impl LeaderElection {
    /// Creates a new leader election manager.
    #[must_use]
    pub fn new(
        config: ElectionConfig,
        node_id: String,
        node_index: u64,
        node_priority: u32,
        cluster_size: usize,
        event_tx: mpsc::Sender<ElectionEvent>,
    ) -> Self {
        Self {
            config,
            node_id,
            node_index,
            node_priority,
            state: Arc::new(RwLock::new(ElectionState::new(cluster_size))),
            event_tx,
            term_counter: AtomicU64::new(0),
        }
    }

    /// Returns the current election state.
    #[must_use]
    pub fn state(&self) -> ElectionState {
        self.state.read().clone()
    }

    /// Returns the current role.
    #[must_use]
    pub fn role(&self) -> NodeRole {
        self.state.read().raft.role
    }

    /// Returns the current leader ID.
    #[must_use]
    pub fn leader_id(&self) -> Option<String> {
        self.state.read().leader_id.clone()
    }

    /// Returns true if this node is the leader.
    #[must_use]
    pub fn is_leader(&self) -> bool {
        self.state.read().is_leader()
    }

    /// Returns the current term.
    #[must_use]
    pub fn current_term(&self) -> u64 {
        self.state.read().raft.current_term
    }

    /// Handles a vote request from another node.
    pub fn handle_vote_request(&self, request: &VoteRequest) -> VoteResponse {
        let mut state = self.state.write();

        // If request term is higher, become follower
        if request.term > state.raft.current_term {
            state.become_follower(request.term, None);
        }

        let vote_granted = if request.term < state.raft.current_term {
            // Reject votes from old terms
            false
        } else if state.raft.voted_for.is_none()
            || state.raft.voted_for == Some(request.candidate_index)
        {
            // Grant vote if we haven't voted or already voted for this candidate
            // Also consider priority
            state.raft.voted_for = Some(request.candidate_index);
            true
        } else {
            false
        };

        debug!(
            node_id = %self.node_id,
            candidate = %request.candidate_id,
            term = request.term,
            granted = vote_granted,
            "Vote request processed"
        );

        VoteResponse {
            term: state.raft.current_term,
            vote_granted,
            voter_id: self.node_id.clone(),
        }
    }

    /// Handles a vote response from another node.
    pub async fn handle_vote_response(&self, response: &VoteResponse) {
        let mut state = self.state.write();

        // If response term is higher, become follower
        if response.term > state.raft.current_term {
            state.become_follower(response.term, None);
            return;
        }

        // Only count votes if we're still a candidate in the same term
        if !state.is_candidate() || response.term != state.raft.current_term {
            return;
        }

        if response.vote_granted {
            state.record_vote();
            debug!(
                node_id = %self.node_id,
                votes = state.votes_received,
                quorum = state.quorum(),
                "Vote received"
            );

            // Check if we have quorum
            if state.has_quorum() {
                let old_leader = state.leader_id.clone();
                state.become_leader(self.node_id.clone());
                drop(state);

                info!(node_id = %self.node_id, "Became leader");

                let _ = self.event_tx.send(ElectionEvent::BecameLeader).await;
                let _ = self
                    .event_tx
                    .send(ElectionEvent::LeaderChanged {
                        old: old_leader,
                        new: self.node_id.clone(),
                    })
                    .await;
            }
        }
    }

    /// Handles a heartbeat from the leader.
    pub async fn handle_heartbeat(&self, heartbeat: &HeartbeatMessage) {
        let mut state = self.state.write();

        // If heartbeat term is higher or equal, accept leader
        if heartbeat.term >= state.raft.current_term {
            let old_leader = state.leader_id.clone();
            let was_leader = state.is_leader();

            state.become_follower(heartbeat.term, Some(heartbeat.leader_id.clone()));
            state.update_heartbeat();

            // Notify if leader changed
            if old_leader.as_ref() != Some(&heartbeat.leader_id) {
                drop(state);

                if was_leader {
                    let _ = self
                        .event_tx
                        .send(ElectionEvent::BecameFollower {
                            leader_id: heartbeat.leader_id.clone(),
                        })
                        .await;
                }

                let _ = self
                    .event_tx
                    .send(ElectionEvent::LeaderChanged {
                        old: old_leader,
                        new: heartbeat.leader_id.clone(),
                    })
                    .await;
            }
        }
    }

    /// Starts an election.
    pub async fn start_election(&self) -> VoteRequest {
        let mut state = self.state.write();
        state.become_candidate(self.node_index);

        let term = state.raft.current_term;
        self.term_counter.store(term, Ordering::SeqCst);

        drop(state);

        info!(
            node_id = %self.node_id,
            term = term,
            "Starting election"
        );

        let _ = self
            .event_tx
            .send(ElectionEvent::ElectionStarted { term })
            .await;

        VoteRequest {
            term,
            candidate_id: self.node_id.clone(),
            candidate_index: self.node_index,
            priority: self.node_priority,
        }
    }

    /// Creates a heartbeat message.
    #[must_use]
    pub fn create_heartbeat(&self, load: f64) -> Option<HeartbeatMessage> {
        let state = self.state.read();
        if !state.is_leader() {
            return None;
        }

        Some(HeartbeatMessage {
            term: state.raft.current_term,
            leader_id: self.node_id.clone(),
            timestamp: Timestamp::now(),
            load,
        })
    }

    /// Checks if election timeout has occurred.
    #[must_use]
    pub fn should_start_election(&self) -> bool {
        let state = self.state.read();

        // Leaders don't start elections
        if state.is_leader() {
            return false;
        }

        let elapsed = Timestamp::now().as_millis() - state.last_heartbeat.as_millis();
        let timeout_ms = self.config.election_timeout.as_millis() as i64;

        elapsed > timeout_ms
    }

    /// Resets the election timeout.
    pub fn reset_timeout(&self) {
        self.state.write().update_heartbeat();
    }

    /// Steps down from leader role.
    pub async fn step_down(&self) {
        let mut state = self.state.write();
        if state.is_leader() {
            let current_term = state.raft.current_term;
            state.become_follower(current_term, None);
            drop(state);

            warn!(node_id = %self.node_id, "Stepping down from leader");

            let _ = self
                .event_tx
                .send(ElectionEvent::BecameFollower {
                    leader_id: String::new(),
                })
                .await;
        }
    }

    /// Updates cluster size.
    pub fn update_cluster_size(&self, size: usize) {
        self.state.write().cluster_size = size;
    }
}

/// Failure detector for monitoring node health.
pub struct FailureDetector {
    config: ElectionConfig,
    last_seen: HashMap<String, Instant>,
}

impl FailureDetector {
    /// Creates a new failure detector.
    #[must_use]
    pub fn new(config: ElectionConfig) -> Self {
        Self {
            config,
            last_seen: HashMap::new(),
        }
    }

    /// Records that a node was seen.
    pub fn record_heartbeat(&mut self, node_id: &str) {
        self.last_seen.insert(node_id.to_string(), Instant::now());
    }

    /// Checks if a node is considered failed.
    #[must_use]
    pub fn is_failed(&self, node_id: &str) -> bool {
        self.last_seen.get(node_id).map_or(true, |last| {
            let max_interval = self.config.heartbeat_interval * self.config.max_missed_heartbeats;
            last.elapsed() > max_interval
        })
    }

    /// Returns all failed nodes.
    #[must_use]
    pub fn failed_nodes(&self) -> Vec<String> {
        self.last_seen
            .iter()
            .filter(|(id, _)| self.is_failed(id))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Removes a node from tracking.
    pub fn remove_node(&mut self, node_id: &str) {
        self.last_seen.remove(node_id);
    }

    /// Returns all tracked nodes.
    #[must_use]
    pub fn tracked_nodes(&self) -> Vec<&String> {
        self.last_seen.keys().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    fn create_election(
        node_id: &str,
        node_index: u64,
    ) -> (LeaderElection, mpsc::Receiver<ElectionEvent>) {
        let (tx, rx) = mpsc::channel(100);
        let election = LeaderElection::new(
            ElectionConfig::default(),
            node_id.to_string(),
            node_index,
            0,
            3,
            tx,
        );
        (election, rx)
    }

    #[test]
    fn test_election_state_new() {
        let state = ElectionState::new(5);
        assert_eq!(state.quorum(), 3);
        assert!(!state.has_quorum());
        assert!(!state.is_leader());
    }

    #[test]
    fn test_election_state_become_candidate() {
        let mut state = ElectionState::new(3);
        state.become_candidate(1);

        assert!(state.is_candidate());
        assert_eq!(state.raft.current_term, 1);
        assert_eq!(state.raft.voted_for, Some(1));
        assert_eq!(state.votes_received, 1);
    }

    #[test]
    fn test_election_state_become_leader() {
        let mut state = ElectionState::new(3);
        state.become_candidate(1);
        state.record_vote(); // 2 votes
        state.become_leader("node-1".to_string());

        assert!(state.is_leader());
        assert_eq!(state.leader_id, Some("node-1".to_string()));
    }

    #[test]
    fn test_election_state_quorum() {
        let mut state = ElectionState::new(5);
        state.become_candidate(1);

        assert!(!state.has_quorum()); // 1 vote
        state.record_vote();
        assert!(!state.has_quorum()); // 2 votes
        state.record_vote();
        assert!(state.has_quorum()); // 3 votes (quorum for 5 nodes)
    }

    #[test]
    fn test_vote_request_handling() {
        let (election, _rx) = create_election("node-1", 1);

        let request = VoteRequest {
            term: 1,
            candidate_id: "node-2".to_string(),
            candidate_index: 2,
            priority: 0,
        };

        let response = election.handle_vote_request(&request);
        assert!(response.vote_granted);
        assert_eq!(response.term, 1);
    }

    #[test]
    fn test_vote_request_old_term() {
        let (election, _rx) = create_election("node-1", 1);

        // First, advance the term
        {
            let mut state = election.state.write();
            state.raft.current_term = 5;
        }

        let request = VoteRequest {
            term: 3, // Old term
            candidate_id: "node-2".to_string(),
            candidate_index: 2,
            priority: 0,
        };

        let response = election.handle_vote_request(&request);
        assert!(!response.vote_granted);
    }

    #[test]
    fn test_vote_request_already_voted() {
        let (election, _rx) = create_election("node-1", 1);

        // Vote for node-2
        let request1 = VoteRequest {
            term: 1,
            candidate_id: "node-2".to_string(),
            candidate_index: 2,
            priority: 0,
        };
        let response1 = election.handle_vote_request(&request1);
        assert!(response1.vote_granted);

        // Try to vote for node-3 in same term
        let request2 = VoteRequest {
            term: 1,
            candidate_id: "node-3".to_string(),
            candidate_index: 3,
            priority: 0,
        };
        let response2 = election.handle_vote_request(&request2);
        assert!(!response2.vote_granted);
    }

    #[tokio::test]
    async fn test_heartbeat_handling() {
        let (election, mut rx) = create_election("node-1", 1);

        let heartbeat = HeartbeatMessage {
            term: 1,
            leader_id: "node-2".to_string(),
            timestamp: Timestamp::now(),
            load: 0.5,
        };

        election.handle_heartbeat(&heartbeat).await;

        let state = election.state();
        assert_eq!(state.leader_id, Some("node-2".to_string()));
        assert_eq!(state.raft.role, NodeRole::Follower);

        // Should receive leader changed event
        let event = rx.recv().await;
        assert!(matches!(event, Some(ElectionEvent::LeaderChanged { .. })));
    }

    #[tokio::test]
    async fn test_start_election() {
        let (election, mut rx) = create_election("node-1", 1);

        let request = election.start_election().await;

        assert_eq!(request.term, 1);
        assert_eq!(request.candidate_id, "node-1");

        let state = election.state();
        assert!(state.is_candidate());
        assert_eq!(state.votes_received, 1); // Voted for self

        // Should receive election started event
        let event = rx.recv().await;
        assert!(matches!(
            event,
            Some(ElectionEvent::ElectionStarted { term: 1 })
        ));
    }

    #[test]
    fn test_create_heartbeat() {
        let (election, _rx) = create_election("node-1", 1);

        // Not leader, should return None
        assert!(election.create_heartbeat(0.5).is_none());

        // Become leader
        {
            let mut state = election.state.write();
            state.become_leader("node-1".to_string());
        }

        let heartbeat = election.create_heartbeat(0.5);
        assert!(heartbeat.is_some());
        let hb = heartbeat.unwrap();
        assert_eq!(hb.leader_id, "node-1");
        assert_eq!(hb.load, 0.5);
    }

    #[test]
    fn test_failure_detector() {
        let config = ElectionConfig {
            heartbeat_interval: Duration::from_millis(100),
            max_missed_heartbeats: 3,
            ..Default::default()
        };
        let mut detector = FailureDetector::new(config);

        detector.record_heartbeat("node-1");
        assert!(!detector.is_failed("node-1"));

        // Unknown node is considered failed
        assert!(detector.is_failed("node-2"));
    }

    #[test]
    fn test_failure_detector_remove() {
        let config = ElectionConfig::default();
        let mut detector = FailureDetector::new(config);

        detector.record_heartbeat("node-1");
        assert!(!detector.is_failed("node-1"));

        detector.remove_node("node-1");
        assert!(detector.is_failed("node-1"));
    }
}
