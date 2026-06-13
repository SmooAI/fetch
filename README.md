<p align="center">
  <a href="https://smoo.ai"><img src="https://smoo.ai/images/logo/logo.svg" alt="Smoo AI" width="220" /></a>
</p>

<h1 align="center">@smooai/fetch</h1>

<p align="center">
  <strong>A resilient, type-safe HTTP client that handles the chaos of real-world APIs for you.</strong>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/@smooai/fetch"><img src="https://img.shields.io/npm/v/@smooai/fetch?style=flat-square&color=00A6A6&label=npm" alt="npm"></a>
  <img src="https://img.shields.io/badge/Smoo_AI-platform-00A6A6?style=flat-square" alt="Smoo AI">
  <img src="https://img.shields.io/badge/license-MIT-F49F0A?style=flat-square" alt="license">
  <img src="https://img.shields.io/badge/TypeScript-3178C6?style=flat-square&logo=typescript&logoColor=white" alt="TypeScript">
  <img src="https://img.shields.io/badge/Python-3776AB?style=flat-square&logo=python&logoColor=white" alt="Python">
  <img src="https://img.shields.io/badge/Rust-000000?style=flat-square&logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/Go-00ADD8?style=flat-square&logo=go&logoColor=white" alt="Go">
</p>

<p align="center">
  <a href="#-features">Features</a> ·
  <a href="#-install">Install</a> ·
  <a href="#-usage">Usage</a> ·
  <a href="#-examples">Examples</a> ·
  <a href="#-part-of-smoo-ai">Platform</a>
</p>

---

> Stop writing the same retry logic over and over. `@smooai/fetch` is a drop-in `fetch` that survives the reality of network failures — exponential backoff, timeouts, rate-limit awareness, circuit breaking, and schema-validated responses — so you can focus on features instead of failure handling. Same API in Node.js and the browser, with native ports in TypeScript, Python, Rust, and Go.

Traditional `fetch` gives you the request, but leaves you to handle the reality of flaky APIs, slow endpoints, and rate limits. `@smooai/fetch` handles them by default.

## ✨ Features

**For unreliable APIs:**

- 🔄 **Smart retries** — exponential backoff with jitter to prevent thundering herds
- ⏱️ **Automatic timeouts** — never hang indefinitely on slow endpoints
- 🚦 **Rate-limit respect** — reads `Retry-After` headers and backs off intelligently
- 🔌 **Circuit breaking** — stop hammering services that are clearly down
- ⚡ **Request deduplication** — prevent duplicate in-flight requests

**For developer experience:**

- 🎯 **Type-safe responses** — schema validation with any Standard Schema validator
- 🔗 **Request lifecycle** — pre/post hooks for authentication and logging
- 📊 **Built-in telemetry** — track success rates and response times
- 🌐 **Universal** — the same API for Node.js and browsers
- 🪶 **Zero dependencies** — just the fetch API and smart patterns

## 📦 Install

```sh
pnpm add @smooai/fetch
```

### Multi-language support

`@smooai/fetch` ships native implementations in TypeScript, Python, Rust, and Go — each built with idiomatic patterns for its ecosystem.

