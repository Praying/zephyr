//! Distributed locking for preventing duplicate operations.
//!
//! This module provides distributed locking capabilities to ensure
//! exclusive access to resources across cluster nodes.

#![allow(unused_imports)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::collapsible_if)]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::oneshot;
use tokio::time::{Instant, sleep};
use tracing::{debug, warn};

use zephyr_core::types::Timestamp;

/// Error type for lock operations.
#[derive(Debug, Clone, Error)]
pub enum LockError {
    /// Lock acquisition timed out.
    #[error("lock acquisition timed out after {0:?}")]
    Timeout(Duration),

    /// Lock is held by another node.
    #[error("lock '{key}' is held by node '{holder}'")]
    AlreadyHeld {
        /// The lock key.
        key: String,
        /// The node holding the lock.
        holder: String,
    },

    /// Lock not found.
    #[error("lock '{0}' not found")]
    NotFound(String),

    /// Invalid lock state.
    #[error("invalid lock state: {0}")]
    InvalidState(String),

    /// Network error during lock operation.
    #[error("network error: {0}")]
    NetworkError(String),

    /// Lock was released by another process.
    #[error("lock was released externally")]
    ReleasedExternally,
}

/// Unique identifier for a lock.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LockId(String);

impl LockId {
    /// Creates a new lock ID.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generates a unique lock ID.
    #[must_use]
    pub fn generate() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        Self(format!("lock-{timestamp}-{count}"))
    }

    /// Returns the ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for LockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Lock entry in the lock table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockEntry {
    /// Lock key.
    pub key: String,
    /// Lock ID (for verification).
    pub lock_id: LockId,
    /// Node holding the lock.
    pub holder: String,
    /// When the lock was acquired.
    pub acquired_at: Timestamp,
    /// When the lock expires.
    pub expires_at: Timestamp,
    /// Lock version for optimistic concurrency.
    pub version: u64,
}

impl LockEntry {
    /// Creates a new lock entry.
    #[must_use]
    pub fn new(key: impl Into<String>, holder: impl Into<String>, ttl: Duration) -> Self {
        let now = Timestamp::now();
        let expires_millis = now.as_millis() + ttl.as_millis() as i64;
        let expires_at = Timestamp::new(expires_millis).unwrap_or_else(|_| Timestamp::now());

        Self {
            key: key.into(),
            lock_id: LockId::generate(),
            holder: holder.into(),
            acquired_at: now,
            expires_at,
            version: 1,
        }
    }

    /// Returns true if the lock has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Timestamp::now() > self.expires_at
    }

    /// Extends the lock TTL.
    pub fn extend(&mut self, ttl: Duration) {
        let expires_millis = Timestamp::now().as_millis() + ttl.as_millis() as i64;
        self.expires_at = Timestamp::new(expires_millis).unwrap_or_else(|_| Timestamp::now());
        self.version += 1;
    }

    /// Returns the remaining TTL.
    #[must_use]
    pub fn remaining_ttl(&self) -> Duration {
        let remaining_ms = self.expires_at.as_millis() - Timestamp::now().as_millis();
        if remaining_ms > 0 {
            Duration::from_millis(remaining_ms as u64)
        } else {
            Duration::ZERO
        }
    }
}

/// Guard that releases the lock when dropped.
pub struct LockGuard {
    key: String,
    lock_id: LockId,
    manager: Arc<LockManager>,
    released: bool,
}

impl LockGuard {
    /// Creates a new lock guard.
    fn new(key: String, lock_id: LockId, manager: Arc<LockManager>) -> Self {
        Self {
            key,
            lock_id,
            manager,
            released: false,
        }
    }

    /// Returns the lock key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns the lock ID.
    #[must_use]
    pub fn lock_id(&self) -> &LockId {
        &self.lock_id
    }

    /// Extends the lock TTL.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock is no longer held.
    pub fn extend(&self, ttl: Duration) -> Result<(), LockError> {
        self.manager.extend_lock(&self.key, &self.lock_id, ttl)
    }

    /// Releases the lock explicitly.
    pub fn release(mut self) -> Result<(), LockError> {
        self.released = true;
        self.manager.release_lock(&self.key, &self.lock_id)
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        if !self.released {
            if let Err(e) = self.manager.release_lock(&self.key, &self.lock_id) {
                warn!(key = %self.key, error = %e, "Failed to release lock on drop");
            }
        }
    }
}

/// Distributed lock interface.
pub trait DistributedLock: Send + Sync {
    /// Attempts to acquire a lock.
    fn try_acquire(&self, key: &str, ttl: Duration) -> Result<LockGuard, LockError>;

