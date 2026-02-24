"""Error classes for the smooai-fetch client."""

from __future__ import annotations

from typing import Any

import httpx


class FetchError(Exception):
    """Base error class for all smooai-fetch errors."""

    pass


class HTTPResponseError(FetchError):
    """Error thrown when an HTTP request fails with a non-2xx status code.

    Parses the response body to extract structured error information when available.
    """

    def __init__(self, response: httpx.Response, msg: str | None = None) -> None:
        self.response = response
        self.status: int = response.status_code
        self.status_text: str = response.reason_phrase or ""
        self.url: str = str(response.url)

        # Try to read the response body
        try:
            self.data_string: str = response.text
        except Exception:
            self.data_string = ""

        # Try to parse as JSON and extract error details
        self.data: Any = None
        self.error_message: str | None = None
        self.error_type: str | None = None
        self.error_code: str | int | None = None

        error_str = ""
        err_is_set = False

        try:
            content_type = response.headers.get("content-type", "")
            if "application/json" in content_type:
                self.data = response.json()
                data = self.data

                if isinstance(data, dict) and "error" in data:
                    error_val = data["error"]
                    if not isinstance(error_val, list):
                        if isinstance(error_val, dict):
                            if "type" in error_val:
                                self.error_type = str(error_val["type"])
                                error_str += f"({error_val['type']}): "
                                err_is_set = True
                            if "code" in error_val:
                                self.error_code = error_val["code"]
                                error_str += f"({error_val['code']}): "
                                err_is_set = True
                            if "message" in error_val:
                                self.error_message = str(error_val["message"])
                                error_str += f"{error_val['message']}"
                                err_is_set = True
                        elif isinstance(error_val, str):
                            self.error_message = error_val
                            error_str += f"{error_val}"
                            err_is_set = True

                if isinstance(data, dict) and "errorMessages" in data:
                    error_messages = data["errorMessages"]
                    if isinstance(error_messages, list):
                        joined = "; ".join(str(m) for m in error_messages)
                        error_str += joined
                        err_is_set = True
        except Exception:
            pass

        if not err_is_set:
            error_str = self.data_string or "Unknown error"

        prefix = f"{msg}; " if msg else ""
        super().__init__(f"{prefix}{error_str}; HTTP Error Response: {self.status} {self.status_text}")


class RetryError(FetchError):
    """Error thrown when all retry attempts have been exhausted."""

    def __init__(self, last_error: Exception, attempts: int) -> None:
        self.last_error = last_error
        self.attempts = attempts
        msg = f"Retry Error: Ran out of retry attempts after {attempts} attempt(s). Last error: {last_error}"
        super().__init__(msg)


class RateLimitError(FetchError):
    """Error thrown when a request is rejected by the rate limiter."""

    def __init__(self, message: str = "Rate limit exceeded") -> None:
        super().__init__(message)


class CircuitBreakerError(FetchError):
    """Error thrown when a request is rejected by the circuit breaker."""

    def __init__(self, message: str = "Circuit breaker is open") -> None:
        super().__init__(message)


class TimeoutError(FetchError):
    """Error thrown when a request exceeds the configured timeout."""

    def __init__(self, timeout_ms: float, url: str = "") -> None:
        self.timeout_ms = timeout_ms
        self.url = url
        super().__init__(f"Request timed out after {timeout_ms}ms" + (f" for {url}" if url else ""))


class SchemaValidationError(FetchError):
    """Error thrown when the response body fails schema validation."""

    def __init__(self, message: str, errors: Any = None) -> None:
        self.validation_errors = errors
        super().__init__(message)
