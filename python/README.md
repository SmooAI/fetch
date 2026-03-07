<!-- Improved compatibility of back to top link: See: https://github.com/othneildrew/Best-README-Template/pull/73 -->

<a name="readme-top"></a>

<!-- PROJECT LOGO -->
<br />
<div align="center">
  <a href="https://smoo.ai">
    <img src="../../images/logo.png" alt="SmooAI Logo" />
  </a>
</div>

<!-- ABOUT THE PROJECT -->

## About SmooAI

SmooAI is an AI-powered platform for helping businesses multiply their customer, employee, and developer experience.

Learn more on [smoo.ai](https://smoo.ai)

## SmooAI Packages

Check out other SmooAI packages at [smoo.ai/open-source](https://smoo.ai/open-source)

## About smooai-fetch (Python)

**Stop writing the same retry logic over and over** - A resilient HTTP client that handles the chaos of real-world APIs, so you can focus on building features instead of handling failures.

![PyPI Version](https://img.shields.io/pypi/v/smooai-fetch?style=for-the-badge)
![PyPI Downloads](https://img.shields.io/pypi/dw/smooai-fetch?style=for-the-badge)
![PyPI Last Update](https://img.shields.io/pypi/last-update/smooai-fetch?style=for-the-badge)

![GitHub License](https://img.shields.io/github/license/SmooAI/fetch?style=for-the-badge)
![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/SmooAI/fetch/release.yml?style=for-the-badge)
![GitHub Repo stars](https://img.shields.io/github/stars/SmooAI/fetch?style=for-the-badge)

### Python Package

This is the Python port of [@smooai/fetch](https://www.npmjs.com/package/@smooai/fetch), built with idiomatic async/await patterns using `httpx` and Pydantic. It provides the same resilient HTTP client capabilities — retries, timeouts, rate limiting, circuit breaking, and request lifecycle hooks — in a Pythonic API.

### Why smooai-fetch?

Ever had your async Python service crash because an API was down for 2 seconds? Or watched your workers pile up because a third-party service hit its rate limit? Traditional `httpx` and `aiohttp` give you the request, but leave you to handle the reality of network failures.

**smooai-fetch automatically handles:**

**For Unreliable APIs:**

- **Smart retries** - Exponential backoff with jitter to prevent thundering herds
- **Automatic timeouts** - Never hang indefinitely on slow endpoints
- **Rate limit respect** - Reads Retry-After headers and backs off intelligently
- **Circuit breaking** - Stop hammering services that are clearly down
- **Pydantic validation** - Validate response shapes with your existing models

**For Developer Experience:**

- **Async-native** - Built on `httpx.AsyncClient` with full async/await support
- **FetchBuilder** - Fluent builder API for reusable configured clients
- **Lifecycle hooks** - Pre-request and post-response hooks for auth and logging
- **Typed responses** - `FetchResponse[T]` wraps parsed Pydantic models

### Install

```bash
pip install smooai-fetch
```

or with [uv](https://docs.astral.sh/uv/):

```bash
uv add smooai-fetch
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

```python
from smooai_fetch import fetch

# This won't crash if the API is temporarily down
response = await fetch("https://flaky-api.com/data")

# Behind the scenes:
# Attempt 1: 500 error - waits 500ms
# Attempt 2: 503 error - waits 1000ms
# Attempt 3: 200 success!
```

Your users never know the API had issues — the request just works.

### Respect Rate Limits Automatically

No more manual retry-after parsing:

```python
response = await fetch("https://api.github.com/user/repos")

# If GitHub says "slow down":
# - Sees 429 status + Retry-After: 60
# - Automatically waits 60 seconds
# - Retries and succeeds
# - Your code continues normally
```

### Production-Ready Examples

#### Simple GET Request

```python
from smooai_fetch import fetch

response = await fetch("https://api.example.com/users")
users = response.data  # parsed JSON as dict
```

#### POST Request with Body

```python
from smooai_fetch import fetch, FetchOptions

response = await fetch(
    "https://api.example.com/users",
    FetchOptions(
        method="POST",
        headers={"Content-Type": "application/json"},
        body={"name": "Alice", "email": "alice@example.com"},
    ),
)
```

#### Pydantic Schema Validation

```python
from pydantic import BaseModel
from smooai_fetch import fetch, FetchOptions

class User(BaseModel):
    id: str
    email: str
    name: str

# Your API returns garbage? You'll know immediately
response = await fetch(
    "https://api.example.com/users/123",
    FetchOptions(schema=User),
)

# response.data is a fully validated User instance
print(response.data.email)
```

#### FetchBuilder for Reusable Clients

```python
from smooai_fetch import FetchBuilder
from smooai_fetch._types import RetryOptions, RateLimitOptions

builder = (
    FetchBuilder()
    .with_timeout(5000)
    .with_retry(RetryOptions(attempts=3, initial_interval_ms=500))
    .with_rate_limit(RateLimitOptions(max_requests=100, window_ms=60_000))
    .with_headers({"X-API-Key": "your-key"})
)

response = await builder.fetch("https://api.example.com/users/123")
```

#### Circuit Breaking for Critical Services

```python
from smooai_fetch import FetchBuilder
from smooai_fetch._types import CircuitBreakerOptions
from smooai_fetch._errors import CircuitBreakerError

# Stop hammering services that are clearly struggling
builder = (
    FetchBuilder()
    .with_circuit_breaker(CircuitBreakerOptions(
        failure_threshold=5,
        success_threshold=2,
        open_state_delay_ms=30_000,
    ))
    .with_timeout(5000)
)

try:
    response = await builder.fetch(
        "https://payment-processor.com/charge",
        method="POST",
        body=charge_data,
    )
except CircuitBreakerError:
    # Circuit is open - service is down, fail fast
    return fallback_response()
```

## Real-World Scenarios

### Handle Authentication Globally

```python
from smooai_fetch import FetchBuilder

builder = (
    FetchBuilder()
    .with_auth(get_token())      # sets Authorization: Bearer <token>
    .with_retry()
)

# All requests automatically include the Authorization header
response = await builder.fetch("https://api.example.com/protected")
```

### Add Custom Headers Per Request

```python
response = await builder.fetch(
    "https://api.example.com/data",
    headers={"X-Request-ID": "req-abc-123"},
)
```

### Pre-Request and Post-Response Hooks

```python
from smooai_fetch import FetchBuilder

def add_trace_header(url, request_kwargs):
    request_kwargs["headers"]["X-Trace-ID"] = generate_trace_id()
    return url, request_kwargs

def log_response(url, request_kwargs, response):
    print(f"GET {url} -> {response.response.status_code}")
    return response

builder = (
    FetchBuilder()
    .with_pre_request_hook(add_trace_header)
    .with_post_response_success_hook(log_response)
)
```

### Graceful Degradation

```python
from smooai_fetch import FetchBuilder
from smooai_fetch._errors import CircuitBreakerError

primary = FetchBuilder().with_circuit_breaker(CircuitBreakerOptions(failure_threshold=3)).with_timeout(5000)
fallback = FetchBuilder().with_timeout(2000)

async def get_weather(city: str):
    try:
        return await primary.fetch(f"https://api1.weather.com/{city}")
    except CircuitBreakerError:
        # Seamlessly fall back to secondary service
        return await fallback.fetch(f"https://api2.weather.com/{city}")
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
- Prevents indefinite hangs
- Configurable per request

**Rate Limit Handling:**

- Respects Retry-After headers
- Automatic backoff on 429 responses
- Prevents API ban hammers

## API Reference

### `fetch(url, options)` — Top-Level Function

The simplest way to make a request with automatic retries and timeout:

```python
from smooai_fetch import fetch, FetchOptions
from smooai_fetch._types import RetryOptions, TimeoutOptions

response = await fetch(
    "https://api.example.com/data",
    FetchOptions(
        method="GET",
        headers={"Authorization": "Bearer token"},
        retry=RetryOptions(attempts=3, initial_interval_ms=500),
        timeout=TimeoutOptions(timeout_ms=10_000),
    ),
)
```

### Error Handling

```python
from smooai_fetch._errors import (
    HTTPResponseError,
    RetryError,
    TimeoutError,
    RateLimitError,
    CircuitBreakerError,
    SchemaValidationError,
)

try:
    response = await fetch("https://api.example.com/data")
except HTTPResponseError as e:
    print(f"HTTP {e.status}: {e.status_text}")
    print(f"Body: {e.data_string}")
except RetryError as e:
    print(f"Failed after {e.attempts} attempts: {e.last_error}")
except TimeoutError as e:
    print(f"Timed out after {e.timeout_ms}ms")
except RateLimitError:
    print("Rate limit exceeded")
except CircuitBreakerError:
    print("Circuit breaker open — service is down")
except SchemaValidationError as e:
    print(f"Validation failed: {e.validation_errors}")
```

## Built With

- Python 3.13+ - Full async/await and type hints support
- [httpx](https://www.python-httpx.org/) - Async HTTP client
- [Pydantic](https://docs.pydantic.dev/) - Data validation and schema enforcement
- Sliding window rate limiter
- Circuit breaker state machine (Closed/Open/HalfOpen)

## Related Packages

- [`@smooai/fetch`](https://www.npmjs.com/package/@smooai/fetch) - TypeScript/JavaScript version
- [`smooai-fetch` (Rust)](https://crates.io/crates/smooai-fetch) - Rust version
- `github.com/SmooAI/fetch/go/fetch` - Go version

## Development

```bash
uv sync
uv run poe install-dev
uv run pytest
uv run poe lint
uv run poe lint:fix   # optional fixer
uv run poe format
uv run poe typecheck
uv run poe build
```

Set `UV_PUBLISH_TOKEN` before running `uv run poe publish` to upload to PyPI.

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
