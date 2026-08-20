---
'@smooai/fetch': minor
---

Add an optional, default-off connect timeout to all five ports. It bounds only the connection-establishment phase, so a black-holed connect — a SYN to a dead pod IP still lingering in a ClusterIP's iptables — fails in ~that window and the configured retry can land on a live endpoint, instead of stalling until the whole-request timeout. Slow-but-alive handlers are unaffected, and leaving it unset preserves the previous behavior exactly.

- TypeScript: `connectTimeoutMs` / `FetchBuilder.withConnectTimeout(ms)`. Node only, via an undici `Agent` dispatcher; `undici` is an **optional peer dependency**, imported lazily and only when a connect timeout is requested. Ignored in browser/worker builds, which expose no such knob.
- Python: `TimeoutOptions(connect_timeout_ms=...)`, mapped to `httpx.Timeout(connect=...)`.
- Rust: `FetchOptions::connect_timeout_ms` / `FetchBuilder::with_connect_timeout`, mapped to `reqwest`'s `connect_timeout`.
- Go: `ClientBuilder.WithConnectTimeout(d)`, applied to a cloned default transport's dialer. A caller-supplied `*http.Client` is left untouched.
- .NET: `SmooFetchOptions.ConnectTimeout` / `SmooFetchBuilder.WithConnectTimeout(ts)`, mapped to `SocketsHttpHandler.ConnectTimeout`. Not applied to `IHttpClientFactory`-owned handlers, which own their own handler.

All five regression tests read their knobs from `spec/connect-timeout-corpus.json` so the timing thresholds cannot drift apart per language.
