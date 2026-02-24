"""Sliding window rate limiter for the smooai-fetch client."""

from __future__ import annotations

import asyncio
import time
from collections import deque

from smooai_fetch._errors import RateLimitError
from smooai_fetch._types import RateLimitOptions


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

    def reset(self) -> None:
        """Clear all recorded timestamps, resetting the rate limiter."""
        self._timestamps.clear()
