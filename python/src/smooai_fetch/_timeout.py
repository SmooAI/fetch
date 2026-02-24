"""Timeout handling for the smooai-fetch client."""

from __future__ import annotations

import httpx

from smooai_fetch._types import TimeoutOptions


def create_timeout(options: TimeoutOptions) -> httpx.Timeout:
    """Create an httpx.Timeout from our TimeoutOptions.

    Args:
        options: Timeout configuration with timeout_ms.

    Returns:
        An httpx.Timeout configured with the specified timeout in seconds.
    """
    timeout_seconds = options.timeout_ms / 1000.0
    return httpx.Timeout(timeout_seconds)
