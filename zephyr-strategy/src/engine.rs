//! Strategy engine orchestrator.
//!
//! The `StrategyEngine` is the main entry point for running trading strategies.
//! It handles:
//! - Loading strategies from configuration
//! - Creating and managing strategy runners
//! - Setting up subscription routing
//! - Broadcasting market data to runners
//! - Graceful shutdown with proper lifecycle management
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     StrategyEngine                          │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
//! │  │ Config      │  │ Subscription│  │ Signal Aggregator   │  │
//! │  │ Loader      │  │ Router      │  │                     │  │
//! │  └─────────────┘  └─────────────┘  └─────────────────────┘  │
//! │                         │                    ▲              │
//! │                         ▼                    │              │
//! │  ┌─────────────────────────────────────────────────────┐    │
//! │  │              Strategy Runners (per strategy)        │    │
//! │  │  ┌──────────────┐  ┌──────────────┐  ┌──────────┐   │    │
//! │  │  │ Runner 1     │  │ Runner 2     │  │ Runner N │   │    │
//! │  │  │ (DualThrust) │  │ (Grid)       │  │ (...)    │   │    │
//! │  │  └──────────────┘  └──────────────┘  └──────────┘   │    │
//! │  └─────────────────────────────────────────────────────┘    │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use zephyr_strategy::engine::{StrategyEngine, EngineConfig};
//!
//! // Load configuration
//! let config = EngineConfig::from_file("strategies.toml")?;
//!
//! // Create and start engine
//! let engine = StrategyEngine::new(config)?;
//! let handle = engine.start().await?;
//!
//! // Broadcast market data
//! handle.broadcast_tick(tick).await?;
//!
//! // Graceful shutdown
//! handle.shutdown().await?;
//! ```

use crate::context::{Clock, PortfolioSnapshot, StrategyContext, SubscriptionCommand, SystemClock};
use crate::loader::{EngineConfig, LoadError, StrategyConfig, StrategyType, load_strategy};
use crate::runner::{RunnerCommand, StrategyRunner};
use crate::signal::{Signal, SignalId};
use crate::r#trait::{DataType, Subscription};
use crate::types::{Bar, Tick};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};
use zephyr_core::types::Symbol;

#[cfg(feature = "hot-reload")]
use crate::runner::HotReloader;

/// Default channel capacity for runner commands.
pub const DEFAULT_COMMAND_CHANNEL_CAPACITY: usize = 1000;

/// Default channel capacity for signals.
pub const DEFAULT_SIGNAL_CHANNEL_CAPACITY: usize = 1000;

/// Default channel capacity for subscription commands.
pub const DEFAULT_SUBSCRIPTION_CHANNEL_CAPACITY: usize = 100;

/// Errors that can occur during engine operations.
#[derive(Debug, Error)]
pub enum EngineError {
    /// Failed to load a strategy
    #[error("failed to load strategy '{name}': {source}")]
    LoadError {
        /// Name of the strategy that failed to load
        name: String,
        /// The underlying error
        #[source]
        source: LoadError,
    },

    /// No strategies configured
    #[error("no strategies configured")]
    NoStrategies,

    /// Engine is already running
    #[error("engine is already running")]
    AlreadyRunning,

    /// Engine is not running
    #[error("engine is not running")]
    NotRunning,

    /// Failed to send command to runner
    #[error("failed to send command to runner '{name}': channel closed")]
    SendError {
        /// Name of the runner
        name: String,
    },

    /// Shutdown error
    #[error("shutdown error: {0}")]
    ShutdownError(String),
}

/// Information about a loaded strategy.
#[derive(Debug)]
pub struct StrategyInfo {
    /// Strategy name
    pub name: String,
    /// Strategy type (Rust or Python)
    pub strategy_type: StrategyType,
    /// Strategy class name
    pub class: String,
    /// Subscriptions declared by the strategy
    pub subscriptions: Vec<Subscription>,
    /// Whether the strategy is a Python strategy
    pub is_python: bool,
}

/// A loaded strategy with its runner channel.
struct LoadedStrategy {
    /// Strategy information
    info: StrategyInfo,
    /// Strategy configuration (for hot reload)
    config: StrategyConfig,
}

