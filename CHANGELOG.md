# @smooai/fetch

## 3.7.0

### Minor Changes

- 43ca80a: Make redirect handling configurable in all five languages

    Redirects were followed unconditionally everywhere, and TypeScript went further:
    `merge({}, init, { redirect: 'follow' })` put the literal last, so a caller
    passing `redirect: 'manual'` had it **silently overwritten**. Python hardcoded
    `follow_redirects=True` into the httpx kwargs; Rust, Go and .NET set nothing and
    inherited platform defaults that follow up to 10 hops.

    That is a security gap, not just an ergonomic one. A caller who resolves a
    hostname and checks it against an SSRF allowlist has that guard defeated by a 302
    to an internal address, because the check was performed on the original host. And
    RFC 8461 forbids following redirects when fetching an MTA-STS policy.
    - **TypeScript** — `redirect` is honoured (defaults first, caller last)
    - **Python** — `FetchOptions.follow_redirects`
    - **Rust** — `RequestInit.follow_redirects: Option<bool>` (`None` inherits, so a
      client-level default survives a per-request `..Default::default()`)
    - **Go** — `ClientBuilder.WithFollowRedirects`, applied to a caller-supplied
      `*http.Client` too
    - **.NET** — `SmooFetchOptions.FollowRedirects` / `WithFollowRedirects`

    Honouring the option was not sufficient on its own: in TS, Rust, Go and .NET a
    3xx is neither "ok" nor "redirected", so it was raised as an error and the option
    was undone a line later. Each now returns a deliberately-unfollowed 3xx as an
    ordinary response. Defaults are unchanged — everything still follows unless a
    caller says otherwise.

## 3.6.2

### Patch Changes

- 8781ce8: Two places where a green result meant nothing.

    The Go test lane ran `go test` without `-count=1`. Go's test cache does not invalidate on a fixture read from _outside_ the package directory, and the connect-timeout suite loads `spec/connect-timeout-corpus.json` from the repo root — so a deliberately corrupted corpus still returned `ok (cached)`.

    The release workflow gated the PyPI, crates.io, Go-tag and NuGet publishes on `steps.changesets.outputs.published == 'true'` — i.e. on the npm publish succeeding in that same run. A run dying after npm left the follow-up run with no changesets to consume, `published == 'false'`, every remaining step skipped, and a green check for a release that published nothing. Those steps now gate on being a publish run and are individually idempotent, a `concurrency` group stops the workflow racing itself, and a final step fails the run if the released version is not live on npm, PyPI, crates.io and the Go tag.

## 3.6.1

### Patch Changes

- 1b26e3f: The Rust `SlidingWindowRateLimiter` no longer reports `remaining_ms: 0` when it rejects a request. `Duration::as_millis` truncates, so any sub-millisecond remainder came back as exactly zero — an error telling the caller to wait no time at all while refusing to serve them, which makes a caller that honors `remaining_ms` spin on the window boundary. It now rounds up to the next whole millisecond, and `acquire()`'s compensating `+ 1` is gone.

## 3.6.0

### Minor Changes

- 193b1e8: The Go port gains a real response-validation entry point: `RequestOptions.Validate func(data any) []string`. Returning messages fails the request with a `*SchemaValidationError` carrying them, which `DefaultRetryOptions` already treats as non-retryable. Until now `SchemaValidationError` was a type the package defined, documented as "returned when response body validation fails", and never constructed — and the README's language matrix promised callers would get one. Go has no Standard Schema equivalent, so this is the seam rather than a bundled validator. Leaving `Validate` unset is behavior-identical.

### Patch Changes

- 932f253: The Rust crate `smooai-fetch` now uses rustls instead of native-tls. `reqwest`'s `default-tls` pulled in `openssl-sys`, which needs a system OpenSSL and its headers at build time — a cross-compile and container-image hazard for a library consumed across the platform. `openssl-sys` and `native-tls` are gone from the lockfile. `http2` and `charset` are re-enabled explicitly, because disabling reqwest's default features would otherwise have silently downgraded every consumer to HTTP/1.1.

## 3.5.1

### Patch Changes

