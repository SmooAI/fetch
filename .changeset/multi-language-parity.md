---
'@smooai/fetch': major
---

Implement fetch library in Python, Rust, and Go

- Python: httpx-based async client with custom circuit breaker, sliding window rate limiter, retry with Retry-After, pydantic schema validation, builder pattern (105 tests)
- Rust: reqwest-based async client with custom circuit breaker, sliding window rate limiter, retry with exponential backoff + jitter, thiserror errors, builder pattern (94 tests)
- Go: net/http client with sony/gobreaker circuit breaker, sliding window rate limiter, retry with Retry-After, builder pattern (76 tests)