/// Handle to a running strategy engine.
///
/// This handle is used to interact with the engine after it has been started.
/// It provides methods for broadcasting market data and shutting down.
pub struct EngineHandle {
    /// Command senders for each runner (by strategy name)
    runners: HashMap<String, mpsc::Sender<RunnerCommand>>,
    /// Subscription routing table: `(symbol, data_type)` -> list of strategy names
    subscriptions: HashMap<(Symbol, DataType), Vec<String>>,
    /// Signal receiver (aggregated from all strategies)
    signal_rx: mpsc::Receiver<(SignalId, Signal)>,
    /// Subscription command receiver
    subscription_rx: mpsc::Receiver<SubscriptionCommand>,
    /// Join handles for runner tasks
    runner_handles: Vec<(String, JoinHandle<()>)>,
    /// Hot reloader (if enabled)
    #[cfg(feature = "hot-reload")]
    hot_reloader: Option<HotReloader>,
}

impl EngineHandle {
    /// Broadcasts a tick to all subscribed strategies.
    ///
    /// Only strategies that have subscribed to the tick's symbol will receive it.
    ///
    /// # Arguments
    ///
    /// * `tick` - The tick data to broadcast
    ///
    /// # Returns
    ///
    /// The number of strategies that received the tick.
    pub async fn broadcast_tick(&self, tick: Tick) -> usize {
        let key = (tick.symbol.clone(), DataType::Tick);
        self.broadcast_to_subscribers(&key, RunnerCommand::tick(tick))
            .await
    }

    /// Broadcasts a bar to all subscribed strategies.
    ///
    /// Only strategies that have subscribed to the bar's symbol and timeframe
    /// will receive it.
    ///
    /// # Arguments
    ///
    /// * `bar` - The bar data to broadcast
    /// * `data_type` - The data type (should be `DataType::Bar(timeframe)`)
    ///
    /// # Returns
    ///
    /// The number of strategies that received the bar.
    pub async fn broadcast_bar(&self, bar: Bar, data_type: DataType) -> usize {
        let key = (bar.symbol.clone(), data_type);
        self.broadcast_to_subscribers(&key, RunnerCommand::bar(bar))
            .await
    }

    /// Broadcasts a command to strategies subscribed to a specific key.
    async fn broadcast_to_subscribers(
        &self,
        key: &(Symbol, DataType),
        cmd: RunnerCommand,
    ) -> usize {
        let Some(strategy_names) = self.subscriptions.get(key) else {
            return 0;
        };

        let mut sent_count = 0;
        for name in strategy_names {
            if let Some(tx) = self.runners.get(name) {
                if tx.send(cmd.clone()).await.is_ok() {
                    sent_count += 1;
                } else {
                    warn!(strategy = %name, "Failed to send command to runner");
                }
            }
        }
        sent_count
    }

    /// Sends a command to a specific strategy by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The strategy name
    /// * `cmd` - The command to send
    ///
    /// # Errors
    ///
    /// Returns an error if the strategy is not found or the channel is closed.
    pub async fn send_to_strategy(
        &self,
        name: &str,
        cmd: RunnerCommand,
    ) -> Result<(), EngineError> {
        let tx = self
            .runners
            .get(name)
            .ok_or_else(|| EngineError::SendError {
                name: name.to_string(),
            })?;

        tx.send(cmd).await.map_err(|_| EngineError::SendError {
            name: name.to_string(),
        })
    }

    /// Receives the next signal from any strategy.
    ///
    /// Returns `None` if all signal channels are closed.
    pub async fn recv_signal(&mut self) -> Option<(SignalId, Signal)> {
        self.signal_rx.recv().await
    }

    /// Tries to receive a signal without blocking.
    ///
    /// Returns `None` if no signal is available.
    pub fn try_recv_signal(&mut self) -> Option<(SignalId, Signal)> {
        self.signal_rx.try_recv().ok()
    }

    /// Receives the next subscription command.
    ///
    /// Returns `None` if all subscription channels are closed.
    pub async fn recv_subscription(&mut self) -> Option<SubscriptionCommand> {
        self.subscription_rx.recv().await
    }

    /// Updates the subscription routing table.
    ///
    /// Call this after receiving a subscription command to update routing.
    pub fn update_subscription(&mut self, strategy_name: &str, cmd: SubscriptionCommand) {
        match cmd {
            SubscriptionCommand::Add { symbol, data_type } => {
                let key = (symbol, data_type);
                self.subscriptions
                    .entry(key)
                    .or_default()
                    .push(strategy_name.to_string());
            }
            SubscriptionCommand::Remove { symbol, data_type } => {
                let key = (symbol, data_type);
                if let Some(names) = self.subscriptions.get_mut(&key) {
                    names.retain(|n| n != strategy_name);
                    if names.is_empty() {
                        self.subscriptions.remove(&key);
                    }
                }
            }
        }
    }

