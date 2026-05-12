---
'@smooai/fetch': patch
---

SMOODEV-949: Rate-limit-specific retry config in Rust + Python. Adds `RateLimitRetryOptions` (an alias for `RetryOptions`, mirroring the Go port) plus `FetchContainerOptions.rate_limit_retry` and a `with_rate_limit_retry(...)` builder method. When configured alongside a rate limiter, rate-limit rejections are retried inside a dedicated inner loop rather than consuming the main retry budget.
