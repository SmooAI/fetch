"""Type definitions for the smooai-fetch client."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
from typing import Any

import httpx
from pydantic import BaseModel


@dataclass
class RetryOptions:
    """Configuration options for retry behavior."""

    attempts: int = 2
    """Number of retry attempts (in addition to the initial request)."""

    initial_interval_ms: float = 500
    """Initial delay between retries in milliseconds."""

    factor: float = 2
    """Multiplier applied to the interval after each retry."""

    jitter: float = 0.5
    """Random jitter factor (0-1) added to retry delays."""

    retryable_statuses: list[int] = field(default_factory=lambda: [429, 500, 502, 503, 504])
    """HTTP status codes that should trigger a retry."""


@dataclass
class TimeoutOptions:
    """Configuration options for request timeout."""

    timeout_ms: float = 30000
    """Timeout duration in milliseconds."""


@dataclass
class RateLimitOptions:
    """Configuration options for rate limiting using a sliding window."""

    max_requests: int = 10
    """Maximum number of requests allowed in the window."""

    window_ms: float = 60000
    """Duration of the sliding window in milliseconds."""


@dataclass
class CircuitBreakerOptions:
    """Configuration options for circuit breaker behavior."""

    failure_threshold: int = 5
    """Number of failures before the circuit opens."""

    success_threshold: int = 2
    """Number of successes in half-open state to close the circuit."""

    timeout: float = 30.0
    """Seconds to wait before transitioning from open to half-open."""


@dataclass
class FetchContainerOptions:
    """Container-level options for rate limiting and circuit breaking."""

    rate_limit: RateLimitOptions | None = None
    """Rate limiting configuration."""

    circuit_breaker: CircuitBreakerOptions | None = None
    """Circuit breaker configuration."""


# Hook type aliases
PreRequestHook = Callable[[str, dict[str, Any]], tuple[str, dict[str, Any]] | None]
"""Hook called before each request. Receives (url, request_kwargs).
Returns modified (url, request_kwargs) tuple or None to keep originals."""

PostResponseSuccessHook = Callable[[str, dict[str, Any], Any], Any]
"""Hook called after a successful response. Receives (url, request_kwargs, response).
Returns modified response or None to keep original."""

PostResponseErrorHook = Callable[[str, dict[str, Any], Exception, httpx.Response | None], Exception | None]
"""Hook called after an error response. Receives (url, request_kwargs, error, response).
Returns modified error or None to keep original."""


@dataclass
class LifecycleHooks:
    """Collection of lifecycle hooks for request/response handling."""

    pre_request: PreRequestHook | None = None
    """Hook that runs before the request is made."""

    post_response_success: PostResponseSuccessHook | None = None
    """Hook that runs after a successful response."""

    post_response_error: PostResponseErrorHook | None = None
    """Hook that runs after a failed response."""


@dataclass
class FetchOptions:
    """Configuration options for HTTP requests."""

    method: str = "GET"
    """HTTP method to use."""

    headers: dict[str, str] | None = None
    """HTTP headers to include in the request."""

    body: Any = None
    """Request body. Dicts/lists will be JSON-serialized."""

    retry: RetryOptions | None = None
    """Retry configuration. None to disable retries."""

    timeout: TimeoutOptions | None = None
    """Timeout configuration. None to use default."""

    schema: type[BaseModel] | None = None
    """Pydantic model for response validation."""

    hooks: LifecycleHooks | None = None
    """Lifecycle hooks for request/response handling."""

    container_options: FetchContainerOptions | None = None
    """Container-level options (rate limit, circuit breaker)."""
