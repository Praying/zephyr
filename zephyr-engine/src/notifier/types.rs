//! Event types and structures for the event notification system.
//!
//! This module defines the core event types that can be broadcast
//! through the event notification system.

#![allow(clippy::disallowed_types)]
#![allow(clippy::collapsible_if)]

use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use zephyr_core::data::{Order, Position};
use zephyr_core::traits::Trade;
use zephyr_core::types::{Amount, Price, Quantity, Symbol, Timestamp};

use crate::portfolio::{PortfolioId, StrategyId};

/// Unique identifier for an event.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId(String);

impl EventId {
    /// Creates a new `EventId`.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generates a new unique `EventId`.
    #[must_use]
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Returns the ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for EventId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for EventId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Unique identifier for a subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SubscriptionId(u64);

impl SubscriptionId {
    /// Creates a new `SubscriptionId`.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the ID as a u64.
    #[must_use]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for SubscriptionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for SubscriptionId {
    fn from(id: u64) -> Self {
        Self(id)
    }
}

/// Event type enumeration for filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// Order submitted event.
    OrderSubmitted,
    /// Order filled event.
    OrderFilled,
    /// Order canceled event.
    OrderCanceled,
    /// Order rejected event.
    OrderRejected,
    /// Position changed event.
    PositionChanged,
    /// Balance changed event.
    BalanceChanged,
    /// Risk alert event.
    RiskAlert,
    /// Strategy signal event.
    StrategySignal,
    /// System event.
    SystemEvent,
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OrderSubmitted => write!(f, "order_submitted"),
            Self::OrderFilled => write!(f, "order_filled"),
            Self::OrderCanceled => write!(f, "order_canceled"),
            Self::OrderRejected => write!(f, "order_rejected"),
            Self::PositionChanged => write!(f, "position_changed"),
            Self::BalanceChanged => write!(f, "balance_changed"),
            Self::RiskAlert => write!(f, "risk_alert"),
            Self::StrategySignal => write!(f, "strategy_signal"),
            Self::SystemEvent => write!(f, "system_event"),
        }
    }
}

/// Risk alert information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAlert {
    /// Alert identifier.
    pub id: String,
    /// Alert severity level.
    pub severity: AlertSeverity,
    /// Alert message.
    pub message: String,
    /// Related symbol, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<Symbol>,
    /// Related portfolio, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portfolio_id: Option<PortfolioId>,
    /// Alert timestamp.
    pub timestamp: Timestamp,
    /// Additional context data.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub context: std::collections::HashMap<String, String>,
}

impl RiskAlert {
    /// Creates a new risk alert.
    #[must_use]
    pub fn new(severity: AlertSeverity, message: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            severity,
            message: message.into(),
            symbol: None,
            portfolio_id: None,
            timestamp: Timestamp::now(),
            context: std::collections::HashMap::new(),
        }
    }

    /// Sets the related symbol.
    #[must_use]
    pub fn with_symbol(mut self, symbol: Symbol) -> Self {
        self.symbol = Some(symbol);
        self
    }

    /// Sets the related portfolio.
    #[must_use]
    pub fn with_portfolio(mut self, portfolio_id: PortfolioId) -> Self {
        self.portfolio_id = Some(portfolio_id);
        self
    }

    /// Adds context data.
    #[must_use]
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }
}

/// Alert severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverity {
    /// Informational alert.
    Info,
    /// Warning alert.
    Warning,
    /// Critical alert requiring immediate attention.
    Critical,
    /// Emergency alert - trading may be halted.
    Emergency,
}

impl fmt::Display for AlertSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Critical => write!(f, "critical"),
            Self::Emergency => write!(f, "emergency"),
        }
    }
}

/// Strategy signal information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategySignalEvent {
    /// Strategy identifier.
    pub strategy_id: StrategyId,
    /// Portfolio identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portfolio_id: Option<PortfolioId>,
    /// Symbol for the signal.
    pub symbol: Symbol,
    /// Target quantity (positive for long, negative for short).
    pub target_quantity: Quantity,
    /// Signal price.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<Price>,
    /// Signal tag/reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Signal timestamp.
    pub timestamp: Timestamp,
}

