//! Zephyr metrics recorder with pre-defined metrics.

use metrics::{counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram};

/// Pre-defined metrics for the Zephyr trading system.
///
/// All metrics follow the naming convention: `zephyr_<category>_<metric>_<unit>`
pub struct ZephyrMetrics;

impl ZephyrMetrics {
    /// Register all metric descriptions.
    #[allow(clippy::too_many_lines)]
    pub fn register() {
        // Order metrics
        describe_counter!(
            "zephyr_order_submitted_total",
            "Total number of orders submitted"
        );
        describe_counter!("zephyr_order_filled_total", "Total number of orders filled");
        describe_counter!(
            "zephyr_order_rejected_total",
            "Total number of orders rejected"
        );
        describe_counter!(
            "zephyr_order_canceled_total",
            "Total number of orders canceled"
        );

        // Latency metrics
        describe_histogram!(
            "zephyr_order_latency_seconds",
            "Order submission to acknowledgment latency"
        );
        describe_histogram!(
            "zephyr_tick_processing_latency_seconds",
            "Market data tick processing latency"
        );
        describe_histogram!(
            "zephyr_strategy_calculation_latency_seconds",
            "Strategy calculation latency"
        );

        // Market data metrics
        describe_counter!(
            "zephyr_tick_received_total",
            "Total number of ticks received"
        );
        describe_counter!(
            "zephyr_kline_received_total",
            "Total number of klines received"
        );

        // Position and PnL metrics
        describe_gauge!(
            "zephyr_position_value",
            "Current position value in quote currency"
        );
        describe_gauge!("zephyr_unrealized_pnl", "Current unrealized PnL");
        describe_counter!("zephyr_realized_pnl_total", "Total realized PnL");
        describe_gauge!("zephyr_active_orders", "Number of active orders");

        // Connection metrics
        describe_counter!(
            "zephyr_websocket_reconnection_total",
            "Total number of WebSocket reconnections"
        );
        describe_gauge!(
            "zephyr_websocket_connected",
            "WebSocket connection status (1=connected, 0=disconnected)"
        );

        // API metrics
        describe_counter!("zephyr_api_request_total", "Total number of API requests");
        describe_gauge!(
            "zephyr_api_rate_limit_remaining",
            "Remaining API rate limit"
        );
        describe_histogram!("zephyr_api_request_latency_seconds", "API request latency");

        // System metrics
        describe_gauge!("zephyr_memory_usage_bytes", "Process memory usage in bytes");
        describe_gauge!("zephyr_strategy_count", "Number of active strategies");
    }

    // ==================== Order Metrics ====================

    /// Record an order submission.
    pub fn order_submitted(exchange: &str, symbol: &str, side: &str) {
        counter!(
            "zephyr_order_submitted_total",
            "exchange" => exchange.to_string(),
            "symbol" => symbol.to_string(),
            "side" => side.to_string()
        )
        .increment(1);
    }

    /// Record an order fill.
    pub fn order_filled(exchange: &str, symbol: &str, side: &str) {
        counter!(
            "zephyr_order_filled_total",
            "exchange" => exchange.to_string(),
            "symbol" => symbol.to_string(),
            "side" => side.to_string()
        )
        .increment(1);
    }

    /// Record an order rejection.
    pub fn order_rejected(exchange: &str, symbol: &str, reason: &str) {
        counter!(
            "zephyr_order_rejected_total",
            "exchange" => exchange.to_string(),
            "symbol" => symbol.to_string(),
            "rejection_reason" => reason.to_string()
        )
        .increment(1);
    }

    /// Record an order cancellation.
    pub fn order_canceled(exchange: &str, symbol: &str) {
        counter!(
            "zephyr_order_canceled_total",
            "exchange" => exchange.to_string(),
            "symbol" => symbol.to_string()
        )
        .increment(1);
    }

    // ==================== Latency Metrics ====================

    /// Record order latency.
    pub fn order_latency(exchange: &str, latency_seconds: f64) {
        histogram!(
            "zephyr_order_latency_seconds",
            "exchange" => exchange.to_string()
        )
        .record(latency_seconds);
    }

    /// Record tick processing latency.
    pub fn tick_processing_latency(exchange: &str, symbol: &str, latency_seconds: f64) {
        histogram!(
            "zephyr_tick_processing_latency_seconds",
            "exchange" => exchange.to_string(),
            "symbol" => symbol.to_string()
        )
        .record(latency_seconds);
    }

    /// Record strategy calculation latency.
    pub fn strategy_latency(strategy_name: &str, latency_seconds: f64) {
        histogram!(
            "zephyr_strategy_calculation_latency_seconds",
            "strategy_name" => strategy_name.to_string()
        )
        .record(latency_seconds);
    }

    // ==================== Market Data Metrics ====================

    /// Record a tick received.
    pub fn tick_received(exchange: &str, symbol: &str) {
        counter!(
            "zephyr_tick_received_total",
            "exchange" => exchange.to_string(),
            "symbol" => symbol.to_string()
        )
        .increment(1);
    }

    /// Record a kline received.
    pub fn kline_received(exchange: &str, symbol: &str, period: &str) {
        counter!(
            "zephyr_kline_received_total",
            "exchange" => exchange.to_string(),
            "symbol" => symbol.to_string(),
            "period" => period.to_string()
        )
        .increment(1);
    }

