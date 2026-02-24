"""Basic tests for smooai-fetch package imports and version."""

from smooai_fetch import (
    DEFAULT_RETRY_OPTIONS,
    DEFAULT_TIMEOUT_MS,
    DEFAULT_TIMEOUT_OPTIONS,
    CircuitBreaker,
    CircuitBreakerError,
    CircuitBreakerOptions,
    FetchBuilder,
    FetchError,
    FetchOptions,
    FetchResponse,
    HTTPResponseError,
    RateLimitError,
    RateLimitOptions,
    RetryError,
    RetryOptions,
    SchemaValidationError,
    SlidingWindowRateLimiter,
    TimeoutError,
    TimeoutOptions,
    __version__,
    calculate_backoff,
    fetch,
    is_retryable,
)


def test_version():
    assert __version__ == "2.1.2"


def test_default_retry_options():
    assert DEFAULT_RETRY_OPTIONS.attempts == 2
    assert DEFAULT_RETRY_OPTIONS.initial_interval_ms == 500
    assert DEFAULT_RETRY_OPTIONS.factor == 2
    assert DEFAULT_RETRY_OPTIONS.jitter == 0.5
    assert 429 in DEFAULT_RETRY_OPTIONS.retryable_statuses
    assert 500 in DEFAULT_RETRY_OPTIONS.retryable_statuses
    assert 502 in DEFAULT_RETRY_OPTIONS.retryable_statuses
    assert 503 in DEFAULT_RETRY_OPTIONS.retryable_statuses
    assert 504 in DEFAULT_RETRY_OPTIONS.retryable_statuses


def test_default_timeout():
    assert DEFAULT_TIMEOUT_MS == 30000
    assert DEFAULT_TIMEOUT_OPTIONS.timeout_ms == 30000


def test_all_exports_importable():
    """Verify all public exports are importable."""
    assert callable(fetch)
    assert FetchBuilder is not None
    assert FetchResponse is not None
    assert FetchOptions is not None
    assert RetryOptions is not None
    assert TimeoutOptions is not None
    assert RateLimitOptions is not None
    assert CircuitBreakerOptions is not None
    assert callable(calculate_backoff)
    assert callable(is_retryable)
    assert SlidingWindowRateLimiter is not None
    assert CircuitBreaker is not None


def test_error_hierarchy():
    """Verify error class inheritance."""
    assert issubclass(HTTPResponseError, FetchError)
    assert issubclass(RetryError, FetchError)
    assert issubclass(RateLimitError, FetchError)
    assert issubclass(CircuitBreakerError, FetchError)
    assert issubclass(TimeoutError, FetchError)
    assert issubclass(SchemaValidationError, FetchError)
    assert issubclass(FetchError, Exception)
