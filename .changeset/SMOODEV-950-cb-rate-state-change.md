---
'@smooai/fetch': patch
---

SMOODEV-950: Circuit breaker — rate-based detection + `on_state_change` callback in Rust/Python/Go. Adds `failure_rate_threshold` + `sliding_window_size` for rate-based tripping (Python, Rust) and an `on_state_change` callback that fires on every state transition (Python, Rust, Go-builder). Mirrors the TS `failureRateThreshold` + `onStateChange` surface.