    /// Acquires a lock, waiting up to timeout.
    fn acquire(
        &self,
        key: &str,
        ttl: Duration,
        timeout: Duration,
    ) -> impl std::future::Future<Output = Result<LockGuard, LockError>> + Send;

    /// Releases a lock.
    fn release(&self, key: &str, lock_id: &LockId) -> Result<(), LockError>;

    /// Extends a lock's TTL.
    fn extend(&self, key: &str, lock_id: &LockId, ttl: Duration) -> Result<(), LockError>;

    /// Checks if a lock is held.
    fn is_locked(&self, key: &str) -> bool;

    /// Gets lock information.
    fn get_lock_info(&self, key: &str) -> Option<LockEntry>;
}

/// Lock manager for managing distributed locks.
pub struct LockManager {
    node_id: String,
    locks: RwLock<HashMap<String, LockEntry>>,
    #[allow(dead_code)]
    default_ttl: Duration,
    self_ref: RwLock<Option<Arc<LockManager>>>,
}

impl LockManager {
    /// Creates a new lock manager.
    #[must_use]
    pub fn new(node_id: impl Into<String>, default_ttl: Duration) -> Arc<Self> {
        let manager = Arc::new(Self {
            node_id: node_id.into(),
            locks: RwLock::new(HashMap::new()),
            default_ttl,
            self_ref: RwLock::new(None),
        });

        // Store self reference for guards
        *manager.self_ref.write() = Some(Arc::clone(&manager));

        manager
    }

    /// Returns the node ID.
    #[must_use]
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// Attempts to acquire a lock without waiting.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock is already held.
    pub fn try_acquire_lock(
        self: &Arc<Self>,
        key: &str,
        ttl: Duration,
    ) -> Result<LockGuard, LockError> {
        let mut locks = self.locks.write();

        // Check if lock exists and is not expired
        if let Some(entry) = locks.get(key) {
            if !entry.is_expired() {
                return Err(LockError::AlreadyHeld {
                    key: key.to_string(),
                    holder: entry.holder.clone(),
                });
            }
        }

        // Create new lock entry
        let entry = LockEntry::new(key, &self.node_id, ttl);
        let lock_id = entry.lock_id.clone();

        debug!(
            node_id = %self.node_id,
            key = %key,
            lock_id = %lock_id,
            ttl = ?ttl,
            "Lock acquired"
        );

        locks.insert(key.to_string(), entry);

        Ok(LockGuard::new(key.to_string(), lock_id, Arc::clone(self)))
    }

