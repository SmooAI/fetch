"""Tests for timeout functionality."""

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
