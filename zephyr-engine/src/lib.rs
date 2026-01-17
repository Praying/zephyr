//! # Zephyr Engine
//!
//! Trading engine and strategy execution for the Zephyr trading system.
//!
//! This crate provides:
//! - CTA strategy context and execution
//! - HFT strategy context and execution
//! - SEL strategy context and execution (scheduled/selection strategies)
//! - UFT strategy context and execution (ultra-high frequency strategies)
//! - Position calculation and management
//! - Signal aggregation and filtering
//! - Execution unit framework (TWAP, VWAP, MinImpact)
//! - Portfolio management and signal merging
//! - Event notification system
//! - Distributed execution with leader election and state sync
//!
//! # Architecture
//!
//! The engine follows the M+1+N architecture:
//! - M strategies generate signals
//! - 1 signal aggregator merges positions
//! - N execution channels handle order routing
//!
//! # Strategy Types
//!
//! - [`CtaStrategyContextImpl`] - For CTA (Commodity Trading Advisor) strategies
//! - [`HftStrategyContextImpl`] - For HFT (High-Frequency Trading) strategies
//! - [`SelStrategyContextImpl`] - For SEL (Selection/Scheduled) strategies
//! - [`UftStrategyContextImpl`] - For UFT (Ultra-High Frequency Trading) strategies
//!
//! # Execution Units
//!
//! The execution framework provides pluggable algorithms for order execution:
//! - [`execute::TwapExecutor`] - Time-Weighted Average Price
//! - [`execute::VwapExecutor`] - Volume-Weighted Average Price
//! - [`execute::MinImpactExecutor`] - Minimum market impact
//!
//! # Portfolio Management
//!
//! The portfolio module provides strategy portfolio management:
//! - [`portfolio::Portfolio`] - Strategy portfolio with allocations
//! - [`portfolio::PortfolioManager`] - Portfolio CRUD and signal merging
//! - [`portfolio::SignalMerger`] - Multi-strategy signal merging with self-trading prevention
//!
//! # Event Notification
//!
//! The notifier module provides event broadcasting:
//! - [`notifier::EventNotifier`] - Event notification trait
//! - [`notifier::TradingEvent`] - Trading event types
//! - [`notifier::EventFilter`] - Event filtering
//!
//! # Distributed Execution
//!
//! The distributed module provides cluster coordination:
//! - [`distributed::DistributedExecutor`] - Distributed task execution
//! - [`distributed::LeaderElection`] - Raft-based leader election
//! - [`distributed::LockManager`] - Distributed locking
//! - [`distributed::StateSync`] - State synchronization across nodes

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

mod cta;
pub mod distributed;
pub mod execute;
mod execution_manager;
pub mod filter;
mod hft;
pub mod notifier;
pub mod portfolio;
mod position;
mod rebalancer;
mod scheduler;
mod sel;
mod signal;
pub mod strategies;
mod uft;

pub use cta::{CtaStrategyContextImpl, CtaStrategyRunner};
pub use distributed::{
    ClusterHealth, ClusterStatus, DistributedExecutor, DistributedExecutorConfig,
    DistributedExecutorImpl, ElectionConfig, ElectionEvent, ElectionState, ExecutionTask,
    ExecutorError, HeartbeatMessage, LeaderElection, LoadBalancer, LoadBalancerConfig, LockError,
    LockGuard, LockManager, NodeAddress, NodeLoad, NodeRole, NodeStatus, OrderState, PositionState,
    RaftState, StateSync, StateSyncConfig, StateSyncError, SyncMessage, TaskAssignment,
    TaskDistributor, TaskId, TaskPriority, TaskSignal, TaskStatus, VoteRequest, VoteResponse,
};
pub use execution_manager::ExecutionManager;
pub use filter::{
    FilterChainResult, FilterContext, FilterEvent, FilterEventType, FilterId, FilterManager,
    FilterManagerConfig, FilterModification, FilterPriority, FilterRejection, FilterResult,
    FilterStats, FilterStatsReport, FrequencyFilter, FrequencyFilterConfig, PositionLimitConfig,
    PositionLimitFilter, PriceDeviationConfig, PriceDeviationFilter, SignalFilter, TimeFilter,
    TimeFilterConfig, TimeWindow,
};
pub use hft::HftStrategyContextImpl;
pub use notifier::{
    AlertSeverity, BalanceChange, CallbackSubscriber, ChannelSubscriber, DeadLetterQueue,
    DeliveryStatus, EventDeduplicator, EventFilter, EventId, EventNotifier, EventNotifierConfig,
    EventNotifierImpl, EventSubscriber, EventType, NotificationChannel, NotificationChannelConfig,
    NotifierError, RetryConfig, RetryPolicy, RiskAlert, StrategySignalEvent, SubscriptionId,
    SystemEvent, SystemEventType, TradingEvent, WebSocketChannel, WebhookChannel,
};
pub use portfolio::{
    ExecutionSignal, MergeConfig, Portfolio, PortfolioConfig, PortfolioError, PortfolioId,
    PortfolioManager, PortfolioManagerImpl, PortfolioMetrics, PortfolioRiskLimits, PortfolioStatus,
    SignalMerger, StrategyAllocation, StrategyAttribution, StrategyId,
};
pub use position::{PositionCalculator, PositionChange, TargetPosition};
pub use rebalancer::{
    PortfolioRebalancer, PortfolioState, RebalanceResult, RebalancerConfig, RebalancerError,
    TargetWeight, equal_weight_targets, normalize_weights,
};
pub use scheduler::{
    CronExpression, CronField, ScheduleMatcher, SchedulerConfig, SchedulerError, SchedulerEvent,
    SelScheduler,
};
pub use sel::{SelStrategyContextImpl, SelStrategyRunner};
pub use signal::{SignalAggregator, StrategySignal};
pub use strategies::{DualThrustConfig, DualThrustStrategy};
pub use uft::{
    // Lock-free data structures
    AtomicPrice,
    AtomicQuantity,
    // CPU affinity
    CpuAffinity,
    CpuAffinityError,
    // Core UFT types
    HardwareTimestamp,
    // Timestamp and latency measurement
    LatencyMeasurement,
    LatencyStats,
    LatencyTracker,
    LockFreeOrderPool,
    OrderIdPool,
    SpscQueue,
    TickRingBuffer,
    TscCalibration,
    UftStrategyContextImpl,
    UftStrategyRunner,
};
