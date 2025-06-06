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

Check out other SmooAI packages at [npmjs.com/org/smooai](https://www.npmjs.com/org/smooai)

## About @smooai/fetch

A powerful fetch client library built on top of the native `fetch` API, designed for both Node.js and browser environments. Features built-in support for retries, timeouts, rate limiting, circuit breaking, and Standard Schema validation.

![NPM Version](https://img.shields.io/npm/v/%40smooai%2Ffetch?style=for-the-badge)
![NPM Downloads](https://img.shields.io/npm/dw/%40smooai%2Ffetch?style=for-the-badge)
![NPM Last Update](https://img.shields.io/npm/last-update/%40smooai%2Ffetch?style=for-the-badge)

![GitHub License](https://img.shields.io/github/license/SmooAI/fetch?style=for-the-badge)
![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/SmooAI/fetch/release.yml?style=for-the-badge)
![GitHub Repo stars](https://img.shields.io/github/stars/SmooAI/fetch?style=for-the-badge)

### Install

```sh
pnpm add @smooai/fetch
```

### Usage

The package provides two entry points:

- `@smooai/fetch` - For Node.js environments
- `@smooai/fetch/browser` - For browser environments

#### Node.js Usage

```typescript
import fetch from '@smooai/fetch';

// Simple GET request
const response = await fetch('https://api.example.com/data');
```

#### Browser Usage

```typescript
import fetch from '@smooai/fetch/browser';

// Simple GET request
const response = await fetch('https://api.example.com/data');
```

### Key Features

#### 🚀 Native Fetch API

- Built on top of the native `fetch` API
- Works seamlessly in both Node.js and browser environments
- Full TypeScript support
- Automatic JSON parsing and stringifying
- Structured error handling with detailed response information

#### ⚙️ Opinionated Defaults

The default export `fetch` comes with carefully chosen defaults for common use cases:

- **Retry Configuration**

    - 2 retry attempts
    - 500ms initial interval with jitter
    - Exponential backoff with factor of 2
    - 0.5 jitter adjustment
    - Smart retry decisions based on:
        - HTTP 5xx errors
        - Rate limit responses (429)
        - Timeout errors
        - Retry-After header support

- **Timeout Settings**

    - 10 second timeout for all requests
    - Automatic timeout error handling

- **Rate Limit Retry**
    - 1 retry attempt for rate limit errors
    - 500ms initial interval
    - Smart handling of rate limit headers
    - 50ms buffer added to retry timing

These defaults are designed to handle common API integration scenarios while providing a good balance between reliability and performance. They can be overridden using the `FetchBuilder` pattern or by passing custom options to the default `fetch` function.

#### ✅ Schema Validation

- Built-in support for [Standard Schema](https://github.com/standard-schema/standard-schema) compatible validators
- Works with Zod, ArkType, and other Standard Schema implementations
- Type-safe response validation
- Human-readable validation errors
- Typed responses

#### 🔄 Lifecycle Hooks

- **Pre-request Hook**

    - Modify URL and request configuration before sending
    - Add custom headers, query parameters, or transform request body
    - Full access to modify both URL and request init

- **Post-response Success Hook**

    - Transform successful responses after schema validation
    - Add metadata or transform response data
    - Read-only access to original request details

- **Post-response Error Hook**

    - Handle or transform errors before they're thrown
    - Create detailed error messages with request context
    - Read-only access to original request details

- **Type Safety**

    - Fully typed with TypeScript
    - Non-editable parameters marked as readonly
    - Schema types preserved throughout lifecycle

- **Integration**
    - Works seamlessly with schema validation
    - Compatible with retry, rate limiting, and circuit breaking
    - Preserves request/response context

#### 🛡️ Resilience Features

- **Retry Mechanism**

    - Configurable retry attempts and intervals
    - Jitter support for distributed retries
    - Smart retry decisions based on response status
    - Automatic handling of Retry-After headers
    - Custom retry callbacks

- **Timeout Control**

    - Configurable timeout duration
    - Optional retry on timeout
    - Automatic timeout error handling

- **Rate Limiting**

    - Configurable request limits per time period
    - Automatic rate limit header handling
    - Smart retry on rate limit errors
    - Custom rate limit retry strategies

- **Circuit Breaking**
    - Sliding window failure rate tracking
    - Configurable failure thresholds
    - Half-open state support
    - Automatic recovery
    - Custom error callbacks

#### 🔄 Automatic Context

- Automatic context propagation (correlation IDs, user agents)
- Structured logging integration

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
import { z } from 'zod';

// Create a custom logger that implements the LoggerInterface
const customLogger = {
    debug: (message: string, ...args: any[]) => {
        console.debug(`[DEBUG] ${message}`, ...args);
    },
    info: (message: string, ...args: any[]) => {
        console.info(`[INFO] ${message}`, ...args);
    },
    warn: (message: string, ...args: any[]) => {
        console.warn(`[WARN] ${message}`, ...args);
    },
    error: (error: Error | unknown, message: string, ...args: any[]) => {
        console.error(`[ERROR] ${message}`, error, ...args);
    },
};

// Create a fetch instance with the custom logger
const fetch = new FetchBuilder(
    z.object({
        id: z.string(),
        name: z.string(),
    }),
)
    .withLogger(customLogger)
    .build();

// All requests will now use your custom logger
const response = await fetch('https://api.example.com/users/123');
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
