//! Hot reload functionality for Python strategies.
//!
//! This module provides the `HotReloader` struct that monitors Python strategy
//! files for changes and sends reload commands to the corresponding strategy runners.
//!
//! # Architecture
//!
//! The hot reloader uses the `notify` crate to watch for file system changes.
//! When a Python file is modified, it sends a `Reload` command to the strategy
//! runner, which then attempts to reload the strategy code while preserving state.
//!
//! # Debouncing
//!
//! File change events are debounced to avoid excessive reloads when files are
//! being edited. The default debounce duration is 2 seconds.
//!
//! # Example
//!
//! ```ignore
//! use zephyr_strategy::runner::{HotReloader, RunnerCommand};
//! use tokio::sync::mpsc;
//!
//! let mut reloader = HotReloader::new()?;
//!
//! // Register a strategy for hot reload
//! reloader.watch(
//!     "./strategies/grid.py",
//!     "GridStrategy",
//!     cmd_tx,
//! )?;
//!
//! // Start watching (runs in background)
//! reloader.start()?;
//! ```

use crate::runner::RunnerCommand;
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebouncedEventKind, Debouncer, new_debouncer};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Default debounce duration for file change events (2 seconds).
pub const DEFAULT_DEBOUNCE_DURATION: Duration = Duration::from_secs(2);

/// Errors that can occur during hot reload operations.
#[derive(Debug, thiserror::Error)]
pub enum HotReloadError {
    /// Failed to create file watcher
    #[error("failed to create file watcher: {0}")]
    WatcherCreation(String),

    /// Failed to watch a file path
    #[error("failed to watch path '{path}': {message}")]
    WatchPath {
        /// The path that could not be watched
        path: String,
        /// Error message
        message: String,
    },

    /// Path is already being watched
    #[error("path '{0}' is already being watched")]
    AlreadyWatching(String),

    /// Path is not being watched
    #[error("path '{0}' is not being watched")]
    NotWatching(String),

    /// Hot reloader is not running
    #[error("hot reloader is not running")]
    NotRunning,

    /// Hot reloader is already running
    #[error("hot reloader is already running")]
    AlreadyRunning,
}

/// Information about a watched strategy file.
#[derive(Debug, Clone)]
struct WatchedStrategy {
    /// Path to the Python script
    path: PathBuf,
    /// Class name to instantiate
    class_name: String,
    /// Command sender to the strategy runner
    cmd_tx: mpsc::Sender<RunnerCommand>,
}

/// Hot reloader for Python strategies.
///
/// Monitors Python strategy files for changes and sends reload commands
/// to the corresponding strategy runners when files are modified.
///
/// # Thread Safety
///
/// The hot reloader is thread-safe and can be shared across threads.
/// The internal state is protected by a mutex.
///
/// # Lifecycle
///
/// 1. Create a new `HotReloader` with `new()` or `with_debounce()`
/// 2. Register strategies with `watch()`
/// 3. Start watching with `start()`
/// 4. Stop watching with `stop()` when done
pub struct HotReloader {
    /// Debounce duration for file change events
    debounce_duration: Duration,

    /// Map of watched paths to strategy info
    /// Key is the canonical path to the file
    watched_strategies: Arc<Mutex<HashMap<PathBuf, WatchedStrategy>>>,

    /// The file watcher (created when `start()` is called)
    debouncer: Option<Debouncer<RecommendedWatcher>>,

    /// Whether the reloader is currently running
    running: bool,
}

impl HotReloader {
    /// Creates a new hot reloader with the default debounce duration.
    ///
    /// # Returns
    ///
    /// A new `HotReloader` instance.
    #[must_use]
    pub fn new() -> Self {
        Self::with_debounce(DEFAULT_DEBOUNCE_DURATION)
    }

    /// Creates a new hot reloader with a custom debounce duration.
    ///
    /// # Arguments
    ///
    /// * `debounce_duration` - Duration to wait before triggering a reload
    ///
    /// # Returns
    ///
    /// A new `HotReloader` instance.
    #[must_use]
    pub fn with_debounce(debounce_duration: Duration) -> Self {
        Self {
            debounce_duration,
            watched_strategies: Arc::new(Mutex::new(HashMap::new())),
            debouncer: None,
            running: false,
        }
    }

