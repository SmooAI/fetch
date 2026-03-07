<!-- Improved compatibility of back to top link: See: https://github.com/othneildrew/Best-README-Template/pull/73 -->

<a name="readme-top"></a>

<!-- PROJECT LOGO -->
<br />
<div align="center">
  <a href="https://smoo.ai">
    <img src="../../../images/logo.png" alt="SmooAI Logo" />
  </a>
</div>

<!-- ABOUT THE PROJECT -->

## About SmooAI

SmooAI is an AI-powered platform for helping businesses multiply their customer, employee, and developer experience.

Learn more on [smoo.ai](https://smoo.ai)

## SmooAI Packages

Check out other SmooAI packages at [smoo.ai/open-source](https://smoo.ai/open-source)

## About smooai-fetch (Go)

**Stop writing the same retry logic over and over** - A resilient HTTP client that handles the chaos of real-world APIs, so you can focus on building features instead of handling failures.

![GitHub License](https://img.shields.io/github/license/SmooAI/fetch?style=for-the-badge)
![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/SmooAI/fetch/release.yml?style=for-the-badge)
![GitHub Repo stars](https://img.shields.io/github/stars/SmooAI/fetch?style=for-the-badge)

### Go Module

A Go port of [@smooai/fetch](https://www.npmjs.com/package/@smooai/fetch) built with idiomatic Go patterns — generics for typed responses, `context.Context` for cancellation and deadlines, and `ClientBuilder` for fluent configuration. It provides the same resilient HTTP client capabilities in a style that feels natural to Go developers.

### Why smooai-fetch?

Ever had a Go microservice pile up goroutines because a downstream API was down? Traditional `net/http` gives you full control, but leaves retry logic, circuit breaking, and rate limiting as boilerplate you write (and debug) over and over.

**smooai-fetch automatically handles:**

**For Unreliable APIs:**

- **Smart retries** - Exponential backoff with jitter to prevent thundering herds
- **Automatic timeouts** - Never hang indefinitely on slow endpoints
- **Rate limit respect** - Reads Retry-After headers and backs off intelligently
- **Circuit breaking** - Closed/Open/HalfOpen state machine stops hammering failing services
- **Context-aware** - Full `context.Context` propagation for cancellation and deadlines

**For Developer Experience:**

- **Generics** - `Fetch[T]`, `Get[T]`, `Post[T]` return `*FetchResponse[T]` with your struct already decoded
- **ClientBuilder** - Fluent builder API for reusable configured clients
- **Lifecycle hooks** - Pre-request and post-response hooks for auth and logging
- **Idiomatic errors** - Typed error types (`*HTTPResponseError`, `*RetryError`, `*TimeoutError`, etc.) that work with `errors.As`

### Install

```bash
go get github.com/SmooAI/fetch/go/fetch
```

| Language   | Package | Install |
| ---------- | ------- | ------- |
| TypeScript | [`@smooai/fetch`](https://www.npmjs.com/package/@smooai/fetch) | `pnpm add @smooai/fetch` |
| Python     | [`smooai-fetch`](https://pypi.org/project/smooai-fetch/) | `pip install smooai-fetch` |
| Rust       | [`smooai-fetch`](https://crates.io/crates/smooai-fetch) | `cargo add smooai-fetch` |
| Go         | `github.com/SmooAI/fetch/go/fetch` | `go get github.com/SmooAI/fetch/go/fetch` |

## The Power of Resilient Fetching

### Never Let a Hiccup Break Your App

Watch how smooai-fetch handles common failure scenarios:

```go
import "github.com/SmooAI/fetch/go/fetch"

type ApiData struct {
    ID    string `json:"id"`
    Value string `json:"value"`
}

// This won't crash if the API is temporarily down
resp, err := fetch.Get[ApiData](ctx, nil, "https://flaky-api.com/data", nil)
if err != nil {
    log.Fatal(err)
}

// Behind the scenes:
// Attempt 1: 500 error - waits 500ms
// Attempt 2: 503 error - waits 1000ms
// Attempt 3: 200 success!
fmt.Println(resp.Data.Value)
```

Passing `nil` as the client uses `NewClient()` with default retry and timeout settings — no setup required.

### Respect Rate Limits Automatically

No more manual retry-after parsing:

```go
type Repos []struct {
    Name string `json:"name"`
}

resp, err := fetch.Get[Repos](ctx, nil, "https://api.github.com/user/repos", nil)

// If GitHub says "slow down":
// - Sees 429 status + Retry-After: 60
// - Automatically waits 60 seconds
// - Retries and succeeds
// - Your code continues normally
```

### Production-Ready Examples

#### ClientBuilder Pattern

```go
import (
    "github.com/SmooAI/fetch/go/fetch"
    "time"
)

type User struct {
    ID    string `json:"id"`
    Name  string `json:"name"`
    Email string `json:"email"`
}

retryOpts := fetch.DefaultRetryOptions
client := fetch.NewClientBuilder().
    WithTimeout(5 * time.Second).
    WithRetry(&retryOpts).
    WithRateLimit(100, time.Minute).
    Build()

resp, err := fetch.Get[User](ctx, client, "https://api.example.com/users/123", nil)
if err != nil {
    log.Fatal(err)
}
fmt.Printf("User: %s <%s>\n", resp.Data.Name, resp.Data.Email)
```

#### POST Request with JSON Body

```go
type CreateUserRequest struct {
    Name  string `json:"name"`
    Email string `json:"email"`
}

type CreateUserResponse struct {
    ID        string `json:"id"`
    CreatedAt string `json:"created_at"`
}

resp, err := fetch.Post[CreateUserResponse](ctx, client,
    "https://api.example.com/users",
    CreateUserRequest{Name: "Alice", Email: "alice@example.com"},
    nil,
)
if err != nil {
    log.Fatal(err)
}
fmt.Println("Created user:", resp.Data.ID)
```

#### Circuit Breaking for Critical Services

```go
import (
    "errors"
    "github.com/SmooAI/fetch/go/fetch"
    "time"
)

// Stop hammering services that are clearly struggling
client := fetch.NewClientBuilder().
    WithCircuitBreaker("payment-service", &fetch.CircuitBreakerOptions{
        FailureThreshold: 5,
        SuccessThreshold: 2,
        OpenStateDelay:   30 * time.Second,
    }).
    WithTimeout(5 * time.Second).
    Build()

type ChargeResponse struct {
    TransactionID string `json:"transaction_id"`
}

resp, err := fetch.Post[ChargeResponse](ctx, client,
    "https://payment-processor.com/charge",
    chargeBody,
    nil,
)
if err != nil {
    var cbErr *fetch.CircuitBreakerError
    if errors.As(err, &cbErr) {
        // Circuit is open - service is down, fail fast instead of waiting
        return fallbackCharge(chargeBody)
    }
    return err
}
```

#### Per-Request Headers and Options

```go
opts := &fetch.RequestOptions{
    Headers: http.Header{
        "Authorization": []string{"Bearer " + token},
        "X-Request-ID":  []string{requestID},
    },
    Retry: &fetch.RetryOptions{
        Attempts: 1, // no retries for this idempotent call
    },
}

resp, err := fetch.Post[Response](ctx, client, "https://api.example.com/idempotent", body, opts)
```

## Real-World Scenarios

### Handle Authentication Globally

```go
import "net/http"

client := fetch.NewClientBuilder().
    WithBaseHeaders(http.Header{
        "Authorization": []string{"Bearer " + getToken()},
        "X-API-Key":     []string{"your-api-key"},
    }).
    Build()

// All requests automatically include the auth headers
resp, err := fetch.Get[User](ctx, client, "https://api.example.com/protected", nil)
```

### Lifecycle Hooks for Tracing

```go
hooks := &fetch.LifecycleHooks{
    PreRequest: func(url string, req *http.Request) (string, *http.Request) {
        // Add a trace ID to every outgoing request
        req.Header.Set("X-Trace-ID", newTraceID())
        return url, req
    },
    PostResponseSuccess: func(url string, req *http.Request, resp *fetch.FetchResponse[any]) *fetch.FetchResponse[any] {
        log.Printf("GET %s -> %d", url, resp.StatusCode)
        return resp
    },
    PostResponseError: func(url string, req *http.Request, err error, resp *fetch.FetchResponse[any]) error {
        log.Printf("GET %s -> ERROR: %v", url, err)
        return err
    },
}

client := fetch.NewClientBuilder().WithHooks(hooks).Build()
```

### Graceful Degradation

```go
primary := fetch.NewClientBuilder().
    WithCircuitBreaker("primary", &fetch.CircuitBreakerOptions{FailureThreshold: 3}).
    Build()

fallback := fetch.NewClientBuilder().
    WithTimeout(2 * time.Second).
    Build()

func getWeather(ctx context.Context, city string) (*WeatherResponse, error) {
    resp, err := fetch.Get[WeatherResponse](ctx, primary, "https://api1.weather.com/"+city, nil)
    if err != nil {
        var cbErr *fetch.CircuitBreakerError
        if errors.As(err, &cbErr) {
            // Seamlessly fall back to secondary service
            log.Println("Primary weather API unavailable, using fallback")
            return fetch.Get[WeatherResponse](ctx, fallback, "https://api2.weather.com/"+city, nil)
        }
        return nil, err
    }
    return resp, nil
}
```

### Using Context for Cancellation

```go
ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
defer cancel()

// Context cancellation propagates into the HTTP client —
// the request aborts immediately if the context is cancelled
resp, err := fetch.Get[Results](ctx, client, "https://api.example.com/long-running", nil)
if err != nil {
    if ctx.Err() == context.DeadlineExceeded {
        log.Println("Overall operation timed out")
    }
}
```

## The Smart Defaults

Out of the box, smooai-fetch is configured for the real world:

**Retry Strategy:**

- 2 automatic retries on failure
- Exponential backoff: 500ms -> 1s -> 2s
- Jitter to prevent thundering herds
- Only retries on network errors or 5xx responses

**Timeout Protection:**

- 10-second default timeout
- Prevents indefinite hangs on slow endpoints
- Configurable per client or per request

**Rate Limit Handling:**

- Respects Retry-After headers from 429 responses
- Sliding window rate limiter
- Prevents API ban hammers

## API Reference

### Top-Level Generic Functions

| Function | Description |
| -------- | ----------- |
| `Fetch[T](ctx, client, method, url, body, opts)` | Generic request with any method |
| `Get[T](ctx, client, url, opts)` | GET request |
| `Post[T](ctx, client, url, body, opts)` | POST request |
| `Put[T](ctx, client, url, body, opts)` | PUT request |
| `Patch[T](ctx, client, url, body, opts)` | PATCH request |
| `Delete[T](ctx, client, url, opts)` | DELETE request |
| `SimpleGet(ctx, client, url, opts)` | GET with untyped `any` response |
| `SimplePost(ctx, client, url, body, opts)` | POST with untyped `any` response |

### `ClientBuilder` Methods

| Method | Description |
| ------ | ----------- |
| `NewClientBuilder()` | Create builder with default retry + timeout |
| `.WithHTTPClient(c)` | Use a custom `*http.Client` |
| `.WithBaseHeaders(headers)` | Set headers included in every request |
| `.WithTimeout(duration)` | Set request timeout |
| `.WithNoTimeout()` | Disable timeout |
| `.WithRetry(opts)` | Configure retry behavior |
| `.WithNoRetry()` | Disable retries |
| `.WithRateLimit(maxRequests, period)` | Configure sliding-window rate limiter |
| `.WithCircuitBreaker(name, opts)` | Configure circuit breaker |
| `.WithHooks(hooks)` | Set lifecycle hooks |
| `.Build()` | Build the `*Client` |

### Error Handling

```go
import (
    "errors"
    "github.com/SmooAI/fetch/go/fetch"
)

resp, err := fetch.Get[Data](ctx, client, "https://api.example.com/data", nil)
if err != nil {
    var httpErr *fetch.HTTPResponseError
    var retryErr *fetch.RetryError
    var timeoutErr *fetch.TimeoutError
    var rateLimitErr *fetch.RateLimitError
    var cbErr *fetch.CircuitBreakerError

    switch {
    case errors.As(err, &httpErr):
        fmt.Printf("HTTP %d: %s\n", httpErr.StatusCode, httpErr.Message)
    case errors.As(err, &retryErr):
        fmt.Printf("Failed after %d attempts: %v\n", retryErr.Attempts, retryErr.Cause)
    case errors.As(err, &timeoutErr):
        fmt.Printf("Timed out after %v\n", timeoutErr.Timeout)
    case errors.As(err, &rateLimitErr):
        fmt.Printf("Rate limited, retry after %v\n", rateLimitErr.RetryAfter)
    case errors.As(err, &cbErr):
        fmt.Println("Circuit breaker open - service is down")
    default:
        fmt.Printf("Unexpected error: %v\n", err)
    }
}
```

### `FetchResponse[T]` Fields

```go
type FetchResponse[T any] struct {
    StatusCode int         // HTTP status code
    Status     string      // HTTP status text (e.g. "200 OK")
    Headers    http.Header // Response headers
    Data       T           // Decoded JSON body (zero value if not JSON)
    IsJSON     bool        // Whether response had JSON content-type
    BodyRaw    []byte      // Raw response body bytes
    OK         bool        // true if StatusCode is 2xx
}
```

## Built With

- Go 1.21+ - Generics for type-safe responses
- Standard `net/http` - No external HTTP client dependency
- `context.Context` - Full cancellation and deadline propagation
- Sliding window rate limiter
- Circuit breaker state machine (Closed/Open/HalfOpen)

## Related Packages

- [`@smooai/fetch`](https://www.npmjs.com/package/@smooai/fetch) - TypeScript/JavaScript version
- [`smooai-fetch` (Python)](https://pypi.org/project/smooai-fetch/) - Python version
- [`smooai-fetch` (Rust)](https://crates.io/crates/smooai-fetch) - Rust version

## Development

### Running Tests

```bash
go test ./...
```

### Running Tests with Race Detection

```bash
go test -race ./...
```

### Linting

```bash
golangci-lint run
```

<!-- CONTACT -->

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Contact

Brent Rager

- [Email](mailto:brent@smoo.ai)
- [LinkedIn](https://www.linkedin.com/in/brentrager/)
- [BlueSky](https://bsky.app/profile/brentragertech.bsky.social)

Smoo Github: [https://github.com/SmooAI](https://github.com/SmooAI)

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## License

MIT © SmooAI
