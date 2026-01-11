//! Data management commands.
//!
//! Provides commands for importing, exporting, and verifying market data.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use tracing::{info, warn};

use zephyr_data::{DataStorage, FileStorageBackend, StorageConfig};

/// Arguments for data import command
#[derive(Parser)]
pub struct ImportArgs {
    /// Input file path
    #[arg(short, long)]
    pub input: String,

    /// Data type (tick, kline)
    #[arg(short = 't', long, default_value = "tick")]
    pub data_type: String,

    /// Symbol (e.g., BTC-USDT)
    #[arg(short, long)]
    pub symbol: String,

    /// Data directory
    #[arg(short, long, default_value = "./data")]
    pub data_dir: String,

    /// Enable compression
    #[arg(long, default_value = "true")]
    pub compress: bool,
}

/// Arguments for data export command
#[derive(Parser)]
pub struct ExportArgs {
    /// Output file path
    #[arg(short, long)]
    pub output: String,

    /// Data type (tick, kline)
    #[arg(short = 't', long, default_value = "tick")]
    pub data_type: String,

    /// Symbol (e.g., BTC-USDT)
    #[arg(short, long)]
    pub symbol: String,

    /// Data directory
    #[arg(short, long, default_value = "./data")]
    pub data_dir: String,

    /// Start timestamp (Unix ms)
    #[arg(long)]
    pub start: Option<i64>,

    /// End timestamp (Unix ms)
    #[arg(long)]
    pub end: Option<i64>,

    /// Output format (json, csv)
    #[arg(short, long, default_value = "json")]
    pub format: String,
}

/// Arguments for data verify command
#[derive(Parser)]
pub struct VerifyArgs {
    /// Data directory
    #[arg(short, long, default_value = "./data")]
    pub data_dir: String,

    /// Fix corrupted data if possible
    #[arg(long)]
    pub fix: bool,
}

/// Arguments for data list command
#[derive(Parser)]
pub struct ListArgs {
    /// Data directory
    #[arg(short, long, default_value = "./data")]
    pub data_dir: String,

    /// Filter by symbol
    #[arg(short, long)]
    pub symbol: Option<String>,

    /// Filter by data type (tick, kline)
    #[arg(short = 't', long)]
    pub data_type: Option<String>,
}

/// Import market data from file.
///
/// # Errors
///
/// Returns error if import fails.
pub async fn import(args: ImportArgs) -> Result<()> {
    info!("Importing {} data from {}", args.data_type, args.input);

    let config = StorageConfig {
        base_dir: PathBuf::from(&args.data_dir),
        compression_enabled: args.compress,
        checksum_enabled: true,
        max_file_size: 1024 * 1024 * 1024,
    };

    let storage =
        FileStorageBackend::new(config).context("Failed to initialize storage backend")?;

    let input_path = PathBuf::from(&args.input);
    if !input_path.exists() {
        anyhow::bail!("Input file not found: {}", args.input);
    }

    let content = std::fs::read_to_string(&input_path).context("Failed to read input file")?;

    match args.data_type.as_str() {
        "tick" => {
            let ticks: Vec<zephyr_core::data::TickData> =
                serde_json::from_str(&content).context("Failed to parse tick data")?;

            info!("Importing {} tick records for {}", ticks.len(), args.symbol);

            storage
                .store_ticks(&ticks)
                .await
                .context("Failed to store tick data")?;

            info!("Successfully imported {} tick records", ticks.len());
        }
        "kline" => {
            let klines: Vec<zephyr_core::data::KlineData> =
                serde_json::from_str(&content).context("Failed to parse kline data")?;

            info!(
                "Importing {} kline records for {}",
                klines.len(),
                args.symbol
            );

            storage
                .store_klines(&klines)
                .await
                .context("Failed to store kline data")?;

            info!("Successfully imported {} kline records", klines.len());
        }
        _ => {
            anyhow::bail!(
                "Unknown data type: {}. Use 'tick' or 'kline'",
                args.data_type
            );
        }
    }

    Ok(())
}

/// Export market data to file.
///
/// # Errors
///
/// Returns error if export fails.
pub async fn export(args: ExportArgs) -> Result<()> {
    info!(
        "Exporting {} data for {} to {}",
        args.data_type, args.symbol, args.output
    );

    let config = StorageConfig {
        base_dir: PathBuf::from(&args.data_dir),
        compression_enabled: true,
        checksum_enabled: true,
        max_file_size: 1024 * 1024 * 1024,
    };

    let storage =
        FileStorageBackend::new(config).context("Failed to initialize storage backend")?;

    let start_time = args.start.unwrap_or(0);
    let end_time = args.end.unwrap_or(i64::MAX);

    let output_content = match args.data_type.as_str() {
        "tick" => {
            let ticks = storage
                .get_ticks(&args.symbol, start_time, end_time)
                .await
                .context("Failed to retrieve tick data")?;

            info!("Exporting {} tick records", ticks.len());

            match args.format.as_str() {
                "json" => serde_json::to_string_pretty(&ticks)?,
                "csv" => ticks_to_csv(&ticks),
                _ => anyhow::bail!("Unknown format: {}. Use 'json' or 'csv'", args.format),
            }
        }
        "kline" => {
            let klines = storage
                .get_klines(&args.symbol, start_time, end_time)
                .await
                .context("Failed to retrieve kline data")?;

            info!("Exporting {} kline records", klines.len());

            match args.format.as_str() {
                "json" => serde_json::to_string_pretty(&klines)?,
                "csv" => klines_to_csv(&klines),
                _ => anyhow::bail!("Unknown format: {}. Use 'json' or 'csv'", args.format),
            }
        }
        _ => {
            anyhow::bail!(
                "Unknown data type: {}. Use 'tick' or 'kline'",
                args.data_type
            );
        }
    };

    std::fs::write(&args.output, output_content).context("Failed to write output file")?;

    info!("Successfully exported data to {}", args.output);

    Ok(())
}

