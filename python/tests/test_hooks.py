"""Tests for lifecycle hooks (pre-request, post-response-success, post-response-error)."""

import httpx
import pytest
import respx

from smooai_fetch import (
    FetchOptions,
    HTTPResponseError,
    RetryOptions,
    fetch,
)
from smooai_fetch._types import LifecycleHooks

URL = "https://api.example.com/data"


@respx.mock
async def test_pre_request_hook_modifies_url():
    """Test that a pre-request hook can modify the URL."""
    route = respx.get("https://api.example.com/data?extra=added").mock(
        return_value=httpx.Response(
            200,
            json={"ok": True},
            headers={"Content-Type": "application/json"},
        )
    )

    def pre_request(url, kwargs):
        new_url = url + "?extra=added"
        return new_url, kwargs

    options = FetchOptions(
        hooks=LifecycleHooks(pre_request=pre_request),
    )
    response = await fetch(URL, options)

    assert response.ok
    assert route.call_count == 1


@respx.mock
async def test_pre_request_hook_modifies_headers():
    """Test that a pre-request hook can add custom headers."""
    route = respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            json={"ok": True},
            headers={"Content-Type": "application/json"},
        )
    )

    def pre_request(url, kwargs):
        headers = kwargs.get("headers", {})
        headers["X-Injected"] = "hook-value"
        kwargs["headers"] = headers
        return url, kwargs

    options = FetchOptions(
        hooks=LifecycleHooks(pre_request=pre_request),
    )
    response = await fetch(URL, options)

    assert response.ok
    request = route.calls[0].request
    assert request.headers["X-Injected"] == "hook-value"


@respx.mock
async def test_pre_request_hook_returns_none():
    """Test that returning None from pre-request hook keeps originals."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            json={"ok": True},
            headers={"Content-Type": "application/json"},
        )
    )

    def pre_request(url, kwargs):
        return None  # Keep originals

    options = FetchOptions(
        hooks=LifecycleHooks(pre_request=pre_request),
    )
    response = await fetch(URL, options)

    assert response.ok


@respx.mock
async def test_post_response_success_hook():
    """Test that a post-response success hook can modify the response."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            json={"id": "123", "name": "test"},
            headers={"Content-Type": "application/json"},
        )
    )

    def post_success(url, kwargs, response):
        # Enrich response data with metadata
        if response.is_json and response.data:
            response.data["_processed"] = True
            response.data["_url"] = url
        return response

    options = FetchOptions(
        hooks=LifecycleHooks(post_response_success=post_success),
    )
    response = await fetch(URL, options)

    assert response.ok
    assert response.data["_processed"] is True
    assert response.data["_url"] == URL
    assert response.data["id"] == "123"


@respx.mock
async def test_post_response_success_hook_returns_none():
    """Test that returning None from post-success hook keeps original response."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            json={"id": "123"},
            headers={"Content-Type": "application/json"},
        )
    )

    def post_success(url, kwargs, response):
        return None  # Keep original

    options = FetchOptions(
        hooks=LifecycleHooks(post_response_success=post_success),
    )
    response = await fetch(URL, options)

    assert response.ok
    assert response.data == {"id": "123"}


@respx.mock
async def test_post_response_error_hook():
    """Test that a post-response error hook can transform the error."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            404,
            json={"error": "Not found"},
            headers={"Content-Type": "application/json"},
        )
    )

    def post_error(url, kwargs, error, response):
        if isinstance(error, HTTPResponseError):
            return ValueError(f"Custom error: {url} returned {error.status}")
        return error

    options = FetchOptions(
        retry=RetryOptions(attempts=0),
        hooks=LifecycleHooks(post_response_error=post_error),
    )

    with pytest.raises(ValueError) as exc_info:
        await fetch(URL, options)

    assert f"Custom error: {URL} returned 404" in str(exc_info.value)


@respx.mock
async def test_post_response_error_hook_returns_none():
    """Test that returning None from post-error hook keeps original error."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            500,
            json={"error": "Server error"},
            headers={"Content-Type": "application/json"},
        )
    )

    def post_error(url, kwargs, error, response):
        return None  # Keep original

    options = FetchOptions(
        retry=RetryOptions(attempts=0),
        hooks=LifecycleHooks(post_response_error=post_error),
    )

    with pytest.raises(HTTPResponseError):
        await fetch(URL, options)


@respx.mock
async def test_all_hooks_together():
    """Test pre-request, post-success, and post-error hooks working together."""
    respx.get(URL + "?ts=12345").mock(
        return_value=httpx.Response(
            200,
            json={"id": "abc"},
            headers={"Content-Type": "application/json"},
        )
    )

    hook_calls = []

    def pre_request(url, kwargs):
        hook_calls.append("pre_request")
        return url + "?ts=12345", kwargs

    def post_success(url, kwargs, response):
        hook_calls.append("post_success")
        if response.data:
            response.data["enriched"] = True
        return response

    def post_error(url, kwargs, error, response):
        hook_calls.append("post_error")
        return error

    options = FetchOptions(
        hooks=LifecycleHooks(
            pre_request=pre_request,
            post_response_success=post_success,
            post_response_error=post_error,
        ),
    )
    response = await fetch(URL, options)

    assert response.ok
    assert response.data["enriched"] is True
    assert hook_calls == ["pre_request", "post_success"]
    # post_error should NOT be called on success
