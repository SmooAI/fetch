"""Response wrapper for the smooai-fetch client."""

from __future__ import annotations

from typing import Generic, TypeVar

import httpx

T = TypeVar("T")


class FetchResponse(Generic[T]):
    """A wrapper around httpx.Response that includes parsed body data and metadata.

    Attributes:
        data: The parsed response body, optionally validated against a schema.
        response: The underlying httpx.Response object.
        is_json: Whether the response body was parsed as JSON.
        data_string: The raw response body as a string.
    """

    def __init__(
        self,
        response: httpx.Response,
        data: T | None = None,
        is_json: bool = False,
        data_string: str = "",
    ) -> None:
        self.response = response
        self.data: T | None = data
        self.is_json: bool = is_json
        self.data_string: str = data_string

    @property
    def status_code(self) -> int:
        """The HTTP status code of the response."""
        return self.response.status_code

    @property
    def ok(self) -> bool:
        """Whether the response was successful (2xx status code)."""
        return self.response.is_success

    @property
    def headers(self) -> httpx.Headers:
        """The response headers."""
        return self.response.headers

    @property
    def url(self) -> str:
        """The final URL after any redirects."""
        return str(self.response.url)
