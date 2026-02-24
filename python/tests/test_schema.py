"""Tests for Pydantic schema validation on responses."""

import httpx
import pytest
import respx
from pydantic import BaseModel

from smooai_fetch import FetchOptions, SchemaValidationError, fetch

URL = "https://api.example.com/data"


class UserResponse(BaseModel):
    id: str
    name: str


class UserWithAge(BaseModel):
    id: str
    name: str
    age: int


class NestedResponse(BaseModel):
    class Preferences(BaseModel):
        theme: str
        notifications: bool

    class User(BaseModel):
        id: str
        name: str

    user: User
    preferences: Preferences
    timestamp: str


class ItemListResponse(BaseModel):
    class Item(BaseModel):
        id: str
        name: str

    items: list[Item]
    total: int


@respx.mock
async def test_valid_schema_passes():
    """Test that valid response data passes schema validation."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            json={"id": "123", "name": "Alice"},
            headers={"Content-Type": "application/json"},
        )
    )

    options = FetchOptions(schema=UserResponse)
    response = await fetch(URL, options)

    assert response.ok
    assert isinstance(response.data, UserResponse)
    assert response.data.id == "123"
    assert response.data.name == "Alice"


@respx.mock
async def test_invalid_schema_raises_error():
    """Test that invalid response data raises SchemaValidationError."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            json={"id": 123, "name": "Alice"},  # id should be str, not int
            headers={"Content-Type": "application/json"},
        )
    )

    options = FetchOptions(schema=UserResponse)

    with pytest.raises(SchemaValidationError) as exc_info:
        await fetch(URL, options)

    assert "validation failed" in str(exc_info.value).lower() or "id" in str(exc_info.value).lower()


@respx.mock
async def test_missing_required_field():
    """Test that missing required fields raise SchemaValidationError."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            json={"id": "123"},  # missing 'name' field
            headers={"Content-Type": "application/json"},
        )
    )

    options = FetchOptions(schema=UserResponse)

    with pytest.raises(SchemaValidationError):
        await fetch(URL, options)


@respx.mock
async def test_extra_fields_are_ignored():
    """Test that extra fields in the response don't cause errors."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            json={"id": "123", "name": "Alice", "extra_field": "ignored"},
            headers={"Content-Type": "application/json"},
        )
    )

    options = FetchOptions(schema=UserResponse)
    response = await fetch(URL, options)

    assert response.ok
    assert isinstance(response.data, UserResponse)
    assert response.data.id == "123"


@respx.mock
async def test_wrong_type_for_field():
    """Test type mismatch on a specific field."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            json={"id": "123", "name": "Alice", "age": "not-a-number"},
            headers={"Content-Type": "application/json"},
        )
    )

    options = FetchOptions(schema=UserWithAge)

    with pytest.raises(SchemaValidationError):
        await fetch(URL, options)


@respx.mock
async def test_nested_schema_valid():
    """Test validation with nested Pydantic models."""
    data = {
        "user": {"id": "1", "name": "Bob"},
        "preferences": {"theme": "dark", "notifications": True},
        "timestamp": "2024-03-20T12:00:00Z",
    }
    respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            json=data,
            headers={"Content-Type": "application/json"},
        )
    )

    options = FetchOptions(schema=NestedResponse)
    response = await fetch(URL, options)

    assert response.ok
    assert isinstance(response.data, NestedResponse)
    assert response.data.user.name == "Bob"
    assert response.data.preferences.theme == "dark"


@respx.mock
async def test_array_schema_valid():
    """Test validation with array fields in the schema."""
    data = {
        "items": [
            {"id": "1", "name": "Item A"},
            {"id": "2", "name": "Item B"},
        ],
        "total": 2,
    }
    respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            json=data,
            headers={"Content-Type": "application/json"},
        )
    )

    options = FetchOptions(schema=ItemListResponse)
    response = await fetch(URL, options)

    assert response.ok
    assert isinstance(response.data, ItemListResponse)
    assert len(response.data.items) == 2
    assert response.data.total == 2


@respx.mock
async def test_schema_not_applied_on_error_response():
    """Test that schema validation is NOT applied to error responses."""
    from smooai_fetch._types import RetryOptions

    respx.get(URL).mock(
        return_value=httpx.Response(
            404,
            json={"error": "not found"},
            headers={"Content-Type": "application/json"},
        )
    )

    options = FetchOptions(
        schema=UserResponse,
        retry=RetryOptions(attempts=0),
    )

    from smooai_fetch import HTTPResponseError

    with pytest.raises(HTTPResponseError) as exc_info:
        await fetch(URL, options)

    # Should raise HTTPResponseError, not SchemaValidationError
    assert exc_info.value.status == 404


@respx.mock
async def test_non_json_response_with_schema():
    """Test that non-JSON responses skip schema validation."""
    respx.get(URL).mock(
        return_value=httpx.Response(
            200,
            text="plain text response",
            headers={"Content-Type": "text/plain"},
        )
    )

    options = FetchOptions(schema=UserResponse)
    response = await fetch(URL, options)

    # Should succeed without schema validation
    assert response.ok
    assert response.is_json is False
    assert response.data is None
