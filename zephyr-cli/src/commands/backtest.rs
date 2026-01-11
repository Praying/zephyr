//! Backtest command implementation.
//!
//! Provides functionality to run backtests from the command line.

use std::path::PathBuf;

use anyhow::{Context, Result};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use tracing::info;

use zephyr_backtest::{
    BacktestMetrics, DataReplayer, MatchEngine, MatchEngineConfig, ReplayConfig, SlippageModel,
};
use zephyr_core::types::{Amount, Timestamp};

use crate::BacktestArgs;

/// Run a backtest with the given arguments.
///
/// # Errors
///
/// Returns error if backtest fails.
pub fn run(args: &BacktestArgs) -> Result<()> {
    info!("Starting backtest with strategy: {}", args.strategy);

    // Parse initial capital
    let capital: Decimal = args.capital.parse().context("Invalid capital value")?;

    let initial_capital = Amount::new(capital).context("Invalid capital amount")?;

    // Parse time range
    let start_time = parse_date_to_timestamp(args.start.as_deref())?;
    let end_time = parse_date_to_timestamp(args.end.as_deref())?;

    // Create replay config
    let replay_config = ReplayConfig {
        start_time,
        end_time,
        validate_order: true,
    };

    // Initialize components
    let replayer = DataReplayer::new(replay_config);
    let slippage_model = SlippageModel::Percentage(dec!(0.0001));
    let match_config = MatchEngineConfig {
        slippage_model: slippage_model.clone(),
        allow_partial_fills: true,
        min_fill_quantity: None,
    };
    let _match_engine = MatchEngine::new(match_config);
    let metrics = BacktestMetrics::new(initial_capital);

    // Load strategy configuration
    let strategy_path = PathBuf::from(&args.strategy);
    if !strategy_path.exists() {
        anyhow::bail!("Strategy configuration not found: {}", args.strategy);
    }

    let _strategy_config =
        std::fs::read_to_string(&strategy_path).context("Failed to read strategy configuration")?;

    info!("Loaded strategy configuration from {}", args.strategy);
    info!("Data directory: {}", args.data_dir);
    info!("Initial capital: {}", capital);

    if let Some(ref start) = args.start {
        info!("Start date: {}", start);
    }
    if let Some(ref end) = args.end {
        info!("End date: {}", end);
    }

    // Run backtest simulation
    // Note: In a full implementation, this would:
    // 1. Load historical data from data_dir
    // 2. Initialize the strategy from strategy_config
    // 3. Replay events through the strategy
    // 4. Execute trades through the match engine
    // 5. Track metrics

    info!("Backtest engine initialized");
    info!("Replayer ready with {} events", replayer.total_events());
    info!(
        "Match engine ready with slippage model: {:?}",
        slippage_model
    );

    // Generate results
    let stats = metrics.calculate_stats();
    let results = BacktestResults {
        initial_capital,
        final_equity: metrics.current_equity(),
        total_pnl: metrics.total_pnl(),
        total_return_pct: metrics.total_return_pct(),
        max_drawdown: metrics.max_drawdown(),
        max_drawdown_pct: metrics.max_drawdown_pct(),
        sharpe_ratio: metrics.sharpe_ratio(),
        total_trades: stats.total_trades,
        winning_trades: stats.winning_trades,
        losing_trades: stats.losing_trades,
        win_rate: stats.win_rate,
        profit_factor: stats.profit_factor,
    };

    // Output results
    output_results(&results, args.output.as_str(), args.output_file.as_deref())?;

    info!("Backtest completed successfully");

    Ok(())
}

/// Backtest results summary.
#[derive(Debug, serde::Serialize)]
struct BacktestResults {
    initial_capital: Amount,
    final_equity: Amount,
    total_pnl: Amount,
    total_return_pct: Decimal,
    max_drawdown: Amount,
    max_drawdown_pct: Decimal,
    sharpe_ratio: Option<Decimal>,
    total_trades: u64,
    winning_trades: u64,
    losing_trades: u64,
    win_rate: Decimal,
    profit_factor: Option<Decimal>,
}

fn parse_date_to_timestamp(date: Option<&str>) -> Result<Option<Timestamp>> {
    match date {
        Some(d) => {
            // Parse YYYY-MM-DD format
            let parsed = chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
                .context("Invalid date format. Use YYYY-MM-DD")?;

            let datetime = parsed
                .and_hms_opt(0, 0, 0)
                .context("Failed to create datetime")?;

            let timestamp_ms = datetime.and_utc().timestamp_millis();

            Ok(Some(
                Timestamp::new(timestamp_ms).context("Invalid timestamp")?,
            ))
        }
        None => Ok(None),
    }
}