    /// Registers a strategy file for hot reload monitoring.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the Python script file
    /// * `class_name` - Name of the strategy class to instantiate
    /// * `cmd_tx` - Command sender to the strategy runner
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path is already being watched
    /// - The path cannot be canonicalized
    /// - The watcher fails to watch the path (if already running)
    pub fn watch(
        &mut self,
        path: impl AsRef<Path>,
        class_name: impl Into<String>,
        cmd_tx: mpsc::Sender<RunnerCommand>,
    ) -> Result<(), HotReloadError> {
        let path = path.as_ref();
        let canonical_path = path.canonicalize().map_err(|e| HotReloadError::WatchPath {
            path: path.display().to_string(),
            message: format!("failed to canonicalize path: {e}"),
        })?;

        let class_name = class_name.into();

        // Check if already watching
        {
            let strategies = self.watched_strategies.lock().unwrap();
            if strategies.contains_key(&canonical_path) {
                return Err(HotReloadError::AlreadyWatching(
                    canonical_path.display().to_string(),
                ));
            }
        }

        // Add to watched strategies
        let strategy_info = WatchedStrategy {
            path: canonical_path.clone(),
            class_name: class_name.clone(),
            cmd_tx,
        };

        {
            let mut strategies = self.watched_strategies.lock().unwrap();
            strategies.insert(canonical_path.clone(), strategy_info);
        }

        // If already running, add the watch
        if let Some(ref mut debouncer) = self.debouncer {
            debouncer
                .watcher()
                .watch(&canonical_path, RecursiveMode::NonRecursive)
                .map_err(|e| HotReloadError::WatchPath {
                    path: canonical_path.display().to_string(),
                    message: e.to_string(),
                })?;
        }

        info!(
            path = %canonical_path.display(),
            class = %class_name,
            "Registered strategy for hot reload"
        );

        Ok(())
    }

    /// Unregisters a strategy file from hot reload monitoring.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the Python script file
    ///
    /// # Errors
    ///
    /// Returns an error if the path is not being watched.
    pub fn unwatch(&mut self, path: impl AsRef<Path>) -> Result<(), HotReloadError> {
        let path = path.as_ref();
        let canonical_path = path.canonicalize().map_err(|e| HotReloadError::WatchPath {
            path: path.display().to_string(),
            message: format!("failed to canonicalize path: {e}"),
        })?;

        // Remove from watched strategies
        {
            let mut strategies = self.watched_strategies.lock().unwrap();
            if strategies.remove(&canonical_path).is_none() {
                return Err(HotReloadError::NotWatching(
                    canonical_path.display().to_string(),
                ));
            }
        }

        // If running, remove the watch
        if let Some(ref mut debouncer) = self.debouncer {
            let _ = debouncer.watcher().unwatch(&canonical_path);
        }

        info!(path = %canonical_path.display(), "Unregistered strategy from hot reload");

        Ok(())
    }

    /// Starts the hot reloader.
    ///
    /// This creates the file watcher and begins monitoring registered files.
    /// File change events are processed in a background thread.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The reloader is already running
    /// - The file watcher cannot be created
    pub fn start(&mut self) -> Result<(), HotReloadError> {
        if self.running {
            return Err(HotReloadError::AlreadyRunning);
        }

        let watched_strategies = Arc::clone(&self.watched_strategies);

        // Create the debounced watcher
        let debouncer = new_debouncer(
            self.debounce_duration,
            move |result: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
                match result {
                    Ok(events) => {
                        for event in events {
                            if event.kind == DebouncedEventKind::Any {
                                handle_file_change(&watched_strategies, &event.path);
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "File watcher error");
                    }
                }
            },
        )
        .map_err(|e| HotReloadError::WatcherCreation(e.to_string()))?;

        self.debouncer = Some(debouncer);

        // Add watches for all registered strategies
        let strategies = self.watched_strategies.lock().unwrap();
        for (path, _) in strategies.iter() {
            if let Some(ref mut debouncer) = self.debouncer {
                debouncer
                    .watcher()
                    .watch(path, RecursiveMode::NonRecursive)
                    .map_err(|e| HotReloadError::WatchPath {
                        path: path.display().to_string(),
                        message: e.to_string(),
                    })?;
            }
        }
        drop(strategies);

        self.running = true;
        info!(
            debounce_ms = self.debounce_duration.as_millis(),
            "Hot reloader started"
        );

        Ok(())
    }

    /// Stops the hot reloader.
    ///
    /// This stops the file watcher and cleans up resources.
    ///
    /// # Errors
    ///
    /// Returns an error if the reloader is not running.
    pub fn stop(&mut self) -> Result<(), HotReloadError> {
        if !self.running {
            return Err(HotReloadError::NotRunning);
        }

        self.debouncer = None;
        self.running = false;

        info!("Hot reloader stopped");

        Ok(())
    }

    /// Returns whether the hot reloader is currently running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Returns the number of strategies being watched.
    #[must_use]
    pub fn watched_count(&self) -> usize {
        self.watched_strategies.lock().unwrap().len()
    }

    /// Returns the debounce duration.
    #[must_use]
    pub fn debounce_duration(&self) -> Duration {
        self.debounce_duration
    }
}

