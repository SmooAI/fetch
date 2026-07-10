"""Tests for timeout functionality."""

import time

import httpx
import pytest
import respx

from smooai_fetch import FetchOptions, RetryOptions, TimeoutOptions, fetch
from smooai_fetch._errors import TimeoutError as FetchTimeoutError

URL = "https://api.example.com/data"


@respx.mock
async def test_timeout_raises_error():
    """Test that a slow response triggers a timeout error."""
    # Mock a slow response by using a side effect that raises TimeoutException
    respx.get(URL).mock(side_effect=httpx.ReadTimeout("Read timed out"))

    options = FetchOptions(
        timeout=TimeoutOptions(timeout_ms=100),
        retry=RetryOptions(attempts=0),
    )

    with pytest.raises(FetchTimeoutError) as exc_info:
        await fetch(URL, options)

    assert exc_info.value.timeout_ms == 100
    assert URL in exc_info.value.url


@respx.mock
async def test_request_completes_in_time():
    """Test that a fast response completes without timeout."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            json={"ok": True},
            headers={"Content-Type": "application/json"},
        )
    )

    options = FetchOptions(
        timeout=TimeoutOptions(timeout_ms=5000),
    )
    response = await fetch(URL, options)

    assert response.ok
    assert response.data == {"ok": True}


@respx.mock
async def test_default_timeout_applied():
    """Test that the default timeout is applied when not specified."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            json={"ok": True},
            headers={"Content-Type": "application/json"},
        )
    )

    # No explicit timeout options, should use default (30000ms)
    response = await fetch(URL)

    assert response.ok


@respx.mock
async def test_connect_timeout():
    """Test that connection timeouts are caught."""
    respx.get(URL).mock(side_effect=httpx.ConnectTimeout("Connect timed out"))

    options = FetchOptions(
        timeout=TimeoutOptions(timeout_ms=100),
        retry=RetryOptions(attempts=0),
    )

    with pytest.raises(FetchTimeoutError):
        await fetch(URL, options)


async def test_connect_timeout_fails_fast_on_black_hole():
    """A bounded connect timeout fails fast on a black-holed connect instead of
    stalling until the (much larger) whole-request timeout.

    Mirrors the Rust `connect_timeout_fails_fast_on_black_hole` test
    (SMOODEV-2513 / fetch#88). 10.255.255.1 is a non-routable address whose SYN
    is dropped, so without a connect timeout the request hangs until the whole
    timeout. With connect_timeout_ms set, it fails in ~the connect window.
    """
    connect_timeout_ms = 500
    options = FetchOptions(
        # Whole timeout is 10x the connect timeout — if the connect timeout is
        # not honored this would take ~5s and the elapsed assertion fails.
        timeout=TimeoutOptions(timeout_ms=5000, connect_timeout_ms=connect_timeout_ms),
        retry=RetryOptions(attempts=0),
    )

    start = time.monotonic()
    with pytest.raises(FetchTimeoutError):
        await fetch("http://10.255.255.1:80/anything", options)
    elapsed = time.monotonic() - start

    assert elapsed < 3.0, (
        f"connect timeout did not fire fast: elapsed {elapsed:.2f}s (connect was {connect_timeout_ms}ms)"
    )
