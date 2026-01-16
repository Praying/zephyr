//! Strategy context (sandbox interface).
//!
//! The `StrategyContext` provides a controlled interface for strategies to interact
//! with the trading system. It ensures strategies remain isolated and cannot directly
//! access system internals.

use crate::Decimal;
use crate::signal::{Signal, SignalId};
use crate::r#trait::DataType;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use zephyr_core::types::{Symbol, Timestamp};

/// Error type for signal emission.
#[derive(Debug, Clone, Error)]
pub enum EmitError {
    /// Signal channel is full, indicating system overload.
    #[error("signal channel is full, system overload")]
    ChannelFull,

    /// Signal channel is closed, strategy should stop.
    #[error("signal channel is closed")]
    ChannelClosed,
}

/// Command for dynamic subscription management.
#[derive(Debug, Clone)]
pub enum SubscriptionCommand {
    /// Add a new subscription
    Add {
        /// Symbol to subscribe to
        symbol: Symbol,
        /// Type of data to receive
        data_type: DataType,
    },
    /// Remove an existing subscription
    Remove {
        /// Symbol to unsubscribe from
        symbol: Symbol,
        /// Type of data to stop receiving
        data_type: DataType,
    },
}

/// Clock trait for time abstraction.
///
/// Allows strategies to get the current time in a way that's consistent
/// between live trading and backtesting.
pub trait Clock: Send + Sync {
    /// Returns the current engine time.
    fn now(&self) -> Timestamp;
}

/// System clock implementation for live trading.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::now()
    }
}

/// Backtest clock implementation for deterministic time.
pub struct BacktestClock {
    current_time: AtomicU64,
}

impl BacktestClock {
    /// Creates a new backtest clock with the given initial time.
    #[must_use]
    #[allow(clippy::cast_sign_loss)]
    pub fn new(initial_time: Timestamp) -> Self {
        Self {
            current_time: AtomicU64::new(initial_time.as_millis() as u64),
        }
    }

    /// Sets the current time.
    #[allow(clippy::cast_sign_loss)]
    pub fn set_time(&self, time: Timestamp) {
        self.current_time
            .store(time.as_millis() as u64, Ordering::SeqCst);
    }

    /// Advances the clock by the given milliseconds.
    pub fn advance(&self, millis: u64) {
        self.current_time.fetch_add(millis, Ordering::SeqCst);
    }
}

impl Clock for BacktestClock {
    #[allow(clippy::cast_possible_wrap)]
    fn now(&self) -> Timestamp {
        Timestamp::new_unchecked(self.current_time.load(Ordering::SeqCst) as i64)
    }
}

/// Position information for a single symbol.
#[derive(Debug, Clone)]
pub struct Position {
    /// Trading pair symbol
    pub symbol: Symbol,
    /// Current position quantity (positive = long, negative = short)
    pub quantity: Decimal,
    /// Average entry price
    pub avg_price: Decimal,
    /// Unrealized profit/loss
    pub unrealized_pnl: Decimal,
}

impl Position {
    /// Creates a new position.
    #[must_use]
    pub fn new(symbol: Symbol) -> Self {
        Self {
            symbol,
            quantity: Decimal::ZERO,
            avg_price: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
        }
    }
}

/// Read-only snapshot of the portfolio state.
#[derive(Debug, Clone, Default)]
pub struct PortfolioSnapshot {
    /// Current positions by symbol
    pub positions: HashMap<Symbol, Position>,
    /// Available cash balance
    pub cash: Decimal,
    /// Total equity (cash + positions value)
    pub equity: Decimal,
    /// Margin currently in use
    pub margin_used: Decimal,
}

impl PortfolioSnapshot {
    /// Returns the position for a symbol, or None if no position exists.
    #[must_use]
    pub fn position(&self, symbol: &Symbol) -> Option<&Position> {
        self.positions.get(symbol)
    }

    /// Returns the quantity held for a symbol (0 if no position).
    #[must_use]
    pub fn quantity(&self, symbol: &Symbol) -> Decimal {
        self.positions
            .get(symbol)
            .map_or(Decimal::ZERO, |p| p.quantity)
    }

    /// Returns true if there's an open position for the symbol.
    #[must_use]
    pub fn has_position(&self, symbol: &Symbol) -> bool {
        self.positions
            .get(symbol)
            .is_some_and(|p| p.quantity != Decimal::ZERO)
    }
}

/// Log level for strategy logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Debug level
    Debug,
    /// Info level
    Info,
    /// Warning level
    Warn,
    /// Error level
    Error,
}

