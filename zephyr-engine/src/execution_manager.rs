//! Execution management for signal-to-order conversion.

use tracing::info;

use crate::signal::PositionChangeListener;
use zephyr_core::types::{Quantity, Symbol};

/// Execution manager that converts aggregated signals to orders.
pub struct ExecutionManager {
    // Placeholder for future execution logic
}

impl ExecutionManager {
    /// Creates a new execution manager.
    pub fn new() -> Self {
        Self {}
    }
}

impl PositionChangeListener for ExecutionManager {
    fn on_position_change(&self, symbol: &Symbol, old: Quantity, new: Quantity) {
        info!(
            "Position change for {}: {} -> {}",
            symbol,
            old.as_decimal(),
            new.as_decimal()
        );
        // TODO: Implement actual order execution logic
    }
}

impl Default for ExecutionManager {
    fn default() -> Self {
        Self::new()
    }
}
