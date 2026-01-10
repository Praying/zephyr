//! UFT (Ultra-High Frequency Trading) Strategy Engine.
//!
//! This module provides the implementation of the UFT strategy engine,
//! which manages lock-free order submission and zero-copy data access
//! for ultra-high frequency trading strategies.
//!
//! # Modules
//!
//! - [`context`] - UFT strategy context implementation
//! - [`cpu_affinity`] - CPU affinity configuration for thread binding
//! - [`lockfree`] - Lock-free data structures for low-latency operations
//! - [`timestamp`] - Hardware timestamp support with latency measurement
//!
//! # Design Principles
//!
//! - **Lock-free operations**: Uses atomic operations and lock-free queues
//! - **Zero-copy data access**: Data is stored in pre-allocated buffers
//! - **Hardware timestamps**: Supports nanosecond precision timing
//! - **Pre-allocated resources**: Order pools and buffers are pre-allocated
//! - **CPU affinity**: Bind threads to specific cores to reduce latency
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    UftStrategyContextImpl                    │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
//! │  │ Order Pool  │  │ Tick Buffer │  │ OrderBook   │         │
//! │  │ (Lock-free) │  │ (Ring)      │  │ Buffer      │         │
//! │  └─────────────┘  └─────────────┘  └─────────────┘         │
//! │                                                             │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
//! │  │ Positions   │  │ Pending     │  │ Hardware    │         │
//! │  │ (Atomic)    │  │ Orders      │  │ Timestamp   │         │
//! │  └─────────────┘  └─────────────┘  └─────────────┘         │
//! └─────────────────────────────────────────────────────────────┘
//! ```

mod context;
pub mod cpu_affinity;
pub mod lockfree;
pub mod timestamp;

// Re-export from context module
pub use context::HardwareTimestamp;
pub use context::OrderIdPool;
pub use context::TickRingBuffer;
pub use context::UftStrategyContextImpl;
pub use context::UftStrategyRunner;

pub use cpu_affinity::{CpuAffinity, CpuAffinityError};
pub use lockfree::{AtomicPrice, AtomicQuantity, LockFreeOrderPool, SpscQueue};
pub use timestamp::{LatencyMeasurement, LatencyStats, LatencyTracker, TscCalibration};