/// Strategy context providing controlled access to system functionality.
///
/// The context is the primary interface between a strategy and the trading system.
/// It provides:
/// - Read-only access to portfolio state
/// - Signal emission for trading intents
/// - Time access (deterministic for backtesting)
/// - Logging interface
/// - Dynamic subscription management
///
/// # Thread Safety
///
/// The context is designed to be used from a single strategy instance.
/// Signal emission uses atomic operations for ID generation.
pub struct StrategyContext {
    /// Strategy name for logging
    strategy_name: String,

    /// Read-only portfolio snapshot
    portfolio: PortfolioSnapshot,

    /// Signal emission channel (bounded)
    signal_tx: mpsc::Sender<(SignalId, Signal)>,

    /// Signal ID generator
    next_signal_id: Arc<AtomicU64>,

    /// Engine clock (deterministic for backtest)
    clock: Arc<dyn Clock>,

    /// Subscription manager handle
    subscription_tx: Option<mpsc::Sender<SubscriptionCommand>>,
}

impl StrategyContext {
    /// Creates a new strategy context.
    #[must_use]
    pub fn new(
        strategy_name: String,
        portfolio: PortfolioSnapshot,
        signal_tx: mpsc::Sender<(SignalId, Signal)>,
        clock: Arc<dyn Clock>,
        subscription_tx: Option<mpsc::Sender<SubscriptionCommand>>,
    ) -> Self {
        Self {
            strategy_name,
            portfolio,
            signal_tx,
            next_signal_id: Arc::new(AtomicU64::new(1)),
            clock,
            subscription_tx,
        }
    }

    /// Creates a context for testing purposes.
    #[must_use]
    pub fn new_test(strategy_name: &str) -> (Self, mpsc::Receiver<(SignalId, Signal)>) {
        let (signal_tx, signal_rx) = mpsc::channel(100);
        let ctx = Self::new(
            strategy_name.to_string(),
            PortfolioSnapshot::default(),
            signal_tx,
            Arc::new(SystemClock),
            None,
        );
        (ctx, signal_rx)
    }

    /// Returns the current engine time.
    ///
    /// In live trading, this returns the system time.
    /// In backtesting, this returns the simulated time.
    #[must_use]
    pub fn now(&self) -> Timestamp {
        self.clock.now()
    }

    /// Returns a read-only reference to the portfolio snapshot.
    #[must_use]
    pub fn portfolio(&self) -> &PortfolioSnapshot {
        &self.portfolio
    }

    /// Updates the portfolio snapshot.
    ///
    /// Called by the runner to update portfolio state before each tick.
    pub fn update_portfolio(&mut self, portfolio: PortfolioSnapshot) {
        self.portfolio = portfolio;
    }