- cd9f9ed: CI runs one job per language instead of a single serial `validate` job, so a failure in one port no longer hides the verdict of the other four. Two test-visibility gaps closed alongside it: `vitest --passWithNoTests` is gone (an empty TypeScript suite went green), and the Rust lane now runs `--all-features`, which is what actually compiles `tests/trace_propagation_tests.rs` — three real trace-propagation tests sat behind `#![cfg(feature = "otel")]` and reported "0 passed; ok" to a bare `cargo test`.

## 3.5.0

### Minor Changes

- a5434b0: Add an optional, default-off connect timeout to all five ports. It bounds only the connection-establishment phase, so a black-holed connect — a SYN to a dead pod IP still lingering in a ClusterIP's iptables — fails in ~that window and the configured retry can land on a live endpoint, instead of stalling until the whole-request timeout. Slow-but-alive handlers are unaffected, and leaving it unset preserves the previous behavior exactly.
    - TypeScript: `connectTimeoutMs` / `FetchBuilder.withConnectTimeout(ms)`. Node only, via an undici `Agent` dispatcher; `undici` is an **optional peer dependency**, imported lazily and only when a connect timeout is requested. Ignored in browser/worker builds, which expose no such knob.
    - Python: `TimeoutOptions(connect_timeout_ms=...)`, mapped to `httpx.Timeout(connect=...)`.
    - Rust: `FetchOptions::connect_timeout_ms` / `FetchBuilder::with_connect_timeout`, mapped to `reqwest`'s `connect_timeout`.
    - Go: `ClientBuilder.WithConnectTimeout(d)`, applied to a cloned default transport's dialer. A caller-supplied `*http.Client` is left untouched.
    - .NET: `SmooFetchOptions.ConnectTimeout` / `SmooFetchBuilder.WithConnectTimeout(ts)`, mapped to `SocketsHttpHandler.ConnectTimeout`. Not applied to `IHttpClientFactory`-owned handlers, which own their own handler.

    All five regression tests read their knobs from `spec/connect-timeout-corpus.json` so the timing thresholds cannot drift apart per language.

### Patch Changes

- 5c8c71e: Drop two runtime dependencies from the published package.

    `@faker-js/faker` — a test-data generator — was a **runtime** dependency, imported at module load, solely to build cosmetic names for the internal mollitia modules (`smooai-fetch-retry-blue-cat`). Those names only need to be unique within the process, so they come from a counter now.

    `@standard-schema/utils` was declared but never imported by anything in `src/`; it reaches this package transitively through `@smooai/utils`, which declares it itself.

    The remaining runtime dependencies (`mollitia`, `lodash.merge`, `@smooai/logger`, `@smooai/utils`, `@standard-schema/spec`) are each load-bearing and stay.

## 3.4.2

### Patch Changes

- d854c7c: Fix the release pipeline so shipped artifacts carry the version they claim, and give the Go module a resolvable path.

    `version:sync` ran _after_ `changeset publish`, mutating manifests in the CI workspace that were never committed — so every git tag shipped stale version constants (`go/fetch/v3.3.10` contained `const Version = "2.1.2"`) and `cargo publish --allow-dirty` existed only to tolerate the dirt. The sync now runs inside `changeset version`, so the bumped manifests land in the release commit; `--allow-dirty` is gone; and `node scripts/sync-versions.mjs --check` runs in CI as a guard that fails loudly on any skew, including a pattern that stopped matching.

    The Go module path gains the `/v3` major suffix Go requires above v1. Import `github.com/SmooAI/fetch/go/fetch/v3` (package identifier is still `fetch`); tags through `go/fetch/v3.4.0` predate the suffix and do not resolve. The suffix is derived from `package.json`'s major and is covered by the same guard, so a future major bump cannot leave it behind.

## 3.4.1

### Patch Changes

- 3f2cbd4: SMOODEV-2716: Redact credentials from everything the client logs. `@smooai/logger` performs no redaction of its own, so the `Authorization` header, the raw query string, the full URL in the log message and the request body were all reaching CloudWatch in plaintext on every request. A new internal `redact` module scrubs headers, query strings, URLs (including userinfo passwords) and bodies (url-encoded, JSON string and object forms) before they are handed to the logger; the request sent on the wire is unchanged. The Rust client gets the same treatment for the URL on its `Sending HTTP request` tracing event. Both suites load the shared cases in `spec/redaction-corpus.json`. Python, Go and .NET log nothing about a request and so are unaffected.

## 3.4.0

### Minor Changes

- 2c20134: Rust: optional `otel` feature that injects W3C trace context (`traceparent`) into
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

