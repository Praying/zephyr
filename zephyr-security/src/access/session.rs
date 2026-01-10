//! Session management for authenticated users.

#![allow(clippy::unused_async)]

use crate::error::{Result, SecurityError};
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::sync::Arc;
use uuid::Uuid;

/// A user session.
///
/// Sessions track authenticated users and their access information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session ID.
    id: String,
    /// Username of the authenticated user.
    username: String,
    /// IP address of the client.
    ip_address: IpAddr,
    /// When the session was created.
    created_at: DateTime<Utc>,
    /// When the session will expire.
    expires_at: DateTime<Utc>,
    /// When the session was last accessed.
    last_accessed: DateTime<Utc>,
}

impl Session {
    /// Creates a new session.
    #[must_use]
    pub fn new(username: impl Into<String>, ip_address: IpAddr, timeout: Duration) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            username: username.into(),
            ip_address,
            created_at: now,
            expires_at: now + timeout,
            last_accessed: now,
        }
    }

    /// Returns the session ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the username.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Returns the IP address.
    #[must_use]
    pub const fn ip_address(&self) -> IpAddr {
        self.ip_address
    }

    /// Returns the creation timestamp.
    #[must_use]
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Returns the expiration timestamp.
    #[must_use]
    pub const fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    /// Returns the last access timestamp.
    #[must_use]
    pub const fn last_accessed(&self) -> DateTime<Utc> {
        self.last_accessed
    }

    /// Checks if the session is expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Updates the last access time.
    pub fn touch(&mut self) {
        self.last_accessed = Utc::now();
    }

    /// Returns the remaining time until expiration.
    #[must_use]
    pub fn time_remaining(&self) -> Option<Duration> {
        let remaining = self.expires_at - Utc::now();
        if remaining.num_seconds() > 0 {
            Some(remaining)
        } else {
            None
        }
    }
}

/// Session manager for handling session lifecycle.
pub struct SessionManager {
    sessions: DashMap<String, Arc<Session>>,
    timeout: Duration,
}

impl SessionManager {
    /// Creates a new session manager.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Session timeout duration
    #[must_use]
    pub fn new(timeout: Duration) -> Self {
        Self {
            sessions: DashMap::new(),
            timeout,
        }
    }

    /// Creates a new session.
    ///
    /// # Arguments
    ///
    /// * `username` - The username
    /// * `ip_address` - The client IP address
    ///
    /// # Returns
    ///
    /// The session ID.
    pub async fn create_session(&self, username: &str, ip_address: IpAddr) -> String {
        let session = Session::new(username, ip_address, self.timeout);
        let session_id = session.id().to_string();
        self.sessions.insert(session_id.clone(), Arc::new(session));
        session_id
    }

    /// Validates a session.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID
    ///
    /// # Errors
    ///
    /// Returns an error if the session is invalid or expired.
    pub async fn validate_session(&self, session_id: &str) -> Result<Arc<Session>> {
        let entry = self
            .sessions
            .get(session_id)
            .ok_or_else(|| SecurityError::invalid_token("Session not found"))?;

        let session = Arc::clone(entry.value());

        if session.is_expired() {
            drop(entry);
            self.sessions.remove(session_id);
            return Err(SecurityError::SessionExpired);
        }

        Ok(session)
    }

    /// Invalidates a session.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID
    pub async fn invalidate_session(&self, session_id: &str) {
        self.sessions.remove(session_id);
    }

    /// Invalidates all sessions for a user.
    ///
    /// # Arguments
    ///
    /// * `username` - The username
    pub fn invalidate_user_sessions(&self, username: &str) {
        self.sessions
            .retain(|_, session| session.username() != username);
    }

    /// Cleans up expired sessions.
    pub fn cleanup_expired(&self) {
        self.sessions.retain(|_, session| !session.is_expired());
    }

    /// Returns the number of active sessions.
    #[must_use]
    pub fn active_session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Lists all active sessions.
    #[must_use]
    pub fn list_sessions(&self) -> Vec<Arc<Session>> {
        self.sessions
            .iter()
            .map(|entry| Arc::clone(entry.value()))
            .collect()
    }

