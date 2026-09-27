# SmooAI.Fetch

[![NuGet](https://img.shields.io/nuget/v/SmooAI.Fetch.svg)](https://www.nuget.org/packages/SmooAI.Fetch)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**HTTP that gets out of your way for .NET 8+ — typed JSON in and out, automatic retry on transient failures, auth token injection, one error type for every non-2xx.**

.NET port of [`@smooai/fetch`](https://github.com/SmooAI/fetch). Stop writing the same retry/backoff/auth wrapper for every API client in every service. Wire-compatible semantics with the TypeScript, Python, Go, and Rust ports.

## Install

```bash
dotnet add package SmooAI.Fetch
```

## What you get

- **Typed JSON** — `GetAsync<User>` and `PostAsync<Dto, User>`. Your request and response shapes, strongly typed. No `JsonSerializer.Deserialize` boilerplate on every call site.
- **Automatic retries on transient failures, for requests that are safe to replay** — network blips, timeouts, and `408` / `429` / `5xx` responses are retried with exponential backoff + jitter for idempotent methods (`GET`, `HEAD`, `OPTIONS`, `TRACE`, `PUT`, `DELETE`). `POST` and `PATCH` are never retried unless you opt in — see [Retries are method-aware](#retries-are-method-aware).
- **`Retry-After` is honored** — when a server tells you "wait 5s", the client waits 5s instead of your default backoff. Never eat a 429 again.
- **Async auth tokens** — register an `AuthTokenProvider` once; every request picks up a fresh bearer token without restarting `HttpClient`.
- **One typed error per non-2xx** — catch `HttpResponseError` and you've got status, body, headers, URI, and method on one exception.
- **Per-request cancellation + timeout** — linked `CancellationTokenSource` under the hood; every method takes a `CancellationToken`.
- **DI-ready** — `AddSmooFetch(options => …)` plugs into `IHttpClientFactory` and your `IServiceCollection`.

## Quick start — standalone

```csharp
using SmooAI.Fetch;

var fetch = SmooFetch.Create(options =>
{
    options.BaseUrl           = "https://api.example.com";
    options.Timeout           = TimeSpan.FromSeconds(30);
    options.RetryPolicy       = RetryPolicy.ExponentialBackoff(maxRetries: 3);
    options.AuthTokenProvider = async ct => await GetBearerTokenAsync(ct);
});

var me      = await fetch.GetAsync<User>("/users/me");
var created = await fetch.PostAsync<CreateUserDto, User>("/users", dto);
```

## Quick start — DI / `IHttpClientFactory`

```csharp
builder.Services.AddSmooFetch(options =>
{
    options.BaseUrl           = builder.Configuration["Api:BaseUrl"];
    options.RetryPolicy       = RetryPolicy.ExponentialBackoff(maxRetries: 3);
    options.AuthTokenProvider = _ => Task.FromResult<string?>(bearerToken);
});

// Inject wherever you need it
public class BillingService(SmooFetch fetch)
{
    public Task<Invoice> GetInvoice(string id) =>
        fetch.GetAsync<Invoice>($"/invoices/{id}");
}
```

## Retry policy — honors `Retry-After`

```csharp
options.RetryPolicy = RetryPolicy.ExponentialBackoff(
    maxRetries: 3,
    baseDelay:  TimeSpan.FromMilliseconds(250),
    maxDelay:   TimeSpan.FromSeconds(10));
```

- Retries on transient exceptions (timeouts, socket errors) and on `408` / `429` / `500` / `502` / `503` / `504` — for retry-eligible requests only (below).
- Honors the `Retry-After` header on `429` / `503` — if the server says "wait 5s", the client waits 5s instead of your backoff.
- Exponential backoff with jitter; bounded by `maxDelay`.

## Retries are method-aware

A `POST` that timed out, or came back `429` / `5xx`, may already have run on the server. Sending it again can charge a card twice, send a message twice, or bill a second generated image. So a failed attempt is retried only when the request is **retry-eligible**:

- its method is idempotent per RFC 9110 §9.2.2 — `GET`, `HEAD`, `OPTIONS`, `TRACE`, `PUT`, `DELETE`; **or**
- the policy sets `AllowNonIdempotent = true`; **or**
- the request carries a non-empty `Idempotency-Key` header (`RetryPolicy.IdempotencyKeyHeader`), i.e. the server has promised to de-duplicate replays.

An ineligible request makes exactly **one** attempt. `OnRejection` is not consulted, and the outcome surfaces as-is: the non-2xx response or the original exception. That includes a `429` with `Retry-After` on a `POST`: it is still not replayed unless you opt in. Eligibility is decided on the final request, after the `AuthTokenProvider` and `PreRequest` hook run, so a hook can add the `Idempotency-Key`. The in-process rate limiter is unaffected, because it waits before anything is sent.

```csharp
// This endpoint is safe to replay: opt the whole client in.
options.RetryPolicy = RetryPolicy.Default with { AllowNonIdempotent = true };

// Or opt in one request, with a key the server de-duplicates on.
using var request = new HttpRequestMessage(HttpMethod.Post, "/payments") { Content = body };
request.Headers.Add(RetryPolicy.IdempotencyKeyHeader, paymentId);
using var response = await fetch.SendAsync(request);
```

> **Changed in 4.0.0.** Through 3.x every method was retried, `POST` included.

## Typed errors — one `catch` per layer

```csharp
try
{
    var user = await fetch.GetAsync<User>("/users/me");
}
catch (HttpResponseError ex)
{
    // ex.StatusCode  — int
    // ex.Body        — string (response body)
    // ex.Headers     — HttpResponseHeaders
    // ex.RequestUri  — Uri
    // ex.Method      — HttpMethod
}
```

`HttpResponseError` is thrown for every non-2xx response. Transient exceptions (timeouts, socket resets) surface as their original type if retries are exhausted.

## Async auth token provider

Auth is fetched **per request** via `AuthTokenProvider: async ct => …` — pair this with a cached/rotating token source and every call gets a fresh `Authorization` header without re-registering the client.

```csharp
options.AuthTokenProvider = async ct =>
{
    var token = await _tokenCache.GetOrRefreshAsync(ct);
    return token; // null => no Authorization header on this request
};
```

## Cancellation + per-request timeout

Every method accepts a `CancellationToken`. The configured `Timeout` applies **per attempt**: each attempt runs under its own `CancellationTokenSource`, linked to yours. When the timeout fires, `HttpClient` aborts the in-flight request and closes its connection before any retry starts, so the server sees the cancel. A timed-out attempt is never left running alongside its retry.

```csharp
using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(5));
var user = await fetch.GetAsync<User>("/users/me", cancellationToken: cts.Token);
```

## Related

- [`@smooai/fetch`](https://www.npmjs.com/package/@smooai/fetch) — TypeScript / Node
- [`smooai-fetch`](https://crates.io/crates/smooai-fetch) — Rust
- [`smooai-fetch`](https://pypi.org/project/smooai-fetch/) — Python
- [`github.com/SmooAI/fetch/go/fetch/v3`](https://github.com/SmooAI/fetch/tree/main/go/fetch) — Go

## License

MIT — © SmooAI
