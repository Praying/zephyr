"""
Zephyr Python - Python bindings for the Zephyr cryptocurrency trading system.

This package provides:
- Core type wrappers (Price, Quantity, Symbol, etc.)
- Market data structures (TickData, KlineData)
- Strategy context interfaces (CTA and HFT)
- NumPy array integration for efficient data access
- Strategy decorators for defining Python strategies

Example usage:

    import zephyr_py as zephyr

    # Create core types
    price = zephyr.Price(42000.50)
    qty = zephyr.Quantity(1.5)
    symbol = zephyr.Symbol("BTC-USDT")

    # Create a CTA strategy context
    ctx = zephyr.CtaStrategyContext("my_strategy")
    
    # Get position
    pos = ctx.get_position(symbol)
    
    # Set position
    ctx.set_position(symbol, qty, "long_signal")

    # Define a strategy using decorators
    @zephyr.cta_strategy(name="my_strategy", symbols=["BTC-USDT"])
    class MyStrategy:
        def on_bar(self, ctx, bar):
            pass
"""

from .zephyr_py import *

__all__ = [
    # Version
    "__version__",
    
    # Exceptions
    "ZephyrException",
    "ValidationException",
    "NetworkException",
    "ExchangeException",
    "DataException",
    "StrategyException",
    "ConfigException",
    "StorageException",
    
    # Core types
    "Price",
    "Quantity",
    "Amount",
    "Symbol",
    "Timestamp",
    "OrderId",
    
    # Data structures
    "KlinePeriod",
    "KlineData",
    "TickData",
    
    # Strategy
    "OrderFlag",
    "LogLevel",
    "CtaStrategyContext",
    "HftStrategyContext",
    
    # Decorators
    "StrategyMeta",
    "cta_strategy",
    "hft_strategy",
    "get_strategy_meta",
]