    /// Acquires a lock, waiting up to the specified timeout.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock cannot be acquired within the timeout.
    pub async fn acquire_lock(
        self: &Arc<Self>,
        key: &str,
        ttl: Duration,
        timeout: Duration,
    ) -> Result<LockGuard, LockError> {
        let start = Instant::now();
        let retry_interval = Duration::from_millis(50);

        loop {
            match self.try_acquire_lock(key, ttl) {
                Ok(guard) => return Ok(guard),
                Err(LockError::AlreadyHeld { .. }) => {
                    if start.elapsed() >= timeout {
                        return Err(LockError::Timeout(timeout));
                    }
                    sleep(retry_interval).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Releases a lock.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock is not held or held by another node.
    pub fn release_lock(&self, key: &str, lock_id: &LockId) -> Result<(), LockError> {
        let mut locks = self.locks.write();

        let entry = locks
            .get(key)
            .ok_or_else(|| LockError::NotFound(key.to_string()))?;

        // Verify lock ID matches
        if &entry.lock_id != lock_id {
            return Err(LockError::InvalidState(format!(
                "lock ID mismatch: expected {}, got {}",
                entry.lock_id, lock_id
            )));
        }

        // Verify holder matches
        if entry.holder != self.node_id {
            return Err(LockError::InvalidState(format!(
                "lock held by different node: {}",
                entry.holder
            )));
        }

        debug!(
            node_id = %self.node_id,
            key = %key,
            lock_id = %lock_id,
            "Lock released"
        );

        locks.remove(key);
        Ok(())
    }

    /// Extends a lock's TTL.
    ///
    /// # Errors
    ///
    /// Returns an error if the lock is not held or held by another node.
    pub fn extend_lock(&self, key: &str, lock_id: &LockId, ttl: Duration) -> Result<(), LockError> {
        let mut locks = self.locks.write();

        let entry = locks
            .get_mut(key)
            .ok_or_else(|| LockError::NotFound(key.to_string()))?;

        // Verify lock ID matches
        if &entry.lock_id != lock_id {
            return Err(LockError::InvalidState(format!(
                "lock ID mismatch: expected {}, got {}",
                entry.lock_id, lock_id
            )));
        }

        // Verify holder matches
        if entry.holder != self.node_id {
            return Err(LockError::InvalidState(format!(
                "lock held by different node: {}",
                entry.holder
            )));
        }

        entry.extend(ttl);

        debug!(
            node_id = %self.node_id,
            key = %key,
            lock_id = %lock_id,
            new_ttl = ?ttl,
            "Lock extended"
        );

        Ok(())
    }

    /// Checks if a lock is currently held.
    #[must_use]
    pub fn is_locked(&self, key: &str) -> bool {
        self.locks
            .read()
            .get(key)
            .map_or(false, |e| !e.is_expired())
    }

    /// Gets information about a lock.
    #[must_use]
    pub fn get_lock_info(&self, key: &str) -> Option<LockEntry> {
        self.locks.read().get(key).cloned()
    }

    /// Returns all currently held locks.
    #[must_use]
    pub fn all_locks(&self) -> Vec<LockEntry> {
        self.locks
            .read()
            .values()
            .filter(|e| !e.is_expired())
            .cloned()
            .collect()
    }

    /// Cleans up expired locks.
    pub fn cleanup_expired(&self) -> usize {
        let mut locks = self.locks.write();
        let before = locks.len();
        locks.retain(|_, e| !e.is_expired());
        before - locks.len()
    }

    /// Forces release of a lock (for recovery).
    ///
    /// This should only be used during node failure recovery.
    pub fn force_release(&self, key: &str) -> bool {
        let mut locks = self.locks.write();
        locks.remove(key).is_some()
    }

    /// Transfers locks from a failed node.
    ///
    /// This is used during failover to take over locks from a failed node.
    pub fn transfer_locks(&self, from_node: &str) -> Vec<String> {
        let mut locks = self.locks.write();
        let mut transferred = Vec::new();

        for (key, entry) in locks.iter_mut() {
            if entry.holder == from_node && !entry.is_expired() {
                entry.holder = self.node_id.clone();
                entry.version += 1;
                transferred.push(key.clone());
            }
        }

        transferred
    }
}

impl DistributedLock for Arc<LockManager> {
    fn try_acquire(&self, key: &str, ttl: Duration) -> Result<LockGuard, LockError> {
        self.try_acquire_lock(key, ttl)
    }

    async fn acquire(
        &self,
        key: &str,
        ttl: Duration,
        timeout: Duration,
    ) -> Result<LockGuard, LockError> {
        self.acquire_lock(key, ttl, timeout).await
    }

    fn release(&self, key: &str, lock_id: &LockId) -> Result<(), LockError> {
        self.release_lock(key, lock_id)
    }

    fn extend(&self, key: &str, lock_id: &LockId, ttl: Duration) -> Result<(), LockError> {
        self.extend_lock(key, lock_id, ttl)
    }

    fn is_locked(&self, key: &str) -> bool {
        LockManager::is_locked(self, key)
    }

    fn get_lock_info(&self, key: &str) -> Option<LockEntry> {
        LockManager::get_lock_info(self, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_entry_new() {
        let entry = LockEntry::new("test-key", "node-1", Duration::from_secs(30));
        assert_eq!(entry.key, "test-key");
        assert_eq!(entry.holder, "node-1");
        assert!(!entry.is_expired());
    }

    #[test]
    fn test_lock_entry_expired() {
        let entry = LockEntry::new("test-key", "node-1", Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(10));
        assert!(entry.is_expired());
    }

    #[test]
    fn test_lock_entry_extend() {
        let mut entry = LockEntry::new("test-key", "node-1", Duration::from_secs(1));
        let old_version = entry.version;
        entry.extend(Duration::from_secs(60));
        assert_eq!(entry.version, old_version + 1);
        assert!(!entry.is_expired());
    }

    #[test]
    fn test_lock_manager_try_acquire() {
        let manager = LockManager::new("node-1", Duration::from_secs(30));

        let guard = manager.try_acquire_lock("test-key", Duration::from_secs(30));
        assert!(guard.is_ok());

        // Second acquire should fail
        let guard2 = manager.try_acquire_lock("test-key", Duration::from_secs(30));
        assert!(matches!(guard2, Err(LockError::AlreadyHeld { .. })));
    }

    #[test]
    fn test_lock_manager_release() {
        let manager = LockManager::new("node-1", Duration::from_secs(30));

        let guard = manager
            .try_acquire_lock("test-key", Duration::from_secs(30))
            .unwrap();
        let _lock_id = guard.lock_id().clone();

        // Release via guard
        assert!(guard.release().is_ok());

        // Should be able to acquire again
        let guard2 = manager.try_acquire_lock("test-key", Duration::from_secs(30));
        assert!(guard2.is_ok());
    }

    #[test]
    fn test_lock_manager_extend() {
        let manager = LockManager::new("node-1", Duration::from_secs(30));

        let guard = manager
            .try_acquire_lock("test-key", Duration::from_secs(1))
            .unwrap();

        // Extend the lock
        assert!(guard.extend(Duration::from_secs(60)).is_ok());

        let info = manager.get_lock_info("test-key").unwrap();
        assert!(info.remaining_ttl() > Duration::from_secs(30));
    }

    #[test]
    fn test_lock_manager_is_locked() {
        let manager = LockManager::new("node-1", Duration::from_secs(30));

        assert!(!manager.is_locked("test-key"));

        let _guard = manager
            .try_acquire_lock("test-key", Duration::from_secs(30))
            .unwrap();
        assert!(manager.is_locked("test-key"));
    }

    #[test]
    fn test_lock_manager_cleanup_expired() {
        let manager = LockManager::new("node-1", Duration::from_secs(30));

        // Acquire a lock with short TTL
        let _guard = manager
            .try_acquire_lock("test-key", Duration::from_millis(1))
            .unwrap();
        std::mem::forget(_guard); // Don't release on drop

        std::thread::sleep(Duration::from_millis(10));

        let cleaned = manager.cleanup_expired();
        assert_eq!(cleaned, 1);
        assert!(!manager.is_locked("test-key"));
    }

    #[test]
    fn test_lock_manager_force_release() {
        let manager = LockManager::new("node-1", Duration::from_secs(30));

        let _guard = manager
            .try_acquire_lock("test-key", Duration::from_secs(30))
            .unwrap();
        std::mem::forget(_guard);

        assert!(manager.is_locked("test-key"));
        assert!(manager.force_release("test-key"));
        assert!(!manager.is_locked("test-key"));
    }

    #[test]
    fn test_lock_manager_transfer_locks() {
        let manager = LockManager::new("node-2", Duration::from_secs(30));

        // Manually insert a lock from another node
        {
            let mut locks = manager.locks.write();
            locks.insert(
                "test-key".to_string(),
                LockEntry::new("test-key", "node-1", Duration::from_secs(30)),
            );
        }

        let transferred = manager.transfer_locks("node-1");
        assert_eq!(transferred.len(), 1);
        assert_eq!(transferred[0], "test-key");

        let info = manager.get_lock_info("test-key").unwrap();
        assert_eq!(info.holder, "node-2");
    }

    #[tokio::test]
    async fn test_lock_manager_acquire_with_timeout() {
        let manager = LockManager::new("node-1", Duration::from_secs(30));

        // Acquire first lock
        let _guard1 = manager
            .try_acquire_lock("test-key", Duration::from_millis(100))
            .unwrap();

        // Try to acquire with short timeout - should fail
        let result = manager
            .acquire_lock(
                "test-key",
                Duration::from_secs(30),
                Duration::from_millis(50),
            )
            .await;
        assert!(matches!(result, Err(LockError::Timeout(_))));
    }

    #[tokio::test]
    async fn test_lock_manager_acquire_after_expiry() {
        let manager = LockManager::new("node-1", Duration::from_secs(30));

        // Acquire first lock with short TTL
        let _guard1 = manager
            .try_acquire_lock("test-key", Duration::from_millis(50))
            .unwrap();
        std::mem::forget(_guard1);

        // Wait for expiry and try to acquire
        let result = manager
            .acquire_lock(
                "test-key",
                Duration::from_secs(30),
                Duration::from_millis(200),
            )
            .await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_lock_guard_drop() {
        let manager = LockManager::new("node-1", Duration::from_secs(30));

        {
            let _guard = manager
                .try_acquire_lock("test-key", Duration::from_secs(30))
                .unwrap();
            assert!(manager.is_locked("test-key"));
        }

        // Lock should be released after guard is dropped
        assert!(!manager.is_locked("test-key"));
    }

    #[test]
    fn test_lock_id_generate() {
        let id1 = LockId::generate();
        let id2 = LockId::generate();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_all_locks() {
        let manager = LockManager::new("node-1", Duration::from_secs(30));

        let _guard1 = manager
            .try_acquire_lock("key-1", Duration::from_secs(30))
            .unwrap();
        let _guard2 = manager
            .try_acquire_lock("key-2", Duration::from_secs(30))
            .unwrap();

        let all = manager.all_locks();
        assert_eq!(all.len(), 2);
    }
}
