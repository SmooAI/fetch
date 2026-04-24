# SmooAI.Fetch

**Resilient HTTP client for .NET 8+ — Polly-backed retry, typed JSON, async auth, `Retry-After` honoring, and a single typed error type for every non-2xx.**

.NET port of [`@smooai/fetch`](https://github.com/SmooAI/fetch). Built on `HttpClientFactory` + [Polly](https://github.com/App-vNext/Polly). Wire-compatible semantics with the TypeScript, Python, Go, and Rust ports.

## Install

```bash
dotnet add package SmooAI.Fetch
```

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
- Honors the `Retry-After` header on `429` / `503` — if the server says "wait 5s", Polly waits 5s instead of your backoff.
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
