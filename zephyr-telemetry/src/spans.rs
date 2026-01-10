//! Span definitions for distributed tracing.
//!
//! Provides pre-defined spans for common operations in the trading system:
//! - Request tracing
//! - Strategy execution
//! - Exchange communication
//! - Order lifecycle

use tracing::{Span, info_span};

/// Create a span for HTTP/API request tracing.
///
/// # Example
///
/// ```
/// use zephyr_telemetry::spans::request_span;
///
/// let span = request_span("req-123", "GET", "/api/positions");
/// let _guard = span.enter();
/// // ... handle request
/// ```
#[must_use]
pub fn request_span(request_id: &str, method: &str, path: &str) -> Span {
    info_span!(
        "request",
        request_id = %request_id,
        method = %method,
        path = %path,
        otel.kind = "server"
    )
}

/// Create a span for strategy execution.
///
/// # Example
///
/// ```
/// use zephyr_telemetry::spans::strategy_span;
///
/// let span = strategy_span("my_strategy", "cta", "BTCUSDT");
/// let _guard = span.enter();
/// // ... execute strategy
/// ```
#[must_use]
pub fn strategy_span(strategy_name: &str, strategy_type: &str, symbol: &str) -> Span {
    info_span!(
        "strategy",
        strategy.name = %strategy_name,
        strategy.type = %strategy_type,
        symbol = %symbol
    )
}

/// Create a span for strategy tick callback.
#[must_use]
pub fn strategy_on_tick_span(strategy_name: &str, symbol: &str, price: &str) -> Span {
    info_span!(
        "strategy.on_tick",
        strategy.name = %strategy_name,
        symbol = %symbol,
        price = %price
    )
}

/// Create a span for strategy bar callback.
#[must_use]
pub fn strategy_on_bar_span(strategy_name: &str, symbol: &str, period: &str) -> Span {
    info_span!(
        "strategy.on_bar",
        strategy.name = %strategy_name,
        symbol = %symbol,
        period = %period
    )
}

/// Create a span for exchange communication.
///
/// # Example
///
/// ```
/// use zephyr_telemetry::spans::exchange_span;
///
/// let span = exchange_span("binance", "order_insert");
/// let _guard = span.enter();
/// // ... communicate with exchange
/// ```
#[must_use]
pub fn exchange_span(exchange: &str, operation: &str) -> Span {
    info_span!(
        "exchange",
        exchange = %exchange,
        operation = %operation,
        otel.kind = "client"
    )
}

/// Create a span for WebSocket connection.
#[must_use]
pub fn websocket_span(exchange: &str, connection_type: &str) -> Span {
    info_span!(
        "websocket",
        exchange = %exchange,
        connection_type = %connection_type,
        otel.kind = "client"
    )
}

/// Create a span for REST API call.
#[must_use]
pub fn rest_api_span(exchange: &str, endpoint: &str, method: &str) -> Span {
    info_span!(
        "rest_api",
        exchange = %exchange,
        endpoint = %endpoint,
        method = %method,
        otel.kind = "client"
    )
}

/// Create a span for order lifecycle tracking.
///
/// # Example
///
/// ```
/// use zephyr_telemetry::spans::order_span;
///
/// let span = order_span("ord-123", "BTCUSDT", "buy", "limit");
/// let _guard = span.enter();
/// // ... process order
/// ```
#[must_use]
pub fn order_span(order_id: &str, symbol: &str, side: &str, order_type: &str) -> Span {
    info_span!(
        "order",
        order_id = %order_id,
        symbol = %symbol,
        side = %side,
        order_type = %order_type
    )
}

/// Create a span for order submission.
#[must_use]
pub fn order_submit_span(exchange: &str, symbol: &str, side: &str, quantity: &str) -> Span {
    info_span!(
        "order.submit",
        exchange = %exchange,
        symbol = %symbol,
        side = %side,
        quantity = %quantity
    )
}

/// Create a span for order cancellation.
#[must_use]
pub fn order_cancel_span(exchange: &str, order_id: &str) -> Span {
    info_span!(
        "order.cancel",
        exchange = %exchange,
        order_id = %order_id
    )
}

/// Create a span for trade execution.
#[must_use]
pub fn trade_span(
    trade_id: &str,
    order_id: &str,
    symbol: &str,
    price: &str,
    quantity: &str,
) -> Span {
    info_span!(
        "trade",
        trade_id = %trade_id,
        order_id = %order_id,
        symbol = %symbol,
        price = %price,
        quantity = %quantity
    )
}

/// Create a span for position update.
#[must_use]
pub fn position_span(symbol: &str, side: &str, quantity: &str) -> Span {
    info_span!(
        "position",
        symbol = %symbol,
        side = %side,
        quantity = %quantity
    )
}

/// Create a span for risk check.
#[must_use]
pub fn risk_check_span(check_type: &str, symbol: &str) -> Span {
    info_span!(
        "risk_check",
        check_type = %check_type,
        symbol = %symbol
    )
}

/// Create a span for data storage operations.
#[must_use]
pub fn storage_span(operation: &str, data_type: &str) -> Span {
    info_span!(
        "storage",
        operation = %operation,
        data_type = %data_type
    )
}

/// Create a span for backtest execution.
#[must_use]
pub fn backtest_span(strategy_name: &str, start_date: &str, end_date: &str) -> Span {
    info_span!(
        "backtest",
        strategy.name = %strategy_name,
        start_date = %start_date,
        end_date = %end_date
    )
}

/// Create a span for market data processing.
#[must_use]
pub fn market_data_span(exchange: &str, symbol: &str, data_type: &str) -> Span {
    info_span!(
        "market_data",
        exchange = %exchange,
        symbol = %symbol,
        data_type = %data_type
    )
}

/// Create a span for execution unit operations.
#[must_use]
pub fn execution_span(unit_type: &str, symbol: &str, target_qty: &str) -> Span {
    info_span!(
        "execution",
        unit_type = %unit_type,
        symbol = %symbol,
        target_qty = %target_qty
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    fn init_test_subscriber() {
        let _ = tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer().with_test_writer())
            .try_init();
    }

    #[test]
    fn test_request_span() {
        init_test_subscriber();
        let span = request_span("req-123", "GET", "/api/positions");
        // Span creation should succeed
        let _guard = span.enter();
    }

    #[test]
    fn test_strategy_span() {
        init_test_subscriber();
        let span = strategy_span("my_strategy", "cta", "BTCUSDT");
        let _guard = span.enter();
    }

    #[test]
    fn test_exchange_span() {
        init_test_subscriber();
        let span = exchange_span("binance", "order_insert");
        let _guard = span.enter();
    }

    #[test]
    fn test_order_span() {
        init_test_subscriber();
        let span = order_span("ord-123", "BTCUSDT", "buy", "limit");
        let _guard = span.enter();
    }
}
