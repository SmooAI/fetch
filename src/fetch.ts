/* eslint-disable @typescript-eslint/no-explicit-any */
import { faker } from '@faker-js/faker';
import Logger, { CONTEXT, ContextHeader, ContextKey, ContextKeyHttp, ContextKeyHttpRequest, ContextKeyHttpResponse } from '@smooai/logger/Logger';
import { handleSchemaValidation, HumanReadableSchemaError } from '@smooai/utils/validation/standardSchema';
import type { StandardSchemaV1 } from '@standard-schema/spec';
import merge from 'lodash.merge';
import { BreakerError, BreakerState, Circuit, Module, Ratelimit, RatelimitError, Retry, RetryMode, SlidingCountBreaker, Timeout, TimeoutError } from 'mollitia';

export { RatelimitError, TimeoutError } from 'mollitia';
export * from 'mollitia';

declare global {
    interface Window {
        fetch: typeof fetch;
    }
    interface Self {
        fetch: typeof fetch;
    }
    // Add declarations for window and self globals
    const window: Window & typeof globalThis;
    const self: Self & typeof globalThis;
}

/**
 * Determine the appropriate fetch implementation based on the environment.
 * Uses global.fetch in Node.js environments and window.fetch in browser environments.
 */
const globalFetch = (): typeof global.fetch => {
    // Browser environment
    if (typeof window !== 'undefined' && window.fetch) {
        return window.fetch.bind(window);
    }

    // Web Worker environment
    if (typeof self !== 'undefined' && self.fetch) {
        return self.fetch.bind(self);
    }

    // Node.js environment
    if (typeof global !== 'undefined') {
        // Modern Node.js (v18+) with built-in fetch
        if (global.fetch) {
            return global.fetch.bind(global);
        }

        // For older Node.js versions, suggest installing node-fetch
        throw new Error(
            'No fetch implementation found in Node.js environment. ' + 'For Node.js versions < 18, please install node-fetch: npm install node-fetch',
        );
    }

    throw new Error('No fetch implementation found. ' + 'Please ensure fetch is available in your environment.');
};

/**
 * Interface for browser-compatible logging functionality.
 * Provides methods for different log levels with context support.
 */
export interface LoggerInterface {
    /**
     * Log a debug message with optional context.
     * @param message - The message to log
     * @param args - Additional arguments to include in the log
     */
    debug(message: string, ...args: any[]): void;

    /**
     * Log an info message with optional context.
     * @param message - The message to log
     * @param args - Additional arguments to include in the log
     */
    info(message: string, ...args: any[]): void;

    /**
     * Log a warning message with optional context.
     * @param message - The message to log
     * @param args - Additional arguments to include in the log
     */
    warn(message: string, ...args: any[]): void;

    /**
     * Log an error message with the error object and optional context.
     * @param error - The error object to log
     * @param message - The error message
     * @param args - Additional arguments to include in the log
     */
    error(error: Error | unknown, message: string, ...args: any[]): void;
}

const contextLogger = new Logger({ name: 'fetch' });

/**
 * Extended Response type that includes parsed body data and metadata.
 * @template T - The type of the response body data
 */
export type ResponseWithBody<T = any> = Response & {
    /** The parsed response body data */
    data?: T;
    /** Whether the response body is JSON */
    isJson: boolean;
    /** The raw response body as a string */
    dataString: string;
};

type Headers = globalThis.Headers;
export type HeadersInit = string[][] | Record<string, string | ReadonlyArray<string>> | Headers;

type RequestInit = globalThis.RequestInit;
export type RequestInfo = string | URL | Request;

type Request = globalThis.Request;
type Response = globalThis.Response;

export type { Headers, RequestInit, Request, Response };

type ResponseType<Schema extends StandardSchemaV1 = never> = Schema extends StandardSchemaV1 ? StandardSchemaV1.InferOutput<Schema> : any;

/**
 * Defaults set below:
 *
 * - DEFAULTS
 * - DEFAULT_RETRY_OPTIONS
 * - DEFAULT_RATE_LIMIT_RETRY_OPTIONS
 */

/**
 * Error thrown when an HTTP request fails with a non-2xx status code.
 * Includes the response data and attempts to extract error information.
 * @template T - The type of the response body data
 */