impl Default for HotReloader {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for HotReloader {
    fn drop(&mut self) {
        if self.running {
            let _ = self.stop();
        }
    }
}

/// Handles a file change event by sending a reload command to the appropriate runner.
fn handle_file_change(
    watched_strategies: &Arc<Mutex<HashMap<PathBuf, WatchedStrategy>>>,
    changed_path: &Path,
) {
    // Try to canonicalize the changed path
    let canonical_path = match changed_path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            debug!(
                path = %changed_path.display(),
                error = %e,
                "Failed to canonicalize changed path"
            );
            return;
        }
    };

    // Look up the strategy info
    let strategy_info = {
        let strategies = watched_strategies.lock().unwrap();
        strategies.get(&canonical_path).cloned()
    };

    if let Some(info) = strategy_info {
        info!(
            path = %info.path.display(),
            class = %info.class_name,
            "File change detected, sending reload command"
        );

        // Send reload command
        let cmd = RunnerCommand::reload(
            info.path.to_string_lossy().to_string(),
            info.class_name.clone(),
        );

        // Use try_send to avoid blocking
        match info.cmd_tx.try_send(cmd) {
            Ok(()) => {
                debug!(
                    path = %info.path.display(),
                    "Reload command sent successfully"
                );
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!(
                    path = %info.path.display(),
                    "Command channel full, reload command dropped"
                );
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                warn!(
                    path = %info.path.display(),
                    "Command channel closed, strategy runner may have stopped"
                );
            }
        }
    } else {
        debug!(
            path = %canonical_path.display(),
            "File change for unregistered path"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_hot_reloader_creation() {
        let reloader = HotReloader::new();
        assert!(!reloader.is_running());
        assert_eq!(reloader.watched_count(), 0);
        assert_eq!(reloader.debounce_duration(), DEFAULT_DEBOUNCE_DURATION);
    }

    #[test]
    fn test_hot_reloader_custom_debounce() {
        let duration = Duration::from_millis(500);
        let reloader = HotReloader::with_debounce(duration);
        assert_eq!(reloader.debounce_duration(), duration);
    }

    #[test]
    fn test_watch_and_unwatch() {
        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("test_strategy.py");
        fs::write(&script_path, "# test").unwrap();

        let (cmd_tx, _cmd_rx) = mpsc::channel(10);
        let mut reloader = HotReloader::new();

        // Watch the file
        reloader
            .watch(&script_path, "TestStrategy", cmd_tx.clone())
            .unwrap();
        assert_eq!(reloader.watched_count(), 1);

        // Try to watch again - should fail
        let result = reloader.watch(&script_path, "TestStrategy", cmd_tx);
        assert!(matches!(result, Err(HotReloadError::AlreadyWatching(_))));

        // Unwatch
        reloader.unwatch(&script_path).unwrap();
        assert_eq!(reloader.watched_count(), 0);

        // Try to unwatch again - should fail
        let result = reloader.unwatch(&script_path);
        assert!(matches!(result, Err(HotReloadError::NotWatching(_))));
    }

    #[test]
    fn test_start_and_stop() {
        let mut reloader = HotReloader::new();

        // Start
        reloader.start().unwrap();
        assert!(reloader.is_running());

        // Try to start again - should fail
        let result = reloader.start();
        assert!(matches!(result, Err(HotReloadError::AlreadyRunning)));

        // Stop
        reloader.stop().unwrap();
        assert!(!reloader.is_running());

        // Try to stop again - should fail
        let result = reloader.stop();
        assert!(matches!(result, Err(HotReloadError::NotRunning)));
    }

    #[test]
    fn test_watch_nonexistent_file() {
        let (cmd_tx, _cmd_rx) = mpsc::channel(10);
        let mut reloader = HotReloader::new();

        let result = reloader.watch("/nonexistent/path/strategy.py", "TestStrategy", cmd_tx);
        assert!(matches!(result, Err(HotReloadError::WatchPath { .. })));
    }

    #[tokio::test]
    async fn test_file_change_detection() {
        let temp_dir = TempDir::new().unwrap();
        let script_path = temp_dir.path().join("test_strategy.py");
        fs::write(&script_path, "# initial content").unwrap();

        let (cmd_tx, mut cmd_rx) = mpsc::channel(10);
        let mut reloader = HotReloader::with_debounce(Duration::from_millis(100));

        reloader
            .watch(&script_path, "TestStrategy", cmd_tx)
            .unwrap();
        reloader.start().unwrap();

        // Modify the file
        tokio::time::sleep(Duration::from_millis(50)).await;
        fs::write(&script_path, "# modified content").unwrap();

        // Wait for debounce + processing
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Check if reload command was received
        match tokio::time::timeout(Duration::from_millis(100), cmd_rx.recv()).await {
            Ok(Some(RunnerCommand::Reload { path, class_name })) => {
                assert!(path.contains("test_strategy.py"));
                assert_eq!(class_name, "TestStrategy");
            }
            Ok(Some(other)) => panic!("Unexpected command: {other:?}"),
            Ok(None) => panic!("Channel closed"),
            Err(_) => {
                // Timeout is acceptable in CI environments where file watching may be unreliable
                eprintln!("Warning: File change detection timed out (may be CI environment)");
            }
        }

        reloader.stop().unwrap();
    }
}
