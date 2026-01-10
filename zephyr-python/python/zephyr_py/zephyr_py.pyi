"""Type stubs for zephyr_py module."""

from typing import Optional, List, Dict
import numpy as np
from numpy.typing import NDArray

__version__: str

# Exceptions
class ZephyrException(Exception):
    """Base exception for all Zephyr errors."""
    ...

class ValidationException(ZephyrException):
    """Validation error for invalid input values."""
    ...

class NetworkException(ZephyrException):
    """Network-related errors (connection, timeout, etc.)."""
    ...

class ExchangeException(ZephyrException):
    """Exchange API errors (auth, rate limit, etc.)."""
    ...

class DataException(ZephyrException):
    """Data parsing and validation errors."""
    ...

class StrategyException(ZephyrException):
    """Strategy execution errors."""
    ...

class ConfigException(ZephyrException):
    """Configuration errors."""
    ...

class StorageException(ZephyrException):
    """Storage and I/O errors."""
    ...

# Core types
class Price:
    """Price type - represents asset prices with decimal precision."""
    
    def __init__(self, value: float) -> None:
        """Creates a new Price from a float value."""
        ...
    
    @classmethod
    def from_str(cls, s: str) -> "Price":
        """Creates a Price from a string representation."""
        ...
    
    @property
    def is_zero(self) -> bool:
        """Returns true if the price is zero."""
        ...
    
    def __float__(self) -> float: ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...
    def __add__(self, other: "Price") -> "Price": ...
    def __sub__(self, other: "Price") -> float: ...
    def __eq__(self, other: object) -> bool: ...
    def __lt__(self, other: "Price") -> bool: ...
    def __le__(self, other: "Price") -> bool: ...
    def __gt__(self, other: "Price") -> bool: ...
    def __ge__(self, other: "Price") -> bool: ...
    def __hash__(self) -> int: ...

class Quantity:
    """Quantity type - represents trading quantities."""
    
    def __init__(self, value: float) -> None:
        """Creates a new Quantity from a float value."""
        ...
    
    @classmethod
    def from_str(cls, s: str) -> "Quantity":
        """Creates a Quantity from a string representation."""
        ...
    
    @property
    def is_zero(self) -> bool:
        """Returns true if the quantity is zero."""
        ...
    
    @property
    def is_positive(self) -> bool:
        """Returns true if the quantity is positive."""
        ...
    
    @property
    def is_negative(self) -> bool:
        """Returns true if the quantity is negative."""
        ...
    
    def abs(self) -> "Quantity":
        """Returns the absolute value."""
        ...
    
    def __float__(self) -> float: ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...
    def __add__(self, other: "Quantity") -> "Quantity": ...
    def __sub__(self, other: "Quantity") -> "Quantity": ...
    def __neg__(self) -> "Quantity": ...
    def __eq__(self, other: object) -> bool: ...
    def __lt__(self, other: "Quantity") -> bool: ...
    def __le__(self, other: "Quantity") -> bool: ...
    def __gt__(self, other: "Quantity") -> bool: ...
    def __ge__(self, other: "Quantity") -> bool: ...
    def __hash__(self) -> int: ...