export class HTTPResponseError<T = any> extends Error {
    /** The response object containing the error details */
    public response: ResponseWithBody<T>;
    constructor(response: ResponseWithBody<T>, msg?: string) {
        let errorStr = '';
        let errIsSet = false;
        if (response.isJson && response.data) {
            const data = response.data as any;
            if (data.error) {
                if (!Array.isArray(data.error)) {
                    if (data.error.type) {
                        errorStr += `(${data.error.type}): `;
                        errIsSet = true;
                    }
                    if (data.error.code) {
                        errorStr += `(${data.error.code}): `;
                        errIsSet = true;
                    }
                    if (data.error.message) {
                        errorStr += `${data.error.message}`;
                        errIsSet = true;
                    }
                    if (typeof data.error === 'string') {
                        errorStr += `${data.error}`;
                        errIsSet = true;
                    }
                }
            }
            if (data.errorMessages) {
                if (Array.isArray(data.errorMessages)) {
                    errorStr += `${data.errorMessages.join('; ')}`;
                    errIsSet = true;
                }
            }
        }
        if (!errIsSet) {
            errorStr = response.dataString || 'Unknown error';
        }
        super(`${msg ? `${msg}; ` : ''}${errorStr}; HTTP Error Response: ${response.status} ${response.statusText}`);
        this.response = response;
    }
}

export function isRetryable(status: number) {
    return status === 429 || status >= 500;
}

export class RetryError<T = any> extends HTTPResponseError<T> {
    constructor(response: ResponseWithBody<T>) {
        super(response, 'Retry Error: Ran out of retry attempts.');
        this.response = response;
    }
}

export type ErrorCallback = (err: any) => boolean;
export type RetryCallback = (err: any, attempt: number) => boolean | number;

/**
 * Configuration options for retry behavior.
 */
interface RetryOptions {
    /** Number of retry attempts */
    attempts: number;
    /** Initial delay between retries in milliseconds */
    initialIntervalMs: number;
    /** Retry mode (e.g., exponential backoff, jitter) */
    mode?: RetryMode;
    /** Factor to multiply the interval by for each retry */
    factor?: number;
    /** Whether to attempt the first retry immediately */
    fastFirst?: boolean;
    /** Maximum delay between retries in milliseconds */
    maxInterval?: number;
    /** Amount of random jitter to add to retry delays */
    jitterAdjustment?: number;
    /** Callback to determine if and when to retry */
    onRejection?: RetryCallback;
}

export const DEFAULT_RETRY_OPTIONS: RetryOptions = {
    attempts: 2,
    initialIntervalMs: 500,
    mode: RetryMode.JITTER,
    factor: 2,
    jitterAdjustment: 0.5,
    onRejection: (error) => {
        if (error instanceof HTTPResponseError) {
            if (isRetryable(error.response.status) && error.response.headers?.has('Retry-After')) {
                return parseInt(error.response.headers.get('Retry-After')!) * 1000;
            }
            return isRetryable(error.response.status);
        } else if (error instanceof RatelimitError) {
            return error.remainingTimeInRatelimit;
        } else if (error instanceof TimeoutError) {
            return true;
        } else if (error instanceof HumanReadableSchemaError) {
            return false;
        }

        return true;
    },
};

export const DEFAULT_RATE_LIMIT_RETRY_OPTIONS: RetryOptions = {
    attempts: 1,
    initialIntervalMs: 500,
    onRejection: (error) => {
        if (error instanceof RatelimitError) {
            return error.remainingTimeInRatelimit + 50;
        }

        return false;
    },
};

/**
 * Hook that runs before the request is made, allowing modification of the request
 */
export type PreRequestHook = (url: string, init: RequestInit) => [string, RequestInit] | void;

/**
 * Hook that runs after a successful response, allowing modification of the response
 */
export type PostResponseSuccessHook<T = any> = (url: string, init: Readonly<RequestInit>, response: ResponseWithBody<T>) => ResponseWithBody<T> | void;

/**
 * Hook that runs after a failed response, allowing modification of the error
 */
export type PostResponseErrorHook<T = any> = (url: string, init: Readonly<RequestInit>, error: Error, response?: ResponseWithBody<T>) => Error | void;

/**
 * Collection of lifecycle hooks for request/response handling
 */
export interface LifecycleHooks<T = any> {
    /** Hook that runs before the request is made */
    preRequest?: PreRequestHook;
    /** Hook that runs after a successful response */
    postResponseSuccess?: PostResponseSuccessHook<T>;
    /** Hook that runs after a failed response */
    postResponseError?: PostResponseErrorHook<T>;
}