/// Verify data integrity.
///
/// # Errors
///
/// Returns error if verification fails.
pub async fn verify(args: VerifyArgs) -> Result<()> {
    info!("Verifying data integrity in {}", args.data_dir);

    let config = StorageConfig {
        base_dir: PathBuf::from(&args.data_dir),
        compression_enabled: true,
        checksum_enabled: true,
        max_file_size: 1024 * 1024 * 1024,
    };

    let storage =
        FileStorageBackend::new(config).context("Failed to initialize storage backend")?;

    match storage.verify_integrity().await {
        Ok(true) => {
            info!("✓ All data files passed integrity verification");
            println!("Data integrity verification: PASSED");
        }
        Ok(false) => {
            warn!("✗ Some data files failed integrity verification");
            println!("Data integrity verification: FAILED");
            if args.fix {
                warn!("Auto-fix is not yet implemented");
            }
        }
        Err(e) => {
            warn!("✗ Verification error: {}", e);
            println!("Data integrity verification: ERROR - {e}");
            return Err(e.into());
        }
    }

    Ok(())
}

/// List available data.
///
/// # Errors
///
/// Returns error if listing fails.
pub fn list(args: &ListArgs) -> Result<()> {
    info!("Listing data in {}", args.data_dir);

    let data_dir = PathBuf::from(&args.data_dir);
    if !data_dir.exists() {
        println!("Data directory does not exist: {}", args.data_dir);
        return Ok(());
    }

    println!("Available Data Files");
    println!("====================");
    println!();

    // List tick data
    let ticks_dir = data_dir.join("ticks");
    if ticks_dir.exists() {
        println!("Tick Data:");
        list_data_files(&ticks_dir, args.symbol.as_deref(), "tick")?;
        println!();
    }

    // List kline data
    let klines_dir = data_dir.join("klines");
    if klines_dir.exists() {
        println!("K-line Data:");
        list_data_files(&klines_dir, args.symbol.as_deref(), "kline")?;
    }

    Ok(())
}

fn list_data_files(dir: &PathBuf, symbol_filter: Option<&str>, data_type: &str) -> Result<()> {
    let entries = std::fs::read_dir(dir).context("Failed to read directory")?;

    let mut files: Vec<_> = entries
        .filter_map(std::result::Result::ok)
        .filter(|e| e.path().is_file())
        .collect();

    files.sort_by_key(std::fs::DirEntry::file_name);

    for entry in files {
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        // Apply symbol filter if provided
        if let Some(symbol) = symbol_filter
            && !file_name_str.contains(symbol)
        {
            continue;
        }

        let metadata = entry.metadata()?;
        let size = metadata.len();
        let size_str = format_size(size);

        println!("  {file_name_str} ({data_type}) - {size_str}");
    }

    Ok(())
}

#[allow(clippy::cast_precision_loss)]
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[allow(clippy::format_push_string)]
fn ticks_to_csv(ticks: &[zephyr_core::data::TickData]) -> String {
    let mut csv =
        String::from("timestamp,symbol,price,volume,best_bid,best_bid_qty,best_ask,best_ask_qty\n");

    for tick in ticks {
        let best_bid = tick
            .bid_prices
            .first()
            .map_or(String::new(), |p| p.as_decimal().to_string());
        let best_bid_qty = tick
            .bid_quantities
            .first()
            .map_or(String::new(), |q| q.as_decimal().to_string());
        let best_ask = tick
            .ask_prices
            .first()
            .map_or(String::new(), |p| p.as_decimal().to_string());
        let best_ask_qty = tick
            .ask_quantities
            .first()
            .map_or(String::new(), |q| q.as_decimal().to_string());

        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            tick.timestamp.as_millis(),
            tick.symbol.as_str(),
            tick.price.as_decimal(),
            tick.volume.as_decimal(),
            best_bid,
            best_bid_qty,
            best_ask,
            best_ask_qty,
        ));
    }

    csv
}

#[allow(clippy::format_push_string)]
fn klines_to_csv(klines: &[zephyr_core::data::KlineData]) -> String {
    let mut csv = String::from("timestamp,symbol,period,open,high,low,close,volume,turnover\n");

    for kline in klines {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            kline.timestamp.as_millis(),
            kline.symbol.as_str(),
            kline.period.as_str(),
            kline.open.as_decimal(),
            kline.high.as_decimal(),
            kline.low.as_decimal(),
            kline.close.as_decimal(),
            kline.volume.as_decimal(),
            kline.turnover.as_decimal(),
        ));
    }

    csv
}
