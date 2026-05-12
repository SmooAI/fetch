---
'@smooai/fetch': patch
---

SMOODEV-946: .NET port — close parity sweep. Adds `SmooFetchBuilder` fluent API, Polly-based circuit breaker, lifecycle hooks (`PreRequest` / `PostRequestOk` / `PostRequestErr`), `OnRejection` retry callback with `OnRejectionDecision` (`Retry` / `RetryWithDelay` / `Abort` / `Skip` / `Default`), and `FastFirst` on `RetryPolicy`. Existing `SmooFetchOptions` + `SmooFetch.Create` factory remain for backwards compatibility. Rate limiter is parked as a follow-up.
