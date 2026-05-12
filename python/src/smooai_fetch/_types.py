"""Type definitions for the smooai-fetch client."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
from enum import StrEnum
from typing import Any

import httpx
from pydantic import BaseModel


class OnRejectionDecisionKind(StrEnum):
    """Kind of decision returned by an `on_rejection` callback."""

    RETRY = "retry"
    """Retry using the built-in exponential+jitter delay (no override)."""

    RETRY_WITH_DELAY = "retry_with_delay"
    """Retry after a caller-supplied delay (in milliseconds)."""

    ABORT = "abort"
    """Stop retrying and surface the most recent error."""

    SKIP = "skip"
    """Skip this retry attempt without sleeping; proceed to the next one."""

    DEFAULT = "default"
    """Fall through to the built-in default delay logic."""


@dataclass(frozen=True)
class OnRejectionDecision:
    """Decision returned by an `on_rejection` callback.

    Construct via the class-method factories rather than directly:

        OnRejectionDecision.retry()
        OnRejectionDecision.retry_with_delay(2_000)
        OnRejectionDecision.abort()
        OnRejectionDecision.skip()
        OnRejectionDecision.default()
    """

    kind: OnRejectionDecisionKind
    delay_ms: float | None = None

    @classmethod
    def retry(cls) -> OnRejectionDecision:
        return cls(OnRejectionDecisionKind.RETRY)

    @classmethod
    def retry_with_delay(cls, delay_ms: float) -> OnRejectionDecision:
        return cls(OnRejectionDecisionKind.RETRY_WITH_DELAY, delay_ms=delay_ms)

    @classmethod
    def abort(cls) -> OnRejectionDecision:
        return cls(OnRejectionDecisionKind.ABORT)

    @classmethod
    def skip(cls) -> OnRejectionDecision:
        return cls(OnRejectionDecisionKind.SKIP)

    @classmethod
    def default(cls) -> OnRejectionDecision:
        return cls(OnRejectionDecisionKind.DEFAULT)


@dataclass
class RetryContext:
    """Context passed to an `on_rejection` callback before each retry."""

    attempt: int
    """1-based attempt number for the retry that is about to be performed."""

    last_error: Exception | None = None
    """The most recent error, if any."""

    last_status: int | None = None
    """HTTP status code from the most recent error, if it was an `HTTPResponseError`."""

    elapsed_ms: float = 0.0
    """Time elapsed since the retry loop started, in milliseconds."""


OnRejectionCallback = Callable[[RetryContext], OnRejectionDecision]
"""Callback invoked before each retry attempt to override default behavior."""


AuthTokenProvider = Callable[[], "Any"]
"""Provider that returns an auth token. May return a `str` directly or an awaitable
that resolves to a `str`. Invoked before every request to populate the
`Authorization` header. Mirrors the .NET delegate of the same name.
"""


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

    max_interval_ms: float | None = None
    """Optional cap on the per-retry delay (after exponential backoff + jitter).

    When set, the computed delay is clamped to `min(delay, max_interval_ms)`.
    """

    fast_first: bool = False
    """When True, the first retry fires immediately with zero delay.

    Subsequent retries use the normal exponential backoff formula.
    """

    on_rejection: OnRejectionCallback | None = None
    """Optional callback consulted before each retry attempt.

    Receives a `RetryContext` and returns an `OnRejectionDecision` that can
    override the default delay, skip the attempt, or abort retrying entirely.
    """


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


CircuitStateChangeCallback = Callable[[str, str], None]
"""Callback invoked when the circuit breaker transitions between states.

Receives `(from_state, to_state)` where each value is one of `"closed"`,
`"open"`, or `"half-open"`. Mirrors the Go port's `OnStateChange` callback.
"""


@dataclass
class CircuitBreakerOptions:
    """Configuration options for circuit breaker behavior."""

    failure_threshold: int = 5
    """Number of failures before the circuit opens.

    Used when `failure_rate_threshold` is None (the default). With a rate-based
    threshold this still acts as the minimum sample count before the rate
    evaluation kicks in.
    """

    success_threshold: int = 2
    """Number of successes in half-open state to close the circuit."""

    timeout: float = 30.0
    """Seconds to wait before transitioning from open to half-open."""

    failure_rate_threshold: float | None = None
    """Optional failure rate (0.0–1.0) over a sliding window that trips the breaker.

    When set, the breaker tracks the most recent `sliding_window_size` outcomes
    and trips when the failure ratio meets or exceeds this threshold (after
    `failure_threshold` minimum samples have been observed). Mirrors the TS
    `failureRateThreshold` setting.
    """

    sliding_window_size: int = 10
    """Number of recent outcomes to retain for rate-based detection.

    Only consulted when `failure_rate_threshold` is set.
    """

    on_state_change: CircuitStateChangeCallback | None = None
    """Optional callback invoked when the breaker transitions between states.

    Receives `(from_state, to_state)`. Useful for telemetry / alerting.
    """


# Rate-limit-specific retry options share the same shape as the main RetryOptions.
# This mirrors the Go port (`type RateLimitRetryOptions = RetryOptions`) and the
# TypeScript container-options `rateLimit.retry` field — both of which reuse the
# main RetryOptions shape.
RateLimitRetryOptions = RetryOptions


@dataclass
class FetchContainerOptions:
    """Container-level options for rate limiting and circuit breaking."""

    rate_limit: RateLimitOptions | None = None
    """Rate limiting configuration."""

    rate_limit_retry: RateLimitRetryOptions | None = None
    """Retry behavior applied specifically to rate-limit rejections.

    When the in-process sliding-window rate limiter rejects a request, the
    rejection is retried within a dedicated inner loop using these options so
    it does not consume the main retry budget. Mirrors the TypeScript
    container-options `rateLimit.retry` field.
    """

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

    auth_token_provider: AuthTokenProvider | None = None
    """Optional sync or async provider invoked before each request to mint an auth token.

    The returned token is injected into the `Authorization` header using
    `auth_scheme` (default `"Bearer"`). Awaitable return values are awaited.
    """

    auth_scheme: str = "Bearer"
    """Auth scheme prefix used with `auth_token_provider`. Defaults to "Bearer"."""