impl StrategySignalEvent {
    /// Creates a new strategy signal event.
    #[must_use]
    pub fn new(strategy_id: StrategyId, symbol: Symbol, target_quantity: Quantity) -> Self {
        Self {
            strategy_id,
            portfolio_id: None,
            symbol,
            target_quantity,
            price: None,
            tag: None,
            timestamp: Timestamp::now(),
        }
    }

    /// Sets the portfolio ID.
    #[must_use]
    pub fn with_portfolio(mut self, portfolio_id: PortfolioId) -> Self {
        self.portfolio_id = Some(portfolio_id);
        self
    }

    /// Sets the signal price.
    #[must_use]
    pub fn with_price(mut self, price: Price) -> Self {
        self.price = Some(price);
        self
    }

    /// Sets the signal tag.
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }
}

/// System event information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEvent {
    /// Event type.
    pub event_type: SystemEventType,
    /// Event message.
    pub message: String,
    /// Event timestamp.
    pub timestamp: Timestamp,
    /// Additional details.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub details: std::collections::HashMap<String, String>,
}

impl SystemEvent {
    /// Creates a new system event.
    #[must_use]
    pub fn new(event_type: SystemEventType, message: impl Into<String>) -> Self {
        Self {
            event_type,
            message: message.into(),
            timestamp: Timestamp::now(),
            details: std::collections::HashMap::new(),
        }
    }

    /// Adds a detail.
    #[must_use]
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }
}

/// System event type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemEventType {
    /// System started.
    Started,
    /// System stopped.
    Stopped,
    /// Component connected.
    Connected,
    /// Component disconnected.
    Disconnected,
    /// Configuration changed.
    ConfigChanged,
    /// Health check status.
    HealthCheck,
    /// Maintenance mode entered.
    MaintenanceStarted,
    /// Maintenance mode exited.
    MaintenanceEnded,
}

impl fmt::Display for SystemEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Started => write!(f, "started"),
            Self::Stopped => write!(f, "stopped"),
            Self::Connected => write!(f, "connected"),
            Self::Disconnected => write!(f, "disconnected"),
            Self::ConfigChanged => write!(f, "config_changed"),
            Self::HealthCheck => write!(f, "health_check"),
            Self::MaintenanceStarted => write!(f, "maintenance_started"),
            Self::MaintenanceEnded => write!(f, "maintenance_ended"),
        }
    }
}

/// Balance change information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceChange {
    /// Currency.
    pub currency: String,
    /// Previous balance.
    pub previous: Amount,
    /// New balance.
    pub current: Amount,
    /// Change amount.
    pub change: Amount,
    /// Reason for change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Change timestamp.
    pub timestamp: Timestamp,
}

impl BalanceChange {
    /// Creates a new balance change.
    #[must_use]
    pub fn new(currency: impl Into<String>, previous: Amount, current: Amount) -> Self {
        let change = Amount::new_unchecked(current.as_decimal() - previous.as_decimal());
        Self {
            currency: currency.into(),
            previous,
            current,
            change,
            reason: None,
            timestamp: Timestamp::now(),
        }
    }

