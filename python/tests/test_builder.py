"""Tests for the FetchBuilder fluent API."""

import httpx
import respx
from pydantic import BaseModel

from smooai_fetch import (
    CircuitBreakerOptions,
    FetchBuilder,
    RateLimitOptions,
    RetryOptions,
)

URL = "https://api.example.com/data"


class ItemResponse(BaseModel):
    id: str
    name: str


class TestFetchBuilderChaining:
    """Tests for fluent method chaining on FetchBuilder."""

    def test_with_retry(self):
        """Test that with_retry returns the builder for chaining."""
        builder = FetchBuilder()
        result = builder.with_retry(RetryOptions(attempts=3))
        assert result is builder

    def test_with_timeout(self):
        """Test that with_timeout returns the builder for chaining."""
        builder = FetchBuilder()
        result = builder.with_timeout(5000)
        assert result is builder

    def test_with_rate_limit(self):
        """Test that with_rate_limit returns the builder for chaining."""
        builder = FetchBuilder()
        result = builder.with_rate_limit(RateLimitOptions(max_requests=10, window_ms=60000))
        assert result is builder

    def test_with_circuit_breaker(self):
        """Test that with_circuit_breaker returns the builder for chaining."""
        builder = FetchBuilder()
        result = builder.with_circuit_breaker(CircuitBreakerOptions())
        assert result is builder

    def test_with_schema(self):
        """Test that with_schema returns the builder for chaining."""
        builder = FetchBuilder()
        result = builder.with_schema(ItemResponse)
        assert result is builder

    def test_with_headers(self):
        """Test that with_headers returns the builder for chaining."""
        builder = FetchBuilder()
        result = builder.with_headers({"X-Custom": "value"})
        assert result is builder

    def test_with_auth(self):
        """Test that with_auth returns the builder for chaining."""
        builder = FetchBuilder()
        result = builder.with_auth("my-token")
        assert result is builder

    def test_full_chain(self):
        """Test full method chaining."""
        builder = (
            FetchBuilder()
            .with_retry(RetryOptions(attempts=3))
            .with_timeout(5000)
            .with_rate_limit(RateLimitOptions(max_requests=10, window_ms=60000))
            .with_circuit_breaker(CircuitBreakerOptions())
            .with_headers({"X-Custom": "value"})
            .with_auth("my-token")
        )
        assert isinstance(builder, FetchBuilder)


class TestFetchBuilderBuild:
    """Tests for FetchBuilder.build()."""

    def test_build_with_retry(self):
        """Test that build produces options with retry configured."""
        opts = FetchBuilder().with_retry(RetryOptions(attempts=5)).build()
        assert opts.retry is not None
        assert opts.retry.attempts == 5

    def test_build_with_timeout(self):
        """Test that build produces options with timeout configured."""
        opts = FetchBuilder().with_timeout(3000).build()
        assert opts.timeout is not None
        assert opts.timeout.timeout_ms == 3000

    def test_build_with_headers(self):
        """Test that build produces options with headers."""
        opts = FetchBuilder().with_headers({"X-Foo": "bar"}).build()
        assert opts.headers is not None
        assert opts.headers["X-Foo"] == "bar"

    def test_build_with_auth(self):
        """Test that build produces options with auth header."""
        opts = FetchBuilder().with_auth("my-token").build()
        assert opts.headers is not None
        assert opts.headers["Authorization"] == "Bearer my-token"

    def test_build_with_custom_auth_scheme(self):
        """Test that build produces options with custom auth scheme."""
        opts = FetchBuilder().with_auth("my-key", scheme="ApiKey").build()
        assert opts.headers is not None
        assert opts.headers["Authorization"] == "ApiKey my-key"

    def test_build_with_rate_limit(self):
        """Test that build includes container options for rate limit."""
        opts = FetchBuilder().with_rate_limit(RateLimitOptions(max_requests=5, window_ms=1000)).build()
        assert opts.container_options is not None
        assert opts.container_options.rate_limit is not None
        assert opts.container_options.rate_limit.max_requests == 5

    def test_build_with_circuit_breaker(self):
        """Test that build includes container options for circuit breaker."""
        opts = FetchBuilder().with_circuit_breaker(CircuitBreakerOptions(failure_threshold=3)).build()
        assert opts.container_options is not None
        assert opts.container_options.circuit_breaker is not None
        assert opts.container_options.circuit_breaker.failure_threshold == 3

    def test_build_with_schema(self):
        """Test that build includes schema."""
        opts = FetchBuilder().with_schema(ItemResponse).build()
        assert opts.schema is ItemResponse

    def test_build_default(self):
        """Test that build with no configuration returns sensible defaults."""
        opts = FetchBuilder().build()
        assert opts.retry is None
        assert opts.timeout is None
        assert opts.headers is None
        assert opts.schema is None
        assert opts.container_options is None


class TestFetchBuilderEndToEnd:
    """End-to-end tests using FetchBuilder.fetch()."""

    @respx.mock
    async def test_basic_fetch(self):
        """Test a basic fetch through the builder."""
        respx.get(URL).mock(
            return_value=httpx.Response(
                200,
                json={"id": "1", "name": "item"},
                headers={"Content-Type": "application/json"},
            )
        )

        builder = FetchBuilder().with_timeout(5000)
        response = await builder.fetch(URL)

        assert response.ok
        assert response.data == {"id": "1", "name": "item"}

    @respx.mock
    async def test_fetch_with_headers(self):
        """Test that builder headers are sent in the request."""
        route = respx.get(URL).mock(
            return_value=httpx.Response(
                200,
                json={"ok": True},
                headers={"Content-Type": "application/json"},
            )
        )

        builder = FetchBuilder().with_headers({"X-Custom": "value"}).with_auth("test-token")
        response = await builder.fetch(URL)

        assert response.ok
        request = route.calls[0].request
        assert request.headers["X-Custom"] == "value"
        assert request.headers["Authorization"] == "Bearer test-token"

    @respx.mock
    async def test_fetch_post_with_body(self):
        """Test a POST request through the builder."""
        respx.post(URL).mock(
            return_value=httpx.Response(
                201,
                json={"id": "new"},
                headers={"Content-Type": "application/json"},
            )
        )

        builder = FetchBuilder().with_headers({"Content-Type": "application/json"})
        response = await builder.fetch(URL, method="POST", body={"name": "test"})

        assert response.ok
        assert response.status_code == 201

    @respx.mock
    async def test_fetch_with_per_request_headers(self):
        """Test that per-request headers merge with builder headers."""
        route = respx.get(URL).mock(
            return_value=httpx.Response(
                200,
                json={"ok": True},
                headers={"Content-Type": "application/json"},
            )
        )

        builder = FetchBuilder().with_headers({"X-Base": "base-value"})
        response = await builder.fetch(URL, headers={"X-Request": "request-value"})

        assert response.ok
        request = route.calls[0].request
        assert request.headers["X-Base"] == "base-value"
        assert request.headers["X-Request"] == "request-value"