    /// Returns the list of strategy names.
    pub fn strategy_names(&self) -> Vec<&str> {
        self.runners.keys().map(String::as_str).collect()
    }

    /// Returns the number of running strategies.
    #[must_use]
    pub fn strategy_count(&self) -> usize {
        self.runners.len()
    }

    /// Gracefully shuts down all strategies.
    ///
    /// This method:
    /// 1. Sends Stop commands to all runners
    /// 2. Waits for all runners to complete
    /// 3. Ensures `on_stop()` is called for all strategies
    ///
    /// # Errors
    ///
    /// Returns an error if shutdown fails.
    pub async fn shutdown(self) -> Result<(), EngineError> {
        info!("Shutting down strategy engine");

        // Send stop commands to all runners
        for (name, tx) in &self.runners {
            if let Err(e) = tx.send(RunnerCommand::stop()).await {
                warn!(strategy = %name, error = %e, "Failed to send stop command");
            }
        }

        // Wait for all runners to complete
        for (name, handle) in self.runner_handles {
            match handle.await {
                Ok(()) => {
                    info!(strategy = %name, "Strategy runner stopped");
                }
                Err(e) => {
                    error!(strategy = %name, error = %e, "Strategy runner panicked");
                }
            }
        }

        info!("Strategy engine shutdown complete");
        Ok(())
    }
}

/// Strategy engine orchestrator.
///
/// The engine is responsible for:
/// - Loading strategies from configuration
/// - Creating strategy contexts and runners
/// - Managing the lifecycle of all strategies
/// - Routing market data to appropriate strategies
///
/// # Lifecycle
///
/// 1. Create engine with `StrategyEngine::new(config)`
/// 2. Start engine with `engine.start().await`
/// 3. Use the returned `EngineHandle` to broadcast data and receive signals
/// 4. Shutdown with `handle.shutdown().await`
pub struct StrategyEngine {
    /// Engine configuration
    config: EngineConfig,
    /// Loaded strategies (before starting)
    strategies: Vec<LoadedStrategy>,
    /// Clock for strategy contexts
    clock: Arc<dyn Clock>,
    /// Portfolio snapshot (shared across strategies)
    portfolio: PortfolioSnapshot,
    /// Signal channel capacity
    signal_capacity: usize,
    /// Command channel capacity
    command_capacity: usize,
    /// Subscription channel capacity
    subscription_capacity: usize,
}

impl StrategyEngine {
    /// Creates a new strategy engine from configuration.
    ///
    /// This loads all strategies defined in the configuration but does not
    /// start them. Call `start()` to begin execution.
    ///
    /// # Arguments
    ///
    /// * `config` - Engine configuration with strategy definitions
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No strategies are configured
    /// - Any strategy fails to load
    pub fn new(config: EngineConfig) -> Result<Self, EngineError> {
        if config.strategies.is_empty() {
            return Err(EngineError::NoStrategies);
        }

        Ok(Self {
            config,
            strategies: Vec::new(),
            clock: Arc::new(SystemClock),
            portfolio: PortfolioSnapshot::default(),
            signal_capacity: DEFAULT_SIGNAL_CHANNEL_CAPACITY,
            command_capacity: DEFAULT_COMMAND_CHANNEL_CAPACITY,
            subscription_capacity: DEFAULT_SUBSCRIPTION_CHANNEL_CAPACITY,
        })
    }

    /// Sets a custom clock for the engine.
    ///
    /// Use this for backtesting with deterministic time.
    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// Sets the initial portfolio snapshot.
    #[must_use]
    pub fn with_portfolio(mut self, portfolio: PortfolioSnapshot) -> Self {
        self.portfolio = portfolio;
        self
    }

    /// Sets the signal channel capacity.
    #[must_use]
    pub fn with_signal_capacity(mut self, capacity: usize) -> Self {
        self.signal_capacity = capacity;
        self
    }

    /// Sets the command channel capacity.
    #[must_use]
    pub fn with_command_capacity(mut self, capacity: usize) -> Self {
        self.command_capacity = capacity;
        self
    }

