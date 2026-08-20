"""Tests for timeout functionality."""

import json
import time
from pathlib import Path

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

    Knobs come from spec/connect-timeout-corpus.json, shared with the other four
    ports -- see the corpus for why they are not inlined here.
    """
    corpus = json.loads((Path(__file__).parents[2] / "spec" / "connect-timeout-corpus.json").read_text())
    options = FetchOptions(
        timeout=TimeoutOptions(
            timeout_ms=corpus["wholeRequestTimeoutMs"],
            connect_timeout_ms=corpus["connectTimeoutMs"],
        ),
        retry=RetryOptions(attempts=0),
    )

    start = time.monotonic()
    with pytest.raises(FetchTimeoutError):
        await fetch(corpus["blackHoleUrl"], options)
    elapsed_ms = (time.monotonic() - start) * 1000

    assert elapsed_ms < corpus["maxElapsedMs"], (
        f"connect timeout did not fire fast: elapsed {elapsed_ms:.0f}ms "
        f"(connect was {corpus['connectTimeoutMs']}ms, whole timeout "
        f"{corpus['wholeRequestTimeoutMs']}ms)"
    )