class Amount:
    """Amount type - represents monetary amounts (price × quantity)."""
    
    def __init__(self, value: float) -> None:
        """Creates a new Amount from a float value."""
        ...
    
    @classmethod
    def from_str(cls, s: str) -> "Amount":
        """Creates an Amount from a string representation."""
        ...
    
    @property
    def is_zero(self) -> bool:
        """Returns true if the amount is zero."""
        ...
    
    def __float__(self) -> float: ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class Symbol:
    """Symbol type - represents trading pair identifiers."""
    
    def __init__(self, value: str) -> None:
        """Creates a new Symbol from a string."""
        ...
    
    @property
    def base_asset(self) -> Optional[str]:
        """Returns the base asset (e.g., 'BTC' from 'BTC-USDT')."""
        ...
    
    @property
    def quote_asset(self) -> Optional[str]:
        """Returns the quote asset (e.g., 'USDT' from 'BTC-USDT')."""
        ...
    
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class Timestamp:
    """Timestamp type - represents Unix millisecond timestamps."""
    
    def __init__(self, millis: int) -> None:
        """Creates a new Timestamp from milliseconds since Unix epoch."""
        ...
    
    @classmethod
    def now(cls) -> "Timestamp":
        """Returns the current timestamp."""
        ...
    
    @classmethod
    def from_secs(cls, secs: int) -> "Timestamp":
        """Creates a Timestamp from seconds since Unix epoch."""
        ...
    
    @property
    def as_millis(self) -> int:
        """Returns the timestamp as milliseconds."""
        ...
    
    @property
    def as_secs(self) -> int:
        """Returns the timestamp as seconds."""
        ...
    
    @property
    def is_zero(self) -> bool:
        """Returns true if the timestamp is zero."""
        ...
    
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...
    def __int__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __lt__(self, other: "Timestamp") -> bool: ...
    def __le__(self, other: "Timestamp") -> bool: ...
    def __gt__(self, other: "Timestamp") -> bool: ...
    def __ge__(self, other: "Timestamp") -> bool: ...
    def __hash__(self) -> int: ...

class OrderId:
    """OrderId type - represents order identifiers."""
    
    def __init__(self, value: str) -> None:
        """Creates a new OrderId from a string."""
        ...
    
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

# Data structures
class KlinePeriod:
    """K-line period enumeration."""
    
    Minute1: "KlinePeriod"
    Minute5: "KlinePeriod"
    Minute15: "KlinePeriod"
    Minute30: "KlinePeriod"
    Hour1: "KlinePeriod"
    Hour4: "KlinePeriod"
    Day1: "KlinePeriod"
    Week1: "KlinePeriod"
    
    @property
    def secs(self) -> int:
        """Returns the duration in seconds."""
        ...
    
    @property
    def millis(self) -> int:
        """Returns the duration in milliseconds."""
        ...
    
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...

class KlineData:
    """K-line (candlestick) data structure."""
    
    def __init__(
        self,
        symbol: Symbol,
        timestamp: Timestamp,
        period: KlinePeriod,
        open: Price,
        high: Price,
        low: Price,
        close: Price,
        volume: Quantity,
        turnover: float,
    ) -> None:
        """Creates a new KlineData."""
        ...
    
    @property
    def symbol(self) -> Symbol: ...
    @property
    def timestamp(self) -> Timestamp: ...
    @property
    def period(self) -> KlinePeriod: ...
    @property
    def open(self) -> Price: ...
    @property
    def high(self) -> Price: ...
    @property
    def low(self) -> Price: ...
    @property
    def close(self) -> Price: ...
    @property
    def volume(self) -> Quantity: ...
    @property
    def turnover(self) -> float: ...
    @property
    def is_bullish(self) -> bool: ...
    @property
    def is_bearish(self) -> bool: ...
    @property
    def range(self) -> float: ...
    @property
    def body(self) -> float: ...
    
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...

class TickData:
    """Tick data structure representing real-time market data."""
    
    def __init__(
        self,
        symbol: Symbol,
        timestamp: Timestamp,
        price: Price,
        volume: Quantity,
        bid_prices: List[Price] = ...,
        bid_quantities: List[Quantity] = ...,
        ask_prices: List[Price] = ...,
        ask_quantities: List[Quantity] = ...,
        open_interest: Optional[float] = None,
        funding_rate: Optional[float] = None,
        mark_price: Optional[float] = None,
    ) -> None:
        """Creates a new TickData."""
        ...
    
    @property
    def symbol(self) -> Symbol: ...
    @property
    def timestamp(self) -> Timestamp: ...
    @property
    def price(self) -> Price: ...
    @property
    def volume(self) -> Quantity: ...
    @property
    def bid_prices(self) -> List[Price]: ...
    @property
    def bid_quantities(self) -> List[Quantity]: ...
    @property
    def ask_prices(self) -> List[Price]: ...
    @property
    def ask_quantities(self) -> List[Quantity]: ...
    @property
    def open_interest(self) -> Optional[float]: ...
    @property
    def funding_rate(self) -> Optional[float]: ...
    @property
    def mark_price(self) -> Optional[float]: ...
    @property
    def spread(self) -> Optional[float]: ...
    
    def best_bid(self) -> Optional[Price]: ...
    def best_ask(self) -> Optional[Price]: ...
    def mid_price(self) -> Optional[Price]: ...
    
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...

