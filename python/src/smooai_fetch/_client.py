"""Core fetch client implementation for smooai-fetch."""

from __future__ import annotations

import json
from typing import Any, TypeVar

import httpx
from pydantic import BaseModel, ValidationError

from smooai_fetch._circuit_breaker import CircuitBreaker
from smooai_fetch._defaults import DEFAULT_RETRY_OPTIONS, DEFAULT_TIMEOUT_OPTIONS
from smooai_fetch._errors import (
    CircuitBreakerError,
    HTTPResponseError,
    RateLimitError,
    RetryError,
    SchemaValidationError,
)
from smooai_fetch._errors import TimeoutError as FetchTimeoutError
from smooai_fetch._rate_limit import SlidingWindowRateLimiter
from smooai_fetch._response import FetchResponse
from smooai_fetch._retry import execute_with_retry, is_retryable
from smooai_fetch._timeout import create_timeout
from smooai_fetch._types import (
    FetchOptions,
    RetryOptions,
    TimeoutOptions,
)

T = TypeVar("T")


def _build_request_kwargs(
    url: str,
    options: FetchOptions | None = None,
    timeout_options: TimeoutOptions | None = None,
) -> dict[str, Any]:
    """Build the keyword arguments for httpx request.

    Args:
        url: The URL to request.
        options: Fetch configuration options.
        timeout_options: Timeout settings.

    Returns:
        A dictionary of keyword arguments for httpx.AsyncClient.request().
    """
    opts = options or FetchOptions()
    kwargs: dict[str, Any] = {
        "method": opts.method,
        "url": url,
        "follow_redirects": True,
    }

    # Headers
    headers: dict[str, str] = {}
    if opts.headers:
        headers.update(opts.headers)
    kwargs["headers"] = headers

    # Body
    if opts.body is not None:
        if isinstance(opts.body, (dict, list)):
            kwargs["content"] = json.dumps(opts.body)
            if "Content-Type" not in headers:
                headers["Content-Type"] = "application/json"
        elif isinstance(opts.body, str):
            kwargs["content"] = opts.body
        elif isinstance(opts.body, bytes):
            kwargs["content"] = opts.body
        else:
            kwargs["content"] = str(opts.body)

    # Timeout
    effective_timeout = opts.timeout or timeout_options or DEFAULT_TIMEOUT_OPTIONS
    kwargs["timeout"] = create_timeout(effective_timeout)

    return kwargs


def _parse_response(response: httpx.Response, schema: type[BaseModel] | None = None) -> FetchResponse[Any]:
    """Parse an httpx.Response into a FetchResponse, optionally validating against a schema.

    Args:
        response: The raw httpx response.
        schema: Optional Pydantic model to validate the response data against.

    Returns:
        A FetchResponse wrapping the parsed data.

    Raises:
        SchemaValidationError: If schema validation fails.
        HTTPResponseError: If the response status is not 2xx.
    """
    is_json = False
    data: Any = None
    data_string = ""

    content_type = response.headers.get("content-type", "")
    if "application/json" in content_type:
        data_string = response.text
        try:
            parsed_data = json.loads(data_string)
            is_json = True

            if response.is_success and schema is not None:
                try:
                    validated = schema.model_validate(parsed_data)
                    data = validated
                except ValidationError as e:
                    raise SchemaValidationError(
                        f"Schema validation failed: {e}",
                        errors=e.errors(),
                    ) from e
            else:
                data = parsed_data
        except SchemaValidationError:
            raise
        except Exception:
            is_json = False

    fetch_response: FetchResponse[Any] = FetchResponse(
        response=response,
        data=data,
        is_json=is_json,
        data_string=data_string,
    )

    if response.is_success or response.is_redirect:
        return fetch_response
    else:
        # Read body for error responses if not already read
        if not data_string:
            try:
                _ = response.text
            except Exception:
                pass
        raise HTTPResponseError(response)


def _get_retry_after(error: Exception) -> float | None:
    """Extract Retry-After header value from an HTTPResponseError.

    Args:
        error: The exception to inspect.

    Returns:
        Retry-After delay in seconds, or None if not available.
    """
    if isinstance(error, HTTPResponseError):
        retry_after = error.response.headers.get("retry-after")
        if retry_after is not None:
            try:
                return float(retry_after)
            except (ValueError, TypeError):
                pass
    return None