    /// Sets the reason for the change.
    #[must_use]
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

/// Trading event enumeration.
///
/// Represents all types of events that can be broadcast through
/// the event notification system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TradingEvent {
    /// Order submitted to exchange.
    OrderSubmitted {
        /// Event ID.
        id: EventId,
        /// Event timestamp.
        timestamp: Timestamp,
        /// Order details.
        order: Order,
    },
    /// Order filled (partially or fully).
    OrderFilled {
        /// Event ID.
        id: EventId,
        /// Event timestamp.
        timestamp: Timestamp,
        /// Order details.
        order: Order,
        /// Trade details.
        trade: Trade,
    },
    /// Order canceled.
    OrderCanceled {
        /// Event ID.
        id: EventId,
        /// Event timestamp.
        timestamp: Timestamp,
        /// Order details.
        order: Order,
    },
    /// Order rejected.
    OrderRejected {
        /// Event ID.
        id: EventId,
        /// Event timestamp.
        timestamp: Timestamp,
        /// Order details.
        order: Order,
        /// Rejection reason.
        reason: String,
    },
    /// Position changed.
    PositionChanged {
        /// Event ID.
        id: EventId,
        /// Event timestamp.
        timestamp: Timestamp,
        /// Position details.
        position: Position,
    },
    /// Balance changed.
    BalanceChanged {
        /// Event ID.
        id: EventId,
        /// Event timestamp.
        timestamp: Timestamp,
        /// Balance change details.
        change: BalanceChange,
    },
    /// Risk alert triggered.
    RiskAlert {
        /// Event ID.
        id: EventId,
        /// Event timestamp.
        timestamp: Timestamp,
        /// Alert details.
        alert: RiskAlert,
    },
    /// Strategy signal generated.
    StrategySignal {
        /// Event ID.
        id: EventId,
        /// Event timestamp.
        timestamp: Timestamp,
        /// Signal details.
        signal: StrategySignalEvent,
    },
    /// System event.
    SystemEvent {
        /// Event ID.
        id: EventId,
        /// Event timestamp.
        timestamp: Timestamp,
        /// System event details.
        event: SystemEvent,
    },
}

impl TradingEvent {
    /// Returns the event ID.
    #[must_use]
    pub fn id(&self) -> &EventId {
        match self {
            Self::OrderSubmitted { id, .. }
            | Self::OrderFilled { id, .. }
            | Self::OrderCanceled { id, .. }
            | Self::OrderRejected { id, .. }
            | Self::PositionChanged { id, .. }
            | Self::BalanceChanged { id, .. }
            | Self::RiskAlert { id, .. }
            | Self::StrategySignal { id, .. }
            | Self::SystemEvent { id, .. } => id,
        }
    }

    /// Returns the event timestamp.
    #[must_use]
    pub fn timestamp(&self) -> Timestamp {
        match self {
            Self::OrderSubmitted { timestamp, .. }
            | Self::OrderFilled { timestamp, .. }
            | Self::OrderCanceled { timestamp, .. }
            | Self::OrderRejected { timestamp, .. }
            | Self::PositionChanged { timestamp, .. }
            | Self::BalanceChanged { timestamp, .. }
            | Self::RiskAlert { timestamp, .. }
            | Self::StrategySignal { timestamp, .. }
            | Self::SystemEvent { timestamp, .. } => *timestamp,
        }
    }

    /// Returns the event type.
    #[must_use]
    pub fn event_type(&self) -> EventType {
        match self {
            Self::OrderSubmitted { .. } => EventType::OrderSubmitted,
            Self::OrderFilled { .. } => EventType::OrderFilled,
            Self::OrderCanceled { .. } => EventType::OrderCanceled,
            Self::OrderRejected { .. } => EventType::OrderRejected,
            Self::PositionChanged { .. } => EventType::PositionChanged,
            Self::BalanceChanged { .. } => EventType::BalanceChanged,
            Self::RiskAlert { .. } => EventType::RiskAlert,
            Self::StrategySignal { .. } => EventType::StrategySignal,
            Self::SystemEvent { .. } => EventType::SystemEvent,
        }
    }

    /// Returns the symbol associated with this event, if any.
    #[must_use]
    pub fn symbol(&self) -> Option<&Symbol> {
        match self {
            Self::OrderSubmitted { order, .. }
            | Self::OrderFilled { order, .. }
            | Self::OrderCanceled { order, .. }
            | Self::OrderRejected { order, .. } => Some(&order.symbol),
            Self::PositionChanged { position, .. } => Some(&position.symbol),
            Self::StrategySignal { signal, .. } => Some(&signal.symbol),
            Self::RiskAlert { alert, .. } => alert.symbol.as_ref(),
            Self::BalanceChanged { .. } | Self::SystemEvent { .. } => None,
        }
    }

    /// Returns the strategy ID associated with this event, if any.
    #[must_use]
    pub fn strategy_id(&self) -> Option<&StrategyId> {
        match self {
            Self::StrategySignal { signal, .. } => Some(&signal.strategy_id),
            _ => None,
        }
    }

