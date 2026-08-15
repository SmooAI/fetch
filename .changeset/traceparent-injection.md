---
'@smooai/fetch': minor
---

Rust: optional `otel` feature that injects W3C trace context (`traceparent`) into
every outbound request, so a call made through this client continues the caller's
trace instead of starting a new root. Off by default — the crate does not link
OpenTelemetry unless you ask for it. Guards an invalid span context and never
overwrites a `traceparent` the caller set explicitly.

TypeScript: the same injection at the same place — the single-request site, so every
retry carries a current traceparent — behind an optional `@opentelemetry/api` peer
dependency. Without it installed (or without a registered SDK) it is a no-op, not a
crash, and no all-zero `traceparent` is ever emitted.

Python: the same injection at the same place — the single-request site, so retries carry
a current traceparent — behind an optional `smooai-fetch[otel]` extra. Without
`opentelemetry-api` installed it is a no-op, not an import error.

Go: the same injection at the same place — `executeHTTPRequest`, the single-request
site inside the retry/timeout/breaker wrappers, so every attempt carries a current
traceparent. Uses the OTel global propagator, which defaults to a no-op, so a
service that never configured one sends nothing extra. Never overwrites a
caller-set `traceparent`, and emits nothing at all when there is no valid span
context.

.NET: no change needed — `HttpClient`'s `DiagnosticsHandler` already injects
`traceparent` from the current `Activity`, ahead of redirects and connection
pooling, and this client keeps that handler chain intact. Tests were added to pin
that behaviour so a future custom primary handler cannot silently drop it.
