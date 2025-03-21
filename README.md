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

SmooAI is a platform for building and deploying AI-powered apps.

Learn more on [smoo.ai](https://smoo.ai)

## SmooAI Packages

Check out other SmooAI packages at [npmjs.com/org/smooai](https://www.npmjs.com/org/smooai)

## About @smooai/fetch

A powerful HTTP client library with built-in support for retries, timeouts, rate limiting, and circuit breaking. Designed for both Node.js and browser environments, with seamless integration with AWS Lambda and structured logging.

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

### Key Features

#### Core HTTP Client

- Full TypeScript support
- Compatible with Node.js and browser environments
- Automatic JSON parsing and stringifying
- Structured error handling with detailed response information
- Automatic context propagation (correlation IDs, user agents)
- TLS 1.2 security by default

#### Resilience Features

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

#### AWS Lambda Integration

- Automatic AWS Lambda context extraction
- Structured logging with @smooai/logger
- Request/response correlation tracking
- CloudWatch optimized logging

### Usage Examples

#### Basic Usage

```typescript
import fetch from '@smooai/fetch';

// Simple GET request
const response = await fetch('https://api.example.com/data');

// POST request with JSON body
const response = await fetch('https://api.example.com/data', {
    method: 'POST',
    headers: {
        'Content-Type': 'application/json',
    },
    body: {
        key: 'value',
    },
});
```

#### With Retry Options

```typescript
import fetch from '@smooai/fetch';

const response = await fetch(
    'https://api.example.com/data',
    {},
    {
        retry: {
            attempts: 3,
            initialIntervalMs: 1000,
            mode: RetryMode.JITTER,
            factor: 2,
            jitterAdjustment: 0.5,
        },
    },
);
```

#### With Rate Limiting and Circuit Breaking

```typescript
import { generateFetchWithOptions } from '@smooai/fetch';

const fetch = generateFetchWithOptions({
    containerOptions: {
        rateLimit: {
            name: 'api-rate-limit',
            limitForPeriod: 100,
            limitPeriodMs: 60000, // 1 minute
        },
        circuitBreaker: {
            name: 'api-circuit-breaker',
            failureRateThreshold: 50,
            slowCallRateThreshold: 80,
            slowCallDurationThresholdMs: 5000,
            slidingWindowSize: 10,
        },
    },
    requestOptions: {
        timeout: {
            timeoutMs: 5000,
        },
    },
});

// Use the configured fetch instance
const response = await fetch('https://api.example.com/data');
```

#### Error Handling

```typescript
import fetch, { HTTPResponseError, RetryError } from '@smooai/fetch';

try {
    const response = await fetch('https://api.example.com/data');
} catch (error) {
    if (error instanceof HTTPResponseError) {
        console.error('HTTP Error:', error.response.status);
        console.error('Response Data:', error.response.data);
    } else if (error instanceof RetryError) {
        console.error('Retry failed after all attempts');
    }
}
```

### Configuration Options

#### Request Options

```typescript
interface RequestOptions {
    logger?: AwsLambdaLogger;
    timeout?: {
        name?: string;
        timeoutMs: number;
        retry?: RetryOptions;
    };
    retry?: RetryOptions;
}
```

#### Container Options

```typescript
interface FetchContainerOptions {
    rateLimit?: {
        name?: string;
        limitForPeriod: number;
        limitPeriodMs: number;
        retry?: RetryOptions;
    };
    circuitBreaker?: {
        name?: string;
        state?: BreakerState;
        failureRateThreshold?: number;
        slowCallRateThreshold?: number;
        slowCallDurationThresholdMs?: number;
        permittedNumberOfCallsInHalfOpenState?: number;
        halfOpenStateMaxDelayMs?: number;
        slidingWindowSize?: number;
        minimumNumberOfCalls?: number;
        openStateDelayMs?: number;
        onError?: ErrorCallback;
    };
}
```

### Default Configurations

The library provides sensible defaults for common use cases:

```typescript
const DEFAULT_RETRY_OPTIONS = {
    attempts: 2,
    initialIntervalMs: 500,
    mode: RetryMode.JITTER,
    factor: 2,
    jitterAdjustment: 0.5,
};

const DEFAULT_RATE_LIMIT_RETRY_OPTIONS = {
    attempts: 1,
    initialIntervalMs: 500,
};
```

### Built With

- TypeScript
- Mollitia (Circuit Breaker, Rate Limiter)
- AWS Lambda Integration
- Structured Logging
- Modern Fetch API

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
