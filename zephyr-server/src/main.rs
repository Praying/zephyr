//! # Zephyr Server
//!
//! Main entry point for the Zephyr cryptocurrency trading system.
//!
//! # Usage

#![allow(clippy::clone_on_copy)]
//!
//! ```bash
//! # Run with default configuration
//! zephyr-server
//!
//! # Run with custom configuration file
//! zephyr-server --config /path/to/config.yaml
//!
//! # Run with environment variable overrides
//! ZEPHYR_SERVER_PORT=9090 zephyr-server
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info};

use zephyr_server::{ServerConfig, ZephyrServer};

/// Zephyr Trading Server
#[derive(Parser, Debug)]
#[command(name = "zephyr-server")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.yaml")]
    config: PathBuf,

    /// Override server host
    #[arg(long, env = "ZEPHYR_SERVER_HOST")]
    host: Option<String>,

    /// Override server port
    #[arg(long, env = "ZEPHYR_SERVER_PORT")]
    port: Option<u16>,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Validate configuration and exit
    #[arg(long)]
    validate: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Load configuration
    let config = match load_config(&args) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to load configuration: {e}");
            std::process::exit(1);
        }
    };

    // Validate only mode
    if args.validate {
        println!("Configuration is valid");
        return;
    }

    // Create and run server
    match run_server(config).await {
        Ok(()) => {
            info!("Zephyr server stopped");
        }
        Err(e) => {
            error!("Server error: {}", e);
            std::process::exit(1);
        }
    }
}

/// Loads configuration from file and applies overrides.
fn load_config(args: &Args) -> Result<ServerConfig, Box<dyn std::error::Error>> {
    let mut config = if args.config.exists() {
        ZephyrServer::load_config(&args.config)?
    } else {
        // Use default configuration if file doesn't exist
        eprintln!(
            "Configuration file not found: {}, using defaults",
            args.config.display()
        );
        ServerConfig::default()
    };

    // Apply command-line overrides
    if let Some(host) = &args.host {
        config.zephyr.server.host.clone_from(host);
    }
    if let Some(port) = args.port {
        config.zephyr.server.port = port;
    }
    if args.debug {
        config.zephyr.logging.level = "debug".to_string();
    }

    Ok(config)
}

/// Creates and runs the server.
async fn run_server(config: ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    // Create server
    let mut server = ZephyrServer::new(config)?;

    // Initialize components
    server.initialize().await?;

    // Run server (blocks until shutdown)
    server.run().await?;

    Ok(())
}
