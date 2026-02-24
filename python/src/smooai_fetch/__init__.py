"""Smoo AI Fetch Client - Python SDK.

A resilient HTTP fetch client with retries, timeouts, rate limiting,
and circuit breaking.
"""

__version__ = "2.1.2"

# Core client
# Builder
from smooai_fetch._builder import FetchBuilder

# Circuit breaker
from smooai_fetch._circuit_breaker import CircuitBreaker
from smooai_fetch._client import fetch

# Defaults
from smooai_fetch._defaults import (
    DEFAULT_RETRY_OPTIONS,
    DEFAULT_TIMEOUT_MS,
    DEFAULT_TIMEOUT_OPTIONS,
)

# Errors
from smooai_fetch._errors import (
    CircuitBreakerError,
    FetchError,
    HTTPResponseError,
    RateLimitError,
    RetryError,
    SchemaValidationError,
)
from smooai_fetch._errors import TimeoutError as TimeoutError

# Rate limiter
from smooai_fetch._rate_limit import SlidingWindowRateLimiter

# Response
from smooai_fetch._response import FetchResponse

# Retry utilities
from smooai_fetch._retry import calculate_backoff, is_retryable

# Types
from smooai_fetch._types import (
    CircuitBreakerOptions,
    FetchContainerOptions,
    FetchOptions,
    LifecycleHooks,
    PostResponseErrorHook,
    PostResponseSuccessHook,
    PreRequestHook,
    RateLimitOptions,
    RetryOptions,
    TimeoutOptions,
)

__all__ = [
    # Core
    "fetch",
    "FetchBuilder",
    "FetchResponse",
    # Types
    "CircuitBreakerOptions",
    "FetchContainerOptions",
    "FetchOptions",
    "LifecycleHooks",
    "PostResponseErrorHook",
    "PostResponseSuccessHook",
    "PreRequestHook",
    "RateLimitOptions",
    "RetryOptions",
    "TimeoutOptions",
    # Defaults
    "DEFAULT_RETRY_OPTIONS",
    "DEFAULT_TIMEOUT_MS",
    "DEFAULT_TIMEOUT_OPTIONS",
    # Errors
    "CircuitBreakerError",
    "FetchError",
    "HTTPResponseError",
    "RateLimitError",
    "RetryError",
    "SchemaValidationError",
    "TimeoutError",
    # Utilities
    "calculate_backoff",
    "is_retryable",
    "SlidingWindowRateLimiter",
    "CircuitBreaker",
]
