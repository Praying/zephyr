//! Signal types for expressing trading intents.
//!
//! Signals represent what a strategy wants to achieve, not how to achieve it.
//! The execution layer is responsible for translating signals into actual orders.

use crate::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use zephyr_core::data::{OrderSide, OrderType, TimeInForce};
use zephyr_core::types::{OrderId, Symbol};

/// Unique identifier for tracking signals through the system.
///
/// Each signal emitted by a strategy receives a unique ID that can be used
/// to correlate order status updates back to the original intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct SignalId(pub u64);

impl SignalId {
    /// Creates a new `SignalId` from a raw value.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the raw ID value.
    #[must_use]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for SignalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sig_{}", self.0)
    }
}

impl From<u64> for SignalId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

impl From<SignalId> for u64 {
    fn from(id: SignalId) -> Self {
        id.0
    }
}

/// Urgency level for trading signals.
///
/// Indicates how quickly the execution layer should attempt to fill the order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Urgency {
    /// Use aggressive taking, accept slippage for speed.
    /// Suitable for time-sensitive signals or stop-loss scenarios.
    High,

    /// Balance between speed and cost.
    /// Default urgency for most trading signals.
    #[default]
    Medium,

    /// Use passive making, minimize market impact.
    /// Suitable for large orders or when cost is more important than speed.
    Low,
}

impl fmt::Display for Urgency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::High => write!(f, "high"),
            Self::Medium => write!(f, "medium"),
            Self::Low => write!(f, "low"),
        }
    }
}

/// Trading signal expressing a strategy's intent.
///
/// Signals are intent-based: they express what the strategy wants to achieve,
/// not the specific orders to place. The execution layer translates signals
/// into actual orders based on current market conditions and risk parameters.
///
/// # Signal Types
///
/// - `SetTargetPosition`: High-level intent to hold a specific quantity
/// - `PlaceOrder`: Low-level order for strategies needing precise control
/// - `CancelOrder`: Cancel a specific order
/// - `CancelAll`: Cancel all orders for a symbol
///
/// # Example
///
/// ```
/// use zephyr_strategy::signal::{Signal, Urgency};
/// use zephyr_core::types::Symbol;
/// use rust_decimal_macros::dec;
///
/// // High-level: "I want to hold 1 BTC"
/// let signal = Signal::SetTargetPosition {
///     symbol: Symbol::new_unchecked("BTC-USDT"),
///     target_qty: dec!(1.0),
///     urgency: Urgency::Medium,
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Signal {
    /// High-level intent: "I want to hold X quantity".
    ///
    /// The execution layer handles the how (buy/sell, algo selection).
    /// Positive quantity = long, negative = short, zero = flat.
    SetTargetPosition {
        /// Trading pair symbol
        symbol: Symbol,
        /// Target position quantity (positive = long, negative = short)
        target_qty: Decimal,
        /// How urgently to reach the target
        urgency: Urgency,
    },

    /// Low-level order for HFT strategies needing precise control.
    ///
    /// Use this when you need exact control over order parameters.
    PlaceOrder {
        /// Trading pair symbol
        symbol: Symbol,
        /// Order side (buy/sell)
        side: OrderSide,
        /// Order price (for limit orders)
        price: Decimal,
        /// Order quantity
        qty: Decimal,
        /// Order type (limit, market, etc.)
        order_type: OrderType,
        /// Time in force
        time_in_force: TimeInForce,
    },

    /// Cancel a specific order by ID.
    CancelOrder {
        /// Order ID to cancel
        order_id: OrderId,
    },

    /// Cancel all orders for a symbol.
    CancelAll {
        /// Symbol to cancel all orders for
        symbol: Symbol,
    },
}

impl Signal {
    /// Creates a new `SetTargetPosition` signal.
    #[must_use]
    pub fn target_position(symbol: Symbol, target_qty: Decimal, urgency: Urgency) -> Self {
        Self::SetTargetPosition {
            symbol,
            target_qty,
            urgency,
        }
    }

    /// Creates a new `SetTargetPosition` signal with medium urgency.
    #[must_use]
    pub fn target_position_medium(symbol: Symbol, target_qty: Decimal) -> Self {
        Self::target_position(symbol, target_qty, Urgency::Medium)
    }

    /// Creates a new limit order signal.
    #[must_use]
    pub fn limit_order(
        symbol: Symbol,
        side: OrderSide,
        price: Decimal,
        qty: Decimal,
        time_in_force: TimeInForce,
    ) -> Self {
        Self::PlaceOrder {
            symbol,
            side,
            price,
            qty,
            order_type: OrderType::Limit,
            time_in_force,
        }
    }