    /// Creates an order submitted event.
    #[must_use]
    pub fn order_submitted(order: Order) -> Self {
        Self::OrderSubmitted {
            id: EventId::generate(),
            timestamp: Timestamp::now(),
            order,
        }
    }

    /// Creates an order filled event.
    #[must_use]
    pub fn order_filled(order: Order, trade: Trade) -> Self {
        Self::OrderFilled {
            id: EventId::generate(),
            timestamp: Timestamp::now(),
            order,
            trade,
        }
    }

    /// Creates an order canceled event.
    #[must_use]
    pub fn order_canceled(order: Order) -> Self {
        Self::OrderCanceled {
            id: EventId::generate(),
            timestamp: Timestamp::now(),
            order,
        }
    }

    /// Creates an order rejected event.
    #[must_use]
    pub fn order_rejected(order: Order, reason: impl Into<String>) -> Self {
        Self::OrderRejected {
            id: EventId::generate(),
            timestamp: Timestamp::now(),
            order,
            reason: reason.into(),
        }
    }

    /// Creates a position changed event.
    #[must_use]
    pub fn position_changed(position: Position) -> Self {
        Self::PositionChanged {
            id: EventId::generate(),
            timestamp: Timestamp::now(),
            position,
        }
    }

    /// Creates a balance changed event.
    #[must_use]
    pub fn balance_changed(change: BalanceChange) -> Self {
        Self::BalanceChanged {
            id: EventId::generate(),
            timestamp: Timestamp::now(),
            change,
        }
    }

    /// Creates a risk alert event.
    #[must_use]
    pub fn risk_alert(alert: RiskAlert) -> Self {
        Self::RiskAlert {
            id: EventId::generate(),
            timestamp: Timestamp::now(),
            alert,
        }
    }

    /// Creates a strategy signal event.
    #[must_use]
    pub fn strategy_signal(signal: StrategySignalEvent) -> Self {
        Self::StrategySignal {
            id: EventId::generate(),
            timestamp: Timestamp::now(),
            signal,
        }
    }

    /// Creates a system event.
    #[must_use]
    pub fn system_event(event: SystemEvent) -> Self {
        Self::SystemEvent {
            id: EventId::generate(),
            timestamp: Timestamp::now(),
            event,
        }
    }
}

/// Event filter for subscribers.
///
/// Allows subscribers to receive only specific event types,
/// symbols, or strategies.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventFilter {
    /// Event types to include. If empty, all types are included.
    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub event_types: HashSet<EventType>,
    /// Symbols to include. If empty, all symbols are included.
    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub symbols: HashSet<Symbol>,
    /// Strategies to include. If empty, all strategies are included.
    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub strategies: HashSet<StrategyId>,
    /// Portfolios to include. If empty, all portfolios are included.
    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub portfolios: HashSet<PortfolioId>,
}

impl EventFilter {
    /// Creates a new empty filter (accepts all events).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a filter for specific event types.
    #[must_use]
    pub fn for_event_types(types: impl IntoIterator<Item = EventType>) -> Self {
        Self {
            event_types: types.into_iter().collect(),
            ..Default::default()
        }
    }

    /// Creates a filter for specific symbols.
    #[must_use]
    pub fn for_symbols(symbols: impl IntoIterator<Item = Symbol>) -> Self {
        Self {
            symbols: symbols.into_iter().collect(),
            ..Default::default()
        }
    }

    /// Creates a filter for specific strategies.
    #[must_use]
    pub fn for_strategies(strategies: impl IntoIterator<Item = StrategyId>) -> Self {
        Self {
            strategies: strategies.into_iter().collect(),
            ..Default::default()
        }
    }

    /// Adds an event type to the filter.
    #[must_use]
    pub fn with_event_type(mut self, event_type: EventType) -> Self {
        self.event_types.insert(event_type);
        self
    }

    /// Adds a symbol to the filter.
    #[must_use]
    pub fn with_symbol(mut self, symbol: Symbol) -> Self {
        self.symbols.insert(symbol);
        self
    }

    /// Adds a strategy to the filter.
    #[must_use]
    pub fn with_strategy(mut self, strategy_id: StrategyId) -> Self {
        self.strategies.insert(strategy_id);
        self
    }