def _should_retry_default(error: Exception, attempt: int, retry_options: RetryOptions) -> bool | float:
    """Default retry decision function.

    Args:
        error: The exception that occurred.
        attempt: The current attempt number.
        retry_options: The retry options to use.

    Returns:
        True to retry with backoff, False to not retry, or a float for custom delay.
    """
    if isinstance(error, SchemaValidationError):
        return False

    if isinstance(error, HTTPResponseError):
        status = error.status
        if is_retryable(status, retry_options):
            # Check Retry-After header
            retry_after = _get_retry_after(error)
            if retry_after is not None:
                return retry_after
            return True
        return False

    if isinstance(error, FetchTimeoutError):
        return True

    if isinstance(error, (RateLimitError, CircuitBreakerError)):
        return False

    # Retry on unknown/network errors
    if isinstance(error, httpx.TimeoutException):
        return True

    return True


async def fetch(url: str, options: FetchOptions | None = None) -> FetchResponse[Any]:
    """Execute an HTTP request with retry, timeout, rate limiting, and circuit breaking.

    Pipeline: hooks -> timeout -> rate_limit -> circuit_breaker -> retry -> execute ->
    parse -> validate_schema -> post_hooks.

    Args:
        url: The URL to request.
        options: Configuration options for the request.

    Returns:
        A FetchResponse containing the parsed response data.

    Raises:
        HTTPResponseError: For non-2xx responses (after retries exhausted).
        RetryError: When all retry attempts are exhausted.
        RateLimitError: When rate limit is exceeded.
        CircuitBreakerError: When circuit breaker is open.
        TimeoutError: When request times out.
        SchemaValidationError: When response fails schema validation.
    """
    opts = options or FetchOptions()
    retry_options = opts.retry if opts.retry is not None else DEFAULT_RETRY_OPTIONS
    timeout_options = opts.timeout or DEFAULT_TIMEOUT_OPTIONS
    schema = opts.schema
    hooks = opts.hooks
    container_options = opts.container_options

    # Build request kwargs
    request_kwargs = _build_request_kwargs(url, opts, timeout_options)
    current_url = url

    # Apply pre-request hook
    if hooks and hooks.pre_request:
        hook_result = hooks.pre_request(current_url, request_kwargs)
        if hook_result is not None:
            current_url, request_kwargs = hook_result
            request_kwargs["url"] = current_url

    # Build rate limiter
    rate_limiter: SlidingWindowRateLimiter | None = None
    if container_options and container_options.rate_limit:
        rate_limiter = SlidingWindowRateLimiter(container_options.rate_limit)

    # Build circuit breaker
    circuit_breaker: CircuitBreaker | None = None
    if container_options and container_options.circuit_breaker:
        circuit_breaker = CircuitBreaker(container_options.circuit_breaker)

    async def _execute() -> FetchResponse[Any]:
        """Inner execution: rate limit -> circuit breaker -> HTTP call -> parse."""
        # Rate limit check
        if rate_limiter is not None:
            await rate_limiter.acquire()

        async def _do_request() -> FetchResponse[Any]:
            try:
                async with httpx.AsyncClient() as client:
                    response = await client.request(**request_kwargs)
                return _parse_response(response, schema)
            except httpx.TimeoutException as e:
                raise FetchTimeoutError(
                    timeout_ms=timeout_options.timeout_ms,
                    url=current_url,
                ) from e

        # Circuit breaker wrapping
        if circuit_breaker is not None:
            return await circuit_breaker.call(_do_request)
        else:
            return await _do_request()

    # Execute with error hook wrapping
    try:
        # Wrap with retry logic
        def should_retry(error: Exception, attempt: int) -> bool | float:
            return _should_retry_default(error, attempt, retry_options)

        result = await execute_with_retry(
            func=_execute,
            options=retry_options,
            should_retry=should_retry,
            get_retry_after=_get_retry_after,
        )
    except Exception as error:
        # Apply post-response error hook
        if hooks and hooks.post_response_error:
            http_response = None
            if isinstance(error, HTTPResponseError):
                http_response = error.response
            elif isinstance(error, RetryError) and isinstance(error.last_error, HTTPResponseError):
                http_response = error.last_error.response

            hook_result = hooks.post_response_error(current_url, request_kwargs, error, http_response)
            if hook_result is not None:
                raise hook_result from error
        raise

    # Apply post-response success hook
    if hooks and hooks.post_response_success:
        hook_result = hooks.post_response_success(current_url, request_kwargs, result)
        if hook_result is not None:
            result = hook_result

    return result