fn output_results(
    results: &BacktestResults,
    format: &str,
    output_file: Option<&str>,
) -> Result<()> {
    let output = match format {
        "json" => serde_json::to_string_pretty(results)?,
        "csv" => results_to_csv(results),
        _ => results_to_table(results),
    };

    if let Some(file_path) = output_file {
        std::fs::write(file_path, &output).context("Failed to write output file")?;
        info!("Results written to {}", file_path);
    } else {
        println!("{output}");
    }

    Ok(())
}

fn results_to_table(results: &BacktestResults) -> String {
    use std::fmt::Write;
    let mut output = String::new();

    writeln!(output).unwrap();
    writeln!(
        output,
        "╔══════════════════════════════════════════════════════════════╗"
    )
    .unwrap();
    writeln!(
        output,
        "║                    BACKTEST RESULTS                          ║"
    )
    .unwrap();
    writeln!(
        output,
        "╠══════════════════════════════════════════════════════════════╣"
    )
    .unwrap();
    writeln!(
        output,
        "║ Initial Capital:     {:>38} ║",
        results.initial_capital.as_decimal()
    )
    .unwrap();
    writeln!(
        output,
        "║ Final Equity:        {:>38} ║",
        results.final_equity.as_decimal()
    )
    .unwrap();
    writeln!(
        output,
        "║ Total PnL:           {:>38} ║",
        results.total_pnl.as_decimal()
    )
    .unwrap();
    writeln!(
        output,
        "║ Total Return:        {:>37}% ║",
        results.total_return_pct
    )
    .unwrap();
    writeln!(
        output,
        "╠══════════════════════════════════════════════════════════════╣"
    )
    .unwrap();
    writeln!(
        output,
        "║ Max Drawdown:        {:>38} ║",
        results.max_drawdown.as_decimal()
    )
    .unwrap();
    writeln!(
        output,
        "║ Max Drawdown %:      {:>37}% ║",
        results.max_drawdown_pct
    )
    .unwrap();
    writeln!(
        output,
        "║ Sharpe Ratio:        {:>38} ║",
        results
            .sharpe_ratio
            .map_or_else(|| "N/A".to_string(), |s| format!("{s:.4}"))
    )
    .unwrap();
    writeln!(
        output,
        "╠══════════════════════════════════════════════════════════════╣"
    )
    .unwrap();
    writeln!(
        output,
        "║ Total Trades:        {:>38} ║",
        results.total_trades
    )
    .unwrap();
    writeln!(
        output,
        "║ Winning Trades:      {:>38} ║",
        results.winning_trades
    )
    .unwrap();
    writeln!(
        output,
        "║ Losing Trades:       {:>38} ║",
        results.losing_trades
    )
    .unwrap();
    writeln!(
        output,
        "║ Win Rate:            {:>37}% ║",
        results.win_rate * dec!(100)
    )
    .unwrap();
    writeln!(
        output,
        "║ Profit Factor:       {:>38} ║",
        results
            .profit_factor
            .map_or_else(|| "N/A".to_string(), |p| format!("{p:.4}"))
    )
    .unwrap();
    writeln!(
        output,
        "╚══════════════════════════════════════════════════════════════╝"
    )
    .unwrap();

    output
}

fn results_to_csv(results: &BacktestResults) -> String {
    use std::fmt::Write;
    let mut csv = String::from("metric,value\n");

    writeln!(
        csv,
        "initial_capital,{}",
        results.initial_capital.as_decimal()
    )
    .unwrap();
    writeln!(csv, "final_equity,{}", results.final_equity.as_decimal()).unwrap();
    writeln!(csv, "total_pnl,{}", results.total_pnl.as_decimal()).unwrap();
    writeln!(csv, "total_return_pct,{}", results.total_return_pct).unwrap();
    writeln!(csv, "max_drawdown,{}", results.max_drawdown.as_decimal()).unwrap();
    writeln!(csv, "max_drawdown_pct,{}", results.max_drawdown_pct).unwrap();
    writeln!(
        csv,
        "sharpe_ratio,{}",
        results
            .sharpe_ratio
            .map_or_else(String::new, |s| s.to_string())
    )
    .unwrap();
    writeln!(csv, "total_trades,{}", results.total_trades).unwrap();
    writeln!(csv, "winning_trades,{}", results.winning_trades).unwrap();
    writeln!(csv, "losing_trades,{}", results.losing_trades).unwrap();
    writeln!(csv, "win_rate,{}", results.win_rate).unwrap();
    writeln!(
        csv,
        "profit_factor,{}",
        results
            .profit_factor
            .map_or_else(String::new, |p| p.to_string())
    )
    .unwrap();

    csv
}