    /// Lists sessions for a specific user.
    ///
    /// # Arguments
    ///
    /// * `username` - The username
    #[must_use]
    pub fn list_user_sessions(&self, username: &str) -> Vec<Arc<Session>> {
        self.sessions
            .iter()
            .filter(|entry| entry.value().username() == username)
            .map(|entry| Arc::clone(entry.value()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_session() {
        let ip: IpAddr = "192.168.1.1".parse().unwrap();
        let session = Session::new("alice", ip, Duration::hours(1));

        assert_eq!(session.username(), "alice");
        assert_eq!(session.ip_address(), ip);
        assert!(!session.is_expired());
    }

    #[test]
    fn test_session_expiration() {
        let ip: IpAddr = "192.168.1.1".parse().unwrap();
        let session = Session::new("alice", ip, Duration::seconds(-1));

        assert!(session.is_expired());
    }

    #[test]
    fn test_session_touch() {
        let ip: IpAddr = "192.168.1.1".parse().unwrap();
        let mut session = Session::new("alice", ip, Duration::hours(1));

        let first_access = session.last_accessed();
        std::thread::sleep(std::time::Duration::from_millis(10));
        session.touch();
        let second_access = session.last_accessed();

        assert!(second_access > first_access);
    }

    #[test]
    fn test_time_remaining() {
        let ip: IpAddr = "192.168.1.1".parse().unwrap();
        let session = Session::new("alice", ip, Duration::hours(1));

        let remaining = session.time_remaining();
        assert!(remaining.is_some());
        assert!(remaining.unwrap().num_seconds() > 0);
    }

    #[tokio::test]
    async fn test_session_manager_create() {
        let manager = SessionManager::new(Duration::hours(1));
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        let session_id = manager.create_session("alice", ip).await;
        assert!(!session_id.is_empty());
    }

    #[tokio::test]
    async fn test_session_manager_validate() {
        let manager = SessionManager::new(Duration::hours(1));
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        let session_id = manager.create_session("alice", ip).await;
        let session = manager.validate_session(&session_id).await.unwrap();

        assert_eq!(session.username(), "alice");
    }

    #[tokio::test]
    async fn test_session_manager_invalid_session() {
        let manager = SessionManager::new(Duration::hours(1));

        let result = manager.validate_session("invalid-id").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_session_manager_invalidate() {
        let manager = SessionManager::new(Duration::hours(1));
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        let session_id = manager.create_session("alice", ip).await;
        manager.invalidate_session(&session_id).await;

        let result = manager.validate_session(&session_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_session_manager_invalidate_user_sessions() {
        let manager = SessionManager::new(Duration::hours(1));
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        let _id1 = manager.create_session("alice", ip).await;
        let _id2 = manager.create_session("alice", ip).await;
        let _id3 = manager.create_session("bob", ip).await;

        assert_eq!(manager.active_session_count(), 3);

        manager.invalidate_user_sessions("alice");
        assert_eq!(manager.active_session_count(), 1);
    }

    #[tokio::test]
    async fn test_session_manager_list_user_sessions() {
        let manager = SessionManager::new(Duration::hours(1));
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        let _id1 = manager.create_session("alice", ip).await;
        let _id2 = manager.create_session("alice", ip).await;
        let _id3 = manager.create_session("bob", ip).await;

        let alice_sessions = manager.list_user_sessions("alice");
        assert_eq!(alice_sessions.len(), 2);

        let bob_sessions = manager.list_user_sessions("bob");
        assert_eq!(bob_sessions.len(), 1);
    }

    #[test]
    fn test_session_manager_cleanup_expired() {
        let manager = SessionManager::new(Duration::hours(1));
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        // Create an expired session manually
        let expired_session = Session::new("alice", ip, Duration::seconds(-1));
        manager
            .sessions
            .insert(expired_session.id().to_string(), Arc::new(expired_session));

        assert_eq!(manager.active_session_count(), 1);

        manager.cleanup_expired();
        assert_eq!(manager.active_session_count(), 0);
    }
}
