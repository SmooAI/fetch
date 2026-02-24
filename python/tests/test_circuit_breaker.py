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