    // ==================== Position and PnL Metrics ====================

    /// Update position value.
    pub fn position_value(exchange: &str, symbol: &str, side: &str, value: f64) {
        gauge!(
            "zephyr_position_value",
            "exchange" => exchange.to_string(),
            "symbol" => symbol.to_string(),
            "side" => side.to_string()
        )
        .set(value);
    }

    /// Update unrealized `PnL`.
    pub fn unrealized_pnl(exchange: &str, symbol: &str, pnl: f64) {
        gauge!(
            "zephyr_unrealized_pnl",
            "exchange" => exchange.to_string(),
            "symbol" => symbol.to_string()
        )
        .set(pnl);
    }

    /// Record realized `PnL`.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn realized_pnl(exchange: &str, symbol: &str, pnl: f64) {
        // Use absolute value for counter, track direction separately
        let abs_pnl = pnl.abs();
        let direction = if pnl >= 0.0 { "profit" } else { "loss" };
        counter!(
            "zephyr_realized_pnl_total",
            "exchange" => exchange.to_string(),
            "symbol" => symbol.to_string(),
            "direction" => direction.to_string()
        )
        .increment(abs_pnl as u64);
    }

    /// Update active orders count.
    pub fn active_orders(exchange: &str, symbol: &str, count: f64) {
        gauge!(
            "zephyr_active_orders",
            "exchange" => exchange.to_string(),
            "symbol" => symbol.to_string()
        )
        .set(count);
    }

    // ==================== Connection Metrics ====================

    /// Record a WebSocket reconnection.
    pub fn websocket_reconnection(exchange: &str, connection_type: &str) {
        counter!(
            "zephyr_websocket_reconnection_total",
            "exchange" => exchange.to_string(),
            "connection_type" => connection_type.to_string()
        )
        .increment(1);
    }

    /// Update WebSocket connection status.
    pub fn websocket_connected(exchange: &str, connection_type: &str, connected: bool) {
        gauge!(
            "zephyr_websocket_connected",
            "exchange" => exchange.to_string(),
            "connection_type" => connection_type.to_string()
        )
        .set(if connected { 1.0 } else { 0.0 });
    }

    // ==================== API Metrics ====================

    /// Record an API request.
    pub fn api_request(exchange: &str, endpoint: &str, status_code: u16) {
        counter!(
            "zephyr_api_request_total",
            "exchange" => exchange.to_string(),
            "endpoint" => endpoint.to_string(),
            "status_code" => status_code.to_string()
        )
        .increment(1);
    }

    /// Update API rate limit remaining.
    pub fn api_rate_limit_remaining(exchange: &str, remaining: f64) {
        gauge!(
            "zephyr_api_rate_limit_remaining",
            "exchange" => exchange.to_string()
        )
        .set(remaining);
    }

    /// Record API request latency.
    pub fn api_request_latency(exchange: &str, endpoint: &str, latency_seconds: f64) {
        histogram!(
            "zephyr_api_request_latency_seconds",
            "exchange" => exchange.to_string(),
            "endpoint" => endpoint.to_string()
        )
        .record(latency_seconds);
    }

    // ==================== System Metrics ====================

    /// Update memory usage.
    pub fn memory_usage(bytes: f64) {
        gauge!("zephyr_memory_usage_bytes").set(bytes);
    }

    /// Update strategy count.
    pub fn strategy_count(count: f64) {
        gauge!("zephyr_strategy_count").set(count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests just verify the functions don't panic.
    // Actual metric values would need a test recorder to verify.

    #[test]
    fn test_order_metrics() {
        // These should not panic even without a recorder
        ZephyrMetrics::order_submitted("binance", "BTCUSDT", "buy");
        ZephyrMetrics::order_filled("binance", "BTCUSDT", "buy");
        ZephyrMetrics::order_rejected("binance", "BTCUSDT", "insufficient_balance");
        ZephyrMetrics::order_canceled("binance", "BTCUSDT");
    }

    #[test]
    fn test_latency_metrics() {
        ZephyrMetrics::order_latency("binance", 0.001);
        ZephyrMetrics::tick_processing_latency("binance", "BTCUSDT", 0.0001);
        ZephyrMetrics::strategy_latency("my_strategy", 0.005);
    }

    #[test]
    fn test_position_metrics() {
        ZephyrMetrics::position_value("binance", "BTCUSDT", "long", 10000.0);
        ZephyrMetrics::unrealized_pnl("binance", "BTCUSDT", 500.0);
        ZephyrMetrics::realized_pnl("binance", "BTCUSDT", 100.0);
        ZephyrMetrics::active_orders("binance", "BTCUSDT", 5.0);
    }

    #[test]
    fn test_connection_metrics() {
        ZephyrMetrics::websocket_reconnection("binance", "market");
        ZephyrMetrics::websocket_connected("binance", "market", true);
    }

    #[test]
    fn test_api_metrics() {
        ZephyrMetrics::api_request("binance", "/api/v3/order", 200);
        ZephyrMetrics::api_rate_limit_remaining("binance", 1000.0);
        ZephyrMetrics::api_request_latency("binance", "/api/v3/order", 0.05);
    }

    #[test]
    fn test_system_metrics() {
        ZephyrMetrics::memory_usage(1024.0 * 1024.0 * 100.0);
        ZephyrMetrics::strategy_count(5.0);
    }
}