    /// Loads all strategies from configuration.
    ///
    /// This is called automatically by `start()`, but can be called manually
    /// to pre-load strategies and check for errors.
    ///
    /// # Errors
    ///
    /// Returns an error if any strategy fails to load.
    pub fn load_strategies(&mut self) -> Result<Vec<StrategyInfo>, EngineError> {
        let mut infos = Vec::new();

        for strategy_config in &self.config.strategies {
            let loaded = Self::load_single_strategy(strategy_config)?;
            infos.push(StrategyInfo {
                name: loaded.info.name.clone(),
                strategy_type: loaded.info.strategy_type,
                class: loaded.info.class.clone(),
                subscriptions: loaded.info.subscriptions.clone(),
                is_python: loaded.info.is_python,
            });
            self.strategies.push(loaded);
        }

        Ok(infos)
    }

    /// Loads a single strategy from configuration.
    fn load_single_strategy(config: &StrategyConfig) -> Result<LoadedStrategy, EngineError> {
        info!(
            strategy = %config.name,
            strategy_type = %config.strategy_type,
            class = %config.class,
            "Loading strategy"
        );

        // Load the strategy
        let strategy = load_strategy(config).map_err(|e| EngineError::LoadError {
            name: config.name.clone(),
            source: e,
        })?;

        // Get subscriptions
        let subscriptions = strategy.required_subscriptions();
        let is_python = config.strategy_type == StrategyType::Python;

        let info = StrategyInfo {
            name: config.name.clone(),
            strategy_type: config.strategy_type,
            class: config.class.clone(),
            subscriptions,
            is_python,
        };

        info!(
            strategy = %config.name,
            subscriptions = ?info.subscriptions,
            "Strategy loaded successfully"
        );

        Ok(LoadedStrategy {
            info,
            config: config.clone(),
        })
    }

    /// Starts the strategy engine.
    ///
    /// This method:
    /// 1. Loads all strategies (if not already loaded)
    /// 2. Creates strategy contexts and runners
    /// 3. Spawns runner tasks
    /// 4. Sets up subscription routing
    /// 5. Returns an `EngineHandle` for interaction
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Strategies fail to load
    /// - Engine is already running
    #[allow(clippy::unused_async)]
    pub async fn start(mut self) -> Result<EngineHandle, EngineError> {
        // Load strategies if not already loaded
        if self.strategies.is_empty() {
            self.load_strategies()?;
        }

        // Create aggregated signal channel
        let (signal_tx, signal_rx) = mpsc::channel(self.signal_capacity);

        // Create subscription command channel
        let (subscription_tx, subscription_rx) = mpsc::channel(self.subscription_capacity);

        // Build subscription routing table
        let mut subscriptions: HashMap<(Symbol, DataType), Vec<String>> = HashMap::new();
        let mut runners: HashMap<String, mpsc::Sender<RunnerCommand>> = HashMap::new();
        let mut runner_handles: Vec<(String, JoinHandle<()>)> = Vec::new();

        // Store config for hot reloader setup (before consuming strategies)
        #[cfg(feature = "hot-reload")]
        let config_strategies = self.config.strategies.clone();

        // Create and start runners for each strategy
        for loaded in self.strategies {
            let name = loaded.info.name.clone();
            let is_python = loaded.info.is_python;

            // Create command channel for this runner
            let (cmd_tx, cmd_rx) = mpsc::channel(self.command_capacity);

            // Create strategy context
            let ctx = StrategyContext::new(
                name.clone(),
                self.portfolio.clone(),
                signal_tx.clone(),
                Arc::clone(&self.clock),
                Some(subscription_tx.clone()),
            );

            // Reload the strategy (we need a fresh instance for the runner)
            let strategy = load_strategy(&loaded.config).map_err(|e| EngineError::LoadError {
                name: name.clone(),
                source: e,
            })?;

            // Create runner
            #[allow(unused_mut)]
            let mut runner = StrategyRunner::new(strategy, ctx, cmd_rx, is_python)
                .with_strategy_config(loaded.config.params.clone());

            // Configure Python settings if applicable
            #[cfg(feature = "python")]
            if is_python {
                runner = runner.with_python_config(self.config.python.clone());
            }

            // Register subscriptions
            for sub in &loaded.info.subscriptions {
                let key = (sub.symbol.clone(), sub.data_type);
                subscriptions.entry(key).or_default().push(name.clone());
            }

            // Spawn runner task
            let handle = tokio::spawn(runner.run());
            runner_handles.push((name.clone(), handle));
            runners.insert(name, cmd_tx);
        }

        info!(
            strategy_count = runners.len(),
            subscription_count = subscriptions.len(),
            "Strategy engine started"
        );

        // Set up hot reloader if enabled
        #[cfg(feature = "hot-reload")]
        let hot_reloader = Self::setup_hot_reloader(&config_strategies, &runners);

        Ok(EngineHandle {
            runners,
            subscriptions,
            signal_rx,
            subscription_rx,
            runner_handles,
            #[cfg(feature = "hot-reload")]
            hot_reloader,
        })
    }