# Strategy
class OrderFlag:
    """Order execution flags for HFT strategies."""
    
    Normal: "OrderFlag"
    Fak: "OrderFlag"
    Fok: "OrderFlag"
    PostOnly: "OrderFlag"
    ReduceOnly: "OrderFlag"
    
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...

class LogLevel:
    """Log level for strategy logging."""
    
    Trace: "LogLevel"
    Debug: "LogLevel"
    Info: "LogLevel"
    Warn: "LogLevel"
    Error: "LogLevel"
    
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...

class CtaStrategyContext:
    """CTA Strategy Context for Python strategies."""
    
    def __init__(self, name: str) -> None:
        """Creates a new CTA strategy context."""
        ...
    
    @property
    def strategy_name(self) -> str:
        """Gets the strategy name."""
        ...
    
    def get_position(self, symbol: Symbol) -> Quantity:
        """Gets the current position for a symbol."""
        ...
    
    def set_position(self, symbol: Symbol, qty: Quantity, tag: str) -> None:
        """Sets the target position for a symbol."""
        ...
    
    def get_price(self, symbol: Symbol) -> Optional[Price]:
        """Gets the current price for a symbol."""
        ...
    
    def get_bars(self, symbol: Symbol, period: str, count: int) -> List[KlineData]:
        """Gets historical K-line data for a symbol."""
        ...
    
    def get_bars_array(
        self, symbol: Symbol, period: str, count: int
    ) -> Dict[str, NDArray[np.float64]]:
        """Gets historical K-line data as NumPy arrays."""
        ...
    
    def get_ticks(self, symbol: Symbol, count: int) -> List[TickData]:
        """Gets recent tick data for a symbol."""
        ...
    
    def get_ticks_array(
        self, symbol: Symbol, count: int
    ) -> Dict[str, NDArray[np.float64]]:
        """Gets recent tick data as NumPy arrays."""
        ...
    
    def subscribe_ticks(self, symbol: Symbol) -> None:
        """Subscribes to tick data for a symbol."""
        ...
    
    def unsubscribe_ticks(self, symbol: Symbol) -> None:
        """Unsubscribes from tick data for a symbol."""
        ...
    
    def current_time(self) -> Timestamp:
        """Gets the current timestamp."""
        ...
    
    def log(self, level: LogLevel, message: str) -> None:
        """Logs a message with the specified level."""
        ...
    
    def symbols(self) -> List[Symbol]:
        """Gets all symbols the strategy is trading."""
        ...
    
    # Test helpers
    def _set_price(self, symbol: Symbol, price: float) -> None: ...
    def _set_current_time(self, millis: int) -> None: ...

