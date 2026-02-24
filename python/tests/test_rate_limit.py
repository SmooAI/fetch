"""Tests for rate limiting functionality."""

import asyncio

import pytest

from smooai_fetch import RateLimitError, RateLimitOptions
from smooai_fetch._rate_limit import SlidingWindowRateLimiter


async def test_within_limit():
    """Test that requests within the rate limit are allowed."""
    limiter = SlidingWindowRateLimiter(RateLimitOptions(max_requests=5, window_ms=1000))

    # Should all succeed without error
    for _ in range(5):
        await limiter.acquire()


async def test_exceeds_limit():
    """Test that exceeding the rate limit raises RateLimitError."""
    limiter = SlidingWindowRateLimiter(RateLimitOptions(max_requests=3, window_ms=1000))

    # First 3 should succeed
    for _ in range(3):
        await limiter.acquire()

    # 4th should fail
    with pytest.raises(RateLimitError) as exc_info:
        await limiter.acquire()

    assert "Rate limit exceeded" in str(exc_info.value)


async def test_window_expiry():
    """Test that requests are allowed again after the window expires."""
    limiter = SlidingWindowRateLimiter(RateLimitOptions(max_requests=2, window_ms=200))

    # Use up the limit
    await limiter.acquire()
    await limiter.acquire()

    # Should fail now
    with pytest.raises(RateLimitError):
        await limiter.acquire()

    # Wait for window to expire
    await asyncio.sleep(0.25)

    # Should succeed again
    await limiter.acquire()


async def test_sliding_window():
    """Test the sliding window behavior - old entries are removed as time passes."""
    limiter = SlidingWindowRateLimiter(RateLimitOptions(max_requests=2, window_ms=300))

    await limiter.acquire()  # t=0
    await asyncio.sleep(0.15)
    await limiter.acquire()  # t~150ms

    # At this point, both are within the window
    with pytest.raises(RateLimitError):
        await limiter.acquire()

    # Wait for first entry to expire (t=0 + 300ms = 300ms, we're at ~150ms so wait ~160ms)
    await asyncio.sleep(0.16)

    # First entry should have expired, allowing a new request
    await limiter.acquire()


async def test_single_request_limit():
    """Test with a limit of 1 request."""
    limiter = SlidingWindowRateLimiter(RateLimitOptions(max_requests=1, window_ms=500))

    await limiter.acquire()

    with pytest.raises(RateLimitError):
        await limiter.acquire()


async def test_reset():
    """Test that reset clears the rate limiter state."""
    limiter = SlidingWindowRateLimiter(RateLimitOptions(max_requests=2, window_ms=5000))

    await limiter.acquire()
    await limiter.acquire()

    # Should fail
    with pytest.raises(RateLimitError):
        await limiter.acquire()

    # Reset
    limiter.reset()

    # Should succeed again
    await limiter.acquire()


async def test_concurrent_acquires():
    """Test concurrent acquire calls are properly serialized."""
    limiter = SlidingWindowRateLimiter(RateLimitOptions(max_requests=3, window_ms=5000))

    results = await asyncio.gather(
        limiter.acquire(),
        limiter.acquire(),
        limiter.acquire(),
        return_exceptions=True,
    )

    # All 3 should succeed (no exceptions)
    assert all(r is None for r in results)

    # The 4th should fail
    with pytest.raises(RateLimitError):
        await limiter.acquire()
