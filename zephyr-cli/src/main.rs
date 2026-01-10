//! # Zephyr CLI
//!
//! Command-line interface for the Zephyr trading system.
//!
//! This CLI provides commands for:
//! - Data management (import, export, verify)
//! - Backtesting strategies
//! - System configuration

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;

use commands::{backtest, data};

/// Zephyr - Enterprise-grade cryptocurrency quantitative trading system
#[derive(Parser)]
#[command(name = "zephyr")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Configuration file path
    #[arg(short, long, global = true, default_value = "config.yaml")]
    config: String,

    /// Subcommand to execute
    #[command(subcommand)]
    command: Commands,
}

/// Available CLI commands
#[derive(Subcommand)]
pub enum Commands {
    /// Data management commands
    #[command(subcommand)]
    Data(DataCommands),

    /// Run backtest
    Backtest(BacktestArgs),

    /// Show system information
    Info,
}

/// Data management subcommands
#[derive(Subcommand)]
pub enum DataCommands {
    /// Import market data from file
    Import(data::ImportArgs),

    /// Export market data to file
    Export(data::ExportArgs),

    /// Verify data integrity
    Verify(data::VerifyArgs),

    /// List available data
    List(data::ListArgs),
}

/// Arguments for backtest command
#[derive(Parser)]
pub struct BacktestArgs {
    /// Strategy configuration file
    #[arg(short, long)]
    strategy: String,

    /// Data directory path
    #[arg(short, long, default_value = "./data")]
    data_dir: String,

    /// Start date (YYYY-MM-DD)
    #[arg(long)]
    start: Option<String>,

    /// End date (YYYY-MM-DD)
    #[arg(long)]
    end: Option<String>,

    /// Initial capital
    #[arg(long, default_value = "10000")]
    capital: String,

    /// Output format (json, table, csv)
    #[arg(short, long, default_value = "table")]
    output: String,

    /// Output file path (optional)
    #[arg(long)]
    output_file: Option<String>,
}

fn setup_logging(verbose: bool) {
    let level = if verbose { Level::DEBUG } else { Level::INFO };

    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_span_events(FmtSpan::CLOSE)
        .with_target(false)
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    setup_logging(cli.verbose);

    match cli.command {
        Commands::Data(cmd) => match cmd {
            DataCommands::Import(args) => data::import(args).await?,
            DataCommands::Export(args) => data::export(args).await?,
            DataCommands::Verify(args) => data::verify(args).await?,
            DataCommands::List(args) => data::list(args).await?,
        },
        Commands::Backtest(args) => backtest::run(args).await?,
        Commands::Info => print_info(),
    }

    Ok(())
}

fn print_info() {
    println!("Zephyr Trading System");
    println!("=====================");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    println!("Rust Edition: 2024");
    println!();
    println!("Supported Exchanges:");
    println!("  - Binance (Spot, Futures)");
    println!("  - OKX (Spot, Perpetual)");
    println!("  - Bitget (Spot, Futures)");
    println!("  - Hyperliquid (DEX)");
    println!();
    println!("Features:");
    println!("  - CTA Strategy Engine");
    println!("  - HFT Strategy Engine");
    println!("  - Backtesting");
    println!("  - Risk Management");
    println!("  - Python Strategy Support");
}