class HftStrategyContext:
    """HFT Strategy Context for Python strategies."""
    
    def __init__(self, name: str) -> None:
        """Creates a new HFT strategy context."""
        ...
    
    @property
    def strategy_name(self) -> str:
        """Gets the strategy name."""
        ...
    
    def buy(
        self, symbol: Symbol, price: Price, qty: Quantity, flag: OrderFlag
    ) -> OrderId:
        """Submits a buy order."""
        ...
    
    def sell(
        self, symbol: Symbol, price: Price, qty: Quantity, flag: OrderFlag
    ) -> OrderId:
        """Submits a sell order."""
        ...
    
    def cancel(self, order_id: OrderId) -> bool:
        """Cancels an order."""
        ...
    
    def cancel_all(self, symbol: Symbol) -> int:
        """Cancels all orders for a symbol."""
        ...
    
    def get_position(self, symbol: Symbol) -> Quantity:
        """Gets the current position for a symbol."""
        ...
    
    def get_price(self, symbol: Symbol) -> Optional[Price]:
        """Gets the current price for a symbol."""
        ...
    
    def current_time(self) -> Timestamp:
        """Gets the current timestamp."""
        ...
    
    def log(self, level: LogLevel, message: str) -> None:
        """Logs a message with the specified level."""
        ...
    
    # Test helpers
    def _set_position(self, symbol: Symbol, qty: float) -> None: ...
    def _set_price(self, symbol: Symbol, price: float) -> None: ...
    def _set_current_time(self, millis: int) -> None: ...

# Strategy Decorators
class StrategyMeta:
    """Strategy metadata extracted from decorated classes."""
    
    def __init__(
        self,
        name: str,
        strategy_type: str,
        symbols: List[str] = ...,
        params: Optional[Dict[str, object]] = None,
    ) -> None:
        """Creates a new strategy metadata."""
        ...
    
    @property
    def name(self) -> str:
        """Strategy name."""
        ...
    
    @property
    def strategy_type(self) -> str:
        """Strategy type ('cta' or 'hft')."""
        ...
    
    @property
    def symbols(self) -> List[str]:
        """Trading symbols."""
        ...
    
    @property
    def params(self) -> Dict[str, object]:
        """Strategy parameters."""
        ...
    
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...

class _CtaDecorator:
    """Internal CTA strategy decorator (returned by cta_strategy)."""
    
    def __call__(self, cls: type) -> type:
        """Decorates a class as a CTA strategy."""
        ...

class _HftDecorator:
    """Internal HFT strategy decorator (returned by hft_strategy)."""
    
    def __call__(self, cls: type) -> type:
        """Decorates a class as an HFT strategy."""
        ...

def cta_strategy(
    name: str,
    symbols: List[str] = ...,
    **kwargs: object,
) -> _CtaDecorator:
    """
    CTA strategy decorator.
    
    Marks a class as a CTA strategy and registers its metadata.
    
    Args:
        name: Strategy name.
        symbols: List of trading symbols.
        **kwargs: Additional strategy parameters.
    
    Returns:
        A decorator that adds strategy metadata to the class.
    
    Example:
        @cta_strategy(name="my_strategy", symbols=["BTC-USDT"])
        class MyStrategy:
            def on_init(self, ctx: CtaStrategyContext):
                pass
            
            def on_bar(self, ctx: CtaStrategyContext, bar: KlineData):
                pass
    """
    ...

def hft_strategy(
    name: str,
    symbols: List[str] = ...,
    **kwargs: object,
) -> _HftDecorator:
    """
    HFT strategy decorator.
    
    Marks a class as an HFT strategy and registers its metadata.
    
    Args:
        name: Strategy name.
        symbols: List of trading symbols.
        **kwargs: Additional strategy parameters.
    
    Returns:
        A decorator that adds strategy metadata to the class.
    
    Example:
        @hft_strategy(name="my_hft", symbols=["BTC-USDT"])
        class MyHftStrategy:
            def on_init(self, ctx: HftStrategyContext):
                pass
            
            def on_tick(self, ctx: HftStrategyContext, tick: TickData):
                pass
    """
    ...

def get_strategy_meta(cls: type) -> Optional[StrategyMeta]:
    """
    Gets the strategy metadata from a decorated class.
    
    Args:
        cls: A class decorated with @cta_strategy or @hft_strategy.
    
    Returns:
        The strategy metadata, or None if the class is not decorated.
    
    Example:
        @cta_strategy(name="test", symbols=["BTC-USDT"])
        class TestStrategy:
            pass
        
        meta = get_strategy_meta(TestStrategy)
        print(meta.name)  # "test"
    """
    ...
