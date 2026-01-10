# Zephyr

Enterprise-grade cryptocurrency quantitative trading system built with Rust.

## Overview

Zephyr is a high-performance quantitative trading platform designed specifically for cryptocurrency markets. It inherits the excellent M+1+N execution architecture from WonderTrader while leveraging Rust's safety and performance guarantees.

## Features

- **High Performance**: Microsecond-level latency with zero-cost abstractions
- **Type Safety**: Compile-time error prevention through NewType patterns
- **Extensible**: Plugin architecture for strategies, execution algorithms, and exchange adapters
- **Reliable**: Comprehensive error handling, circuit breakers, and state recovery
- **Multi-Exchange**: Support for Binance, OKX, Bitget, Hyperliquid, and more
- **Python Support**: Write strategies in Python with full type hints

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Zephyr Trading Platform                   │
├─────────────────────────────────────────────────────────────┤
│  Strategies (CTA/HFT/UFT) → Signal Aggregator → Execution   │
├─────────────────────────────────────────────────────────────┤
│  Exchange Adapters: Binance | OKX | Bitget | Hyperliquid    │
├─────────────────────────────────────────────────────────────┤
│  Data Manager | Risk Control | Metrics | Logger             │
└─────────────────────────────────────────────────────────────┘
```

## Workspace Structure

| Crate | Description |
|-------|-------------|
| `zephyr-core` | Core types, traits, and interfaces |
| `zephyr-data` | Data storage and management |
| `zephyr-engine` | Trading engine and strategy execution |
| `zephyr-gateway` | Exchange adapters and network communication |
| `zephyr-risk` | Risk control and management |
| `zephyr-backtest` | Backtesting engine |
| `zephyr-python` | Python bindings (PyO3) |
| `zephyr-api` | REST and WebSocket API |
| `zephyr-cli` | Command-line interface |
| `zephyr-server` | Main server entry point |

## Requirements

- Rust 1.92+ (edition 2024)
- Python 3.10+ (for Python bindings)

## Quick Start

```bash
# Build all crates
cargo build

# Run tests
cargo test

# Run with release optimizations
cargo build --release

# Run clippy lints
cargo lint

# Format code
cargo fmt
```

## Configuration

Zephyr supports YAML and TOML configuration files. See `config/` directory for examples.

## License

MIT License - see [LICENSE](LICENSE) for details.
