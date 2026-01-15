//! Built-in strategy implementations.
//!
//! This module contains example strategies that demonstrate how to implement
//! the `Strategy` trait and register strategies with the loader.

mod dual_thrust;

pub use dual_thrust::{DualThrust, DualThrustBuilder, DualThrustParams};