## 3.3.10

### Patch Changes

- d174206: Migrate build tooling from tsup to tsdown — faster, oxc-based, drop-in replacement. The `esbuild-plugin-alias` shim used to swap `@smooai/logger` Node entries for browser variants is replaced with `@rollup/plugin-alias` (rolldown-compatible). Output extensions shift from `.js`/`.mjs`/`.d.ts` to `.cjs`/`.mjs`/`.d.cts`/`.d.mts` (tsdown defaults); the `exports` map is updated to match. No public API change.

## 3.3.9

### Patch Changes

- 3607d13: SMOODEV-968: .NET — add sliding-window rate limiter to `SmooFetchBuilder.WithRateLimit(maxRequests, window, onRejected?)`. Built on `System.Threading.RateLimiting.SlidingWindowRateLimiter` so state is shared across every call on the constructed `SmooFetch`, matching the Rust / Go ports. Requests acquire a permit before dispatch and the optional `OnRejected` callback fires for every would-be rejection for observability. Closes the parity gap left open by SMOODEV-946.
- 23e86e9: SMOODEV-969: Python — share the sliding-window rate-limiter state across `fetch()` calls made through a single `FetchBuilder`. Previously `_client.fetch()` reconstructed the limiter per call, defeating the cross-call rate limit. The builder now lazily constructs one `SlidingWindowRateLimiter`, hands the same instance to every `fetch()` it dispatches, and rebuilds it when the caller changes options via `with_rate_limit`. A new `SlidingWindowRateLimiter.acquire_wait()` method blocks until a slot is free (mirroring the Rust port's `acquire` loop) so successive builder-mediated calls naturally queue instead of raising `RateLimitError`. The low-level `fetch()` entrypoint retains its raise-on-full `acquire()` semantics for back-compat with `rate_limit_retry` plumbing.

## 3.3.8

### Patch Changes

- 62e0c22: SMOODEV-946: .NET port — close parity sweep. Adds `SmooFetchBuilder` fluent API, Polly-based circuit breaker, lifecycle hooks (`PreRequest` / `PostRequestOk` / `PostRequestErr`), `OnRejection` retry callback with `OnRejectionDecision` (`Retry` / `RetryWithDelay` / `Abort` / `Skip` / `Default`), and `FastFirst` on `RetryPolicy`. Existing `SmooFetchOptions` + `SmooFetch.Create` factory remain for backwards compatibility. Rate limiter is parked as a follow-up.
- 148364b: SMOODEV-948: Async auth-token provider across TS, Python, Rust, Go. Adds a first-class hook that's invoked before every request to mint / refresh an auth token (sync or async), with the resulting `Authorization` header injected using a configurable scheme (default `Bearer`). Mirrors the existing .NET `AuthTokenProvider` delegate.
- ab2588b: SMOODEV-950: Circuit breaker — rate-based detection + `on_state_change` callback in Rust/Python/Go. Adds `failure_rate_threshold` + `sliding_window_size` for rate-based tripping (Python, Rust) and an `on_state_change` callback that fires on every state transition (Python, Rust, Go-builder). Mirrors the TS `failureRateThreshold` + `onStateChange` surface.

## 3.3.7

### Patch Changes

- 5fa920a: SMOODEV-949: Rate-limit-specific retry config in Rust + Python. Adds `RateLimitRetryOptions` (an alias for `RetryOptions`, mirroring the Go port) plus `FetchContainerOptions.rate_limit_retry` and a `with_rate_limit_retry(...)` builder method. When configured alongside a rate limiter, rate-limit rejections are retried inside a dedicated inner loop rather than consuming the main retry budget.

## 3.3.6

### Patch Changes

- 620e2db: SMOODEV-947: Python port — close SMOODEV-627 retry parity. Add `on_rejection` callback (`RETRY` / `RETRY_WITH_DELAY` / `ABORT` / `SKIP` / `DEFAULT`), `fast_first` (skip the initial retry delay), and `max_interval_ms` (cap on per-retry delay) to `RetryOptions`. Brings the Python port in line with Rust + Go.

## 3.3.5

### Patch Changes

- e464834: SMOODEV-928: Bump `@smooai/logger` to `^4.1.4` and `@smooai/utils` to `^1.3.3`. Picks up the ESM `__filename` TDZ fix from logger 4.1.4 across the runtime dep graph (utils itself was on logger 3.x prior to 1.3.3). Also drops the deprecated `baseUrl: "./"` from tsconfig (TS 5.9+/6.x emit TS5101 with `ignoreDeprecations: "5.0"`); fetch has no `paths` entries so this is a no-op for type resolution.

## 3.3.4

### Patch Changes

- 9c9375d: SMOODEV-667: Fix release pipeline so PyPI + crates.io + NuGet actually publish. `pnpm build` produces a Python wheel at the pre-sync version (the Cargo/pyproject bumps happen later, inside `ci:publish`), so the publish step was trying to re-upload the stale wheel and getting rejected. Clean `dist/` before `uv run poe publish` so only the freshly-built version ships. Drop `--locked` from the cargo publish step because sync-versions only updates `Cargo.toml` (not `Cargo.lock`), which would trip `--locked` as soon as crates.io is reached. Net effect: `SmooAI.Fetch` NuGet package publishes for the first time; PyPI advances from the stalled 3.0.0.

## 3.3.3

### Patch Changes

- affe721: SMOODEV-666: Multi-target the SmooAI.Fetch NuGet package to `net8.0;net9.0;net10.0` so consumers on every current .NET LTS + STS release get a native `lib/` folder match. Polly v8, Microsoft.Extensions.Http, and Microsoft.Extensions.Http.Polly all resolve cleanly on all three TFMs — no per-TFM conditionals needed.

## 3.3.2

### Patch Changes

- 9cf41be: SMOODEV-664: Rewrite the .NET (NuGet) README to value-frame the package — lead with "HTTP that gets out of your way": typed JSON, automatic retries on transient failures, auth token injection, one error type per non-2xx. Drop the "Polly-backed" implementation lead. Republishes SmooAI.Fetch with the new README.

## 3.3.1

### Patch Changes

- 203479e: SMOODEV-662: Sync SmooAI.Fetch NuGet version to package.json + polish NuGet README

## 3.3.0

### Minor Changes

- 2662911: Add SmooAI.Fetch NuGet package — .NET 8+ port of @smooai/fetch with Polly-based retry (exponential backoff + jitter + Retry-After support), per-request timeout, HttpClientFactory integration, typed JSON helpers, async auth token provider, and typed HttpResponseError carrying status/body/headers.

## 3.2.0

### Minor Changes

- 0f57151: SMOODEV-627: Close TS→Rust/Go drift on retry options and builder surface. Rust + Go `RetryOptions` now match TS: `on_rejection` / `OnRejection` callback (decisions: Retry with custom delay, Abort, Skip, Default), plus `fast_first` / `FastFirst` for zero-delay first retry. Go also gets `WithRateLimitRetry(opts)` (configurable per-client rate-limit retry) and `WithContainerOptions(FetchContainerOptions)` batch setter mirroring TS's container-options ergonomics. Also gitignore `.smooai-logs/` so the pre-commit hook stops committing ephemeral test logs.

## 3.1.0

### Minor Changes

- 5d12e43: **Add top-level `browser` export condition**

    `@smooai/fetch` already shipped a browser-safe build under `./browser`, but the top-level `.` entry had no `browser` condition in the exports map. Browser bundlers (Vite, webpack with `target: 'web'`, esbuild with `platform: 'browser'`) therefore resolved `import fetch from '@smooai/fetch'` to the Node entry, pulling `@smooai/logger` + `rotating-file-stream` + other Node-only dependencies into the browser bundle.

    Adding the `browser` condition on `.` means consumers can now do:

    ```ts
    import fetch from '@smooai/fetch';
    ```

    …and the bundler automatically picks the browser-safe dist when building for a browser target. No aliasing, no explicit `/browser` subpath import required.

    Consumers that were aliasing `@smooai/fetch` → `@smooai/fetch/browser/index` as a workaround (e.g. `@smooai/config`'s tsup build) can drop that alias on upgrade.

## 3.0.2

### Patch Changes

- 001f556: Add explicit `./browser` subpath export so `import fetch from '@smooai/fetch/browser'` resolves without the trailing `/index`. The existing `./browser/*` wildcard doesn't match the bare `./browser` specifier per the Node.js exports spec — the `*` requires at least one character — so consumers previously had to write `@smooai/fetch/browser/index`, which contradicts the documented API. Adds a dedicated entry pointing at `dist/browser/index.{mjs,js,d.ts}`. The wildcard form continues to work for any future browser-side subpaths.

## 3.0.1

### Patch Changes

- ab17b63: Add Python, Rust, and Go language-specific READMEs with idiomatic usage examples, cross-language install table, and API reference.

## 3.0.0

### Major Changes

- 8c0d28b: Implement fetch library in Python, Rust, and Go
    - Python: httpx-based async client with custom circuit breaker, sliding window rate limiter, retry with Retry-After, pydantic schema validation, builder pattern (105 tests)
    - Rust: reqwest-based async client with custom circuit breaker, sliding window rate limiter, retry with exponential backoff + jitter, thiserror errors, builder pattern (94 tests)
    - Go: net/http client with sony/gobreaker circuit breaker, sliding window rate limiter, retry with Retry-After, builder pattern (76 tests)

## 2.1.2

### Patch Changes

- b9768f8: Update @smooai/logger and other smoo dependencies.
- a369ec7: Update SmooAI Packages link in README to point to smoo.ai/open-source for consistency across all SmooAI packages.

## 2.1.1

### Patch Changes

- 0f1a840: Update @smooai/logger and other smoo dependencies.

## 2.1.0

### Minor Changes

- 5893679: Update zod 3 to zod 4.

### Patch Changes

- 5893679: Update readme.

## 2.0.1

### Patch Changes

- 260482b: Update readme.

## 2.0.0

### Major Changes

- d8ed851: Changed how we exported browser for better build safety.

## 1.6.2

### Patch Changes

- efd83d6: Update smoo dependencies.

## 1.6.1

### Patch Changes

- 7de1ffa: Update smoo dependencies.

## 1.6.0

### Minor Changes

- 53a3cc7: Added Browser export.

## 1.5.0

### Minor Changes

- 8e9855f: Fix package exports.

## 1.4.2

### Patch Changes

- 361a81a: Update readme.

## 1.4.1

### Patch Changes

- d4aecdc: Fix issue with JSON error message.

## 1.4.0

### Minor Changes

- 88f6e41: Fix issue with pre-using response body and update prettier plugins.

## 1.3.0

### Minor Changes

- 937a5cd: Changed FetchBuilder to take the schema in the constructor to fix type inference.

### Patch Changes

- 937a5cd: Updated all vite dependencies.

## 1.2.1

### Patch Changes

- 081e6ff: Fix package description.

## 1.2.0

### Minor Changes

- 7cbaa0b: Add lifecycle hooks to fetch implementation and update README
    - Introduced lifecycle hooks: pre-request, post-response success, and post-response error, allowing for enhanced request and response handling.
    - Updated README with detailed descriptions of lifecycle hooks and examples demonstrating their usage.
    - Refactored fetch implementation to integrate hooks, improving flexibility and error handling capabilities.

### Patch Changes

- 7cbaa0b: Enhance README and fetch implementation with new options
    - Added detailed section on opinionated defaults for the fetch function, including retry configuration, timeout settings, and rate limit retry options.
    - Updated examples to demonstrate usage of new options in fetch requests.
    - Introduced `RequestInitWithOptions` type to support additional options in fetch requests, within the same fetch argument footprint.
    - Improved error handling and response type inference in the fetch implementation.

    This update aims to provide better guidance for users and enhance the flexibility of the fetch functionality.

## 1.1.0

### Minor Changes

- 07df8fe: Enhance fetch functionality with schema validation
    - Enhanced fetch implementation with a FetchBuilder class for better configuration options, including schema validation, retry, and rate limiting.
    - Improved error handling and logging capabilities in the fetch module.
    - Updated README to reflect new features and usage examples.

## 1.0.7

### Patch Changes

- 3503fdb: Fix index export via @smooai/utils update.

## 1.0.6

### Patch Changes

- 4277a0f: Fix package file selection."

## 1.0.5

### Patch Changes

- 4d45f19: Fix npm publishing.

## 1.0.4

### Patch Changes

- 300d106: Fixed package.json for publishing.

## 1.0.3

### Patch Changes

- 8ceaebc: Updating @smooai/fetch to be its own package.

## 1.0.2

### Patch Changes

- 44fd23b: Fix publish for Github releases.

## 1.0.1

### Patch Changes

- 52c9eb1: Initial check-in.