/**
 * Configuration options for HTTP requests.
 * @template Schema - The schema type for response validation. Must be a StandardSchemaV1 compatible schema (e.g., Zod schema)
 */
export interface RequestOptions<Schema extends StandardSchemaV1 = never> {
    /** Custom logger for request logging */
    logger?: LoggerInterface;
    /** Timeout configuration */
    timeout?: {
        /** Timeout duration in milliseconds */
        timeoutMs: number;
    };
    /** Retry configuration */
    retry?: RetryOptions;
    /** Schema for response validation. Must be a StandardSchemaV1 compatible schema (e.g., Zod schema) */
    schema?: Schema;
    /** Lifecycle hooks for request/response handling */
    hooks?: LifecycleHooks<ResponseType<Schema>>;
}

const DEFAULTS: RequestOptions<never> = {
    logger: contextLogger,
    retry: DEFAULT_RETRY_OPTIONS,
    timeout: {
        timeoutMs: 10000,
    },
};

// RateLimit, CircuitBreaker below is using https://genesys.github.io/mollitia/overview/introduction
/**
 * Configuration options for fetch container features like rate limiting and circuit breaking.
 */
export interface FetchContainerOptions {
    /** Rate limiting configuration */
    rateLimit?: {
        /** Maximum number of requests allowed in the period */
        limitForPeriod: number;
        /** Duration of the rate limit period in milliseconds */
        limitPeriodMs: number;
        /** Retry configuration for rate limit handling */
        retry?: RetryOptions;
    };
    /** Circuit breaker configuration */
    circuitBreaker?: {
        /** Current state of the circuit breaker */
        state?: BreakerState;
        /** Failure rate threshold as a percentage */
        failureRateThreshold?: number;
        /** Slow call rate threshold as a percentage */
        slowCallRateThreshold?: number;
        /** Duration threshold for slow calls in milliseconds */
        slowCallDurationThresholdMs?: number;
        /** Number of calls allowed in half-open state */
        permittedNumberOfCallsInHalfOpenState?: number;
        /** Maximum delay in half-open state in milliseconds */
        halfOpenStateMaxDelayMs?: number;
        /** Size of the sliding window for failure rate calculation */
        slidingWindowSize?: number;
        /** Minimum number of calls for failure rate calculation */
        minimumNumberOfCalls?: number;
        /** Time to stay in open state in milliseconds */
        openStateDelayMs?: number;
        /** Callback to determine if an error should trigger circuit breaker */
        onError?: ErrorCallback;
    };
}

function mergeHeadersWithContext(headersInit?: HeadersInit): HeadersInit {
    let newHeadersInit: HeadersInit = {};
    if (
        CONTEXT[ContextKey.Http] &&
        CONTEXT[ContextKey.Http][ContextKeyHttp.Request] &&
        CONTEXT[ContextKey.Http][ContextKeyHttp.Request][ContextKeyHttpRequest.UserAgent]
    ) {
        newHeadersInit[ContextHeader.UserAgent] = CONTEXT[ContextKey.Http][ContextKeyHttp.Request][ContextKeyHttpRequest.UserAgent];
    }
    if (CONTEXT[ContextKey.CorrelationId]) {
        newHeadersInit[ContextHeader.CorrelationId] = CONTEXT[ContextKey.CorrelationId];
    }
    let initialHeaders = headersInit;
    if ((initialHeaders && initialHeaders instanceof Headers) || initialHeaders instanceof Map) {
        initialHeaders = Array.from(initialHeaders.entries()).reduce((result, entry) => {
            result[entry[0]] = entry[1];
            return result;
        }, {} as any);
    }
    newHeadersInit = merge({}, initialHeaders, newHeadersInit);
    return newHeadersInit;
}

function getHeadersObject(headers: Headers | Record<string, string> | undefined): Record<string, string> {
    if (!headers) return {};
    if (headers instanceof Headers) {
        const result: Record<string, string> = {};
        headers.forEach((value, key) => {
            result[key] = value;
        });
        return result;
    }
    return headers;
}

function getRequestBody(body: any): string | undefined {
    if (!body) return undefined;
    if (typeof body === 'string') return body;
    if (typeof body === 'object') return JSON.stringify(body);
    return String(body);
}

