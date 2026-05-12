"""Retry logic for the smooai-fetch client."""

from __future__ import annotations

import asyncio
import random
import time
from collections.abc import Awaitable, Callable
from typing import TypeVar

from smooai_fetch._errors import HTTPResponseError, RetryError
from smooai_fetch._types import (
    OnRejectionDecisionKind,
    RetryContext,
    RetryOptions,
)

T = TypeVar("T")


def calculate_backoff(attempt: int, options: RetryOptions) -> float:
    """Calculate the backoff delay in seconds for a given retry attempt.

    Uses exponential backoff with jitter:
        delay = initial_interval * (factor ^ attempt) * (1 + random(-jitter, jitter))

    If `options.max_interval_ms` is set, the exponential delay is capped at that
    value *before* jitter is applied (matching the Rust + Go ports).

    Args:
        attempt: The zero-based attempt number (0 = first retry).
        options: Retry configuration options.

    Returns:
        Delay in seconds.
    """
    base_delay_ms = options.initial_interval_ms * (options.factor**attempt)

    # Cap before jitter so jitter still spreads load around the cap.
    if options.max_interval_ms is not None:
        base_delay_ms = min(base_delay_ms, options.max_interval_ms)

    jitter_factor = 1.0 + random.uniform(-options.jitter, options.jitter)
    delay_ms = base_delay_ms * jitter_factor
    return max(0, delay_ms / 1000.0)


def is_retryable(status_code: int, options: RetryOptions) -> bool:
    """Check if an HTTP status code should trigger a retry.

    Args:
        status_code: The HTTP response status code.
        options: Retry configuration with retryable status list.

    Returns:
        True if the status code is in the retryable list.
    """
    return status_code in options.retryable_statuses


def _status_from_error(error: Exception) -> int | None:
    """Extract HTTP status code from an HTTPResponseError, if applicable."""
    if isinstance(error, HTTPResponseError):
        return error.status
    return None


async def execute_with_retry(
    func: Callable[..., Awaitable[T]],
    options: RetryOptions,
    should_retry: Callable[[Exception, int], bool | float] | None = None,
    get_retry_after: Callable[[Exception], float | None] | None = None,
) -> T:
    """Execute an async function with retry logic.

    Args:
        func: The async function to execute (takes no arguments, use a lambda/closure).
        options: Retry configuration.
        should_retry: Optional callback that receives (error, attempt) and returns
            True to retry with calculated backoff, False to stop, or a float
            specifying the delay in seconds.
        get_retry_after: Optional callback that extracts a Retry-After delay (in seconds)
            from an exception.

    Returns:
        The result of the function call.

    Raises:
        RetryError: If all retry attempts are exhausted and the error was retryable.
        Exception: The original error if it's not retryable.
    """
    last_error: Exception | None = None
    total_attempts = 1 + options.attempts  # initial + retries
    started_at = time.monotonic()

    for attempt in range(total_attempts):
        try:
            return await func()
        except Exception as e:
            last_error = e

            # Check if we should retry (before checking if last attempt)
            custom_delay: float | None = None

            if should_retry is not None:
                result = should_retry(e, attempt)
                if result is False:
                    # Not retryable - raise original error immediately
                    raise e
                if isinstance(result, (int, float)) and not isinstance(result, bool) and result > 0:
                    custom_delay = result

            # If this was the last attempt, wrap in RetryError
            if attempt >= total_attempts - 1:
                if total_attempts > 1:
                    # We actually attempted retries, so wrap in RetryError
                    raise RetryError(last_error, total_attempts) from last_error
                else:
                    # No retries configured (attempts=0), just raise original
                    raise e

            # Consult on_rejection callback before computing default delay.
            # `attempt` here is 0-based for the just-failed call, so the retry
            # we are about to perform is `attempt + 1` (1-based).
            if options.on_rejection is not None:
                ctx = RetryContext(
                    attempt=attempt + 1,
                    last_error=e,
                    last_status=_status_from_error(e),
                    elapsed_ms=(time.monotonic() - started_at) * 1000.0,
                )
                decision = options.on_rejection(ctx)
                kind = decision.kind

                if kind is OnRejectionDecisionKind.ABORT:
                    # Surface the underlying error unwrapped.
                    raise e
                if kind is OnRejectionDecisionKind.SKIP:
                    # No sleep, no request; move to the next attempt.
                    continue
                if kind is OnRejectionDecisionKind.RETRY_WITH_DELAY:
                    delay_ms = decision.delay_ms or 0.0
                    await asyncio.sleep(max(0.0, delay_ms / 1000.0))
                    continue
                if kind is OnRejectionDecisionKind.RETRY:
                    # Override `should_retry`'s custom delay (Retry-After
                    # already handled below) and force exponential backoff.
                    custom_delay = None
                # DEFAULT falls through to the built-in logic below.

            # Apply delay before next attempt
            if custom_delay is not None:
                await asyncio.sleep(custom_delay)
                continue

            # Check for Retry-After header
            retry_after_delay: float | None = None
            if get_retry_after is not None:
                retry_after_delay = get_retry_after(e)

            if retry_after_delay is not None and retry_after_delay > 0:
                await asyncio.sleep(retry_after_delay)
                continue

            # fast_first: skip delay on the very first retry.
            if options.fast_first and attempt == 0:
                continue

            # Default: exponential backoff with jitter.
            delay = calculate_backoff(attempt, options)
            await asyncio.sleep(delay)

    # Should not reach here, but just in case
    raise RetryError(last_error or Exception("Unknown error"), total_attempts)
