//! Configuration for access control.

use chrono::Duration;
use serde::{Deserialize, Serialize};

/// Configuration for the access control system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessControlConfig {
    /// Session timeout duration.
    #[serde(default = "default_session_timeout")]
    pub session_timeout: Duration,

    /// Whether to enforce IP whitelist.
    #[serde(default = "default_enforce_ip_whitelist")]
    pub enforce_ip_whitelist: bool,

    /// Maximum number of concurrent sessions per user.
    #[serde(default = "default_max_sessions_per_user")]
    pub max_sessions_per_user: usize,

    /// Whether to require password change on first login.
    #[serde(default = "default_require_password_change")]
    pub require_password_change_on_first_login: bool,

    /// Minimum password length.
    #[serde(default = "default_min_password_length")]
    pub min_password_length: usize,

    /// Whether to enforce strong passwords.
    #[serde(default = "default_enforce_strong_passwords")]
    pub enforce_strong_passwords: bool,

    /// Maximum failed login attempts before lockout.
    #[serde(default = "default_max_failed_attempts")]
    pub max_failed_login_attempts: u32,

    /// Account lockout duration in seconds.
    #[serde(default = "default_lockout_duration")]
    pub account_lockout_duration_secs: u64,
}

fn default_session_timeout() -> Duration {
    Duration::hours(8)
}

fn default_enforce_ip_whitelist() -> bool {
    false
}

fn default_max_sessions_per_user() -> usize {
    5
}

fn default_require_password_change() -> bool {
    false
}

fn default_min_password_length() -> usize {
    12
}

fn default_enforce_strong_passwords() -> bool {
    true
}

fn default_max_failed_attempts() -> u32 {
    5
}

fn default_lockout_duration() -> u64 {
    900 // 15 minutes
}

impl Default for AccessControlConfig {
    fn default() -> Self {
        Self {
            session_timeout: default_session_timeout(),
            enforce_ip_whitelist: default_enforce_ip_whitelist(),
            max_sessions_per_user: default_max_sessions_per_user(),
            require_password_change_on_first_login: default_require_password_change(),
            min_password_length: default_min_password_length(),
            enforce_strong_passwords: default_enforce_strong_passwords(),
            max_failed_login_attempts: default_max_failed_attempts(),
            account_lockout_duration_secs: default_lockout_duration(),
        }
    }
}

impl AccessControlConfig {
    /// Creates a new configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the session timeout.
    #[must_use]
    pub const fn with_session_timeout(mut self, timeout: Duration) -> Self {
        self.session_timeout = timeout;
        self
    }

    /// Sets whether to enforce IP whitelist.
    #[must_use]
    pub const fn with_enforce_ip_whitelist(mut self, enforce: bool) -> Self {
        self.enforce_ip_whitelist = enforce;
        self
    }

    /// Sets the maximum sessions per user.
    #[must_use]
    pub const fn with_max_sessions_per_user(mut self, max: usize) -> Self {
        self.max_sessions_per_user = max;
        self
    }

    /// Sets whether to require password change on first login.
    #[must_use]
    pub const fn with_require_password_change_on_first_login(mut self, require: bool) -> Self {
        self.require_password_change_on_first_login = require;
        self
    }

    /// Sets the minimum password length.
    #[must_use]
    pub const fn with_min_password_length(mut self, length: usize) -> Self {
        self.min_password_length = length;
        self
    }

    /// Sets whether to enforce strong passwords.
    #[must_use]
    pub const fn with_enforce_strong_passwords(mut self, enforce: bool) -> Self {
        self.enforce_strong_passwords = enforce;
        self
    }

    /// Sets the maximum failed login attempts.
    #[must_use]
    pub const fn with_max_failed_login_attempts(mut self, max: u32) -> Self {
        self.max_failed_login_attempts = max;
        self
    }

    /// Sets the account lockout duration.
    #[must_use]
    pub const fn with_account_lockout_duration(mut self, secs: u64) -> Self {
        self.account_lockout_duration_secs = secs;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AccessControlConfig::default();
        assert_eq!(config.session_timeout, Duration::hours(8));
        assert!(!config.enforce_ip_whitelist);
        assert_eq!(config.max_sessions_per_user, 5);
    }

    #[test]
    fn test_config_builder() {
        let config = AccessControlConfig::new()
            .with_enforce_ip_whitelist(true)
            .with_max_sessions_per_user(3)
            .with_min_password_length(16);

        assert!(config.enforce_ip_whitelist);
        assert_eq!(config.max_sessions_per_user, 3);
        assert_eq!(config.min_password_length, 16);
    }

    #[test]
    fn test_config_serde() {
        let config = AccessControlConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AccessControlConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config.session_timeout, parsed.session_timeout);
        assert_eq!(config.enforce_ip_whitelist, parsed.enforce_ip_whitelist);
    }
}