function prepareDefaultInit(init?: RequestInit): RequestInit {
    const requestInit: RequestInit = {
        headers: {
            ...mergeHeadersWithContext(init?.headers),
        },
        method: init && init.method ? init.method : 'GET',
    };
    if (init && init.headers) {
        delete init.headers;
    }
    return merge({}, init, requestInit);
}

function prepareDefaultOptions<Schema extends StandardSchemaV1 = never>(options?: RequestOptions<Schema>): RequestOptions<Schema> {
    return merge({}, DEFAULTS, options);
}

function generateRandomName(prefix: string): string {
    return `${prefix}-${faker.color.human()}-${faker.animal.type()}`;
}

function prepareCircuitModules<Schema extends StandardSchemaV1 = never>(options: RequestOptions<Schema>): Module[] {
    const modules: Module[] = [];
    const logger = options.logger || contextLogger;

    if (options.retry) {
        modules.push(
            new Retry({
                name: generateRandomName('smooai-fetch-retry'),
                logger: logger,
                attempts: options.retry.attempts,
                interval: options.retry.initialIntervalMs,
                mode: options.retry.mode,
                factor: options.retry.factor,
                jitterAdjustment: options.retry.jitterAdjustment,
                onRejection: options.retry.onRejection,
            }),
        );
    }

    if (options.timeout) {
        modules.push(
            new Timeout({
                name: generateRandomName('smooai-fetch-timeout'),
                logger: logger,
                delay: options.timeout.timeoutMs,
            }),
        );
    }

    return modules;
}

function prepareFetchContainerModules(options: RequestOptions, containerOptions?: FetchContainerOptions): Module[] {
    const modules: Module[] = [];
    const logger = options.logger || contextLogger;

    if (containerOptions) {
        if (containerOptions.rateLimit) {
            const retryOptions = containerOptions.rateLimit.retry || DEFAULT_RATE_LIMIT_RETRY_OPTIONS;
            modules.push(
                new Retry({
                    name: generateRandomName('smooai-fetch-rate-limit-retry'),
                    logger: logger,
                    attempts: retryOptions.attempts,
                    interval: retryOptions.initialIntervalMs,
                    mode: retryOptions.mode,
                    factor: retryOptions.factor,
                    jitterAdjustment: retryOptions.jitterAdjustment,
                    onRejection: retryOptions.onRejection,
                }),
            );

            modules.push(
                new Ratelimit({
                    name: generateRandomName('smooai-fetch-rate-limit'),
                    logger: logger,
                    limitPeriod: containerOptions.rateLimit.limitPeriodMs,
                    limitForPeriod: containerOptions.rateLimit.limitForPeriod,
                }),
            );
        }

        if (containerOptions.circuitBreaker) {
            modules.push(
                new SlidingCountBreaker({
                    name: generateRandomName('smooai-fetch-circuit-breaker'),
                    logger: logger,
                    state: containerOptions.circuitBreaker.state,
                    failureRateThreshold: containerOptions.circuitBreaker.failureRateThreshold,
                    slowCallRateThreshold: containerOptions.circuitBreaker.slowCallRateThreshold,
                    slowCallDurationThreshold: containerOptions.circuitBreaker.slowCallDurationThresholdMs,
                    permittedNumberOfCallsInHalfOpenState: containerOptions.circuitBreaker.permittedNumberOfCallsInHalfOpenState,
                    halfOpenStateMaxDelay: containerOptions.circuitBreaker.halfOpenStateMaxDelayMs,
                    slidingWindowSize: containerOptions.circuitBreaker.slidingWindowSize,
                    minimumNumberOfCalls: containerOptions.circuitBreaker.minimumNumberOfCalls,
                    openStateDelay: containerOptions.circuitBreaker.openStateDelayMs,
                    onError: containerOptions.circuitBreaker.onError,
                }),
            );
        }
    }

    return modules;
}

