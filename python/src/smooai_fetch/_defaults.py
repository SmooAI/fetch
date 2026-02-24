"""Default configuration values for the smooai-fetch client."""

from smooai_fetch._types import RetryOptions, TimeoutOptions

DEFAULT_RETRY_OPTIONS = RetryOptions(
    attempts=2,
    initial_interval_ms=500,
    factor=2,
    jitter=0.5,
    retryable_statuses=[429, 500, 502, 503, 504],
)
"""Default retry configuration: 2 attempts, 500ms initial interval,
exponential backoff with factor 2, 0.5 jitter, retries on 429/5xx."""

DEFAULT_TIMEOUT_MS: float = 30000
"""Default timeout in milliseconds (30 seconds)."""

DEFAULT_TIMEOUT_OPTIONS = TimeoutOptions(timeout_ms=DEFAULT_TIMEOUT_MS)
"""Default timeout configuration."""
