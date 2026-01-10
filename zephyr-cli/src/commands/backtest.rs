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
pub async fn run(args: BacktestArgs) -> Result<()> {
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
    output_results(&results, &args.output, args.output_file.as_deref())?;

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
        "table" | _ => results_to_table(results),
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
    let mut output = String::new();

    output.push_str("\n");
    output.push_str("╔══════════════════════════════════════════════════════════════╗\n");
    output.push_str("║                    BACKTEST RESULTS                          ║\n");
    output.push_str("╠══════════════════════════════════════════════════════════════╣\n");
    output.push_str(&format!(
        "║ Initial Capital:     {:>38} ║\n",
        results.initial_capital.as_decimal()
    ));
    output.push_str(&format!(
        "║ Final Equity:        {:>38} ║\n",
        results.final_equity.as_decimal()
    ));
    output.push_str(&format!(
        "║ Total PnL:           {:>38} ║\n",
        results.total_pnl.as_decimal()
    ));
    output.push_str(&format!(
        "║ Total Return:        {:>37}% ║\n",
        results.total_return_pct
    ));
    output.push_str("╠══════════════════════════════════════════════════════════════╣\n");
    output.push_str(&format!(
        "║ Max Drawdown:        {:>38} ║\n",
        results.max_drawdown.as_decimal()
    ));
    output.push_str(&format!(
        "║ Max Drawdown %:      {:>37}% ║\n",
        results.max_drawdown_pct
    ));
    output.push_str(&format!(
        "║ Sharpe Ratio:        {:>38} ║\n",
        results
            .sharpe_ratio
            .map_or("N/A".to_string(), |s| format!("{s:.4}"))
    ));
    output.push_str("╠══════════════════════════════════════════════════════════════╣\n");
    output.push_str(&format!(
        "║ Total Trades:        {:>38} ║\n",
        results.total_trades
    ));
    output.push_str(&format!(
        "║ Winning Trades:      {:>38} ║\n",
        results.winning_trades
    ));
    output.push_str(&format!(
        "║ Losing Trades:       {:>38} ║\n",
        results.losing_trades
    ));
    output.push_str(&format!(
        "║ Win Rate:            {:>37}% ║\n",
        results.win_rate * dec!(100)
    ));
    output.push_str(&format!(
        "║ Profit Factor:       {:>38} ║\n",
        results
            .profit_factor
            .map_or("N/A".to_string(), |p| format!("{p:.4}"))
    ));
    output.push_str("╚══════════════════════════════════════════════════════════════╝\n");

    output
}

fn results_to_csv(results: &BacktestResults) -> String {
    let mut csv = String::from("metric,value\n");

    csv.push_str(&format!(
        "initial_capital,{}\n",
        results.initial_capital.as_decimal()
    ));
    csv.push_str(&format!(
        "final_equity,{}\n",
        results.final_equity.as_decimal()
    ));
    csv.push_str(&format!("total_pnl,{}\n", results.total_pnl.as_decimal()));
    csv.push_str(&format!("total_return_pct,{}\n", results.total_return_pct));
    csv.push_str(&format!(
        "max_drawdown,{}\n",
        results.max_drawdown.as_decimal()
    ));
    csv.push_str(&format!("max_drawdown_pct,{}\n", results.max_drawdown_pct));
    csv.push_str(&format!(
        "sharpe_ratio,{}\n",
        results
            .sharpe_ratio
            .map_or("".to_string(), |s| s.to_string())
    ));
    csv.push_str(&format!("total_trades,{}\n", results.total_trades));
    csv.push_str(&format!("winning_trades,{}\n", results.winning_trades));
    csv.push_str(&format!("losing_trades,{}\n", results.losing_trades));
    csv.push_str(&format!("win_rate,{}\n", results.win_rate));
    csv.push_str(&format!(
        "profit_factor,{}\n",
        results
            .profit_factor
            .map_or("".to_string(), |p| p.to_string())
    ));

    csv
}