async function doGlobalFetch<Schema extends StandardSchemaV1 = never>(
    url: RequestInfo,
    init?: RequestInit,
    options?: RequestOptions<Schema>,
): Promise<ResponseWithBody<ResponseType<Schema>>> {
    const useInit: RequestInit = merge({}, init, { redirect: 'follow' });

    // Stringify JSON body if needed
    if ((useInit?.headers as Record<string, string>)?.['Content-Type'] === 'application/json' && typeof useInit.body === 'object') {
        useInit.body = JSON.stringify(useInit.body);
    }

    const response = await globalFetch()(url, useInit);
    let isJson = false;
    let data: ResponseType<Schema> | undefined;
    let dataString: string = '';
    let read = false;

    const responseClone = response.clone();

    if (responseClone.headers?.has('Content-Type') && responseClone.headers?.get('Content-Type')?.includes('application/json')) {
        dataString = await responseClone.text();
        read = true;
        try {
            const parsedData = JSON.parse(dataString);
            if (responseClone.ok && options?.schema) {
                data = (await handleSchemaValidation(options.schema, parsedData)) as Schema extends StandardSchemaV1
                    ? StandardSchemaV1.InferOutput<Schema>
                    : any;
            } else {
                data = parsedData as ResponseType<Schema>;
            }
            isJson = true;
        } catch (error) {
            if (error instanceof HumanReadableSchemaError) {
                throw error;
            }
            isJson = false;
        }
    } else {
        isJson = false;
    }

    const responseWithBody = response as any;
    responseWithBody.isJson = isJson;
    responseWithBody.dataString = dataString;
    responseWithBody.data = data;

    if (responseClone.ok || responseClone.redirected) {
        return responseWithBody;
    } else {
        if (!read) {
            responseWithBody.dataString = await responseClone.text();
        }
        throw new HTTPResponseError<ResponseType<Schema>>(responseWithBody);
    }
}

async function doFetch<Schema extends StandardSchemaV1 = never>(
    url: RequestInfo,
    init: RequestInit,
    options: RequestOptions<Schema>,
): Promise<ResponseWithBody<ResponseType<Schema>>> {
    const circuit = new Circuit({
        name: 'node-fetch-circuit',
        func: doGlobalFetch,
        options: {
            modules: prepareCircuitModules(options),
        },
    });
    const logger = options.logger || contextLogger;

    // Apply pre-request hook if present
    let modifiedInit = init;
    if (options.hooks?.preRequest) {
        const hookResult = options.hooks.preRequest(url.toString(), init);
        if (hookResult) {
            modifiedInit = hookResult[1];
            url = hookResult[0];
        }
    }

    const urlObj = new URL(url.toString());

    logger.debug(`Sending HTTP request "${modifiedInit.method} ${url}"`, {
        [ContextKey.Http]: {
            [ContextKeyHttp.Request]: {
                [ContextKeyHttpRequest.Method]: modifiedInit.method,
                [ContextKeyHttpRequest.Host]: urlObj.host,
                [ContextKeyHttpRequest.Path]: urlObj.pathname,
                [ContextKeyHttpRequest.QueryString]: urlObj.search,
                [ContextKeyHttpRequest.Headers]: getHeadersObject(modifiedInit.headers as Headers),
                [ContextKeyHttpRequest.Body]: getRequestBody(modifiedInit.body),
            },
        },
    });

    let response: ResponseWithBody<ResponseType<Schema>>;
    try {
        response = await circuit.execute(url, modifiedInit, options);

        // Apply post-response success hook if present
        if (options.hooks?.postResponseSuccess) {
            const hookResult = options.hooks.postResponseSuccess(url.toString(), init, response);
            if (hookResult) {
                response = hookResult;
            }
        }

        return response;
    } catch (error) {
        // Apply post-response error hook if present
        if (options.hooks?.postResponseError) {
            const hookResult = options.hooks.postResponseError(
                url.toString(),
                init,
                error as Error,
                error instanceof HTTPResponseError ? error.response : undefined,
            );
            if (hookResult) {
                throw hookResult;
            }
        }

        if (error instanceof TimeoutError) {
            logger.error(error, `HTTP request "${modifiedInit.method} ${url}" timed out (${error.name}) after ${options.timeout!.timeoutMs} ms`, {
                [ContextKey.Http]: {
                    [ContextKeyHttp.Request]: {
                        [ContextKeyHttpRequest.Method]: modifiedInit.method,
                        [ContextKeyHttpRequest.Host]: urlObj.host,
                        [ContextKeyHttpRequest.Path]: urlObj.pathname,
                        [ContextKeyHttpRequest.QueryString]: urlObj.search,
                        [ContextKeyHttpRequest.Headers]: getHeadersObject(modifiedInit.headers as Headers),
                        [ContextKeyHttpRequest.Body]: getRequestBody(modifiedInit.body),
                    },
                },
            });
        } else if (options.retry && error instanceof HTTPResponseError) {
            if (options.retry.onRejection && options.retry.onRejection(error, 1)) {
                logger.error(error, `HTTP request "${modifiedInit.method} ${url}" retries failed after ${options.retry.attempts} retries`, {
                    [ContextKey.Http]: {
                        [ContextKeyHttp.Request]: {
                            [ContextKeyHttpRequest.Method]: modifiedInit.method,
                            [ContextKeyHttpRequest.Host]: urlObj.host,
                            [ContextKeyHttpRequest.Path]: urlObj.pathname,
                            [ContextKeyHttpRequest.QueryString]: urlObj.search,
                            [ContextKeyHttpRequest.Headers]: getHeadersObject(modifiedInit.headers as Headers),
                            [ContextKeyHttpRequest.Body]: getRequestBody(modifiedInit.body),
                        },
                        [ContextKeyHttp.Response]: {
                            [ContextKeyHttpResponse.StatusCode]: error.response.status,
                            [ContextKeyHttpResponse.Headers]: getHeadersObject(error.response.headers),
                            [ContextKeyHttpResponse.Body]: error.response.dataString,
                        },
                    },
                });
                throw new RetryError<ResponseType<Schema>>(error.response);
            }
        }
        throw error;
    }
}

