"""Sliding window rate limiter for the smooai-fetch client."""

from __future__ import annotations

import asyncio
import time
from collections import deque

from smooai_fetch._errors import RateLimitError
from smooai_fetch._types import RateLimitOptions

# Minimum time (in seconds) we sleep when waiting for a window slot to free up.
# Prevents pathological busy-loops if the remaining time computation lands at
# zero due to floating-point quirks.
_MIN_WAIT_SECONDS = 0.001


class SlidingWindowRateLimiter:
    """A sliding window rate limiter using asyncio.Lock.

    Tracks timestamps of recent requests in a deque and rejects
    requests that would exceed the configured limit within the window.
    """

    def __init__(self, options: RateLimitOptions) -> None:
        self._max_requests = options.max_requests
        self._window_seconds = options.window_ms / 1000.0
        self._timestamps: deque[float] = deque()
        self._lock = asyncio.Lock()

    async def acquire(self) -> None:
        """Attempt to acquire a rate limit token.

        Removes expired timestamps from the window and checks if a new
        request is allowed.

        Raises:
            RateLimitError: If the rate limit would be exceeded.
        """
        async with self._lock:
            now = time.monotonic()

            # Remove expired timestamps
            while self._timestamps and (now - self._timestamps[0]) >= self._window_seconds:
                self._timestamps.popleft()

            if len(self._timestamps) >= self._max_requests:
                oldest = self._timestamps[0]
                remaining_ms = (oldest + self._window_seconds - now) * 1000
                raise RateLimitError(
                    f"Rate limit exceeded: {self._max_requests} requests per "
                    f"{self._window_seconds:.1f}s window. "
                    f"Try again in {remaining_ms:.0f}ms."
                )

            self._timestamps.append(now)

    async def acquire_wait(self) -> None:
        """Acquire a token, sleeping until a slot becomes available.

        Mirrors the Rust port's ``acquire`` loop: try to acquire, and if the
        window is full sleep for the remaining time before retrying. Used by
        ``FetchBuilder`` so successive ``fetch()`` calls naturally wait for
        the next slot instead of surfacing ``RateLimitError`` to the caller.

        The raise-on-full ``acquire`` API remains so that callers using the
        low-level ``fetch()`` entrypoint with ``container_options.rate_limit``
        + ``rate_limit_retry`` retain their existing behavior.
        """
        while True:
            try:
                await self.acquire()
                return
            except RateLimitError as err:
                # RateLimitError messages always include a millisecond hint —
                # parse it back out rather than threading a second value out
                # of acquire(). Fall back to a small sleep if parsing fails.
                wait_seconds = _extract_remaining_seconds(err) or _MIN_WAIT_SECONDS
                await asyncio.sleep(max(wait_seconds, _MIN_WAIT_SECONDS))

    def reset(self) -> None:
        """Clear all recorded timestamps, resetting the rate limiter."""
        self._timestamps.clear()


def _extract_remaining_seconds(err: RateLimitError) -> float | None:
    """Pull the ``Try again in N ms`` hint out of a RateLimitError message."""
    msg = str(err)
    marker = "Try again in "
    idx = msg.find(marker)
    if idx == -1:
        return None
    tail = msg[idx + len(marker) :]
    end = tail.find("ms")
    if end == -1:
        return None
    try:
        return float(tail[:end].strip()) / 1000.0
    except ValueError:
        return None
