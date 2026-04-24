# SmooAI.Fetch

Resilient HTTP client for .NET 8+. Port of [@smooai/fetch](https://github.com/SmooAI/fetch). Built on `HttpClientFactory` + [Polly](https://github.com/App-vNext/Polly) for retry and backoff.

## Features

- Polly-backed retry with exponential backoff + jitter
- Honors the `Retry-After` header on 429 / 503
- Per-request timeout via `CancellationTokenSource`
- Typed JSON helpers: `GetAsync<T>`, `PostAsync<TReq, TRes>`, etc.
- Async auth token provider (called per request)
- `HttpResponseError` on non-2xx with status, body, headers, and request metadata
- First-class `IHttpClientFactory` / DI registration

## Install

```
dotnet add package SmooAI.Fetch
```

## Usage

### Standalone

```csharp
using SmooAI.Fetch;

var fetch = SmooFetch.Create(options =>
{
    options.BaseUrl = "https://api.example.com";
    options.RetryPolicy = RetryPolicy.ExponentialBackoff(maxRetries: 3);
    options.Timeout = TimeSpan.FromSeconds(30);
    options.AuthTokenProvider = async ct => await GetTokenAsync(ct);
});

var me = await fetch.GetAsync<User>("/users/me");
var created = await fetch.PostAsync<CreateUserDto, User>("/users", dto);
```

### DI / HttpClientFactory

```csharp
builder.Services.AddSmooFetch(options =>
{
    options.BaseUrl = builder.Configuration["Api:BaseUrl"];
    options.RetryPolicy = RetryPolicy.ExponentialBackoff(maxRetries: 3);
    options.AuthTokenProvider = _ => Task.FromResult<string?>(bearerToken);
});

// Inject:
public class Foo(SmooFetch fetch) { ... }
```

## Error handling

Non-2xx responses throw `HttpResponseError`:

```csharp
try
{
    var user = await fetch.GetAsync<User>("/users/me");
}
catch (HttpResponseError ex)
{
    // ex.StatusCode, ex.Body, ex.Headers, ex.RequestUri
}
```

If a retry budget is exhausted on a response-based failure, the final `HttpResponseError` is thrown by the last attempt. Transient exceptions (timeouts, socket errors) surface as the original exception type once retries are exhausted.