export type RequestInitWithOptions<Schema extends StandardSchemaV1 = never> = RequestInit & {
    options?: RequestOptions<Schema>;
};

// Overload signatures
export default async function fetch(url: RequestInfo, init?: RequestInitWithOptions<never>): Promise<ResponseWithBody<any>>;

export default async function fetch<Schema extends StandardSchemaV1>(
    url: RequestInfo,
    init?: RequestInitWithOptions<Schema>,
): Promise<ResponseWithBody<ResponseType<Schema>>>;

// Implementation
export default async function fetch<Schema extends StandardSchemaV1 = never>(
    url: RequestInfo,
    init?: RequestInitWithOptions<Schema>,
): Promise<ResponseWithBody<ResponseType<Schema>>> {
    const { options, ...requestInit } = init || {};
    return doFetch<Schema>(url, prepareDefaultInit(requestInit), prepareDefaultOptions(options));
}

/**
 * Example usage:
 *
 * const fetch = generateFetchWithOptions({
    containerOptions: {
        rateLimit: {
            name: 'finch-node-fetch-rate-limit',
            limitForPeriod: 2,
            limitPeriodMs: 62 * 1000,
            retry: DEFAULT_RATE_LIMIT_RETRY_OPTIONS,
        },
    },
    requestOptions: {
        logger,
        timeout: {
            name: 'finch-node-fetch-timeout',
            timeoutMs: 60 * 1000,
        },
        retry: DEFAULT_RETRY_OPTIONS,
    },
});
 * @param options
 * @returns
 */
