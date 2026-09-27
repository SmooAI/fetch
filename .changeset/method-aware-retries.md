---
'@smooai/fetch': major
---

SMOODEV-3375: Retries never duplicate a side effect: they now check the HTTP method, and timeouts cancel the attempt. This changes a default, so it is a major release.

**What was wrong.** Retries ignored the HTTP method. A `POST` that timed out or got a 429/5xx was sent again, up to twice more. In TypeScript the per-attempt timeout (mollitia's `Timeout` module) did not stop the losing request, so the first attempt kept running on the server while the retry sent it again. Image generation takes 20-50s against the 10s default timeout, so it billed three images and returned nothing. Any non-idempotent call (CRM writes, message sends, payments) could run its side effect more than once.

**New default, in all five languages:**

- **Only idempotent methods are retried:** `GET`, `HEAD`, `OPTIONS`, `TRACE`, `PUT` and `DELETE` (RFC 9110 §9.2.2).
- **`POST`, `PATCH` and any other method make exactly one attempt.** You get that attempt's own error (`HTTPResponseError` / `TimeoutError` and the equivalents in other languages), not the retries-exhausted wrapper.
- **A 429 with `Retry-After` on a `POST` is not retried either.**
- **`onRejection` is never called for these requests**, so it cannot turn their retries back on.

There are two ways to opt back in:

- Send a non-empty `Idempotency-Key` header, in any casing. The server then deduplicates.
- Set `retry: { allowNonIdempotent: true }`. The name in each language:
    - TypeScript: `allowNonIdempotent`
    - Python and Rust: `allow_non_idempotent`
    - Go and .NET: `AllowNonIdempotent`

Eligibility is checked after the pre-request hooks and the auth provider run, so a hook can add the key. The client-side rate limiter's retry loop is unchanged, because it rejects requests before anything is sent.

**Timeouts cancel the attempt.** TypeScript now aborts each attempt with an `AbortController`, combined with the caller's own `signal`, so the connection is closed before any retry. A `fetch` that ignores `signal` still times out on schedule. Rust, Go, Python and .NET already cancelled the attempt. A new test in every language proves the first attempt's connection closes. Go now also waits for the cancelled attempt to finish before it retries. Python now puts a hard deadline on each attempt with `asyncio.timeout`. Before, httpx only had per-phase timeouts, so a server that kept sending bytes slowly never timed out.

**Also fixed:** in TypeScript, a request the caller aborted is no longer retried.

**Other API changes:**

- **TypeScript:**
    - New exports: `isIdempotentMethod`, `isRetryEligible`, `IDEMPOTENCY_KEY_HEADER` and the `RetryOptions` type.
    - `options.retry` and `FetchBuilder.withRetry` now take a partial. It is merged over the defaults, so `retry: { allowNonIdempotent: true }` keeps every other default.
    - The `signal` that `fetch` receives now combines the caller's signal with the timeout's. It is no longer the caller's own object.
- **Rust:**
    - `RetryOptions` gains `allow_non_idempotent`. This breaks existing `RetryOptions { .. }` struct literals.
    - `RetryOptions` now implements `Default`, so literals can end with `..Default::default()` from now on.
    - New: `is_idempotent_method`, `is_retry_eligible` and `IDEMPOTENCY_KEY_HEADER`.
- **Go:**
    - New: `RetryOptions.AllowNonIdempotent`, `IsIdempotentMethod` and `IdempotencyKeyHeader`.
    - The module path moves to `/v4`.
- **Python:**
    - New: `RetryOptions.allow_non_idempotent`.
    - `is_idempotent_method` and `IDEMPOTENCY_KEY_HEADER` are exported.
- **.NET:**
    - New on `RetryPolicy`: `AllowNonIdempotent`, `IsIdempotentMethod`, `IsRetryEligible` and `IdempotencyKeyHeader`.
    - `Microsoft.SourceLink.GitHub` goes to 10.0.303. 8.0.0 pulls in `Microsoft.Build.Tasks.Git` 8.0.0, which has an advisory against it (GHSA-23fw-v26w-5fgq), and a clean restore now fails with NU1902.

**Why a major version:**

- A caller that relied on `POST` retries quietly loses them.
- In Rust, adding a field to a struct that callers construct breaks their code. The 3.7.1 entry below explains why that must not ship as a minor.

The shared `spec/retry-idempotency-corpus.json` pins the rule. Each language's tests run it against a real local server and count the requests the server received.
