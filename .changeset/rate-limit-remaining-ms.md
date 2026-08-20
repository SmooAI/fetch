---
'@smooai/fetch': patch
---

The Rust `SlidingWindowRateLimiter` no longer reports `remaining_ms: 0` when it rejects a request. `Duration::as_millis` truncates, so any sub-millisecond remainder came back as exactly zero — an error telling the caller to wait no time at all while refusing to serve them, which makes a caller that honors `remaining_ms` spin on the window boundary. It now rounds up to the next whole millisecond, and `acquire()`'s compensating `+ 1` is gone.
