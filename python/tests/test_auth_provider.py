"""Tests for `FetchBuilder.with_auth_provider` / `FetchOptions.auth_token_provider`."""

import httpx
import respx

from smooai_fetch import FetchBuilder, FetchOptions, fetch

URL = "https://api.example.com/data"


@respx.mock
async def test_sync_auth_provider_injects_bearer():
    """A sync provider injects an Authorization header with the default Bearer scheme."""
    route = respx.get(URL).mock(
        return_value=httpx.Response(200, json={"ok": True}, headers={"Content-Type": "application/json"}),
    )

    fetcher = FetchBuilder().with_auth_provider(lambda: "tok-sync")
    r = await fetcher.fetch(URL)
    assert r.ok

    headers = route.calls.last.request.headers
    assert headers["authorization"] == "Bearer tok-sync"


@respx.mock
async def test_async_auth_provider_is_awaited():
    """An async provider is awaited inline before the request fires."""
    route = respx.get(URL).mock(
        return_value=httpx.Response(200, json={"ok": True}, headers={"Content-Type": "application/json"}),
    )

    call_count = 0

    async def provider() -> str:
        nonlocal call_count
        call_count += 1
        return f"tok-async-{call_count}"

    fetcher = FetchBuilder().with_auth_provider(provider)

    await fetcher.fetch(URL)
    await fetcher.fetch(URL)

    assert call_count == 2
    # Last call should carry the second token.
    headers = route.calls.last.request.headers
    assert headers["authorization"] == "Bearer tok-async-2"


@respx.mock
async def test_custom_auth_scheme():
    """The configured scheme prefixes the token."""
    route = respx.get(URL).mock(
        return_value=httpx.Response(200, json={"ok": True}, headers={"Content-Type": "application/json"}),
    )

    fetcher = FetchBuilder().with_auth_provider(lambda: "abc", scheme="Token")
    await fetcher.fetch(URL)

    assert route.calls.last.request.headers["authorization"] == "Token abc"


@respx.mock
async def test_auth_provider_via_fetch_options_directly():
    """`FetchOptions.auth_token_provider` works without the builder."""
    route = respx.get(URL).mock(
        return_value=httpx.Response(200, json={"ok": True}, headers={"Content-Type": "application/json"}),
    )

    async def provider() -> str:
        return "direct-tok"

    opts = FetchOptions(auth_token_provider=provider, auth_scheme="Bearer")
    r = await fetch(URL, opts)
    assert r.ok
    assert route.calls.last.request.headers["authorization"] == "Bearer direct-tok"


@respx.mock
async def test_auth_provider_overrides_static_auth_header():
    """The provider runs after the pre-request hook and overrides any prior Authorization header."""
    route = respx.get(URL).mock(
        return_value=httpx.Response(200, json={"ok": True}, headers={"Content-Type": "application/json"}),
    )

    fetcher = (
        FetchBuilder()
        .with_auth("stale-static-token")  # Sets Authorization header on the builder
        .with_auth_provider(lambda: "fresh-token-from-provider")
    )

    await fetcher.fetch(URL)
    assert route.calls.last.request.headers["authorization"] == "Bearer fresh-token-from-provider"
