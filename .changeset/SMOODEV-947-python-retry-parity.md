---
'@smooai/fetch': patch
---

SMOODEV-947: Python port — close SMOODEV-627 retry parity. Add `on_rejection` callback (`RETRY` / `RETRY_WITH_DELAY` / `ABORT` / `SKIP` / `DEFAULT`), `fast_first` (skip the initial retry delay), and `max_interval_ms` (cap on per-retry delay) to `RetryOptions`. Brings the Python port in line with Rust + Go.
