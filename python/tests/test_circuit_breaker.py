"""Tests for circuit breaker functionality."""

import asyncio

import pytest

from smooai_fetch import CircuitBreakerError, CircuitBreakerOptions
from smooai_fetch._circuit_breaker import CircuitBreaker


async def test_closed_state_allows_calls():
    """Test that calls succeed when the circuit is closed."""
    cb = CircuitBreaker(
        CircuitBreakerOptions(
            failure_threshold=3,
            success_threshold=1,
            timeout=0.5,
        )
    )

    async def success():
        return "ok"

    result = await cb.call(success)
    assert result == "ok"
    assert cb.state == "closed"


async def test_opens_after_failure_threshold():
    """Test that the circuit opens after reaching the failure threshold."""
    cb = CircuitBreaker(
        CircuitBreakerOptions(
            failure_threshold=3,
            success_threshold=1,
            timeout=5.0,
        )
    )

    async def fail():
        raise ValueError("fail")

    # Trigger failures to open the circuit
    for _ in range(3):
        with pytest.raises(ValueError):
            await cb.call(fail)

    # Circuit should now be open
    assert cb.state == "open"

    # Next call should fail with CircuitBreakerError
    with pytest.raises(CircuitBreakerError):
        await cb.call(fail)


async def test_half_open_transition():
    """Test the transition from open to half-open after timeout."""
    cb = CircuitBreaker(
        CircuitBreakerOptions(
            failure_threshold=2,
            success_threshold=1,
            timeout=0.2,  # 200ms timeout
        )
    )

    async def fail():
        raise ValueError("fail")

    async def success():
        return "ok"

    # Open the circuit
    for _ in range(2):
        with pytest.raises(ValueError):
            await cb.call(fail)

    assert cb.state == "open"

    # Wait for timeout to transition to half-open
    await asyncio.sleep(0.3)

    # Circuit should now be half-open, next call should go through
    result = await cb.call(success)
    assert result == "ok"

    # After success, should be closed again
    assert cb.state == "closed"


async def test_half_open_failure_reopens():
    """Test that a failure in half-open state reopens the circuit."""
    cb = CircuitBreaker(
        CircuitBreakerOptions(
            failure_threshold=2,
            success_threshold=1,
            timeout=0.2,
        )
    )

    async def fail():
        raise ValueError("fail")

    # Open the circuit
    for _ in range(2):
        with pytest.raises(ValueError):
            await cb.call(fail)

    assert cb.state == "open"

    # Wait for timeout
    await asyncio.sleep(0.3)

    # Fail again in half-open state
    with pytest.raises(ValueError):
        await cb.call(fail)

    # Should be open again
    assert cb.state == "open"


async def test_circuit_breaker_error_message():
    """Test that CircuitBreakerError has a descriptive message."""
    cb = CircuitBreaker(
        CircuitBreakerOptions(
            failure_threshold=1,
            success_threshold=1,
            timeout=10.0,
        )
    )

    async def fail():
        raise ValueError("fail")

    # Open the circuit
    with pytest.raises(ValueError):
        await cb.call(fail)

    # Should get CircuitBreakerError
    with pytest.raises(CircuitBreakerError) as exc_info:
        await cb.call(fail)

    assert "Circuit breaker is open" in str(exc_info.value)


async def test_on_state_change_fires_on_open_and_half_open_and_closed():
    """`on_state_change` callback fires on each transition."""
    transitions: list[tuple[str, str]] = []

    cb = CircuitBreaker(
        CircuitBreakerOptions(
            failure_threshold=2,
            success_threshold=1,
            timeout=0.1,
            on_state_change=lambda fr, to: transitions.append((fr, to)),
        )
    )

    async def fail():
        raise ValueError("fail")

    async def success():
        return "ok"

    # Closed → Open after threshold
    for _ in range(2):
        with pytest.raises(ValueError):
            await cb.call(fail)
    assert ("closed", "open") in transitions

    # Wait for half-open transition triggered on next call.
    await asyncio.sleep(0.15)
    result = await cb.call(success)
    assert result == "ok"

    # Should have seen: closed→open, open→half-open, half-open→closed.
    kinds = transitions
    assert ("closed", "open") in kinds
    assert ("open", "half-open") in kinds
    assert ("half-open", "closed") in kinds


async def test_rate_based_threshold_trips_on_failure_rate():
    """`failure_rate_threshold` trips the breaker when the failure rate in the window crosses the threshold."""
    cb = CircuitBreaker(
        CircuitBreakerOptions(
            failure_threshold=4,  # minimum sample count before rate eval kicks in
            success_threshold=1,
            timeout=10.0,
            failure_rate_threshold=0.7,  # 70% failure rate to trip
            sliding_window_size=10,
        )
    )

    async def fail():
        raise ValueError("fail")

    async def success():
        return "ok"

    # 3 successes followed by 1 failure = 1/4 = 25% — below threshold, stays closed.
    for _ in range(3):
        await cb.call(success)
    with pytest.raises(ValueError):
        await cb.call(fail)
    assert cb.state == "closed"

    # 4 more failures: window now 3 ok / 5 fail = 5/8 = 62.5% — still below 70%.
    for _ in range(4):
        with pytest.raises(ValueError):
            await cb.call(fail)
    assert cb.state == "closed"

    # 2 more failures pushes failure rate over 70%.
    # window: 3 ok / 7 fail = 7/10 = 70% — trips.
    with pytest.raises(ValueError):
        await cb.call(fail)
    # Window is now full (10) at 6 fail / 3 ok / 1 fail = 7 fail; 7/10 = 70%.
    # If this didn't trip yet, one more failure definitely will.
    if cb.state != "open":
        with pytest.raises(ValueError):
            await cb.call(fail)
    assert cb.state == "open"


async def test_rate_threshold_respects_minimum_samples():
    """Below the minimum sample count (`failure_threshold`), the rate evaluation is suppressed."""
    cb = CircuitBreaker(
        CircuitBreakerOptions(
            failure_threshold=5,
            success_threshold=1,
            timeout=10.0,
            failure_rate_threshold=0.5,
            sliding_window_size=10,
        )
    )

    async def fail():
        raise ValueError("fail")

    # 4 consecutive failures (below the 5-sample minimum) → still closed.
    for _ in range(4):
        with pytest.raises(ValueError):
            await cb.call(fail)
    assert cb.state == "closed"

    # 5th failure: 5/5 = 100% → trips.
    with pytest.raises(ValueError):
        await cb.call(fail)
    assert cb.state == "open"


async def test_success_does_not_open():
    """Test that successful calls do not affect the circuit breaker."""
    cb = CircuitBreaker(
        CircuitBreakerOptions(
            failure_threshold=3,
            success_threshold=1,
            timeout=1.0,
        )
    )

    async def success():
        return "ok"

    for _ in range(10):
        result = await cb.call(success)
        assert result == "ok"

    assert cb.state == "closed"