function generateFetchWithOptions(options: { init?: RequestInit; requestOptions?: RequestOptions<never>; containerOptions?: FetchContainerOptions }): {
    (url: RequestInfo, init?: RequestInitWithOptions<never>): Promise<ResponseWithBody<any>>;
    <Schema extends StandardSchemaV1>(url: RequestInfo, init?: RequestInitWithOptions<Schema>): Promise<ResponseWithBody<ResponseType<Schema>>>;
} {
    const _init = prepareDefaultInit(options.init);
    const _requestOptions = prepareDefaultOptions(options.requestOptions);
    const circuit = new Circuit({
        name: 'node-fetch-container-circuit',
        options: {
            modules: prepareFetchContainerModules(_requestOptions, options.containerOptions),
        },
    });
    return <Schema extends StandardSchemaV1 = never>(
        url: RequestInfo,
        init?: RequestInitWithOptions<Schema>,
    ): Promise<ResponseWithBody<ResponseType<Schema>>> => {
        circuit.fn(doFetch);
        const { options: requestOptions, ...requestInit } = init || {};
        const __init = prepareDefaultInit(merge({}, _init, requestInit));
        const __requestOptions = prepareDefaultOptions(merge({}, _requestOptions, requestOptions));
        const logger = __requestOptions.logger || contextLogger;
        const urlObj = new URL(url.toString());
        const headers = getHeadersObject(__init.headers as Headers);
        const requestBody = getRequestBody(__init.body);

        try {
            return circuit.execute(url, prepareDefaultInit(__init), prepareDefaultOptions(__requestOptions));
        } catch (error) {
            if (error instanceof RatelimitError) {
                logger.error(
                    error,
                    `HTTP request "${__init.method} ${url}" rate limited (${error.name}) - more than ${
                        options.containerOptions!.rateLimit?.limitForPeriod
                    } in ${options.containerOptions!.rateLimit?.limitPeriodMs} ms - ${error.remainingTimeInRatelimit} ms left in rate limit`,
                    {
                        [ContextKey.Http]: {
                            [ContextKeyHttp.Request]: {
                                [ContextKeyHttpRequest.Method]: __init.method,
                                [ContextKeyHttpRequest.Host]: urlObj.host,
                                [ContextKeyHttpRequest.Path]: urlObj.pathname,
                                [ContextKeyHttpRequest.QueryString]: urlObj.search,
                                [ContextKeyHttpRequest.Headers]: headers,
                                [ContextKeyHttpRequest.Body]: requestBody,
                            },
                        },
                    },
                );
            } else if (error instanceof BreakerError) {
                logger.error(error, `HTTP request "${__init.method} ${url}" circuit open (${error.name})`, {
                    [ContextKey.Http]: {
                        [ContextKeyHttp.Request]: {
                            [ContextKeyHttpRequest.Method]: __init.method,
                            [ContextKeyHttpRequest.Host]: urlObj.host,
                            [ContextKeyHttpRequest.Path]: urlObj.pathname,
                            [ContextKeyHttpRequest.QueryString]: urlObj.search,
                            [ContextKeyHttpRequest.Headers]: headers,
                            [ContextKeyHttpRequest.Body]: requestBody,
                        },
                    },
                });
            }
            throw error;
        }
    };
}

/**
 * Builder class for creating configured fetch instances with retry, rate limiting, and circuit breaking.
 * Provides a fluent interface for configuring fetch options.
 * @template Schema - The schema type for response validation. Must be a StandardSchemaV1 compatible schema (e.g., Zod schema)
 */
export class FetchBuilder<Schema extends StandardSchemaV1 = never> {
    private _init?: RequestInit;
    private _requestOptions?: RequestOptions<Schema>;
    private _containerOptions?: FetchContainerOptions;

    constructor(schema?: Schema) {
        if (schema) this.withSchema(schema);
    }

    /**
     * Sets the initial request configuration.
     * @param init - The initial request configuration
     * @returns The builder instance for method chaining
     */
    withInit(init: RequestInit): FetchBuilder<Schema> {
        this._init = init;
        return this;
    }

    /**
     * Sets the request timeout.
     * @param timeoutMs - Timeout duration in milliseconds
     * @returns The builder instance for method chaining
     */
    withTimeout(timeoutMs: number): FetchBuilder<Schema> {
        this._requestOptions = {
            ...this._requestOptions,
            timeout: { timeoutMs },
        };
        return this;
    }

    /**
     * Configures retry behavior for failed requests.
     * If not specified, uses DEFAULT_RETRY_OPTIONS.
     * @param options - Retry configuration options
     * @returns The builder instance for method chaining
     */
    withRetry(options: RetryOptions = DEFAULT_RETRY_OPTIONS): FetchBuilder<Schema> {
        this._requestOptions = {
            ...this._requestOptions,
            retry: options,
        };
        return this;
    }

    /**
     * Configures rate limiting for requests.
     * If retryOptions is not specified, uses DEFAULT_RATE_LIMIT_RETRY_OPTIONS.
     * @param limitForPeriod - Maximum number of requests allowed in the period
     * @param limitPeriodMs - Duration of the rate limit period in milliseconds
     * @param retryOptions - Optional retry configuration for rate limit handling
     * @returns The builder instance for method chaining
     */
    withRateLimit(limitForPeriod: number, limitPeriodMs: number, retryOptions: RetryOptions = DEFAULT_RATE_LIMIT_RETRY_OPTIONS): FetchBuilder<Schema> {
        this._containerOptions = {
            ...this._containerOptions,
            rateLimit: {
                limitForPeriod,
                limitPeriodMs,
                retry: retryOptions,
            },
        };
        return this;
    }

    /**
     * Configures circuit breaker behavior.
     *
     * Note: This is not typically used, but can be useful for advanced use cases.
     *
     * @param options - Circuit breaker configuration options
     * @returns The builder instance for method chaining
     */
    withCircuitBreaker(options: FetchContainerOptions['circuitBreaker']): FetchBuilder<Schema> {
        this._containerOptions = {
            ...this._containerOptions,
            circuitBreaker: options,
        };
        return this;
    }

