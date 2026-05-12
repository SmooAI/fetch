"""Tests for retry functionality."""

import time

import httpx
import pytest
import respx

from smooai_fetch import (
    FetchOptions,
    HTTPResponseError,
    OnRejectionDecision,
    RetryError,
    RetryOptions,
    fetch,
)
from smooai_fetch._retry import calculate_backoff, is_retryable

URL = "https://api.example.com/data"


class TestCalculateBackoff:
    """Tests for the calculate_backoff function."""

    def test_first_attempt_backoff(self):
        """First retry should use initial_interval_ms as base."""
        options = RetryOptions(initial_interval_ms=500, factor=2, jitter=0)
        delay = calculate_backoff(0, options)
        assert delay == pytest.approx(0.5, abs=0.01)

    def test_second_attempt_backoff(self):
        """Second retry should multiply by factor."""
        options = RetryOptions(initial_interval_ms=500, factor=2, jitter=0)
        delay = calculate_backoff(1, options)
        assert delay == pytest.approx(1.0, abs=0.01)

    def test_third_attempt_backoff(self):
        """Third retry should multiply by factor^2."""
        options = RetryOptions(initial_interval_ms=500, factor=2, jitter=0)
        delay = calculate_backoff(2, options)
        assert delay == pytest.approx(2.0, abs=0.01)

    def test_jitter_adds_randomness(self):
        """Jitter should add randomness to the delay."""
        options = RetryOptions(initial_interval_ms=1000, factor=1, jitter=0.5)
        delays = [calculate_backoff(0, options) for _ in range(100)]
        # With jitter=0.5, delays should be between 0.5s and 1.5s
        assert all(0.5 <= d <= 1.5 for d in delays)
        # There should be variation
        assert len(set(round(d, 3) for d in delays)) > 1


class TestIsRetryable:
    """Tests for the is_retryable function."""

    def test_429_is_retryable(self):
        options = RetryOptions()
        assert is_retryable(429, options) is True

    def test_500_is_retryable(self):
        options = RetryOptions()
        assert is_retryable(500, options) is True

    def test_502_is_retryable(self):
        options = RetryOptions()
        assert is_retryable(502, options) is True

    def test_503_is_retryable(self):
        options = RetryOptions()
        assert is_retryable(503, options) is True

    def test_504_is_retryable(self):
        options = RetryOptions()
        assert is_retryable(504, options) is True

    def test_400_not_retryable(self):
        options = RetryOptions()
        assert is_retryable(400, options) is False

    def test_401_not_retryable(self):
        options = RetryOptions()
        assert is_retryable(401, options) is False

    def test_404_not_retryable(self):
        options = RetryOptions()
        assert is_retryable(404, options) is False

    def test_custom_retryable_statuses(self):
        options = RetryOptions(retryable_statuses=[408, 503])
        assert is_retryable(408, options) is True
        assert is_retryable(503, options) is True
        assert is_retryable(500, options) is False