    /// Adds a portfolio to the filter.
    #[must_use]
    pub fn with_portfolio(mut self, portfolio_id: PortfolioId) -> Self {
        self.portfolios.insert(portfolio_id);
        self
    }

    /// Checks if an event matches this filter.
    #[must_use]
    pub fn matches(&self, event: &TradingEvent) -> bool {
        // Check event type
        if !self.event_types.is_empty() && !self.event_types.contains(&event.event_type()) {
            return false;
        }

        // Check symbol
        if !self.symbols.is_empty() {
            if let Some(symbol) = event.symbol() {
                if !self.symbols.contains(symbol) {
                    return false;
                }
            }
        }

        // Check strategy
        if !self.strategies.is_empty() {
            if let Some(strategy_id) = event.strategy_id() {
                if !self.strategies.contains(strategy_id) {
                    return false;
                }
            }
        }

        true
    }

    /// Returns true if this filter accepts all events.
    #[must_use]
    pub fn accepts_all(&self) -> bool {
        self.event_types.is_empty()
            && self.symbols.is_empty()
            && self.strategies.is_empty()
            && self.portfolios.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use zephyr_core::data::{
        MarginType, OrderSide, OrderStatus, OrderType, PositionSide, TimeInForce,
    };
    use zephyr_core::types::{Leverage, OrderId, Price, Quantity};

    fn create_test_order() -> Order {
        Order {
            order_id: OrderId::new("test-order-1").unwrap(),
            client_order_id: None,
            symbol: Symbol::new("BTC-USDT").unwrap(),
            side: OrderSide::Buy,
            order_type: OrderType::Limit,
            status: OrderStatus::New,
            price: Price::new(dec!(50000)).unwrap(),
            stop_price: None,
            quantity: Quantity::new(dec!(1.0)).unwrap(),
            filled_quantity: Quantity::ZERO,
            avg_price: Price::ZERO,
            time_in_force: TimeInForce::Gtc,
            reduce_only: false,
            post_only: false,
            create_time: Timestamp::now(),
            update_time: Timestamp::now(),
        }
    }

    fn create_test_position() -> Position {
        Position {
            symbol: Symbol::new("BTC-USDT").unwrap(),
            side: PositionSide::Long,
            quantity: Quantity::new(dec!(1.0)).unwrap(),
            entry_price: Some(Price::new(dec!(50000)).unwrap()),
            mark_price: Some(Price::new(dec!(51000)).unwrap()),
            liquidation_price: None,
            unrealized_pnl: None,
            realized_pnl: None,
            leverage: Some(rust_decimal::Decimal::from(10)),
            margin_type: Some(MarginType::Cross),
            initial_margin: None,
            maintenance_margin: None,
            update_time: Some(Timestamp::now()),
        }
    }

    #[test]
    fn test_event_id_generate() {
        let id1 = EventId::generate();
        let id2 = EventId::generate();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_event_id_from_string() {
        let id: EventId = "test-event".into();
        assert_eq!(id.as_str(), "test-event");
    }

    #[test]
    fn test_subscription_id() {
        let id = SubscriptionId::new(42);
        assert_eq!(id.as_u64(), 42);
        assert_eq!(format!("{id}"), "42");
    }

    #[test]
    fn test_event_type_display() {
        assert_eq!(format!("{}", EventType::OrderSubmitted), "order_submitted");
        assert_eq!(format!("{}", EventType::RiskAlert), "risk_alert");
    }

    #[test]
    fn test_risk_alert_builder() {
        let alert = RiskAlert::new(AlertSeverity::Warning, "Position limit exceeded")
            .with_symbol(Symbol::new("BTC-USDT").unwrap())
            .with_context("limit", "100")
            .with_context("current", "150");

        assert_eq!(alert.severity, AlertSeverity::Warning);
        assert!(alert.symbol.is_some());
        assert_eq!(alert.context.len(), 2);
    }

    #[test]
    fn test_strategy_signal_event() {
        let signal = StrategySignalEvent::new(
            StrategyId::new("momentum"),
            Symbol::new("ETH-USDT").unwrap(),
            Quantity::new(dec!(10.0)).unwrap(),
        )
        .with_price(Price::new(dec!(3000)).unwrap())
        .with_tag("breakout");

        assert_eq!(signal.strategy_id.as_str(), "momentum");
        assert!(signal.price.is_some());
        assert_eq!(signal.tag, Some("breakout".to_string()));
    }

    #[test]
    fn test_system_event() {
        let event = SystemEvent::new(SystemEventType::Started, "System started successfully")
            .with_detail("version", "1.0.0");

        assert_eq!(event.event_type, SystemEventType::Started);
        assert_eq!(event.details.get("version"), Some(&"1.0.0".to_string()));
    }

    #[test]
    fn test_balance_change() {
        let change = BalanceChange::new(
            "USDT",
            Amount::new_unchecked(dec!(10000)),
            Amount::new_unchecked(dec!(10500)),
        )
        .with_reason("Trade profit");

        assert_eq!(change.currency, "USDT");
        assert_eq!(change.change.as_decimal(), dec!(500));
        assert_eq!(change.reason, Some("Trade profit".to_string()));
    }

    #[test]
    fn test_trading_event_order_submitted() {
        let order = create_test_order();
        let event = TradingEvent::order_submitted(order.clone());

        assert_eq!(event.event_type(), EventType::OrderSubmitted);
        assert_eq!(event.symbol(), Some(&order.symbol));
    }

    #[test]
    fn test_trading_event_position_changed() {
        let position = create_test_position();
        let event = TradingEvent::position_changed(position.clone());

        assert_eq!(event.event_type(), EventType::PositionChanged);
        assert_eq!(event.symbol(), Some(&position.symbol));
    }

    #[test]
    fn test_trading_event_risk_alert() {
        let alert = RiskAlert::new(AlertSeverity::Critical, "Margin call");
        let event = TradingEvent::risk_alert(alert);

        assert_eq!(event.event_type(), EventType::RiskAlert);
    }

    #[test]
    fn test_event_filter_empty() {
        let filter = EventFilter::new();
        assert!(filter.accepts_all());

        let order = create_test_order();
        let event = TradingEvent::order_submitted(order);
        assert!(filter.matches(&event));
    }

    #[test]
    fn test_event_filter_by_type() {
        let filter =
            EventFilter::for_event_types([EventType::OrderSubmitted, EventType::OrderFilled]);

        let order = create_test_order();
        let submitted = TradingEvent::order_submitted(order.clone());
        let canceled = TradingEvent::order_canceled(order);

        assert!(filter.matches(&submitted));
        assert!(!filter.matches(&canceled));
    }

    #[test]
    fn test_event_filter_by_symbol() {
        let filter = EventFilter::for_symbols([Symbol::new("BTC-USDT").unwrap()]);

        let btc_order = create_test_order();
        let btc_event = TradingEvent::order_submitted(btc_order);

        assert!(filter.matches(&btc_event));
    }

    #[test]
    fn test_event_filter_combined() {
        let filter = EventFilter::new()
            .with_event_type(EventType::OrderSubmitted)
            .with_symbol(Symbol::new("BTC-USDT").unwrap());

        let order = create_test_order();
        let event = TradingEvent::order_submitted(order);

        assert!(filter.matches(&event));
    }

    #[test]
    fn test_trading_event_serde_roundtrip() {
        let order = create_test_order();
        let event = TradingEvent::order_submitted(order);

        let json = serde_json::to_string(&event).unwrap();
        let parsed: TradingEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(event.id(), parsed.id());
        assert_eq!(event.event_type(), parsed.event_type());
    }

    #[test]
    fn test_event_filter_serde_roundtrip() {
        let filter = EventFilter::new()
            .with_event_type(EventType::OrderSubmitted)
            .with_symbol(Symbol::new("BTC-USDT").unwrap());

        let json = serde_json::to_string(&filter).unwrap();
        let parsed: EventFilter = serde_json::from_str(&json).unwrap();

        assert_eq!(filter.event_types, parsed.event_types);
        assert_eq!(filter.symbols, parsed.symbols);
    }
}
