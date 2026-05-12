---
'@smooai/fetch': patch
---

SMOODEV-968: .NET — add sliding-window rate limiter to `SmooFetchBuilder.WithRateLimit(maxRequests, window, onRejected?)`. Built on `System.Threading.RateLimiting.SlidingWindowRateLimiter` so state is shared across every call on the constructed `SmooFetch`, matching the Rust / Go ports. Requests acquire a permit before dispatch and the optional `OnRejected` callback fires for every would-be rejection for observability. Closes the parity gap left open by SMOODEV-946.