class TestRetryExecution:
    """Tests for retry execution with the fetch client."""

    @respx.mock
    async def test_success_after_failure(self):
        """Test that fetch retries and succeeds after initial failures."""
        route = respx.get(URL)
        route.side_effect = [
            httpx.Response(500, json={"error": "fail"}, headers={"Content-Type": "application/json"}),
            httpx.Response(200, json={"ok": True}, headers={"Content-Type": "application/json"}),
        ]

        options = FetchOptions(
            retry=RetryOptions(attempts=2, initial_interval_ms=10, jitter=0),
        )
        response = await fetch(URL, options)

        assert response.ok
        assert response.data == {"ok": True}
        assert route.call_count == 2

    @respx.mock
    async def test_retry_exhaustion(self):
        """Test that RetryError is raised when all attempts fail."""
        route = respx.get(URL)
        route.side_effect = [
            httpx.Response(500, json={"error": "fail 1"}, headers={"Content-Type": "application/json"}),
            httpx.Response(502, json={"error": "fail 2"}, headers={"Content-Type": "application/json"}),
            httpx.Response(503, json={"error": "fail 3"}, headers={"Content-Type": "application/json"}),
        ]

        options = FetchOptions(
            retry=RetryOptions(attempts=2, initial_interval_ms=10, jitter=0),
        )

        with pytest.raises(RetryError) as exc_info:
            await fetch(URL, options)

        err = exc_info.value
        assert err.attempts == 3
        assert route.call_count == 3

    @respx.mock
    async def test_non_retryable_status_not_retried(self):
        """Test that non-retryable status codes are not retried."""
        route = respx.get(URL)
        route.side_effect = [
            httpx.Response(400, json={"error": "bad request"}, headers={"Content-Type": "application/json"}),
        ]

        options = FetchOptions(
            retry=RetryOptions(attempts=3, initial_interval_ms=10, jitter=0),
        )

        with pytest.raises(HTTPResponseError) as exc_info:
            await fetch(URL, options)

        assert exc_info.value.status == 400
        assert route.call_count == 1

    @respx.mock
    async def test_retry_after_header_respected(self):
        """Test that Retry-After header is respected during retries."""
        route = respx.get(URL)
        route.side_effect = [
            httpx.Response(
                429,
                json={"error": "rate limited"},
                headers={"Content-Type": "application/json", "Retry-After": "1"},
            ),
            httpx.Response(200, json={"ok": True}, headers={"Content-Type": "application/json"}),
        ]

        options = FetchOptions(
            retry=RetryOptions(attempts=2, initial_interval_ms=10, jitter=0),
        )
        response = await fetch(URL, options)

        assert response.ok
        assert route.call_count == 2

    @respx.mock
    async def test_no_retries_when_attempts_zero(self):
        """Test that setting attempts=0 disables retries."""
        route = respx.get(URL)
        route.side_effect = [
            httpx.Response(500, json={"error": "fail"}, headers={"Content-Type": "application/json"}),
        ]

        options = FetchOptions(
            retry=RetryOptions(attempts=0, initial_interval_ms=10, jitter=0),
        )

        with pytest.raises(HTTPResponseError):
            await fetch(URL, options)

        assert route.call_count == 1

    @respx.mock
    async def test_max_interval_caps_backoff(self):
        """max_interval_ms should cap the per-retry delay."""
        options = RetryOptions(
            initial_interval_ms=1000,
            factor=10,  # base * 10^attempt grows quickly
            jitter=0,
            max_interval_ms=500,
        )
        # attempt 0: 1000 → capped at 500
        assert calculate_backoff(0, options) == pytest.approx(0.5, abs=0.01)
        # attempt 1: 10_000 → capped at 500
        assert calculate_backoff(1, options) == pytest.approx(0.5, abs=0.01)

    @respx.mock
    async def test_fast_first_skips_initial_delay(self):
        """fast_first=True should fire the first retry immediately."""
        route = respx.get(URL)
        route.side_effect = [
            httpx.Response(500, json={"error": "fail"}, headers={"Content-Type": "application/json"}),
            httpx.Response(200, json={"ok": True}, headers={"Content-Type": "application/json"}),
        ]

        options = FetchOptions(
            retry=RetryOptions(
                attempts=2,
                # Massive interval to make it obvious if delay isn't skipped
                initial_interval_ms=5_000,
                jitter=0,
                fast_first=True,
            ),
        )

        start = time.monotonic()
        response = await fetch(URL, options)
        elapsed = time.monotonic() - start

        assert response.ok
        assert route.call_count == 2
        # Without fast_first this would block ~5s. Should be far below that.
        assert elapsed < 1.0, f"fast_first did not skip initial delay (took {elapsed:.2f}s)"

    @respx.mock
    async def test_on_rejection_abort_stops_retries(self):
        """on_rejection returning ABORT surfaces the underlying error immediately."""
        route = respx.get(URL)
        route.side_effect = [
            httpx.Response(500, json={"error": "fail"}, headers={"Content-Type": "application/json"}),
            httpx.Response(200, json={"ok": True}, headers={"Content-Type": "application/json"}),
        ]

        calls: list[int] = []

        def on_rejection(ctx):
            calls.append(ctx.attempt)
            return OnRejectionDecision.abort()

        options = FetchOptions(
            retry=RetryOptions(
                attempts=3,
                initial_interval_ms=10,
                jitter=0,
                on_rejection=on_rejection,
            ),
        )

        with pytest.raises(HTTPResponseError):
            await fetch(URL, options)

        assert calls == [1]  # consulted once before the would-be first retry
        assert route.call_count == 1

    @respx.mock
    async def test_on_rejection_retry_with_delay_overrides_default(self):
        """on_rejection returning RETRY_WITH_DELAY uses that delay (not exp backoff)."""
        route = respx.get(URL)
        route.side_effect = [
            httpx.Response(500, json={"error": "fail"}, headers={"Content-Type": "application/json"}),
            httpx.Response(200, json={"ok": True}, headers={"Content-Type": "application/json"}),
        ]

        def on_rejection(_ctx):
            return OnRejectionDecision.retry_with_delay(20)  # 20ms

        options = FetchOptions(
            retry=RetryOptions(
                attempts=2,
                # Default backoff would block ~5s; the callback should override.
                initial_interval_ms=5_000,
                jitter=0,
                on_rejection=on_rejection,
            ),
        )

        start = time.monotonic()
        response = await fetch(URL, options)
        elapsed = time.monotonic() - start

        assert response.ok
        assert route.call_count == 2
        assert elapsed < 1.0, f"retry_with_delay did not override default backoff (took {elapsed:.2f}s)"

    @respx.mock
    async def test_on_rejection_skip_skips_attempt(self):
        """on_rejection returning SKIP consumes the slot but never re-fires the request."""
        route = respx.get(URL)
        route.side_effect = [
            httpx.Response(500, json={"error": "fail"}, headers={"Content-Type": "application/json"}),
        ]

        def on_rejection(_ctx):
            return OnRejectionDecision.skip()

        options = FetchOptions(
            retry=RetryOptions(
                attempts=2,
                initial_interval_ms=10,
                jitter=0,
                on_rejection=on_rejection,
            ),
        )

        with pytest.raises(RetryError):
            await fetch(URL, options)

        # initial + 2 skipped retries → only the initial real call hits the route
        assert route.call_count == 1

    @respx.mock
    async def test_multiple_retries_then_success(self):
        """Test success after multiple consecutive failures."""
        route = respx.get(URL)
        route.side_effect = [
            httpx.Response(500, json={"error": "fail"}, headers={"Content-Type": "application/json"}),
            httpx.Response(503, json={"error": "fail"}, headers={"Content-Type": "application/json"}),
            httpx.Response(502, json={"error": "fail"}, headers={"Content-Type": "application/json"}),
            httpx.Response(200, json={"ok": True}, headers={"Content-Type": "application/json"}),
        ]

        options = FetchOptions(
            retry=RetryOptions(attempts=4, initial_interval_ms=10, jitter=0),
        )
        response = await fetch(URL, options)

        assert response.ok
        assert route.call_count == 4
