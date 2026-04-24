# @smooai/fetch

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