    /**
     * Sets container options directly.
     *
     * Note: This is not typically used, but can be useful for advanced use cases.
     *
     * @param options - Container configuration options
     * @returns The builder instance for method chaining
     */
    withContainerOptions(options: FetchContainerOptions): FetchBuilder<Schema> {
        this._containerOptions = {
            ...this._containerOptions,
            ...options,
        };
        return this;
    }

    /**
     * Sets a custom logger for request logging.
     * If not specified, uses the default contextLogger.
     * @param logger - The logger instance to use
     * @returns The builder instance for method chaining
     */
    withLogger(logger: LoggerInterface = contextLogger): FetchBuilder<Schema> {
        this._requestOptions = {
            ...this._requestOptions,
            logger,
        };
        return this;
    }

    /**
     * Sets a schema for response validation.
     * The schema must be StandardSchemaV1 compatible (e.g., a Zod schema).
     * The response body will be validated against this schema before being returned.
     *
     * @example
     * ```typescript
     * const schema = z.object({
     *   id: z.string(),
     *   name: z.string()
     * });
     *
     * const fetch = new FetchBuilder()
     *   .withSchema(schema)
     *   .build();
     * ```
     *
     * @param schema - The StandardSchemaV1 compatible schema to use for validation
     * @returns The builder instance for method chaining
     */
    private withSchema(schema: Schema): FetchBuilder<Schema> {
        this._requestOptions = {
            ...this._requestOptions,
            schema,
        };
        return this;
    }

    /**
     * Sets lifecycle hooks for request/response handling
     * @param hooks - The lifecycle hooks to use
     * @returns The builder instance for method chaining
     */
    withHooks(hooks: LifecycleHooks<ResponseType<Schema>>): FetchBuilder<Schema> {
        this._requestOptions = {
            ...this._requestOptions,
            hooks,
        };
        return this;
    }

    /**
     * Builds and returns a configured fetch function.
     * Applies default options for any unset configurations.
     * @returns A configured fetch function with the specified options
     */
    build(): {
        (url: RequestInfo, init?: RequestInitWithOptions<Schema>): Promise<ResponseWithBody<ResponseType<Schema>>>;
    } {
        // Apply defaults for request options
        const requestOptions = {
            ...DEFAULTS,
            ...this._requestOptions,
        };

        // Apply defaults for container options if rate limit is set
        const containerOptions = this._containerOptions?.rateLimit
            ? {
                  ...this._containerOptions,
                  rateLimit: {
                      ...this._containerOptions.rateLimit,
                      retry: this._containerOptions.rateLimit.retry || DEFAULT_RATE_LIMIT_RETRY_OPTIONS,
                  },
              }
            : this._containerOptions;

        return generateFetchWithOptions({
            init: this._init,
            requestOptions: requestOptions as RequestOptions<never>,
            containerOptions,
        });
    }
}

// Example usage:
/*
const fetch = new FetchBuilder()
    .withTimeout(60 * 1000)
    .withRetry(DEFAULT_RETRY_OPTIONS)
    .withRateLimit(2, 62 * 1000, DEFAULT_RATE_LIMIT_RETRY_OPTIONS)
    .withLogger(logger)
    .withHooks({
        preRequest: (url, init) => {
            // Modify URL and request before sending
            const modifiedUrl = new URL(url.toString());
            modifiedUrl.searchParams.set('timestamp', Date.now().toString());
            
            init.headers = {
                ...init.headers,
                'Custom-Header': 'value'
            };
            
            return [modifiedUrl, init];
        },
        postResponseSuccess: (url, init, response) => {
            // Modify successful response with access to original request details
            if (response.isJson && response.data) {
                response.data = {
                    ...response.data,
                    processed: true,
                    requestUrl: url.toString(),
                    requestMethod: init.method
                };
            }
            return response;
        },
        postResponseError: (url, init, error, response) => {
            // Handle or modify error with access to request details
            if (error instanceof HTTPResponseError) {
                console.error(`HTTP Error for ${init.method} ${url}:`, error.response.status);
                // Could modify error message to include request details
                return new Error(`Request to ${url} failed with status ${error.response.status}`);
            }
            return error;
        }
    })
    .build();
*/
