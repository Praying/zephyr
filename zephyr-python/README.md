# Zephyr Python

Python bindings for the Zephyr cryptocurrency trading system.

## Features

- **Core Types**: Type-safe wrappers for financial primitives (Price, Quantity, Symbol, etc.)
- **Market Data**: TickData and KlineData structures with NumPy integration
- **Strategy Contexts**: CTA and HFT strategy interfaces for Python strategies
- **Type Hints**: Full type stub support for IDE autocompletion

## Installation

```bash
pip install zephyr-py
```

Or build from source:

```bash
cd zephyr/zephyr-python
maturin develop
```

## Quick Start

```python
import zephyr_py as zephyr

# Create core types
price = zephyr.Price(42000.50)
qty = zephyr.Quantity(1.5)
symbol = zephyr.Symbol("BTC-USDT")

print(f"Price: {price}")
print(f"Quantity: {qty}")
print(f"Symbol: {symbol}")
print(f"Base asset: {symbol.base_asset}")
print(f"Quote asset: {symbol.quote_asset}")
```

## CTA Strategy Example

```python
import zephyr_py as zephyr

@zephyr.cta_strategy(name="sma_crossover", symbols=["BTC-USDT"])
class SimpleMovingAverageStrategy:
    """Simple moving average crossover strategy."""
    
    def __init__(self, fast_period: int = 10, slow_period: int = 20):
        self.fast_period = fast_period
        self.slow_period = slow_period
    
    def on_bar(self, ctx: zephyr.CtaStrategyContext, bar: zephyr.KlineData):
        # Get historical bars as numpy arrays
        bars = ctx.get_bars_array(bar.symbol, "1h", self.slow_period)
        closes = bars["close"]
        
        if len(closes) < self.slow_period:
            return
        
        # Calculate moving averages
        fast_ma = closes[-self.fast_period:].mean()
        slow_ma = closes[-self.slow_period:].mean()
        
        # Get current position
        pos = ctx.get_position(bar.symbol)
        
        # Trading logic
        if fast_ma > slow_ma and float(pos) <= 0:
            ctx.set_position(bar.symbol, zephyr.Quantity(1.0), "long_signal")
        elif fast_ma < slow_ma and float(pos) >= 0:
            ctx.set_position(bar.symbol, zephyr.Quantity(-1.0), "short_signal")

# Access strategy metadata
meta = zephyr.get_strategy_meta(SimpleMovingAverageStrategy)
print(f"Strategy: {meta.name}, Type: {meta.strategy_type}")
```

## HFT Strategy Example

```python
import zephyr_py as zephyr

@zephyr.hft_strategy(name="simple_mm", symbols=["BTC-USDT"])
class SimpleMarketMaker:
    """Simple market making strategy."""
    
    def __init__(self, spread: float = 0.001, size: float = 0.1):
        self.spread = spread
        self.size = size
        self.buy_order: zephyr.OrderId | None = None
        self.sell_order: zephyr.OrderId | None = None
    
    def on_tick(self, ctx: zephyr.HftStrategyContext, tick: zephyr.TickData):
        # Cancel existing orders
        if self.buy_order:
            ctx.cancel(self.buy_order)
        if self.sell_order:
            ctx.cancel(self.sell_order)
        
        # Calculate prices
        mid = float(tick.price)
        buy_price = zephyr.Price(mid * (1 - self.spread))
        sell_price = zephyr.Price(mid * (1 + self.spread))
        qty = zephyr.Quantity(self.size)
        
        # Place new orders
        self.buy_order = ctx.buy(
            tick.symbol, buy_price, qty, zephyr.OrderFlag.PostOnly
        )
        self.sell_order = ctx.sell(
            tick.symbol, sell_price, qty, zephyr.OrderFlag.PostOnly
        )
```

## NumPy Integration

The library provides efficient NumPy array access for market data:

```python
import zephyr_py as zephyr
import numpy as np

ctx = zephyr.CtaStrategyContext("test")
symbol = zephyr.Symbol("BTC-USDT")

# Get K-line data as NumPy arrays
bars = ctx.get_bars_array(symbol, "1h", 100)

# Access individual arrays
timestamps: np.ndarray = bars["timestamp"]
opens: np.ndarray = bars["open"]
highs: np.ndarray = bars["high"]
lows: np.ndarray = bars["low"]
closes: np.ndarray = bars["close"]
volumes: np.ndarray = bars["volume"]

# Perform calculations
returns = np.diff(closes) / closes[:-1]
volatility = np.std(returns)
```

## Type Safety

All types are validated on construction:

```python
import zephyr_py as zephyr

# Valid
price = zephyr.Price(100.0)

# Raises ValidationException
try:
    price = zephyr.Price(-100.0)  # Negative price
except zephyr.ValidationException as e:
    print(f"Error: {e}")

# Valid symbol
symbol = zephyr.Symbol("BTC-USDT")

# Raises ValidationException
try:
    symbol = zephyr.Symbol("")  # Empty symbol
except zephyr.ValidationException as e:
    print(f"Error: {e}")
```

## API Reference

### Core Types

- `Price` - Asset prices (non-negative decimal)
- `Quantity` - Trading quantities (can be negative for shorts)
- `Amount` - Monetary amounts (price × quantity)
- `Symbol` - Trading pair identifiers
- `Timestamp` - Unix millisecond timestamps
- `OrderId` - Order identifiers

### Data Structures

- `KlinePeriod` - K-line time periods (1m, 5m, 15m, 30m, 1h, 4h, 1d, 1w)
- `KlineData` - OHLCV candlestick data
- `TickData` - Real-time tick/trade data

### Strategy Contexts

- `CtaStrategyContext` - For CTA (position-based) strategies
- `HftStrategyContext` - For HFT (order-based) strategies

### Decorators

- `cta_strategy(name, symbols, **kwargs)` - Decorator for CTA strategies
- `hft_strategy(name, symbols, **kwargs)` - Decorator for HFT strategies
- `get_strategy_meta(cls)` - Get metadata from a decorated strategy class
- `StrategyMeta` - Strategy metadata class

### Enums

- `OrderFlag` - Order execution flags (Normal, Fak, Fok, PostOnly, ReduceOnly)
- `LogLevel` - Logging levels (Trace, Debug, Info, Warn, Error)

### Exceptions

- `ZephyrException` - Base exception
- `ValidationException` - Input validation errors
- `NetworkException` - Network errors
- `ExchangeException` - Exchange API errors
- `DataException` - Data parsing errors
- `StrategyException` - Strategy execution errors
- `ConfigException` - Configuration errors
- `StorageException` - Storage/IO errors

## License

MIT License
