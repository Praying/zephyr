# Zephyr Core

Core types, traits, and interfaces for the Zephyr cryptocurrency trading system.

## Overview

This crate provides foundational building blocks for Zephyr trading platform:

- **`NewType` Wrappers**: Type-safe wrappers for financial primitives (Price, Quantity, Amount, Symbol, etc.)
- **Data Structures**: Market data structures (`TickData`, `KlineData`, `OrderBook`)
- **Error Framework**: Hierarchical error types with context preservation
- **Trait Definitions**: Core interfaces for parsers, traders, and strategies

## Usage

```rust,ignore
use zephyr_core::prelude::*;
use rust_decimal_macros::dec;

// Create type-safe financial values
let price = Price::new(dec!(50000.00)).unwrap();
let quantity = Quantity::new(dec!(0.5)).unwrap();
let amount = Amount::from_price_qty(price, quantity);
```

## Design Principles

1. **Type Safety**: Compile-time prevention of type mixing errors
2. **Decimal Precision**: Use of `rust_decimal` for exact financial calculations
3. **Zero-Cost Abstractions**: `NewType` pattern with `#[repr(transparent)]`
4. **Minimal Dependencies**: Only essential dependencies for core functionality
