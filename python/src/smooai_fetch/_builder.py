"""Fluent builder for configuring fetch instances."""

from __future__ import annotations

from typing import Any, TypeVar

from pydantic import BaseModel

from smooai_fetch._client import fetch as _fetch
from smooai_fetch._defaults import DEFAULT_RETRY_OPTIONS
from smooai_fetch._response import FetchResponse
from smooai_fetch._types import (
    CircuitBreakerOptions,
    FetchContainerOptions,
    FetchOptions,
    LifecycleHooks,
    PostResponseErrorHook,
    PostResponseSuccessHook,
    PreRequestHook,
    RateLimitOptions,
    RetryOptions,
    TimeoutOptions,
)

T = TypeVar("T")


class FetchBuilder:
    """Builder class for creating configured fetch instances with retry,
    rate limiting, and circuit breaking.

    Provides a fluent interface for configuring fetch options.

    Example::

        builder = FetchBuilder()
        builder = (
            builder
            .with_retry(RetryOptions(attempts=3))
            .with_timeout(5000)
            .with_rate_limit(RateLimitOptions(max_requests=10, window_ms=60000))
            .with_headers({"Authorization": "Bearer token"})
        )
        response = await builder.fetch("https://api.example.com/data")
    """

    def __init__(self) -> None:
        self._retry: RetryOptions | None = None
        self._timeout: TimeoutOptions | None = None
        self._rate_limit: RateLimitOptions | None = None
        self._circuit_breaker: CircuitBreakerOptions | None = None
        self._schema: type[BaseModel] | None = None
        self._headers: dict[str, str] = {}
        self._hooks: LifecycleHooks = LifecycleHooks()

    def with_retry(self, options: RetryOptions | None = None) -> FetchBuilder:
        """Configure retry behavior.

        Args:
            options: Retry configuration. Defaults to DEFAULT_RETRY_OPTIONS if None.

        Returns:
            The builder instance for method chaining.
        """
        self._retry = options if options is not None else DEFAULT_RETRY_OPTIONS
        return self

    def with_timeout(self, timeout_ms: float) -> FetchBuilder:
        """Set the request timeout.

        Args:
            timeout_ms: Timeout duration in milliseconds.

        Returns:
            The builder instance for method chaining.
        """
        self._timeout = TimeoutOptions(timeout_ms=timeout_ms)
        return self

    def with_rate_limit(self, options: RateLimitOptions) -> FetchBuilder:
        """Configure rate limiting.

        Args:
            options: Rate limit configuration.

        Returns:
            The builder instance for method chaining.
        """
        self._rate_limit = options
        return self

    def with_circuit_breaker(self, options: CircuitBreakerOptions) -> FetchBuilder:
        """Configure circuit breaker behavior.

        Args:
            options: Circuit breaker configuration.

        Returns:
            The builder instance for method chaining.
        """
        self._circuit_breaker = options
        return self

    def with_schema(self, schema: type[BaseModel]) -> FetchBuilder:
        """Set a Pydantic model for response validation.

        Args:
            schema: The Pydantic model class to validate against.

        Returns:
            The builder instance for method chaining.
        """
        self._schema = schema
        return self

    def with_headers(self, headers: dict[str, str]) -> FetchBuilder:
        """Set default headers for all requests.

        Args:
            headers: Dictionary of header name-value pairs.

        Returns:
            The builder instance for method chaining.
        """
        self._headers.update(headers)
        return self

    def with_auth(self, token: str, scheme: str = "Bearer") -> FetchBuilder:
        """Set an authorization header.

        Args:
            token: The authentication token.
            scheme: The auth scheme (default: "Bearer").

        Returns:
            The builder instance for method chaining.
        """
        self._headers["Authorization"] = f"{scheme} {token}"
        return self

    def with_pre_request_hook(self, hook: PreRequestHook) -> FetchBuilder:
        """Set a pre-request hook.

        Args:
            hook: Function called before each request.

        Returns:
            The builder instance for method chaining.
        """
        self._hooks.pre_request = hook
        return self

    def with_post_response_success_hook(self, hook: PostResponseSuccessHook) -> FetchBuilder:
        """Set a post-response success hook.

        Args:
            hook: Function called after a successful response.

        Returns:
            The builder instance for method chaining.
        """
        self._hooks.post_response_success = hook
        return self

    def with_post_response_error_hook(self, hook: PostResponseErrorHook) -> FetchBuilder:
        """Set a post-response error hook.

        Args:
            hook: Function called after a failed response.

        Returns:
            The builder instance for method chaining.
        """
        self._hooks.post_response_error = hook
        return self

    def build(self) -> FetchOptions:
        """Build the FetchOptions from the current configuration.

        Returns:
            A FetchOptions instance with all configured settings.
        """
        container_options: FetchContainerOptions | None = None
        if self._rate_limit or self._circuit_breaker:
            container_options = FetchContainerOptions(
                rate_limit=self._rate_limit,
                circuit_breaker=self._circuit_breaker,
            )

        return FetchOptions(
            headers=self._headers if self._headers else None,
            retry=self._retry,
            timeout=self._timeout,
            schema=self._schema,
            hooks=self._hooks,
            container_options=container_options,
        )

    async def fetch(
        self,
        url: str,
        method: str = "GET",
        headers: dict[str, str] | None = None,
        body: Any = None,
    ) -> FetchResponse[Any]:
        """Execute a request using the built configuration.

        Args:
            url: The URL to request.
            method: HTTP method (default: "GET").
            headers: Additional headers for this specific request.
            body: Request body.

        Returns:
            A FetchResponse containing the parsed response data.
        """
        opts = self.build()
        opts.method = method

        # Merge per-request headers with builder headers
        merged_headers = dict(self._headers) if self._headers else {}
        if headers:
            merged_headers.update(headers)
        if merged_headers:
            opts.headers = merged_headers

        if body is not None:
            opts.body = body

        return await _fetch(url, opts)