| Language   | Package                                                        | Install                                   |
| ---------- | -------------------------------------------------------------- | ----------------------------------------- |
| TypeScript | [`@smooai/fetch`](https://www.npmjs.com/package/@smooai/fetch) | `pnpm add @smooai/fetch`                  |
| Python     | [`smooai-fetch`](https://pypi.org/project/smooai-fetch/)       | `pip install smooai-fetch`                |
| Rust       | [`smooai-fetch`](https://crates.io/crates/smooai-fetch)        | `cargo add smooai-fetch`                  |
| Go         | `github.com/SmooAI/fetch/go/fetch`                             | `go get github.com/SmooAI/fetch/go/fetch` |

Language-specific source lives in the [`python/`](./python/), [`rust/`](./rust/), and [`go/`](./go/) directories.

## 🚀 Usage

It's just `fetch`, but resilient.

```typescript
import fetch from '@smooai/fetch';

// This won't crash if the API is temporarily down
const response = await fetch('https://flaky-api.com/data');

// Behind the scenes:
// Attempt 1: 500 error — waits 500ms
// Attempt 2: 503 error — waits 1000ms
// Attempt 3: 200 success ✅
```

### Respect rate limits automatically

```typescript
const response = await fetch('https://api.github.com/user/repos');

// If GitHub says "slow down":
// - Sees 429 + Retry-After: 60
// - Automatically waits 60 seconds
// - Retries and succeeds
```

### Node.js and browser

```typescript
// Node.js
import fetch from '@smooai/fetch';
const response = await fetch('https://api.example.com/users');
const users = await response.json();

// Browser — same API, different entry point
import fetch from '@smooai/fetch/browser';
const response = await fetch('/api/checkout', {
    method: 'POST',
    body: { items: cart },
});
```

### Schema validation that makes sense

```typescript
import { z } from 'zod';

const UserSchema = z.object({
    id: z.string(),
    email: z.string().email(),
});

const response = await fetch('https://api.example.com/user', {
    options: { schema: UserSchema },
});

// response.data is fully typed as { id: string; email: string }
// No more runtime surprises in production
```

### Circuit breaking for critical services

```typescript
import { FetchBuilder } from '@smooai/fetch';

const criticalAPI = new FetchBuilder()
    .withCircuitBreaker({
        failureThreshold: 5, // 5 failures
        failureWindow: 60000, // in 60 seconds
        recoveryTime: 30000, // try again after 30s
    })
    .build();

try {
    await criticalAPI('https://payment-processor.com/charge');
} catch (error) {
    // Circuit is open — service is down. Show fallback UI immediately.
}
```

## 📖 Smart defaults

Out of the box, `@smooai/fetch` is configured for the real world:

**Retry strategy** — 2 automatic retries, exponential backoff (500ms → 1s → 2s), jitter to prevent thundering herds, and retries only on network errors or 5xx responses.

**Timeout protection** — 10-second default timeout, configurable per request, so requests never hang indefinitely.

**Rate-limit handling** — respects `Retry-After` headers and backs off automatically on 429 responses.

### Handle authentication globally

```typescript
const api = new FetchBuilder()
    .withHooks({
        preRequest: (url, init) => {
            init.headers = {
                ...init.headers,
                Authorization: `Bearer ${getToken()}`,
            };
            return [url, init];
        },
        postResponseError: (url, init, error) => {
            if (error.response?.status === 401) {
                refreshToken(); // Token expired — refresh and retry
            }
            return error;
        },
    })
    .build();
```

### Track performance automatically

```typescript
const api = new FetchBuilder()
    .withHooks({
        postResponseSuccess: (url, init, response) => {
            metrics.record({
                endpoint: url.pathname,
                duration: response.headers.get('x-response-time'),
                status: response.status,
            });
            return response;
        },
    })
    .build();
```

### Graceful degradation

```typescript
const primaryAPI = new FetchBuilder().withCircuitBreaker({ failureThreshold: 3 }).build();
const fallbackAPI = new FetchBuilder().withTimeout(2000).build();

async function getWeather(city: string) {
    try {
        return await primaryAPI(`https://api1.weather.com/${city}`);
    } catch (error) {
        console.warn('Primary weather API failed, using fallback');
        return await fallbackAPI(`https://api2.weather.com/${city}`);
    }
}
```

## 🔗 Pairs with @smooai/logger

`@smooai/fetch` works with [@smooai/logger](https://github.com/SmooAI/logger) for complete observability across distributed systems.

### Automatic correlation ID propagation

```typescript
import fetch, { FetchBuilder } from '@smooai/fetch';
import { AwsServerLogger } from '@smooai/logger/AwsServerLogger';

const logger = new AwsServerLogger({ name: 'APIClient' });

const api = new FetchBuilder()
    .withLogger(logger) // That's it
    .build();

// In Service A
logger.info('Starting user flow'); // Correlation ID: abc-123
const user = await api('/users/123'); // Correlation ID sent as header

// In Service B, the correlation ID is automatically extracted and logs are linked.
```

### Debug production issues faster

When something goes wrong, you have the complete story — initial request, each retry attempt, circuit-breaker state changes, and the final error with a full stack trace:

```typescript
try {
    const response = await api('/flaky-endpoint');
} catch (error) {
    logger.error('Request failed after retries', error);
}

// In your logs:
// {
//   "correlationId": "abc-123",
//   "message": "Request failed after retries",
//   "error": { "attempts": 3, "lastError": "TimeoutError", "circuitState": "open" },
//   "callerContext": { "stack": ["/src/services/UserService.ts:42:16"] }
// }
```

## 📚 Examples

- [Basic usage](#basic-usage)
- [FetchBuilder pattern](#fetchbuilder-pattern)
- [Retry](#retry-example)
- [Timeout](#timeout-example)
- [Rate limit](#rate-limit-example)
- [Schema validation](#schema-validation-example)
- [Lifecycle hooks](#lifecycle-hooks-example)
- [Predefined authentication](#predefined-authentication-example)
- [Custom logger](#custom-logger-example)
- [Error handling](#error-handling)

#### Basic usage <a name="basic-usage"></a>

```typescript
import fetch from '@smooai/fetch';

// Simple GET request
const response = await fetch('https://api.example.com/data');

// POST request with JSON body and options
const response = await fetch('https://api.example.com/data', {
    method: 'POST',
    headers: {
        'Content-Type': 'application/json',
    },
    body: {
        key: 'value',
    },
    options: {
        timeout: {
            timeoutMs: 5000,
        },
        retry: {
            attempts: 3,
        },
    },
});
```

<p align="right">(<a href="#-examples">back to examples</a>)</p>

#### FetchBuilder pattern <a name="fetchbuilder-pattern"></a>

The `FetchBuilder` provides a fluent interface for configuring fetch instances:

```typescript
import { FetchBuilder, RetryMode } from '@smooai/fetch';
import { z } from 'zod';

const UserSchema = z.object({
    id: z.string(),
    name: z.string(),
    email: z.string().email(),
});

const fetch = new FetchBuilder(UserSchema)
    .withTimeout(5000) // 5 second timeout
    .withRetry({
        attempts: 3,
        initialIntervalMs: 1000,
        mode: RetryMode.JITTER,
    })
    .withRateLimit(100, 60000) // 100 requests per minute
    .build();

const response = await fetch('https://api.example.com/users/123');
// response.data is typed as { id: string; name: string; email: string }
```

<p align="right">(<a href="#-examples">back to examples</a>)</p>

#### Retry <a name="retry-example"></a>

```typescript
import { FetchBuilder, RetryMode } from '@smooai/fetch';

// Using the default fetch
const response = await fetch('https://api.example.com/data', {
    options: {
        retry: {
            attempts: 3,
            initialIntervalMs: 1000,
            mode: RetryMode.JITTER,
            factor: 2,
            jitterAdjustment: 0.5,
            onRejection: (error) => {
                if (error instanceof HTTPResponseError) {
                    return error.response.status >= 500;
                }
                return false;
            },
        },
    },
});

// Or using FetchBuilder
const fetch = new FetchBuilder()
    .withRetry({
        attempts: 3,
        initialIntervalMs: 1000,
        mode: RetryMode.JITTER,
        factor: 2,
        jitterAdjustment: 0.5,
        onRejection: (error) => {
            if (error instanceof HTTPResponseError) {
                return error.response.status >= 500;
            }
            return false;
        },
    })
    .build();
```

<p align="right">(<a href="#-examples">back to examples</a>)</p>

#### Timeout <a name="timeout-example"></a>

```typescript
import { FetchBuilder } from '@smooai/fetch';

// Using the default fetch
const response = await fetch('https://api.example.com/slow-endpoint', {
    options: {
        timeout: {
            timeoutMs: 5000,
        },
    },
});

// Or using FetchBuilder
const fetch = new FetchBuilder()
    .withTimeout(5000) // 5 second timeout
    .build();

try {
    const response = await fetch('https://api.example.com/slow-endpoint');
} catch (error) {
    if (error instanceof TimeoutError) {
        console.error('Request timed out');
    }
}
```

<p align="right">(<a href="#-examples">back to examples</a>)</p>

#### Rate limit <a name="rate-limit-example"></a>

```typescript
import { FetchBuilder } from '@smooai/fetch';

// Using the default fetch
const response = await fetch('https://api.example.com/data', {
    options: {
        retry: {
            attempts: 1,
            initialIntervalMs: 1000,
            onRejection: (error) => {
                if (error instanceof RatelimitError) {
                    return error.remainingTimeInRatelimit;
                }
                return false;
            },
        },
    },
});

// Or using FetchBuilder
const fetch = new FetchBuilder()
    .withRateLimit(100, 60000, {
        attempts: 1,
        initialIntervalMs: 1000,
        onRejection: (error) => {
            if (error instanceof RatelimitError) {
                return error.remainingTimeInRatelimit;
            }
            return false;
        },
    })
    .build();
```

<p align="right">(<a href="#-examples">back to examples</a>)</p>

#### Schema validation <a name="schema-validation-example"></a>

```typescript
import { FetchBuilder } from '@smooai/fetch';
import { z } from 'zod';

const UserSchema = z.object({
    id: z.string(),
    name: z.string(),
    email: z.string().email(),
});

// Using the default fetch
const response = await fetch('https://api.example.com/users/123', {
    options: {
        schema: UserSchema,
    },
});

// Or using FetchBuilder
const fetch = new FetchBuilder(UserSchema).build();

try {
    const response = await fetch('https://api.example.com/users/123');
    // response.data is typed as { id: string; name: string; email: string }
} catch (error) {
    if (error instanceof HumanReadableSchemaError) {
        console.error('Validation failed:', error.message);
        // Example output:
        // Validation failed: Invalid email format at path: email
    }
}
```

<p align="right">(<a href="#-examples">back to examples</a>)</p>

#### Lifecycle hooks <a name="lifecycle-hooks-example"></a>

```typescript
import { FetchBuilder } from '@smooai/fetch';
import { z } from 'zod';

const UserSchema = z.object({
    id: z.string(),
    name: z.string(),
    email: z.string().email(),
});

const fetch = new FetchBuilder(UserSchema)
    .withHooks({
        // Pre-request hook can modify both URL and request configuration
        preRequest: (url, init) => {
            const modifiedUrl = new URL(url.toString());
            modifiedUrl.searchParams.set('timestamp', Date.now().toString());

            init.headers = {
                ...init.headers,
                'X-Custom-Header': 'value',
            };

            return [modifiedUrl, init];
        },

        // Post-response success hook can modify the response
        // Note: url and init are readonly in this hook
        postResponseSuccess: (url, init, response) => {
            if (response.isJson && response.data) {
                response.data = {
                    ...response.data,
                    _metadata: {
                        requestUrl: url.toString(),
                        requestMethod: init.method,
                        processedAt: new Date().toISOString(),
                    },
                };
            }
            return response;
        },

        // Post-response error hook can handle or transform errors
        // Note: url and init are readonly in this hook
        postResponseError: (url, init, error, response) => {
            if (error instanceof HTTPResponseError) {
                return new Error(`Request to ${url} failed with status ${error.response.status}. ` + `Method: ${init.method}`);
            }
            return error;
        },
    })
    .build();

try {
    const response = await fetch('https://api.example.com/users/123');
    console.log(response.data); // includes the _metadata added by postResponseSuccess
} catch (error) {
    console.error(error.message); // includes details added by postResponseError
}
```

<p align="right">(<a href="#-examples">back to examples</a>)</p>

#### Predefined authentication <a name="predefined-authentication-example"></a>

```typescript
import { FetchBuilder } from '@smooai/fetch';
import { z } from 'zod';

const UserSchema = z.object({
    id: z.string(),
    name: z.string(),
    email: z.string().email(),
});

// Using the default fetch
const response = await fetch('https://api.example.com/users/123', {
    headers: {
        Authorization: 'Bearer your-auth-token',
        'X-API-Key': 'your-api-key',
        'X-Client-ID': 'your-client-id',
    },
    options: {
        schema: UserSchema,
    },
});

// Or using FetchBuilder
const fetch = new FetchBuilder(UserSchema)
    .withInit({
        headers: {
            Authorization: 'Bearer your-auth-token',
            'X-API-Key': 'your-api-key',
            'X-Client-ID': 'your-client-id',
        },
    })
    .build();

// All requests automatically include the auth headers
const response = await fetch('https://api.example.com/users/123');
```

<p align="right">(<a href="#-examples">back to examples</a>)</p>

#### Custom logger <a name="custom-logger-example"></a>

```typescript
import { FetchBuilder } from '@smooai/fetch';
import { AwsServerLogger } from '@smooai/logger/AwsServerLogger';
import { z } from 'zod';

// Use @smooai/logger for automatic context and correlation
const logger = new AwsServerLogger({
    name: 'MyAPI',
    prettyPrint: true, // Human-readable logs in development
});

const fetch = new FetchBuilder(
    z.object({
        id: z.string(),
        name: z.string(),
    }),
)
    .withLogger(logger)
    .build();

// All requests now include correlation IDs, performance tracking,
// full error context, and request/response details.
const response = await fetch('https://api.example.com/users/123');

// Or bring your own logger that implements LoggerInterface
const customLogger = {
    debug: (message: string, ...args: any[]) => {
        /* ... */
    },
    info: (message: string, ...args: any[]) => {
        /* ... */
    },
    warn: (message: string, ...args: any[]) => {
        /* ... */
    },
    error: (error: Error | unknown, message: string, ...args: any[]) => {
        /* ... */
    },
};
```

<p align="right">(<a href="#-examples">back to examples</a>)</p>

#### Error handling <a name="error-handling"></a>

```typescript
import fetch, { HTTPResponseError, RatelimitError, RetryError, TimeoutError } from '@smooai/fetch';

try {
    const response = await fetch('https://api.example.com/data');
} catch (error) {
    if (error instanceof HTTPResponseError) {
        console.error('HTTP Error:', error.response.status);
        console.error('Response Data:', error.response.data);
    } else if (error instanceof RetryError) {
        console.error('Retry failed after all attempts');
    } else if (error instanceof TimeoutError) {
        console.error('Request timed out');
    } else if (error instanceof RatelimitError) {
        console.error('Rate limit exceeded');
    }
}
```

<p align="right">(<a href="#-examples">back to examples</a>)</p>

### Built with

- TypeScript
- Native Fetch API
- [Mollitia](https://github.com/genesys/mollitia) — circuit breaker and rate limiter
- [Standard Schema](https://github.com/standard-schema/standard-schema)
- [@smooai/logger](https://github.com/SmooAI/logger) — structured logging (bring your own logger supported)
- [@smooai/utils](https://github.com/SmooAI/utils) — Standard Schema validation and human-readable error generation

## 🧩 Part of Smoo AI

`@smooai/fetch` is part of the [Smoo AI](https://smoo.ai) platform — an AI-powered business platform with AI built into every product. It's one of a family of open-source packages we maintain to keep our own stack honest:

- [@smooai/logger](https://github.com/SmooAI/logger) — contextual logging for AWS and the browser
- [@smooai/config](https://github.com/SmooAI/config) — type-safe config, secrets, and feature flags
- [smooth](https://github.com/SmooAI/smooth) — the SmooAI developer toolchain

## 🤝 Contributing

Contributions are welcome. This project uses [changesets](https://github.com/changesets/changesets) to manage versions and releases.

1. Fork the repository.
2. Create your branch (`git checkout -b amazing-feature`).
3. Make your changes.
4. Add a changeset to document them: `pnpm changeset` — it prompts for the version bump type (patch, minor, or major) and a description.
5. Commit and push your branch.
6. Open a pull request, referencing any related issues.

The maintainers will review your PR and may request changes before merging.

## 📄 License

MIT © SmooAI. See [LICENSE](LICENSE).

## Contact

Brent Rager

- [Email](mailto:brent@smoo.ai)
- [LinkedIn](https://www.linkedin.com/in/brentrager/)
- [BlueSky](https://bsky.app/profile/brentragertech.bsky.social)
- [TikTok](https://www.tiktok.com/@brentragertech)
- [Instagram](https://www.instagram.com/brentragertech/)

Smoo GitHub: [github.com/SmooAI](https://github.com/SmooAI)

---

<p align="center">
  Built by <a href="https://smoo.ai"><strong>Smoo AI</strong></a> — AI built into every product.
</p>
