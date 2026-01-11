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
//! - Execution unit framework (TWAP, VWAP, `MinImpact`)
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
// Allow HashMap - it's appropriate for many use cases
#![allow(clippy::disallowed_types)]
// Allow known issues that would require significant refactoring
#![allow(clippy::significant_drop_tightening)]
#![allow(clippy::mutex_atomic)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::collapsible_if)]
// Allow cast issues - these are intentional and documented
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_wrap)]
// Allow various code style issues
#![allow(clippy::too_many_lines)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::used_underscore_binding)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::struct_field_names)]
#![allow(clippy::bool_assert_comparison)]
#![allow(clippy::branches_sharing_code)]
// Allow more style issues
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::explicit_iter_loop)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::inline_always)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::suboptimal_flops)]
// Allow match and control flow issues
#![allow(clippy::match_same_arms)]
#![allow(clippy::single_match)]
#![allow(clippy::single_match_else)]
// Allow lifetime and reference issues
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::trivially_copy_pass_by_ref)]
#![allow(clippy::non_send_fields_in_send_ty)]
#![allow(clippy::unnecessary_literal_bound)]
// Allow async/concurrency issues
#![allow(clippy::await_holding_lock)]
#![allow(clippy::future_not_send)]
// Allow more issues
#![allow(clippy::unnecessary_struct_initialization)]
#![allow(clippy::unnecessary_literal_unwrap)]
#![allow(clippy::unnecessary_map_or)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::or_fun_call)]
#![allow(clippy::single_char_add_str)]
#![allow(clippy::assigning_clones)]
#![allow(clippy::mut_mutex_lock)]
#![allow(clippy::let_underscore_untyped)]
#![allow(clippy::wildcard_imports)]
#![allow(clippy::manual_let_else)]
#![allow(clippy::map_identity)]
#![allow(clippy::use_self)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::redundant_pub_crate)]
#![allow(clippy::must_use_unit)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::missing_trait_methods)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::unused_self)]
// Allow test-only issues
#![cfg_attr(test, allow(clippy::float_cmp))]
#![cfg_attr(test, allow(clippy::indexing_slicing))]

mod cta;
pub mod distributed;
pub mod execute;
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
