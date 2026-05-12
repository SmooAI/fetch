"""Circuit breaker implementation for the smooai-fetch client.

Implements a simple async-compatible circuit breaker without relying on
pybreaker's async support (which requires tornado).
"""

from __future__ import annotations

import asyncio
import time
from collections import deque
from collections.abc import Awaitable, Callable
from enum import Enum
from typing import TypeVar

from smooai_fetch._errors import CircuitBreakerError
from smooai_fetch._types import CircuitBreakerOptions

T = TypeVar("T")


class CircuitState(Enum):
    """Possible states of the circuit breaker."""

    CLOSED = "closed"
    OPEN = "open"
    HALF_OPEN = "half-open"


class CircuitBreaker:
    """An async-compatible circuit breaker.

    State transitions:
    - CLOSED: Normal operation. Failures are counted (and optionally tracked over a
      sliding window when `failure_rate_threshold` is set).
    - OPEN: Requests are rejected immediately. After `timeout`, transitions to HALF_OPEN.
    - HALF_OPEN: A limited number of requests are allowed through. If they succeed
      (reaching success_threshold), transitions to CLOSED. If one fails, transitions
      back to OPEN.
    """

    def __init__(self, options: CircuitBreakerOptions) -> None:
        self._failure_threshold = options.failure_threshold
        self._success_threshold = options.success_threshold
        self._timeout = options.timeout  # seconds
        self._failure_rate_threshold = options.failure_rate_threshold
        self._sliding_window_size = max(1, options.sliding_window_size)
        self._on_state_change = options.on_state_change

        self._state = CircuitState.CLOSED
        self._failure_count = 0
        self._success_count = 0
        self._last_failure_time: float | None = None
        # Recent outcomes window: True = success, False = failure.
        self._outcomes: deque[bool] = deque(maxlen=self._sliding_window_size)
        self._lock = asyncio.Lock()

    @property
    def state(self) -> str:
        """Current state of the circuit breaker as a string."""
        # Check if we should transition from OPEN to HALF_OPEN
        if self._state == CircuitState.OPEN and self._last_failure_time is not None:
            elapsed = time.monotonic() - self._last_failure_time
            if elapsed >= self._timeout:
                return CircuitState.HALF_OPEN.value
        return self._state.value

    async def call(self, func: Callable[..., Awaitable[T]]) -> T:
        """Execute an async function through the circuit breaker.

        Args:
            func: The async callable to execute.

        Returns:
            The result of the function.

        Raises:
            CircuitBreakerError: If the circuit is open and timeout has not elapsed.
        """
        async with self._lock:
            current_state = self._get_state()

            if current_state == CircuitState.OPEN:
                raise CircuitBreakerError("Circuit breaker is open")

            if current_state == CircuitState.HALF_OPEN:
                # Allow the request through, but track carefully
                pass

        # Execute the function outside the lock
        try:
            result = await func()
        except Exception:
            async with self._lock:
                self._record_failure()
            raise

        async with self._lock:
            self._record_success()

        return result

    def _transition(self, target: CircuitState) -> None:
        """Move to `target` and fire the on_state_change callback (if registered)."""
        if self._state is target:
            return
        previous = self._state
        self._state = target
        if self._on_state_change is not None:
            # Errors in user callbacks should not break the breaker's bookkeeping.
            try:
                self._on_state_change(previous.value, target.value)
            except Exception:
                pass

    def _get_state(self) -> CircuitState:
        """Get the current state, potentially transitioning OPEN -> HALF_OPEN."""
        if self._state == CircuitState.OPEN and self._last_failure_time is not None:
            elapsed = time.monotonic() - self._last_failure_time
            if elapsed >= self._timeout:
                self._transition(CircuitState.HALF_OPEN)
                self._success_count = 0
                return CircuitState.HALF_OPEN
        return self._state

    def _record_success(self) -> None:
        """Record a successful call."""
        self._outcomes.append(True)
        if self._state == CircuitState.HALF_OPEN:
            self._success_count += 1
            if self._success_count >= self._success_threshold:
                self._transition(CircuitState.CLOSED)
                self._failure_count = 0
                self._success_count = 0
                self._outcomes.clear()
        elif self._state == CircuitState.CLOSED:
            # Reset failure count on success in closed state.
            self._failure_count = 0

    def _record_failure(self) -> None:
        """Record a failed call."""
        self._outcomes.append(False)
        if self._state == CircuitState.HALF_OPEN:
            # Any failure in half-open goes back to open.
            self._transition(CircuitState.OPEN)
            self._last_failure_time = time.monotonic()
            self._success_count = 0
            return
        if self._state == CircuitState.CLOSED:
            self._failure_count += 1
            # Rate-based detection: when a threshold is configured and enough
            # samples have been observed, evaluate the failure ratio over the
            # sliding window.
            if self._failure_rate_threshold is not None and len(self._outcomes) >= self._failure_threshold:
                failures = sum(1 for ok in self._outcomes if not ok)
                rate = failures / len(self._outcomes)
                if rate >= self._failure_rate_threshold:
                    self._transition(CircuitState.OPEN)
                    self._last_failure_time = time.monotonic()
                    return
            # Count-based detection: trip when consecutive failures reach the threshold.
            if self._failure_rate_threshold is None and self._failure_count >= self._failure_threshold:
                self._transition(CircuitState.OPEN)
                self._last_failure_time = time.monotonic()