    /// Creates a new market order signal.
    #[must_use]
    pub fn market_order(symbol: Symbol, side: OrderSide, qty: Decimal) -> Self {
        Self::PlaceOrder {
            symbol,
            side,
            price: Decimal::ZERO,
            qty,
            order_type: OrderType::Market,
            time_in_force: TimeInForce::Ioc,
        }
    }

    /// Creates a cancel order signal.
    #[must_use]
    pub fn cancel(order_id: OrderId) -> Self {
        Self::CancelOrder { order_id }
    }

    /// Creates a cancel all orders signal.
    #[must_use]
    pub fn cancel_all(symbol: Symbol) -> Self {
        Self::CancelAll { symbol }
    }

    /// Returns the symbol associated with this signal, if any.
    #[must_use]
    pub fn symbol(&self) -> Option<&Symbol> {
        match self {
            Self::SetTargetPosition { symbol, .. }
            | Self::PlaceOrder { symbol, .. }
            | Self::CancelAll { symbol } => Some(symbol),
            Self::CancelOrder { .. } => None,
        }
    }
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SetTargetPosition {
                symbol,
                target_qty,
                urgency,
            } => {
                write!(
                    f,
                    "SetTarget({symbol}, qty={target_qty}, urgency={urgency})"
                )
            }
            Self::PlaceOrder {
                symbol,
                side,
                price,
                qty,
                order_type,
                ..
            } => {
                write!(
                    f,
                    "PlaceOrder({symbol}, {side:?} {qty} @ {price}, {order_type:?})"
                )
            }
            Self::CancelOrder { order_id } => {
                write!(f, "CancelOrder({order_id})")
            }
            Self::CancelAll { symbol } => {
                write!(f, "CancelAll({symbol})")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_signal_id_creation() {
        let id = SignalId::new(42);
        assert_eq!(id.as_u64(), 42);
        assert_eq!(format!("{id}"), "sig_42");
    }

    #[test]
    fn test_signal_id_conversion() {
        let id: SignalId = 123u64.into();
        let raw: u64 = id.into();
        assert_eq!(raw, 123);
    }

    #[test]
    fn test_urgency_default() {
        assert_eq!(Urgency::default(), Urgency::Medium);
    }

    #[test]
    fn test_urgency_display() {
        assert_eq!(format!("{}", Urgency::High), "high");
        assert_eq!(format!("{}", Urgency::Medium), "medium");
        assert_eq!(format!("{}", Urgency::Low), "low");
    }

    #[test]
    fn test_signal_target_position() {
        let signal =
            Signal::target_position(Symbol::new_unchecked("BTC-USDT"), dec!(1.5), Urgency::High);

        match signal {
            Signal::SetTargetPosition {
                symbol,
                target_qty,
                urgency,
            } => {
                assert_eq!(symbol.as_str(), "BTC-USDT");
                assert_eq!(target_qty, dec!(1.5));
                assert_eq!(urgency, Urgency::High);
            }
            _ => panic!("Expected SetTargetPosition"),
        }
    }

    #[test]
    fn test_signal_market_order() {
        let signal = Signal::market_order(
            Symbol::new_unchecked("ETH-USDT"),
            OrderSide::Buy,
            dec!(10.0),
        );

        match signal {
            Signal::PlaceOrder {
                symbol,
                side,
                qty,
                order_type,
                ..
            } => {
                assert_eq!(symbol.as_str(), "ETH-USDT");
                assert_eq!(side, OrderSide::Buy);
                assert_eq!(qty, dec!(10.0));
                assert_eq!(order_type, OrderType::Market);
            }
            _ => panic!("Expected PlaceOrder"),
        }
    }

    #[test]
    fn test_signal_symbol() {
        let target = Signal::target_position_medium(Symbol::new_unchecked("BTC-USDT"), dec!(1.0));
        assert_eq!(target.symbol().unwrap().as_str(), "BTC-USDT");

        let cancel = Signal::cancel(OrderId::new_unchecked("order123"));
        assert!(cancel.symbol().is_none());
    }

    #[test]
    fn test_signal_display() {
        let signal = Signal::target_position_medium(Symbol::new_unchecked("BTC-USDT"), dec!(1.0));
        let display = format!("{signal}");
        assert!(display.contains("BTC-USDT"));
        assert!(display.contains("SetTarget"));
    }

    #[test]
    fn test_signal_serde_roundtrip() {
        let signal =
            Signal::target_position(Symbol::new_unchecked("BTC-USDT"), dec!(1.5), Urgency::High);

        let json = serde_json::to_string(&signal).unwrap();
        let parsed: Signal = serde_json::from_str(&json).unwrap();
        assert_eq!(signal, parsed);
    }
}