    /// Emits a trading signal.
    ///
    /// Returns a unique `SignalId` that can be used to correlate future
    /// order status updates back to this signal.
    ///
    /// # Errors
    ///
    /// Returns `EmitError::ChannelFull` if the signal channel is full.
    /// This indicates system overload and the strategy should reduce signal rate.
    ///
    /// Returns `EmitError::ChannelClosed` if the channel is closed.
    /// This indicates the strategy should stop.
    pub fn emit_signal(&mut self, signal: Signal) -> Result<SignalId, EmitError> {
        let id = SignalId(self.next_signal_id.fetch_add(1, Ordering::SeqCst));

        self.signal_tx.try_send((id, signal)).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => {
                error!(
                    strategy = %self.strategy_name,
                    signal_id = %id,
                    "CRITICAL: Signal channel full, system overload"
                );
                EmitError::ChannelFull
            }
            mpsc::error::TrySendError::Closed(_) => {
                error!(
                    strategy = %self.strategy_name,
                    "Signal channel closed"
                );
                EmitError::ChannelClosed
            }
        })?;

        Ok(id)
    }

    /// Subscribes to market data for a symbol.
    ///
    /// The subscription takes effect asynchronously.
    pub fn subscribe(&self, symbol: &Symbol, data_type: DataType) {
        if let Some(tx) = &self.subscription_tx
            && let Err(e) = tx.try_send(SubscriptionCommand::Add {
                symbol: symbol.clone(),
                data_type,
            })
        {
            warn!(
                strategy = %self.strategy_name,
                symbol = %symbol,
                data_type = %data_type,
                error = %e,
                "Failed to send subscribe command"
            );
        }
    }

    /// Unsubscribes from market data for a symbol.
    ///
    /// The unsubscription takes effect asynchronously.
    pub fn unsubscribe(&self, symbol: &Symbol, data_type: DataType) {
        if let Some(tx) = &self.subscription_tx
            && let Err(e) = tx.try_send(SubscriptionCommand::Remove {
                symbol: symbol.clone(),
                data_type,
            })
        {
            warn!(
                strategy = %self.strategy_name,
                symbol = %symbol,
                data_type = %data_type,
                error = %e,
                "Failed to send unsubscribe command"
            );
        }
    }

    /// Logs a message at the specified level.
    pub fn log(&self, level: LogLevel, msg: &str) {
        match level {
            LogLevel::Debug => tracing::debug!(strategy = %self.strategy_name, "{}", msg),
            LogLevel::Info => info!(strategy = %self.strategy_name, "{}", msg),
            LogLevel::Warn => warn!(strategy = %self.strategy_name, "{}", msg),
            LogLevel::Error => error!(strategy = %self.strategy_name, "{}", msg),
        }
    }

    /// Logs an info message.
    pub fn log_info(&self, msg: &str) {
        self.log(LogLevel::Info, msg);
    }

    /// Logs a warning message.
    pub fn log_warn(&self, msg: &str) {
        self.log(LogLevel::Warn, msg);
    }

    /// Logs an error message.
    pub fn log_error(&self, msg: &str) {
        self.log(LogLevel::Error, msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::Urgency;
    use rust_decimal_macros::dec;

    #[test]
    fn test_system_clock() {
        let clock = SystemClock;
        let t1 = clock.now();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let t2 = clock.now();
        assert!(t2.as_millis() >= t1.as_millis());
    }

    #[test]
    fn test_backtest_clock() {
        let clock = BacktestClock::new(Timestamp::new_unchecked(1000));
        assert_eq!(clock.now().as_millis(), 1000);

        clock.advance(500);
        assert_eq!(clock.now().as_millis(), 1500);

        clock.set_time(Timestamp::new_unchecked(2000));
        assert_eq!(clock.now().as_millis(), 2000);
    }

    #[test]
    fn test_portfolio_snapshot() {
        let mut positions = HashMap::new();
        positions.insert(
            Symbol::new_unchecked("BTC-USDT"),
            Position {
                symbol: Symbol::new_unchecked("BTC-USDT"),
                quantity: dec!(1.5),
                avg_price: dec!(42000),
                unrealized_pnl: dec!(500),
            },
        );

        let portfolio = PortfolioSnapshot {
            positions,
            cash: dec!(10000),
            equity: dec!(73500),
            margin_used: dec!(21000),
        };

        assert!(portfolio.has_position(&Symbol::new_unchecked("BTC-USDT")));
        assert!(!portfolio.has_position(&Symbol::new_unchecked("ETH-USDT")));
        assert_eq!(
            portfolio.quantity(&Symbol::new_unchecked("BTC-USDT")),
            dec!(1.5)
        );
        assert_eq!(
            portfolio.quantity(&Symbol::new_unchecked("ETH-USDT")),
            dec!(0)
        );
    }

    #[tokio::test]
    async fn test_emit_signal() {
        let (mut ctx, mut rx) = StrategyContext::new_test("test_strategy");

        let signal = Signal::target_position(
            Symbol::new_unchecked("BTC-USDT"),
            dec!(1.0),
            Urgency::Medium,
        );

        let id = ctx.emit_signal(signal.clone()).unwrap();
        assert_eq!(id.as_u64(), 1);

        let (received_id, received_signal) = rx.recv().await.unwrap();
        assert_eq!(received_id, id);
        assert_eq!(received_signal, signal);
    }

    #[tokio::test]
    async fn test_signal_id_uniqueness() {
        let (mut ctx, _rx) = StrategyContext::new_test("test_strategy");

        let id1 = ctx
            .emit_signal(Signal::cancel_all(Symbol::new_unchecked("BTC-USDT")))
            .unwrap();
        let id2 = ctx
            .emit_signal(Signal::cancel_all(Symbol::new_unchecked("ETH-USDT")))
            .unwrap();
        let id3 = ctx
            .emit_signal(Signal::cancel_all(Symbol::new_unchecked("SOL-USDT")))
            .unwrap();

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[tokio::test]
    async fn test_emit_signal_channel_full() {
        let (signal_tx, _rx) = mpsc::channel(1);
        let mut ctx = StrategyContext::new(
            "test".to_string(),
            PortfolioSnapshot::default(),
            signal_tx,
            Arc::new(SystemClock),
            None,
        );

        // First signal should succeed
        ctx.emit_signal(Signal::cancel_all(Symbol::new_unchecked("BTC-USDT")))
            .unwrap();

        // Second signal should fail (channel full)
        let result = ctx.emit_signal(Signal::cancel_all(Symbol::new_unchecked("ETH-USDT")));
        assert!(matches!(result, Err(EmitError::ChannelFull)));
    }
}
