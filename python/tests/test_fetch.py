"""Tests for core fetch functionality: GET, POST, PUT, DELETE, JSON parsing, error handling."""

import json

import httpx
import pytest
import respx

from smooai_fetch import FetchOptions, HTTPResponseError, fetch
from smooai_fetch._types import RetryOptions

URL = "https://api.example.com/data"


@respx.mock
async def test_basic_get():
    """Test a basic successful GET request."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            json={"id": "123", "name": "test"},
            headers={"Content-Type": "application/json"},
        )
    )

    response = await fetch(URL)

    assert response.ok
    assert response.status_code == 200
    assert response.is_json
    assert response.data == {"id": "123", "name": "test"}


@respx.mock
async def test_basic_post():
    """Test a basic POST request with JSON body."""
    respx.post(URL).mock(
        return_value=httpx.Response(
            201,
            json={"id": "456", "created": True},
            headers={"Content-Type": "application/json"},
        )
    )

    options = FetchOptions(
        method="POST",
        headers={"Content-Type": "application/json"},
        body={"name": "new item"},
    )
    response = await fetch(URL, options)

    assert response.ok
    assert response.status_code == 201
    assert response.data == {"id": "456", "created": True}


@respx.mock
async def test_put_request():
    """Test a PUT request."""
    respx.put(URL).mock(
        return_value=httpx.Response(
            200,
            json={"id": "123", "updated": True},
            headers={"Content-Type": "application/json"},
        )
    )

    options = FetchOptions(
        method="PUT",
        headers={"Content-Type": "application/json"},
        body={"name": "updated item"},
    )
    response = await fetch(URL, options)

    assert response.ok
    assert response.status_code == 200
    assert response.data["updated"] is True


@respx.mock
async def test_delete_request():
    """Test a DELETE request."""
    respx.delete(URL).mock(return_value=httpx.Response(204))

    options = FetchOptions(method="DELETE")
    response = await fetch(URL, options)

    assert response.ok
    assert response.status_code == 204
    assert response.is_json is False


@respx.mock
async def test_json_parsing():
    """Test JSON response parsing with Content-Type header."""
    data = {"items": [1, 2, 3], "total": 3}
    respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            json=data,
            headers={"Content-Type": "application/json; charset=utf-8"},
        )
    )

    response = await fetch(URL)

    assert response.is_json
    assert response.data == data
    # httpx may serialize JSON without spaces; just verify it's valid JSON matching the data
    assert json.loads(response.data_string) == data


@respx.mock
async def test_non_json_response():
    """Test handling of non-JSON response bodies."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            text="Hello, World!",
            headers={"Content-Type": "text/plain"},
        )
    )

    response = await fetch(URL)

    assert response.ok
    assert response.is_json is False
    assert response.data is None
    assert response.data_string == ""


@respx.mock
async def test_http_error_response_basic():
    """Test that non-2xx responses raise HTTPResponseError."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            404,
            json={"error": "Not found"},
            headers={"Content-Type": "application/json"},
        )
    )

    # Disable retries to test the error directly
    options = FetchOptions(retry=RetryOptions(attempts=0))
    with pytest.raises(HTTPResponseError) as exc_info:
        await fetch(URL, options)

    err = exc_info.value
    assert err.status == 404
    assert "Not found" in str(err)


@respx.mock
async def test_http_error_with_structured_error():
    """Test error parsing with structured error response."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            400,
            json={
                "error": {
                    "type": "ValidationError",
                    "code": "INVALID_INPUT",
                    "message": "Field 'email' is required",
                }
            },
            headers={"Content-Type": "application/json"},
        )
    )

    options = FetchOptions(retry=RetryOptions(attempts=0))
    with pytest.raises(HTTPResponseError) as exc_info:
        await fetch(URL, options)

    err = exc_info.value
    assert err.status == 400
    assert err.error_type == "ValidationError"
    assert err.error_code == "INVALID_INPUT"
    assert err.error_message == "Field 'email' is required"
    assert "ValidationError" in str(err)
    assert "INVALID_INPUT" in str(err)
    assert "Field 'email' is required" in str(err)


@respx.mock
async def test_http_error_with_string_error():
    """Test error parsing with string error response."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            400,
            json={"error": "Something went wrong"},
            headers={"Content-Type": "application/json"},
        )
    )

    options = FetchOptions(retry=RetryOptions(attempts=0))
    with pytest.raises(HTTPResponseError) as exc_info:
        await fetch(URL, options)

    err = exc_info.value
    assert "Something went wrong" in str(err)


@respx.mock
async def test_http_error_with_error_messages_array():
    """Test error parsing with errorMessages array."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            400,
            json={"errorMessages": ["Error 1", "Error 2", "Error 3"]},
            headers={"Content-Type": "application/json"},
        )
    )

    options = FetchOptions(retry=RetryOptions(attempts=0))
    with pytest.raises(HTTPResponseError) as exc_info:
        await fetch(URL, options)

    err = exc_info.value
    assert "Error 1" in str(err)
    assert "Error 2" in str(err)


@respx.mock
async def test_custom_headers():
    """Test that custom headers are sent correctly."""
    route = respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            json={"ok": True},
            headers={"Content-Type": "application/json"},
        )
    )

    options = FetchOptions(
        headers={
            "Authorization": "Bearer test-token",
            "X-Custom-Header": "custom-value",
        },
    )
    response = await fetch(URL, options)

    assert response.ok
    # Verify headers were sent
    request = route.calls[0].request
    assert request.headers["Authorization"] == "Bearer test-token"
    assert request.headers["X-Custom-Header"] == "custom-value"


@respx.mock
async def test_auth_header():
    """Test authorization header."""
    route = respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            json={"authenticated": True},
            headers={"Content-Type": "application/json"},
        )
    )

    options = FetchOptions(
        headers={"Authorization": "Bearer my-secret-token"},
    )
    response = await fetch(URL, options)

    assert response.ok
    request = route.calls[0].request
    assert request.headers["Authorization"] == "Bearer my-secret-token"


@respx.mock
async def test_server_error_500():
    """Test 500 internal server error."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            500,
            json={"error": "Internal Server Error"},
            headers={"Content-Type": "application/json"},
        )
    )

    # Disable retries
    options = FetchOptions(retry=RetryOptions(attempts=0))
    with pytest.raises(HTTPResponseError) as exc_info:
        await fetch(URL, options)

    assert exc_info.value.status == 500


@respx.mock
async def test_empty_response_body():
    """Test handling of empty response body with JSON content type."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            text="",
            headers={"Content-Type": "application/json"},
        )
    )

    response = await fetch(URL)

    assert response.ok
    # Empty body cannot be parsed as JSON
    assert response.is_json is False
