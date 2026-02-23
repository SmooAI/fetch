<!-- Improved compatibility of back to top link: See: https://github.com/othneildrew/Best-README-Template/pull/73 -->

<a name="readme-top"></a>

<!--
*** Thanks for checking out the Best-README-Template. If you have a suggestion
*** that would make this better, please fork the repo and create a pull request
*** or simply open an issue with the tag "enhancement".
*** Don't forget to give the project a star!
*** Thanks again! Now go create something AMAZING! :D
-->

<!-- PROJECT SHIELDS -->
<!--
*** I'm using markdown "reference style" links for readability.
*** Reference links are enclosed in brackets [ ] instead of parentheses ( ).
*** See the bottom of this document for the declaration of the reference variables
*** for contributors-url, forks-url, etc. This is an optional, concise syntax you may use.
*** https://www.markdownguide.org/basic-syntax/#reference-style-links
-->

<!-- PROJECT LOGO -->
<br />
<div align="center">
  <a href="https://smoo.ai">
    <img src="images/logo.png" alt="SmooAI Logo" />
  </a>
</div>

<!-- ABOUT THE PROJECT -->

## About SmooAI

SmooAI is an AI-powered platform for helping businesses multiply their customer, employee, and developer experience.

Learn more on [smoo.ai](https://smoo.ai)

## SmooAI Packages

Check out other SmooAI packages at [smoo.ai/open-source](https://smoo.ai/open-source)

## About @smooai/fetch

**Stop writing the same retry logic over and over** - A resilient HTTP client that handles the chaos of real-world APIs, so you can focus on building features instead of handling failures.

![NPM Version](https://img.shields.io/npm/v/%40smooai%2Ffetch?style=for-the-badge)
![NPM Downloads](https://img.shields.io/npm/dw/%40smooai%2Ffetch?style=for-the-badge)
![NPM Last Update](https://img.shields.io/npm/last-update/%40smooai%2Ffetch?style=for-the-badge)

![GitHub License](https://img.shields.io/github/license/SmooAI/fetch?style=for-the-badge)
![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/SmooAI/fetch/release.yml?style=for-the-badge)
![GitHub Repo stars](https://img.shields.io/github/stars/SmooAI/fetch?style=for-the-badge)

### Why @smooai/fetch?

Ever had your app crash because an API was down for 2 seconds? Or watched your users stare at loading spinners because a third-party service hit its rate limit? Traditional fetch gives you the request, but leaves you to handle the reality of network failures.

**@smooai/fetch automatically handles:**

**For Unreliable APIs:**

- 🔄 **Smart retries** - Exponential backoff with jitter to prevent thundering herds
- ⏱️ **Automatic timeouts** - Never hang indefinitely on slow endpoints
- 🚦 **Rate limit respect** - Reads Retry-After headers and backs off intelligently
- 🔌 **Circuit breaking** - Stop hammering services that are clearly down
- ⚡ **Request deduplication** - Prevent duplicate in-flight requests

**For Developer Experience:**

- 🎯 **Type-safe responses** - Schema validation with any Standard Schema validator
- 🔗 **Request lifecycle** - Pre/post hooks for authentication and logging
- 📊 **Built-in telemetry** - Track success rates and response times
- 🌐 **Universal** - Same API for Node.js and browsers
- 🪶 **Zero dependencies** - Just the fetch API and smart patterns

### Install

```sh
pnpm add @smooai/fetch
```

## The Power of Resilient Fetching

### Never Let a Hiccup Break Your App

Watch how @smooai/fetch handles common failure scenarios:

```typescript
import fetch from '@smooai/fetch';

// This won't crash if the API is temporarily down
const response = await fetch('https://flaky-api.com/data');

// Behind the scenes:
// Attempt 1: 500 error - waits 500ms
// Attempt 2: 503 error - waits 1000ms
// Attempt 3: 200 success! ✅
```

Your users never know the API had issues - the request just works.

### Respect Rate Limits Automatically

No more manual retry-after parsing:

```typescript
const response = await fetch('https://api.github.com/user/repos');

// If GitHub says "slow down":
// - Sees 429 status + Retry-After: 60
// - Automatically waits 60 seconds
// - Retries and succeeds
// - Your code continues normally
```

### Production-Ready Examples

#### Node.js Usage

```typescript
import fetch from '@smooai/fetch';

// It's just fetch, but resilient
const response = await fetch('https://api.example.com/users');
const users = await response.json();
```

#### Browser Usage

```typescript
import fetch from '@smooai/fetch/browser';

// Same API, different entry point
const response = await fetch('/api/checkout', {
    method: 'POST',
    body: { items: cart },
});
```

#### Schema Validation That Makes Sense

```typescript
import { z } from 'zod';

const UserSchema = z.object({
    id: z.string(),
    email: z.string().email(),
});

// Your API returns garbage? You'll know immediately
const response = await fetch('https://api.example.com/user', {
    options: { schema: UserSchema },
});

// response.data is fully typed as { id: string; email: string }
// No more runtime surprises in production
```

#### Circuit Breaking for Critical Services

```typescript
import { FetchBuilder } from '@smooai/fetch';

// Stop hammering services that are clearly struggling
const criticalAPI = new FetchBuilder()
    .withCircuitBreaker({
        failureThreshold: 5, // 5 failures
        failureWindow: 60000, // in 60 seconds
        recoveryTime: 30000, // try again after 30s
    })
    .build();

// If the service is down, this fails fast instead of waiting
try {
    await criticalAPI('https://payment-processor.com/charge');
} catch (error) {
    // Circuit is open - service is down
    // Show fallback UI immediately
}
```

## Real-World Scenarios

### Handle Authentication Globally

```typescript
const api = new FetchBuilder()
    .withHooks({
        preRequest: (url, init) => {
            // Add auth header to every request
            init.headers = {
                ...init.headers,
                Authorization: `Bearer ${getToken()}`,
            };
            return [url, init];
        },
        postResponseError: (url, init, error) => {
            if (error.response?.status === 401) {
                // Token expired - refresh and retry
                refreshToken();
            }
            return error;
        },
    })
    .build();
```

### Track Performance Automatically

```typescript
const api = new FetchBuilder()
    .withHooks({
        postResponseSuccess: (url, init, response) => {
            // Send metrics to your monitoring service
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

### Graceful Degradation

```typescript
// Primary API with circuit breaker
const primaryAPI = new FetchBuilder().withCircuitBreaker({ failureThreshold: 3 }).build();

// Fallback API for resilience
const fallbackAPI = new FetchBuilder()
    .withTimeout(2000) // Faster timeout for fallback
    .build();

async function getWeather(city: string) {
    try {
        return await primaryAPI(`https://api1.weather.com/${city}`);
    } catch (error) {
        // Seamlessly fall back to secondary service
        console.warn('Primary weather API failed, using fallback');
        return await fallbackAPI(`https://api2.weather.com/${city}`);
    }
}
```

## The Smart Defaults

Out of the box, @smooai/fetch is configured for the real world:

**Retry Strategy:**

- 2 automatic retries on failure
- Exponential backoff: 500ms → 1s → 2s
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

## Seamless Integration with @smooai/logger

@smooai/fetch works perfectly with [@smooai/logger](https://github.com/SmooAI/logger) to provide complete observability across your distributed systems:

### Automatic Correlation ID Propagation

```typescript
import fetch from '@smooai/fetch';
import { AwsServerLogger } from '@smooai/logger/AwsServerLogger';

const logger = new AwsServerLogger({ name: 'APIClient' });

// Correlation IDs flow automatically through your requests
const api = new FetchBuilder()
    .withLogger(logger) // That's it!
    .build();

// In Service A
logger.info('Starting user flow'); // Correlation ID: abc-123
const user = await api('/users/123'); // Correlation ID sent as header

// In Service B (receiving the request)
// The correlation ID is automatically extracted and logs are linked!
```

### Track Every Request with Context

```typescript
const api = new FetchBuilder()
    .withLogger(logger)
    .withHooks({
        postResponseSuccess: (url, init, response) => {
            // Logger automatically captures:
            // - Correlation ID
            // - Request method & URL
            // - Response status
            // - Duration
            // - Any errors with full context
            logger.info('API request completed', {
                endpoint: url.pathname,
                status: response.status,
            });
            return response;
        },
    })
    .build();
```

### Debug Production Issues Faster

When something goes wrong, you'll have the complete story:

```typescript
try {
    const response = await api('/flaky-endpoint');
} catch (error) {
    // Logger captures the entire request lifecycle:
    // - Initial request with headers
    // - Each retry attempt
    // - Circuit breaker state changes
    // - Final error with full stack trace
    logger.error('Request failed after retries', error);
}

// In your logs:
// {
//   "correlationId": "abc-123",
//   "message": "Request failed after retries",
//   "error": {
//     "attempts": 3,
//     "lastError": "TimeoutError",
//     "circuitState": "open"
//   },
//   "callerContext": {
//     "stack": ["/src/services/UserService.ts:42:16"]
//   }
// }
```

### Examples

- [Basic Usage](#basic-usage)
- [FetchBuilder Pattern](#fetchbuilder-pattern)
- [Retry Example](#retry-example)
- [Timeout Example](#timeout-example)
- [Rate Limit Example](#rate-limit-example)
- [Circuit Breaker Example](#circuit-breaker-example)
- [Schema Validation Example](#schema-validation-example)
- [Predefined Authentication Example](#predefined-authentication-example)
- [Custom Logger Example](#custom-logger-example)
- [Error Handling](#error-handling)

#### Basic Usage <a name="basic-usage"></a>

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

<p align="right">(<a href="#examples">back to examples</a>)</p>

#### FetchBuilder Pattern

The `FetchBuilder` provides a fluent interface for configuring fetch instances:

```typescript
import { FetchBuilder, RetryMode } from '@smooai/fetch';
import { z } from 'zod';

// Define a response schema
const UserSchema = z.object({
    id: z.string(),
    name: z.string(),
    email: z.string().email(),
});

// Create a configured fetch instance
const fetch = new FetchBuilder(UserSchema)
    .withTimeout(5000) // 5 second timeout
    .withRetry({
        attempts: 3,
        initialIntervalMs: 1000,
        mode: RetryMode.JITTER,
    })
    .withRateLimit(100, 60000) // 100 requests per minute
    .build();

// Use the configured fetch instance
const response = await fetch('https://api.example.com/users/123');
// response.data is now typed as { id: string; name: string; email: string }
```

<p align="right">(<a href="#examples">back to examples</a>)</p>

#### Retry Example <a name="retry-example"></a>

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
                // Custom retry logic
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

<p align="right">(<a href="#examples">back to examples</a>)</p>

#### Timeout Example <a name="timeout-example"></a>

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

<p align="right">(<a href="#examples">back to examples</a>)</p>

#### Rate Limit Example <a name="rate-limit-example"></a>

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

<p align="right">(<a href="#examples">back to examples</a>)</p>

#### Schema Validation Example

```typescript
import { FetchBuilder } from '@smooai/fetch';
import { z } from 'zod';

// Define response schema
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

<p align="right">(<a href="#examples">back to examples</a>)</p>

#### Lifecycle Hooks Example

```typescript
import { FetchBuilder } from '@smooai/fetch';
import { z } from 'zod';

// Define response schema
const UserSchema = z.object({
    id: z.string(),
    name: z.string(),
    email: z.string().email(),
});

// Create a fetch instance with hooks
const fetch = new FetchBuilder(UserSchema)
    .withHooks({
        // Pre-request hook can modify both URL and request configuration
        preRequest: (url, init) => {
            // Add timestamp to URL
            const modifiedUrl = new URL(url.toString());
            modifiedUrl.searchParams.set('timestamp', Date.now().toString());

            // Add custom headers
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
                // Add request metadata to response
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
                // Create a more detailed error message
                return new Error(`Request to ${url} failed with status ${error.response.status}. ` + `Method: ${init.method}`);
            }
            return error;
        },
    })
    .build();

// Use the configured fetch instance
try {
    const response = await fetch('https://api.example.com/users/123');
    // response.data includes the _metadata added by postResponseSuccess
    console.log(response.data);
} catch (error) {
    // Error message includes details added by postResponseError
    console.error(error.message);
}
```

<p align="right">(<a href="#examples">back to examples</a>)</p>

#### Predefined Authentication Example

```typescript
import { FetchBuilder } from '@smooai/fetch';
import { z } from 'zod';

// Define response schema
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

// All requests will automatically include the auth headers
const response = await fetch('https://api.example.com/users/123');
```

<p align="right">(<a href="#examples">back to examples</a>)</p>

#### Custom Logger Example

```typescript
import { FetchBuilder } from '@smooai/fetch';
import { AwsServerLogger } from '@smooai/logger/AwsServerLogger';
import { z } from 'zod';

// Use @smooai/logger for automatic context and correlation
const logger = new AwsServerLogger({
    name: 'MyAPI',
    prettyPrint: true, // Human-readable logs in development
});

// Create a fetch instance with the logger
const fetch = new FetchBuilder(
    z.object({
        id: z.string(),
        name: z.string(),
    }),
)
    .withLogger(logger)
    .build();

// All requests now include:
// - Correlation IDs that flow across services
// - Automatic performance tracking
// - Full error context with stack traces
// - Request/response details
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

<p align="right">(<a href="#examples">back to examples</a>)</p>

#### Error Handling <a name="error-handling"></a>

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

<p align="right">(<a href="#examples">back to examples</a>)</p>

### Built With

- TypeScript
- Native Fetch API
- [Mollitia](https://github.com/genesys/mollitia) (Circuit Breaker, Rate Limiter)
- [Standard Schema](https://github.com/standard-schema/standard-schema)
- [@smooai/logger](https://github.com/SmooAI/logger) for structured logging (bring your own logger supported)
- [@smooai/utils](https://github.com/SmooAI/utils) for Standard Schema validation and human-readable error generation

## Contributing

Contributions are welcome! This project uses [changesets](https://github.com/changesets/changesets) to manage versions and releases.

### Development Workflow

1. Fork the repository
2. Create your branch (`git checkout -b amazing-feature`)
3. Make your changes
4. Add a changeset to document your changes:

    ```sh
    pnpm changeset
    ```

    This will prompt you to:

    - Choose the type of version bump (patch, minor, or major)
    - Provide a description of the changes

5. Commit your changes (`git commit -m 'Add some amazing feature'`)
6. Push to the branch (`git push origin feature/amazing-feature`)
7. Open a Pull Request

### Pull Request Guidelines

- Reference any related issues in your PR description

The maintainers will review your PR and may request changes before merging.

<!-- CONTACT -->

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Contact

Brent Rager

- [Email](mailto:brent@smoo.ai)
- [LinkedIn](https://www.linkedin.com/in/brentrager/)
- [BlueSky](https://bsky.app/profile/brentragertech.bsky.social)
- [TikTok](https://www.tiktok.com/@brentragertech)
- [Instagram](https://www.instagram.com/brentragertech/)

Smoo Github: [https://github.com/SmooAI](https://github.com/SmooAI)

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- MARKDOWN LINKS & IMAGES -->
<!-- https://www.markdownguide.org/basic-syntax/#reference-style-links -->

[sst.dev-url]: https://reactjs.org/
[sst]: https://img.shields.io/badge/sst-EDE1DA?style=for-the-badge&logo=sst&logoColor=E27152
[sst-url]: https://sst.dev/
[next]: https://img.shields.io/badge/next.js-000000?style=for-the-badge&logo=nextdotjs&logoColor=white
[next-url]: https://nextjs.org/
[aws]: https://img.shields.io/badge/aws-232F3E?style=for-the-badge&logo=amazonaws&logoColor=white
[aws-url]: https://tailwindcss.com/
[tailwindcss]: https://img.shields.io/badge/tailwind%20css-0B1120?style=for-the-badge&logo=tailwindcss&logoColor=#06B6D4
[tailwindcss-url]: https://tailwindcss.com/
[zod]: https://img.shields.io/badge/zod-3E67B1?style=for-the-badge&logoColor=3E67B1
[zod-url]: https://zod.dev/
[sanity]: https://img.shields.io/badge/sanity-F36458?style=for-the-badge
[sanity-url]: https://www.sanity.io/
[vitest]: https://img.shields.io/badge/vitest-1E1E20?style=for-the-badge&logo=vitest&logoColor=#6E9F18
[vitest-url]: https://vitest.dev/
[pnpm]: https://img.shields.io/badge/pnpm-F69220?style=for-the-badge&logo=pnpm&logoColor=white
[pnpm-url]: https://pnpm.io/
[turborepo]: https://img.shields.io/badge/turborepo-000000?style=for-the-badge&logo=turborepo&logoColor=#EF4444
[turborepo-url]: https://turbo.build/
