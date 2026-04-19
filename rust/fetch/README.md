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

## About smooai-fetch (Rust)

**Stop writing the same retry logic over and over** - A resilient HTTP client that handles the chaos of real-world APIs, so you can focus on building features instead of handling failures.

![Crates.io Version](https://img.shields.io/crates/v/smooai-fetch?style=for-the-badge)
![Crates.io Downloads](https://img.shields.io/crates/d/smooai-fetch?style=for-the-badge)
![Crates.io License](https://img.shields.io/crates/l/smooai-fetch?style=for-the-badge)

![GitHub License](https://img.shields.io/github/license/SmooAI/fetch?style=for-the-badge)
![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/SmooAI/fetch/release.yml?style=for-the-badge)
![GitHub Repo stars](https://img.shields.io/github/stars/SmooAI/fetch?style=for-the-badge)

### Rust Crate

A Rust port of [@smooai/fetch](https://www.npmjs.com/package/@smooai/fetch) that mirrors the feature set of the TypeScript and Python versions. Built on `reqwest` and `tokio`, it exposes a type-safe, async-first API with the builder pattern, automatic retries with exponential backoff, sliding-window rate limiting, and a circuit breaker state machine — all idiomatic Rust.

### Why smooai-fetch?

Ever had a Rust service grind to a halt because a downstream API was flaky for 30 seconds? Traditional `reqwest` gives you the HTTP primitives but leaves retry logic, timeouts, and circuit breaking as an exercise for the reader.

**smooai-fetch automatically handles:**

**For Unreliable APIs:**

- **Smart retries** - Exponential backoff with jitter to prevent thundering herds
- **Automatic timeouts** - Never hang indefinitely on slow endpoints
- **Rate limit respect** - Reads Retry-After headers and backs off intelligently
- **Circuit breaking** - Closed/Open/HalfOpen state machine stops hammering failing services
- **Serde integration** - Responses deserialize directly into your types

**For Developer Experience:**

- **Type-safe responses** - `FetchResponse<T>` deserializes JSON into your `Deserialize` types
- **Builder pattern** - `FetchBuilder<T>` fluent API for reusable configured clients
- **Lifecycle hooks** - Pre-request and post-response hooks for auth and logging
- **Tokio-native** - Fully async, no blocking I/O

### Install

```toml
[dependencies]
smooai-fetch = "2"
```

or via cargo:

```bash
cargo add smooai-fetch
```

| Language   | Package                                                        | Install                                   |
| ---------- | -------------------------------------------------------------- | ----------------------------------------- |
| TypeScript | [`@smooai/fetch`](https://www.npmjs.com/package/@smooai/fetch) | `pnpm add @smooai/fetch`                  |
| Python     | [`smooai-fetch`](https://pypi.org/project/smooai-fetch/)       | `pip install smooai-fetch`                |
| Rust       | [`smooai-fetch`](https://crates.io/crates/smooai-fetch)        | `cargo add smooai-fetch`                  |
| Go         | `github.com/SmooAI/fetch/go/fetch`                             | `go get github.com/SmooAI/fetch/go/fetch` |

## The Power of Resilient Fetching

### Never Let a Hiccup Break Your App

Watch how smooai-fetch handles common failure scenarios:

```rust
use smooai_fetch::{fetch, types::{RequestInit, Method}};

#[derive(serde::Deserialize, Clone, Debug)]
struct ApiData {
    id: String,
    value: String,
}

// This won't crash if the API is temporarily down
let init = RequestInit { method: Method::GET, ..Default::default() };
let response = fetch::<ApiData>("https://flaky-api.com/data", init).await?;

// Behind the scenes:
// Attempt 1: 500 error - waits 500ms
// Attempt 2: 503 error - waits 1000ms
// Attempt 3: 200 success!
println!("Got: {:?}", response.data);
```

### Respect Rate Limits Automatically

No more manual retry-after parsing:

```rust
let init = RequestInit { method: Method::GET, ..Default::default() };
let response = fetch::<serde_json::Value>("https://api.github.com/user/repos", init).await?;

// If GitHub says "slow down":
// - Sees 429 status + Retry-After: 60
// - Automatically waits 60 seconds
// - Retries and succeeds
// - Your code continues normally
```

### Production-Ready Examples

#### FetchBuilder Pattern

```rust
use smooai_fetch::builder::FetchBuilder;
use smooai_fetch::defaults::default_retry_options;
use smooai_fetch::types::{RequestInit, Method};
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
struct User {
    id: String,
    name: String,
    email: String,
}

let client = FetchBuilder::<User>::new()
    .with_timeout(5000)                    // 5 second timeout
    .with_retry(default_retry_options())   // exponential backoff
    .with_rate_limit(100, 60_000)          // 100 requests per minute
    .build();

let init = RequestInit { method: Method::GET, ..Default::default() };
let response = client.fetch("https://api.example.com/users/123", init).await?;

if let Some(user) = &response.data {
    println!("User: {} <{}>", user.name, user.email);
}
```

#### Circuit Breaking for Critical Services

```rust
use smooai_fetch::builder::FetchBuilder;
use smooai_fetch::error::FetchError;
use smooai_fetch::types::{RequestInit, Method};

// Stop hammering services that are clearly struggling
let client = FetchBuilder::<serde_json::Value>::new()
    .with_circuit_breaker(
        5,       // failure_threshold: open after 5 consecutive failures
        2,       // success_threshold: close after 2 successes in half-open
        30_000,  // open_state_delay_ms: try again after 30s
    )
    .with_timeout(5000)
    .build();

let init = RequestInit { method: Method::POST, ..Default::default() };
match client.fetch("https://payment-processor.com/charge", init).await {
    Ok(response) => { /* success */ }
    Err(FetchError::CircuitBreaker) => {
        // Circuit is open - service is down, fail fast instead of waiting
        eprintln!("Payment service unavailable, using fallback");
    }
    Err(e) => return Err(e.into()),
}
```

#### Default Headers and Authentication

```rust
use smooai_fetch::builder::FetchBuilder;
use smooai_fetch::types::{RequestInit, Method};
use std::collections::HashMap;

let mut default_headers = HashMap::new();
default_headers.insert("Authorization".to_string(), "Bearer your-token".to_string());
default_headers.insert("X-API-Key".to_string(), "your-api-key".to_string());

let client = FetchBuilder::<serde_json::Value>::new()
    .with_init(RequestInit {
        method: Method::GET,
        headers: default_headers,
        ..Default::default()
    })
    .build();

// All requests automatically include the auth headers
let init = RequestInit { method: Method::GET, ..Default::default() };
let response = client.fetch("https://api.example.com/protected", init).await?;
```

## Real-World Scenarios

### Lifecycle Hooks for Logging and Tracing

```rust
use smooai_fetch::builder::FetchBuilder;
use smooai_fetch::hooks::PreRequestHook;
use std::sync::Arc;

let pre_hook: PreRequestHook = Arc::new(|url, mut init| {
    // Add a correlation ID to every outgoing request
    init.headers.insert(
        "X-Correlation-ID".to_string(),
        uuid::Uuid::new_v4().to_string(),
    );
    (url.to_string(), init)
});

let client = FetchBuilder::<serde_json::Value>::new()
    .with_pre_request_hook(pre_hook)
    .build();
```

### Per-Request Option Overrides

```rust
use smooai_fetch::types::{FetchOptions, TimeoutOptions, RequestInit, Method};

// Override options for a single request
let options = FetchOptions {
    timeout: Some(TimeoutOptions { timeout_ms: 2000 }),  // tighter timeout
    retry: None,                                          // no retries for this call
};

let init = RequestInit { method: Method::GET, ..Default::default() };
let response = client.fetch_with_options(
    "https://api.example.com/fast",
    init,
    options,
).await?;
```

### Graceful Degradation

```rust
use smooai_fetch::builder::FetchBuilder;
use smooai_fetch::error::FetchError;
use smooai_fetch::types::{RequestInit, Method};

let primary = FetchBuilder::<serde_json::Value>::new()
    .with_circuit_breaker(3, 1, 30_000)
    .build();

let fallback = FetchBuilder::<serde_json::Value>::new()
    .with_timeout(2000)
    .build();

async fn get_weather(city: &str) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let init = RequestInit { method: Method::GET, ..Default::default() };
    match primary.fetch(&format!("https://api1.weather.com/{city}"), init.clone()).await {
        Ok(resp) => Ok(resp.data.unwrap_or_default()),
        Err(FetchError::CircuitBreaker) => {
            // Seamlessly fall back to secondary service
            let resp = fallback.fetch(&format!("https://api2.weather.com/{city}"), init).await?;
            Ok(resp.data.unwrap_or_default())
        }
        Err(e) => Err(e.into()),
    }
}
```

## The Smart Defaults

Out of the box, smooai-fetch is configured for the real world:

**Retry Strategy:**

- 2 automatic retries on failure
- Exponential backoff: 500ms -> 1s -> 2s
- Jitter to prevent thundering herds
- Only retries on network errors, timeouts, or 5xx responses

**Timeout Protection:**

- 10-second default timeout
- Prevents indefinite hangs on slow endpoints
- Configurable per request or per client

**Rate Limit Handling:**

- Respects Retry-After headers from 429 responses
- Sliding window rate limiter
- Prevents API ban hammers

## API Reference

### Top-Level `fetch` Function

Convenience function for one-off requests with default options:

```rust
use smooai_fetch::{fetch, types::{RequestInit, Method}};

let init = RequestInit { method: Method::GET, ..Default::default() };
let response = fetch::<MyType>("https://api.example.com/data", init).await?;
```

### `FetchBuilder<T>` Methods

| Method                                           | Description                                 |
| ------------------------------------------------ | ------------------------------------------- |
| `FetchBuilder::new()`                            | Create builder with default retry + timeout |
| `.with_timeout(ms)`                              | Set timeout in milliseconds                 |
| `.without_timeout()`                             | Disable timeout                             |
| `.with_retry(options)`                           | Configure retry behavior                    |
| `.without_retry()`                               | Disable retries                             |
| `.with_rate_limit(limit, period_ms)`             | Configure sliding-window rate limiter       |
| `.with_circuit_breaker(fail, success, delay_ms)` | Configure circuit breaker                   |
| `.with_init(init)`                               | Set default headers/method for all requests |
| `.with_pre_request_hook(hook)`                   | Hook called before each request             |
| `.with_post_response_success_hook(hook)`         | Hook called on successful response          |
| `.with_post_response_error_hook(hook)`           | Hook called on error response               |
| `.build()`                                       | Build the `FetchClient<T>`                  |

### Error Handling

```rust
use smooai_fetch::error::FetchError;

match client.fetch("https://api.example.com/data", init).await {
    Ok(response) => println!("Status: {}", response.status),
    Err(FetchError::HttpResponse { status, message, .. }) => {
        eprintln!("HTTP {status}: {message}");
    }
    Err(FetchError::Retry { attempts, source }) => {
        eprintln!("Failed after {attempts} attempts: {source}");
    }
    Err(FetchError::Timeout { timeout_ms }) => {
        eprintln!("Timed out after {timeout_ms}ms");
    }
    Err(FetchError::RateLimit { remaining_ms }) => {
        eprintln!("Rate limited, retry in {remaining_ms}ms");
    }
    Err(FetchError::CircuitBreaker) => {
        eprintln!("Circuit breaker open - service is down");
    }
    Err(FetchError::SchemaValidation { message }) => {
        eprintln!("Deserialization failed: {message}");
    }
    Err(e) => eprintln!("Request error: {e}"),
}
```

## Built With

- Rust 2021 Edition - Memory safety and performance
- [reqwest](https://docs.rs/reqwest) - Async HTTP client
- [tokio](https://tokio.rs/) - Async runtime
- [serde](https://serde.rs/) / [serde_json](https://docs.rs/serde_json) - JSON serialization
- [thiserror](https://docs.rs/thiserror) - Error type derivation
- Sliding window rate limiter (in-process, no external deps)
- Circuit breaker state machine (Closed/Open/HalfOpen)

## Related Packages

- [`@smooai/fetch`](https://www.npmjs.com/package/@smooai/fetch) - TypeScript/JavaScript version
- [`smooai-fetch` (Python)](https://pypi.org/project/smooai-fetch/) - Python version
- `github.com/SmooAI/fetch/go/fetch` - Go version

## Development

### Running Tests

```bash
cargo test
```

### Building

```bash
cargo build --release
```

### Linting and Formatting

```bash
cargo clippy
cargo fmt
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
