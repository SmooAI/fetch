"""Hook type aliases for the smooai-fetch client.

These are re-exported from _types for convenience.
"""

from smooai_fetch._types import (
    LifecycleHooks,
    PostResponseErrorHook,
    PostResponseSuccessHook,
    PreRequestHook,
)

__all__ = [
    "LifecycleHooks",
    "PostResponseErrorHook",
    "PostResponseSuccessHook",
    "PreRequestHook",
]
