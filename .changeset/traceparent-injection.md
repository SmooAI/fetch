---
'@smooai/fetch': minor
---

Rust: optional `otel` feature that injects W3C trace context (`traceparent`) into
every outbound request, so a call made through this client continues the caller's
trace instead of starting a new root. Off by default — the crate does not link
OpenTelemetry unless you ask for it. Guards an invalid span context and never
overwrites a `traceparent` the caller set explicitly.
