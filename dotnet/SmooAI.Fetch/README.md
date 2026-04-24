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
- **Automatic retries on transient failures** — network blips, timeouts, and `408` / `425` / `429` / `5xx` responses are retried with exponential backoff + jitter.
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
    maxRetries:     3,
    initialDelay:   TimeSpan.FromMilliseconds(250),
    maxDelay:       TimeSpan.FromSeconds(10),
    backoffFactor:  2.0,
    jitter:         true);
```

- Retries on transient exceptions (timeouts, socket errors) and on `408` / `425` / `429` / `500` / `502` / `503` / `504`.
- Honors the `Retry-After` header on `429` / `503` — if the server says "wait 5s", the client waits 5s instead of your backoff.
- Exponential backoff with jitter; bounded by `maxDelay`.

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

Every method accepts a `CancellationToken`. The configured `Timeout` creates a linked `CancellationTokenSource` under the hood:

```csharp
using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(5));
var user = await fetch.GetAsync<User>("/users/me", cancellationToken: cts.Token);
```

## Related

- [`@smooai/fetch`](https://www.npmjs.com/package/@smooai/fetch) — TypeScript / Node
- [`smooai-fetch`](https://crates.io/crates/smooai-fetch) — Rust
- [`smooai-fetch`](https://pypi.org/project/smooai-fetch/) — Python
- [`github.com/SmooAI/fetch/go/fetch`](https://github.com/SmooAI/fetch/tree/main/go/fetch) — Go

## License

MIT — © SmooAI
