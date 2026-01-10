//! Strategy implementations.
//!
//! This module provides example strategy implementations for the Zephyr trading system.
//!
//! # Available Strategies
//!
//! - [`DualThrustStrategy`] - Classic intraday breakout strategy

mod dual_thrust;

pub use dual_thrust::{DualThrustConfig, DualThrustStrategy};
