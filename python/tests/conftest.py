"""Shared fixtures for smooai-fetch tests."""

import pytest


@pytest.fixture
def base_url() -> str:
    """Base URL used across tests."""
    return "https://api.example.com"
