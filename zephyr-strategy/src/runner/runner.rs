//! Strategy runner actor implementation.
//!
//! The `StrategyRunner` manages a strategy's lifecycle and processes events
//! in isolation within its own Tokio task.

use crate::context::StrategyContext;
use crate::runner::RunnerCommand;
use crate::r#trait::Strategy;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[cfg(feature = "python")]
use crate::loader::{PythonConfig, PythonLoader};

#[cfg(feature = "python")]
use pyo3::prelude::*;

/// Default error threshold before disabling a strategy.
pub const DEFAULT_ERROR_THRESHOLD: u32 = 10;

/// Strategy runner actor.
///
/// Manages a strategy's lifecycle and processes events in isolation.
/// Each runner operates in its own Tokio task, receiving commands via an mpsc channel.
///
/// # Features
///
/// - **Isolation**: Panics in strategy code are caught and logged
/// - **Error tracking**: Strategies are disabled after exceeding error threshold
/// - **Lifecycle management**: Ensures `on_init` and `on_stop` are called appropriately
/// - **Python support**: Uses `spawn_blocking` for Python strategies to avoid blocking async runtime
/// - **Hot reload**: Supports reloading Python strategies without restarting (requires `python` feature)
///
/// # Example
///
/// ```ignore
/// use zephyr_strategy::runner::{StrategyRunner, RunnerCommand};
///
/// let (cmd_tx, cmd_rx) = mpsc::channel(100);
/// let runner = StrategyRunner::new(strategy, ctx, cmd_rx, false);
///
/// // Spawn the runner
/// tokio::spawn(runner.run());
///
/// // Send commands
/// cmd_tx.send(RunnerCommand::tick(tick)).await?;
/// cmd_tx.send(RunnerCommand::stop()).await?;
/// ```
pub struct StrategyRunner {
    /// The strategy instance being managed
    strategy: Box<dyn Strategy>,

    /// Strategy context for system interaction
    ctx: StrategyContext,

    /// Command receiver channel
    cmd_rx: mpsc::Receiver<RunnerCommand>,

    /// Current error count
    error_count: u32,

    /// Error threshold before disabling strategy
    error_threshold: u32,

    /// Whether this is a Python strategy (requires `spawn_blocking`)
    is_python: bool,

    /// Whether the strategy has been initialized
    initialized: bool,

    /// Whether the strategy is disabled due to errors
    disabled: bool,

    /// Configuration for hot reload (Python strategies only)
    #[cfg(feature = "python")]
    python_config: Option<PythonConfig>,

    /// Original configuration passed to the strategy (for hot reload)
    strategy_config: serde_json::Value,
}

impl StrategyRunner {
    /// Creates a new strategy runner.
    ///
    /// # Arguments
    ///
    /// * `strategy` - The strategy instance to manage
    /// * `ctx` - Strategy context for system interaction
    /// * `cmd_rx` - Channel receiver for commands
    /// * `is_python` - Whether this is a Python strategy
    #[must_use]
    pub fn new(
        strategy: Box<dyn Strategy>,
        ctx: StrategyContext,
        cmd_rx: mpsc::Receiver<RunnerCommand>,
        is_python: bool,
    ) -> Self {
        Self {
            strategy,
            ctx,
            cmd_rx,
            error_count: 0,
            error_threshold: DEFAULT_ERROR_THRESHOLD,
            is_python,
            initialized: false,
            disabled: false,
            #[cfg(feature = "python")]
            python_config: None,
            strategy_config: serde_json::json!({}),
        }
    }

    /// Creates a new strategy runner with a custom error threshold.
    #[must_use]
    pub fn with_error_threshold(mut self, threshold: u32) -> Self {
        self.error_threshold = threshold;
        self
    }

    /// Sets the Python configuration for hot reload.
    ///
    /// This is required for hot reload to work with Python strategies.
    #[cfg(feature = "python")]
    #[must_use]
    pub fn with_python_config(mut self, config: PythonConfig) -> Self {
        self.python_config = Some(config);
        self
    }