    /// Sets up the hot reloader for Python strategies.
    #[cfg(feature = "hot-reload")]
    fn setup_hot_reloader(
        config_strategies: &[StrategyConfig],
        runners: &HashMap<String, mpsc::Sender<RunnerCommand>>,
    ) -> Option<HotReloader> {
        // Find Python strategies with paths
        let python_strategies: Vec<_> = config_strategies
            .iter()
            .filter(|s| s.strategy_type == StrategyType::Python)
            .filter_map(|s| {
                s.path
                    .as_ref()
                    .map(|p| (s.name.clone(), p.clone(), s.class.clone()))
            })
            .collect();

        if python_strategies.is_empty() {
            return None;
        }

        let mut reloader = HotReloader::new();
        for (name, path, class) in python_strategies {
            if let Some(tx) = runners.get(&name) {
                if let Err(e) = reloader.watch(path.to_string_lossy().as_ref(), &class, tx.clone())
                {
                    warn!(
                        strategy = %name,
                        path = %path.display(),
                        error = %e,
                        "Failed to set up hot reload"
                    );
                } else {
                    info!(
                        strategy = %name,
                        path = %path.display(),
                        "Hot reload enabled"
                    );
                }
            }
        }

        // Start the hot reloader
        if let Err(e) = reloader.start() {
            warn!(error = %e, "Failed to start hot reloader");
            return None;
        }

        Some(reloader)
    }
}

/// Builder for creating a strategy engine with custom settings.
pub struct EngineBuilder {
    config: EngineConfig,
    clock: Option<Arc<dyn Clock>>,
    portfolio: Option<PortfolioSnapshot>,
    signal_capacity: usize,
    command_capacity: usize,
    subscription_capacity: usize,
}

impl EngineBuilder {
    /// Creates a new engine builder from configuration.
    #[must_use]
    pub fn new(config: EngineConfig) -> Self {
        Self {
            config,
            clock: None,
            portfolio: None,
            signal_capacity: DEFAULT_SIGNAL_CHANNEL_CAPACITY,
            command_capacity: DEFAULT_COMMAND_CHANNEL_CAPACITY,
            subscription_capacity: DEFAULT_SUBSCRIPTION_CHANNEL_CAPACITY,
        }
    }

