---
'@smooai/fetch': patch
---

SMOODEV-969: Python — share the sliding-window rate-limiter state across `fetch()` calls made through a single `FetchBuilder`. Previously `_client.fetch()` reconstructed the limiter per call, defeating the cross-call rate limit. The builder now lazily constructs one `SlidingWindowRateLimiter`, hands the same instance to every `fetch()` it dispatches, and rebuilds it when the caller changes options via `with_rate_limit`. A new `SlidingWindowRateLimiter.acquire_wait()` method blocks until a slot is free (mirroring the Rust port's `acquire` loop) so successive builder-mediated calls naturally queue instead of raising `RateLimitError`. The low-level `fetch()` entrypoint retains its raise-on-full `acquire()` semantics for back-compat with `rate_limit_retry` plumbing.