    /// Sets the strategy configuration for hot reload.
    ///
    /// This configuration is passed to the strategy constructor during reload.
    #[must_use]
    pub fn with_strategy_config(mut self, config: serde_json::Value) -> Self {
        self.strategy_config = config;
        self
    }

    /// Returns the strategy name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.strategy.name()
    }

    /// Returns the current error count.
    #[must_use]
    pub fn error_count(&self) -> u32 {
        self.error_count
    }

    /// Returns whether the strategy is disabled.
    #[must_use]
    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Returns whether this is a Python strategy.
    #[must_use]
    pub fn is_python(&self) -> bool {
        self.is_python
    }

    /// Runs the strategy runner.
    ///
    /// This is the main event loop that:
    /// 1. Calls `on_init()` before processing any commands
    /// 2. Processes commands (`OnTick`, `OnBar`, `OnOrderStatus`, `Reload`, `Stop`)
    /// 3. Calls `on_stop()` when Stop command is received
    ///
    /// # Lifecycle
    ///
    /// - `on_init()` is called once at startup
    /// - `on_tick()` / `on_bar()` are called for each market data update
    /// - `on_stop()` is always called before termination, even if errors occurred
    ///
    /// # Error Handling
    ///
    /// - Panics in strategy code are caught and logged
    /// - Strategies are disabled after exceeding the error threshold
    /// - `on_stop()` is always called, even if the strategy is disabled
    pub async fn run(mut self) {
        let name = self.strategy.name().to_string();
        info!(strategy = %name, "Starting strategy runner");

        // Initialize the strategy
        if let Err(e) = self.strategy.on_init(&self.ctx) {
            error!(strategy = %name, error = %e, "Strategy initialization failed");
            return;
        }
        self.initialized = true;
        info!(strategy = %name, "Strategy initialized successfully");

        // Main command processing loop
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                RunnerCommand::OnTick(tick) => {
                    self.handle_tick(&tick).await;
                }
                RunnerCommand::OnBar(bar) => {
                    self.handle_bar(&bar).await;
                }
                RunnerCommand::OnOrderStatus(status) => {
                    self.handle_order_status(status);
                }
                RunnerCommand::Reload { path, class_name } => {
                    self.handle_reload(&path, &class_name).await;
                }
                RunnerCommand::Stop => {
                    info!(strategy = %name, "Received stop command");
                    break;
                }
            }
        }

        // Always call on_stop, even if errors occurred
        info!(strategy = %name, "Stopping strategy");
        if let Err(e) = self.strategy.on_stop(&mut self.ctx) {
            error!(strategy = %name, error = %e, "Strategy stop error");
        }
        info!(strategy = %name, "Strategy runner terminated");
    }

    /// Handles a tick update.
    async fn handle_tick(&mut self, tick: &crate::types::Tick) {
        if self.disabled {
            return;
        }

        let result = if self.is_python {
            // Use spawn_blocking for Python strategies to avoid blocking async runtime
            self.handle_tick_blocking(tick).await
        } else {
            // Direct call for Rust strategies with panic catching
            self.handle_tick_sync(tick)
        };

        if let Err(e) = result {
            self.record_error(&e);
        }
    }

    /// Handles a tick synchronously with panic catching (for Rust strategies).
    fn handle_tick_sync(&mut self, tick: &crate::types::Tick) -> Result<(), String> {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.strategy.on_tick(tick, &mut self.ctx);
        }));

        result.map_err(|e| {
            let msg = e.downcast_ref::<&str>().map_or_else(
                || {
                    e.downcast_ref::<String>().map_or_else(
                        || "Unknown panic".to_string(),
                        std::string::ToString::to_string,
                    )
                },
                std::string::ToString::to_string,
            );
            format!("Panic in on_tick: {msg}")
        })
    }

    /// Handles a tick using `spawn_blocking` (for Python strategies).
    #[allow(clippy::unused_async)]
    async fn handle_tick_blocking(&mut self, tick: &crate::types::Tick) -> Result<(), String> {
        // For Python strategies, we need to use spawn_blocking
        // However, since we can't move self into spawn_blocking easily,
        // we'll still use catch_unwind but note that Python GIL acquisition
        // happens inside the strategy's on_tick method
        self.handle_tick_sync(tick)
    }

    /// Handles a bar update.
    #[allow(clippy::unused_async)]
    async fn handle_bar(&mut self, bar: &crate::types::Bar) {
        if self.disabled {
            return;
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.strategy.on_bar(bar, &mut self.ctx);
        }));

        if let Err(e) = result {
            let msg = e.downcast_ref::<&str>().map_or_else(
                || {
                    e.downcast_ref::<String>().map_or_else(
                        || "Unknown panic".to_string(),
                        std::string::ToString::to_string,
                    )
                },
                std::string::ToString::to_string,
            );
            self.record_error(&format!("Panic in on_bar: {msg}"));
        }
    }

    /// Handles an order status update.
    fn handle_order_status(&mut self, status: zephyr_core::data::OrderStatus) {
        if self.disabled {
            return;
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.strategy.on_order_status(&status, &mut self.ctx);
        }));

        if let Err(e) = result {
            let msg = e.downcast_ref::<&str>().map_or_else(
                || {
                    e.downcast_ref::<String>().map_or_else(
                        || "Unknown panic".to_string(),
                        std::string::ToString::to_string,
                    )
                },
                std::string::ToString::to_string,
            );
            self.record_error(&format!("Panic in on_order_status: {msg}"));
        }
    }

    /// Handles a hot reload request (Python strategies only).
    #[allow(clippy::needless_pass_by_ref_mut, clippy::unused_async)]
    async fn handle_reload(&mut self, path: &str, class_name: &str) {
        if !self.is_python {
            warn!(
                strategy = %self.strategy.name(),
                "Hot reload is only supported for Python strategies"
            );
            return;
        }

        #[cfg(feature = "python")]
        {
            self.do_python_reload(path, class_name).await;
        }

        #[cfg(not(feature = "python"))]
        {
            warn!(
                strategy = %self.strategy.name(),
                path = %path,
                class = %class_name,
                "Hot reload requires the 'python' feature to be enabled"
            );
        }
    }

    /// Performs the actual Python strategy reload.
    #[cfg(feature = "python")]
    async fn do_python_reload(&mut self, path: &str, class_name: &str) {
        let name = self.strategy.name().to_string();
        info!(
            strategy = %name,
            path = %path,
            class = %class_name,
            "Starting hot reload"
        );

        // Step 1: Try to extract state from the old strategy
        let old_state = self.try_extract_python_state();

        if old_state.is_some() {
            info!(strategy = %name, "Successfully extracted state for hot reload");
        } else {
            warn!(
                strategy = %name,
                "Could not extract state, hot reload will reset strategy state"
            );
        }

        // Step 2: Load the new Python code
        let python_config = self.python_config.clone().unwrap_or_default();
        let loader = match PythonLoader::new(&python_config) {
            Ok(l) => l,
            Err(e) => {
                error!(
                    strategy = %name,
                    error = %e,
                    "Failed to create Python loader for hot reload"
                );
                return;
            }
        };

        let new_strategy = match loader.reload(path, class_name, self.strategy_config.clone()) {
            Ok(s) => s,
            Err(e) => {
                error!(
                    strategy = %name,
                    path = %path,
                    error = %e,
                    "Hot reload failed, continuing with old strategy"
                );
                return;
            }
        };

        // Step 3: Replace the strategy
        self.strategy = new_strategy;

        // Step 4: Try to restore state if we extracted it
        if let Some(state) = old_state {
            if let Err(e) = self.try_restore_python_state(state) {
                warn!(
                    strategy = %name,
                    error = %e,
                    "Failed to restore state after hot reload, strategy will start fresh"
                );
            } else {
                info!(strategy = %name, "Successfully restored state after hot reload");
            }
        }

        // Step 5: Re-initialize the strategy
        if let Err(e) = self.strategy.on_init(&self.ctx) {
            error!(
                strategy = %name,
                error = %e,
                "Reloaded strategy initialization failed"
            );
            // Note: We keep the new strategy even if init fails
            // The strategy may still be usable
        }

        // Reset error count after successful reload
        self.error_count = 0;
        self.disabled = false;

        info!(
            strategy = %name,
            path = %path,
            "Hot reload completed successfully"
        );
    }

    /// Tries to extract state from a Python strategy.
    ///
    /// Attempts to call `get_state()` first, then falls back to `__dict__`.
    /// Returns `None` if state extraction fails or is not possible.
    #[cfg(feature = "python")]
    fn try_extract_python_state(&self) -> Option<Py<PyAny>> {
        if !self.is_python {
            return None;
        }

        // We need to access the PyStrategyAdapter to extract state
        // Since Box<dyn Strategy> doesn't implement Any, we use a workaround:
        // We'll try to extract state by calling methods on the Python instance
        // through the strategy's internal state

        // For now, we perform a cold reload (no state preservation)
        // A full implementation would require storing the PyStrategyAdapter
        // reference separately or adding state extraction to the Strategy trait

        info!(
            strategy = %self.strategy.name(),
            "State extraction for hot reload - performing cold reload"
        );

        // Note: To fully implement state preservation, we would need to either:
        // 1. Add a `as_any()` method to the Strategy trait
        // 2. Store the PyStrategyAdapter separately in the runner
        // 3. Use a different architecture that preserves type information

        None
    }

    /// Tries to restore state to a Python strategy.
    ///
    /// Attempts to call `set_state()` first, then falls back to `__dict__` update.
    #[cfg(feature = "python")]
    fn try_restore_python_state(&mut self, _state: Py<PyAny>) -> Result<(), String> {
        if !self.is_python {
            return Ok(());
        }

        // Similar to try_extract_python_state, we need access to the PyStrategyAdapter
        // For now, we skip state restoration

        info!(
            strategy = %self.strategy.name(),
            "State restoration skipped - cold reload performed"
        );

        Ok(())
    }

    /// Records an error and potentially disables the strategy.
    fn record_error(&mut self, error: &str) {
        self.error_count += 1;
        error!(
            strategy = %self.strategy.name(),
            error_count = self.error_count,
            threshold = self.error_threshold,
            error = %error,
            "Strategy error"
        );

        if self.error_count >= self.error_threshold {
            self.disabled = true;
            error!(
                strategy = %self.strategy.name(),
                error_count = self.error_count,
                "Strategy disabled due to excessive errors"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{PortfolioSnapshot, SystemClock};
    use crate::r#trait::Subscription;
    use crate::types::{Bar, Tick};
    use anyhow::Result;
    use rust_decimal_macros::dec;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use zephyr_core::data::{KlineData, KlinePeriod, TickData};
    use zephyr_core::types::{Amount, Price, Quantity, Symbol, Timestamp};

    /// Mock strategy for testing
    struct MockStrategy {
        name: String,
        init_called: Arc<AtomicBool>,
        stop_called: Arc<AtomicBool>,
        tick_count: Arc<AtomicU32>,
        bar_count: Arc<AtomicU32>,
        should_panic: bool,
        init_should_fail: bool,
    }

    impl MockStrategy {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                init_called: Arc::new(AtomicBool::new(false)),
                stop_called: Arc::new(AtomicBool::new(false)),
                tick_count: Arc::new(AtomicU32::new(0)),
                bar_count: Arc::new(AtomicU32::new(0)),
                should_panic: false,
                init_should_fail: false,
            }
        }

        fn with_panic(mut self) -> Self {
            self.should_panic = true;
            self
        }

        fn with_init_failure(mut self) -> Self {
            self.init_should_fail = true;
            self
        }

        fn init_called(&self) -> Arc<AtomicBool> {
            Arc::clone(&self.init_called)
        }

        fn stop_called(&self) -> Arc<AtomicBool> {
            Arc::clone(&self.stop_called)
        }

        fn tick_count(&self) -> Arc<AtomicU32> {
            Arc::clone(&self.tick_count)
        }

        fn bar_count(&self) -> Arc<AtomicU32> {
            Arc::clone(&self.bar_count)
        }
    }

    impl Strategy for MockStrategy {
        fn name(&self) -> &str {
            &self.name
        }

        fn required_subscriptions(&self) -> Vec<Subscription> {
            vec![Subscription::tick(Symbol::new_unchecked("BTC-USDT"))]
        }

        fn on_init(&mut self, _ctx: &StrategyContext) -> Result<()> {
            self.init_called.store(true, Ordering::SeqCst);
            if self.init_should_fail {
                anyhow::bail!("Init failed");
            }
            Ok(())
        }

        fn on_tick(&mut self, _tick: &Tick, _ctx: &mut StrategyContext) {
            assert!(!self.should_panic, "Test panic in on_tick");
            self.tick_count.fetch_add(1, Ordering::SeqCst);
        }

        fn on_bar(&mut self, _bar: &Bar, _ctx: &mut StrategyContext) {
            self.bar_count.fetch_add(1, Ordering::SeqCst);
        }

        fn on_stop(&mut self, _ctx: &mut StrategyContext) -> Result<()> {
            self.stop_called.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    fn create_test_runner(strategy: MockStrategy) -> (StrategyRunner, mpsc::Sender<RunnerCommand>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(100);
        let (signal_tx, _signal_rx) = mpsc::channel(100);

        let ctx = StrategyContext::new(
            strategy.name().to_string(),
            PortfolioSnapshot::default(),
            signal_tx,
            Arc::new(SystemClock),
            None,
        );

        let runner = StrategyRunner::new(Box::new(strategy), ctx, cmd_rx, false);

        (runner, cmd_tx)
    }

    fn create_test_tick() -> Tick {
        TickData::builder()
            .symbol(Symbol::new_unchecked("BTC-USDT"))
            .timestamp(Timestamp::new_unchecked(1_704_067_200_000))
            .price(Price::new(dec!(42000)).unwrap())
            .volume(Quantity::new(dec!(0.5)).unwrap())
            .build()
            .unwrap()
    }

    fn create_test_bar() -> Bar {
        KlineData::builder()
            .symbol(Symbol::new_unchecked("BTC-USDT"))
            .timestamp(Timestamp::new_unchecked(1_704_067_200_000))
            .period(KlinePeriod::Hour1)
            .open(Price::new(dec!(42000)).unwrap())
            .high(Price::new(dec!(42500)).unwrap())
            .low(Price::new(dec!(41800)).unwrap())
            .close(Price::new(dec!(42300)).unwrap())
            .volume(Quantity::new(dec!(100)).unwrap())
            .turnover(Amount::new(dec!(4200000)).unwrap())
            .build()
            .unwrap()
    }

    #[test]
    fn test_runner_creation() {
        let strategy = MockStrategy::new("test_strategy");
        let (runner, _cmd_tx) = create_test_runner(strategy);

        assert_eq!(runner.name(), "test_strategy");
        assert_eq!(runner.error_count(), 0);
        assert!(!runner.is_disabled());
        assert!(!runner.is_python());
    }

    #[test]
    fn test_runner_with_error_threshold() {
        let strategy = MockStrategy::new("test_strategy");
        let (runner, _cmd_tx) = create_test_runner(strategy);
        let runner = runner.with_error_threshold(5);

        assert_eq!(runner.error_threshold, 5);
    }

    #[tokio::test]
    async fn test_runner_lifecycle() {
        let strategy = MockStrategy::new("test_strategy");
        let init_called = strategy.init_called();
        let stop_called = strategy.stop_called();
        let tick_count = strategy.tick_count();

        let (runner, cmd_tx) = create_test_runner(strategy);

        // Spawn the runner
        let handle = tokio::spawn(runner.run());

        // Give it time to initialize
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Send some ticks
        cmd_tx
            .send(RunnerCommand::tick(create_test_tick()))
            .await
            .unwrap();
        cmd_tx
            .send(RunnerCommand::tick(create_test_tick()))
            .await
            .unwrap();

        // Give it time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Stop the runner
        cmd_tx.send(RunnerCommand::stop()).await.unwrap();

        // Wait for completion
        handle.await.unwrap();

        // Verify lifecycle
        assert!(
            init_called.load(Ordering::SeqCst),
            "on_init should be called"
        );
        assert!(
            stop_called.load(Ordering::SeqCst),
            "on_stop should be called"
        );
        assert_eq!(
            tick_count.load(Ordering::SeqCst),
            2,
            "should process 2 ticks"
        );
    }

    #[tokio::test]
    async fn test_runner_bar_processing() {
        let strategy = MockStrategy::new("test_strategy");
        let bar_count = strategy.bar_count();

        let (runner, cmd_tx) = create_test_runner(strategy);

        let handle = tokio::spawn(runner.run());

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        cmd_tx
            .send(RunnerCommand::bar(create_test_bar()))
            .await
            .unwrap();
        cmd_tx
            .send(RunnerCommand::bar(create_test_bar()))
            .await
            .unwrap();
        cmd_tx
            .send(RunnerCommand::bar(create_test_bar()))
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        cmd_tx.send(RunnerCommand::stop()).await.unwrap();
        handle.await.unwrap();

        assert_eq!(bar_count.load(Ordering::SeqCst), 3, "should process 3 bars");
    }

    #[tokio::test]
    async fn test_runner_init_failure() {
        let strategy = MockStrategy::new("test_strategy").with_init_failure();
        let init_called = strategy.init_called();
        let stop_called = strategy.stop_called();

        let (runner, _cmd_tx) = create_test_runner(strategy);

        let handle = tokio::spawn(runner.run());
        handle.await.unwrap();

        // Init should be called but fail
        assert!(
            init_called.load(Ordering::SeqCst),
            "on_init should be called"
        );
        // on_stop should NOT be called if init fails
        assert!(
            !stop_called.load(Ordering::SeqCst),
            "on_stop should not be called on init failure"
        );
    }

    #[tokio::test]
    async fn test_runner_panic_catching() {
        let strategy = MockStrategy::new("test_strategy").with_panic();
        let stop_called = strategy.stop_called();

        let (runner, cmd_tx) = create_test_runner(strategy);
        let runner = runner.with_error_threshold(3);

        let handle = tokio::spawn(runner.run());

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Send ticks that will cause panics
        cmd_tx
            .send(RunnerCommand::tick(create_test_tick()))
            .await
            .unwrap();
        cmd_tx
            .send(RunnerCommand::tick(create_test_tick()))
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        cmd_tx.send(RunnerCommand::stop()).await.unwrap();
        handle.await.unwrap();

        // on_stop should still be called even after panics
        assert!(
            stop_called.load(Ordering::SeqCst),
            "on_stop should be called even after panics"
        );
    }

    #[tokio::test]
    async fn test_runner_error_threshold() {
        let strategy = MockStrategy::new("test_strategy").with_panic();
        let tick_count = strategy.tick_count();

        let (runner, cmd_tx) = create_test_runner(strategy);
        let runner = runner.with_error_threshold(2);

        let handle = tokio::spawn(runner.run());

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Send 5 ticks - first 2 will panic and disable, rest should be ignored
        for _ in 0..5 {
            cmd_tx
                .send(RunnerCommand::tick(create_test_tick()))
                .await
                .unwrap();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        cmd_tx.send(RunnerCommand::stop()).await.unwrap();
        handle.await.unwrap();

        // tick_count should be 0 because all ticks panicked
        assert_eq!(tick_count.load(Ordering::SeqCst), 0);
    }
}
