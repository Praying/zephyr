//! Graceful shutdown handling.
//!
//! Provides shutdown coordination for the Zephyr server,
//! including signal handling and component cleanup.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::{broadcast, watch};
use tracing::{info, warn};

/// Shutdown controller for coordinating graceful shutdown.
///
/// Manages shutdown signals and coordinates cleanup across components.
#[derive(Debug, Clone)]
pub struct ShutdownController {
    /// Whether shutdown has been initiated.
    shutdown_initiated: Arc<AtomicBool>,
    /// Sender for shutdown notification.
    shutdown_tx: broadcast::Sender<()>,
    /// Watch channel for shutdown completion.
    completion_tx: Arc<watch::Sender<bool>>,
    /// Receiver for shutdown completion.
    completion_rx: watch::Receiver<bool>,
}

impl Default for ShutdownController {
    fn default() -> Self {
        Self::new()
    }
}

impl ShutdownController {
    /// Creates a new shutdown controller.
    #[must_use]
    pub fn new() -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        let (completion_tx, completion_rx) = watch::channel(false);

        Self {
            shutdown_initiated: Arc::new(AtomicBool::new(false)),
            shutdown_tx,
            completion_tx: Arc::new(completion_tx),
            completion_rx,
        }
    }

    /// Initiates shutdown.
    ///
    /// This will notify all listeners that shutdown has begun.
    pub fn initiate_shutdown(&self) {
        if self
            .shutdown_initiated
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            info!("Shutdown initiated");
            let _ = self.shutdown_tx.send(());
        }
    }

    /// Returns whether shutdown has been initiated.
    #[must_use]
    pub fn is_shutdown_initiated(&self) -> bool {
        self.shutdown_initiated.load(Ordering::SeqCst)
    }

    /// Returns a future that completes when shutdown is initiated.
    pub async fn wait_for_shutdown(&self) {
        let mut rx = self.shutdown_tx.subscribe();
        let _ = rx.recv().await;
    }

    /// Returns a receiver for shutdown signals.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    /// Marks shutdown as complete.
    pub fn mark_complete(&self) {
        let _ = self.completion_tx.send(true);
    }

    /// Waits for shutdown to complete with a timeout.
    ///
    /// # Returns
    ///
    /// Returns `true` if shutdown completed, `false` if timeout occurred.
    pub async fn wait_for_completion(&self, timeout: Duration) -> bool {
        let mut rx = self.completion_rx.clone();

        tokio::select! {
            result = rx.changed() => {
                result.is_ok() && *rx.borrow()
            }
            () = tokio::time::sleep(timeout) => {
                warn!("Shutdown completion timeout after {:?}", timeout);
                false
            }
        }
    }
}

/// Sets up signal handlers for graceful shutdown.
///
/// Listens for SIGINT (Ctrl+C) and SIGTERM signals.
///
/// # Arguments
///
/// * `controller` - The shutdown controller to notify
pub async fn setup_signal_handlers(controller: ShutdownController) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigint = signal(SignalKind::interrupt()).expect("Failed to setup SIGINT handler");
        let mut sigterm = signal(SignalKind::terminate()).expect("Failed to setup SIGTERM handler");

        tokio::select! {
            _ = sigint.recv() => {
                info!("Received SIGINT (Ctrl+C)");
            }
            _ = sigterm.recv() => {
                info!("Received SIGTERM");
            }
        }

        controller.initiate_shutdown();
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to setup Ctrl+C handler");
        info!("Received Ctrl+C");
        controller.initiate_shutdown();
    }
}

/// Shutdown guard that ensures cleanup on drop.
///
/// Use this to ensure resources are cleaned up even if a panic occurs.
pub struct ShutdownGuard {
    controller: ShutdownController,
    name: String,
}

impl ShutdownGuard {
    /// Creates a new shutdown guard.
    #[must_use]
    pub fn new(controller: ShutdownController, name: impl Into<String>) -> Self {
        Self {
            controller,
            name: name.into(),
        }
    }
}

impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        if !self.controller.is_shutdown_initiated() {
            warn!(
                "ShutdownGuard '{}' dropped without graceful shutdown",
                self.name
            );
            self.controller.initiate_shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shutdown_controller_new() {
        let controller = ShutdownController::new();
        assert!(!controller.is_shutdown_initiated());
    }

    #[tokio::test]
    async fn test_shutdown_initiation() {
        let controller = ShutdownController::new();

        controller.initiate_shutdown();
        assert!(controller.is_shutdown_initiated());

        // Second call should be idempotent
        controller.initiate_shutdown();
        assert!(controller.is_shutdown_initiated());
    }

    #[tokio::test]
    async fn test_shutdown_subscription() {
        let controller = ShutdownController::new();
        let mut rx = controller.subscribe();

        // Spawn a task to initiate shutdown
        let ctrl = controller.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            ctrl.initiate_shutdown();
        });

        // Wait for shutdown signal
        let result = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_shutdown_completion() {
        let controller = ShutdownController::new();

        controller.initiate_shutdown();
        controller.mark_complete();

        let completed = controller
            .wait_for_completion(Duration::from_millis(100))
            .await;
        assert!(completed);
    }

    #[tokio::test]
    async fn test_shutdown_completion_timeout() {
        let controller = ShutdownController::new();

        controller.initiate_shutdown();
        // Don't mark complete

        let completed = controller
            .wait_for_completion(Duration::from_millis(50))
            .await;
        assert!(!completed);
    }
}