    /// Sets a custom clock.
    #[must_use]
    pub fn clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = Some(clock);
        self
    }

    /// Sets the initial portfolio.
    #[must_use]
    pub fn portfolio(mut self, portfolio: PortfolioSnapshot) -> Self {
        self.portfolio = Some(portfolio);
        self
    }

    /// Sets the signal channel capacity.
    #[must_use]
    pub fn signal_capacity(mut self, capacity: usize) -> Self {
        self.signal_capacity = capacity;
        self
    }

    /// Sets the command channel capacity.
    #[must_use]
    pub fn command_capacity(mut self, capacity: usize) -> Self {
        self.command_capacity = capacity;
        self
    }

    /// Sets the subscription channel capacity.
    #[must_use]
    pub fn subscription_capacity(mut self, capacity: usize) -> Self {
        self.subscription_capacity = capacity;
        self
    }

    /// Builds the strategy engine.
    ///
    /// # Errors
    ///
    /// Returns an error if no strategies are configured.
    pub fn build(self) -> Result<StrategyEngine, EngineError> {
        let mut engine = StrategyEngine::new(self.config)?;

        if let Some(clock) = self.clock {
            engine.clock = clock;
        }

        if let Some(portfolio) = self.portfolio {
            engine.portfolio = portfolio;
        }

        engine.signal_capacity = self.signal_capacity;
        engine.command_capacity = self.command_capacity;
        engine.subscription_capacity = self.subscription_capacity;

        Ok(engine)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::BacktestClock;
    use rust_decimal_macros::dec;
    use zephyr_core::types::{Symbol, Timestamp};

    fn create_rust_strategy_config(name: &str, class: &str) -> StrategyConfig {
        StrategyConfig {
            name: name.to_string(),
            strategy_type: StrategyType::Rust,
            class: class.to_string(),
            path: None,
            params: serde_json::json!({}),
        }
    }

    #[test]
    fn test_engine_creation_no_strategies() {
        let config = EngineConfig::default();
        let result = StrategyEngine::new(config);
        assert!(matches!(result, Err(EngineError::NoStrategies)));
    }

    #[test]
    fn test_engine_creation_with_strategies() {
        let config = EngineConfig {
            strategies: vec![create_rust_strategy_config("test", "DualThrust")],
            ..Default::default()
        };
        let result = StrategyEngine::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_engine_builder() {
        let config = EngineConfig {
            strategies: vec![create_rust_strategy_config("test", "DualThrust")],
            ..Default::default()
        };

        let clock = Arc::new(BacktestClock::new(Timestamp::new_unchecked(1000)));
        let portfolio = PortfolioSnapshot {
            cash: dec!(10000),
            ..Default::default()
        };

        let engine = EngineBuilder::new(config)
            .clock(clock)
            .portfolio(portfolio)
            .signal_capacity(500)
            .command_capacity(500)
            .build();

        assert!(engine.is_ok());
        let engine = engine.unwrap();
        assert_eq!(engine.signal_capacity, 500);
        assert_eq!(engine.command_capacity, 500);
    }

    #[test]
    fn test_engine_with_custom_clock() {
        let config = EngineConfig {
            strategies: vec![create_rust_strategy_config("test", "DualThrust")],
            ..Default::default()
        };

        let clock = Arc::new(BacktestClock::new(Timestamp::new_unchecked(1000)));
        let engine = StrategyEngine::new(config).unwrap().with_clock(clock);

        // Clock should be set
        assert_eq!(engine.clock.now().as_millis(), 1000);
    }

    #[test]
    fn test_strategy_info() {
        let info = StrategyInfo {
            name: "test_strategy".to_string(),
            strategy_type: StrategyType::Rust,
            class: "DualThrust".to_string(),
            subscriptions: vec![Subscription::tick(Symbol::new_unchecked("BTC-USDT"))],
            is_python: false,
        };

        assert_eq!(info.name, "test_strategy");
        assert!(!info.is_python);
        assert_eq!(info.subscriptions.len(), 1);
    }

    #[test]
    fn test_error_display() {
        let err = EngineError::NoStrategies;
        assert!(format!("{err}").contains("no strategies"));

        let err = EngineError::AlreadyRunning;
        assert!(format!("{err}").contains("already running"));

        let err = EngineError::SendError {
            name: "test".to_string(),
        };
        assert!(format!("{err}").contains("test"));
    }

    // Integration tests require async runtime
    #[tokio::test]
    async fn test_engine_handle_subscription_update() {
        // Create a minimal handle for testing subscription updates
        let (_signal_tx, signal_rx) = mpsc::channel(10);
        let (_subscription_tx, subscription_rx) = mpsc::channel(10);

        let mut handle = EngineHandle {
            runners: HashMap::new(),
            subscriptions: HashMap::new(),
            signal_rx,
            subscription_rx,
            runner_handles: Vec::new(),
            #[cfg(feature = "hot-reload")]
            hot_reloader: None,
        };

        // Add a subscription
        let cmd = SubscriptionCommand::Add {
            symbol: Symbol::new_unchecked("BTC-USDT"),
            data_type: DataType::Tick,
        };
        handle.update_subscription("strategy1", cmd);

        let key = (Symbol::new_unchecked("BTC-USDT"), DataType::Tick);
        assert!(handle.subscriptions.contains_key(&key));
        assert_eq!(handle.subscriptions[&key], vec!["strategy1"]);

        // Add another strategy to the same subscription
        let cmd = SubscriptionCommand::Add {
            symbol: Symbol::new_unchecked("BTC-USDT"),
            data_type: DataType::Tick,
        };
        handle.update_subscription("strategy2", cmd);
        assert_eq!(handle.subscriptions[&key].len(), 2);

        // Remove a subscription
        let cmd = SubscriptionCommand::Remove {
            symbol: Symbol::new_unchecked("BTC-USDT"),
            data_type: DataType::Tick,
        };
        handle.update_subscription("strategy1", cmd);
        assert_eq!(handle.subscriptions[&key], vec!["strategy2"]);

        // Remove the last subscription
        let cmd = SubscriptionCommand::Remove {
            symbol: Symbol::new_unchecked("BTC-USDT"),
            data_type: DataType::Tick,
        };
        handle.update_subscription("strategy2", cmd);
        assert!(!handle.subscriptions.contains_key(&key));
    }

    #[tokio::test]
    async fn test_engine_handle_strategy_names() {
        let (_signal_tx, signal_rx) = mpsc::channel(10);
        let (_subscription_tx, subscription_rx) = mpsc::channel(10);

        let mut runners = HashMap::new();
        let (tx1, _rx1) = mpsc::channel(10);
        let (tx2, _rx2) = mpsc::channel(10);
        runners.insert("strategy1".to_string(), tx1);
        runners.insert("strategy2".to_string(), tx2);

        let handle = EngineHandle {
            runners,
            subscriptions: HashMap::new(),
            signal_rx,
            subscription_rx,
            runner_handles: Vec::new(),
            #[cfg(feature = "hot-reload")]
            hot_reloader: None,
        };

        assert_eq!(handle.strategy_count(), 2);
        let names = handle.strategy_names();
        assert!(names.contains(&"strategy1"));
        assert!(names.contains(&"strategy2"));
    }

    #[tokio::test]
    async fn test_engine_full_lifecycle() {
        // Create a config with DualThrust strategy
        let config = EngineConfig {
            strategies: vec![StrategyConfig {
                name: "test_dual_thrust".to_string(),
                strategy_type: StrategyType::Rust,
                class: "DualThrust".to_string(),
                path: None,
                params: serde_json::json!({
                    "name": "test_dual_thrust",
                    "symbol": "BTC-USDT",
                    "lookback": 4,
                    "k1": "0.5",
                    "k2": "0.5",
                    "position_size": "1.0"
                }),
            }],
            ..Default::default()
        };

        // Create and start the engine
        let engine = StrategyEngine::new(config).unwrap();
        let handle = engine.start().await.unwrap();

        // Verify the strategy is running
        assert_eq!(handle.strategy_count(), 1);
        assert!(handle.strategy_names().contains(&"test_dual_thrust"));

        // Give the runner time to initialize
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Shutdown gracefully
        let result = handle.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_engine_multiple_strategies() {
        // Create a config with multiple strategies
        let config = EngineConfig {
            strategies: vec![
                StrategyConfig {
                    name: "strategy_1".to_string(),
                    strategy_type: StrategyType::Rust,
                    class: "DualThrust".to_string(),
                    path: None,
                    params: serde_json::json!({
                        "name": "strategy_1",
                        "symbol": "BTC-USDT"
                    }),
                },
                StrategyConfig {
                    name: "strategy_2".to_string(),
                    strategy_type: StrategyType::Rust,
                    class: "DualThrust".to_string(),
                    path: None,
                    params: serde_json::json!({
                        "name": "strategy_2",
                        "symbol": "ETH-USDT"
                    }),
                },
            ],
            ..Default::default()
        };

        // Create and start the engine
        let engine = StrategyEngine::new(config).unwrap();
        let handle = engine.start().await.unwrap();

        // Verify both strategies are running
        assert_eq!(handle.strategy_count(), 2);
        let names = handle.strategy_names();
        assert!(names.contains(&"strategy_1"));
        assert!(names.contains(&"strategy_2"));

        // Give the runners time to initialize
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Shutdown gracefully
        let result = handle.shutdown().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_engine_load_strategies_before_start() {
        let config = EngineConfig {
            strategies: vec![StrategyConfig {
                name: "preload_test".to_string(),
                strategy_type: StrategyType::Rust,
                class: "DualThrust".to_string(),
                path: None,
                params: serde_json::json!({
                    "name": "preload_test",
                    "symbol": "BTC-USDT"
                }),
            }],
            ..Default::default()
        };

        let mut engine = StrategyEngine::new(config).unwrap();

        // Pre-load strategies
        let infos = engine.load_strategies().unwrap();
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].name, "preload_test");
        assert_eq!(infos[0].class, "DualThrust");
        assert!(!infos[0].is_python);
        assert!(!infos[0].subscriptions.is_empty());

        // Start the engine
        let handle = engine.start().await.unwrap();
        assert_eq!(handle.strategy_count(), 1);

        // Shutdown
        handle.shutdown().await.unwrap();
    }
}
