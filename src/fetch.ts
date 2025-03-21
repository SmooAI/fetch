/* eslint-disable @typescript-eslint/no-explicit-any */
import merge from 'lodash.merge';
import { BreakerError, BreakerState, Circuit, Module, Ratelimit, RatelimitError, Retry, RetryMode, SlidingCountBreaker, Timeout, TimeoutError } from 'mollitia';

import tls from 'tls';

import AwsLambdaLogger, { CONTEXT, ContextHeader, ContextKey, ContextKeyHttp, ContextKeyHttpRequest } from '@smooai/logger/AwsLambdaLogger';
export { RatelimitError, TimeoutError } from 'mollitia';
export * from 'mollitia';
const contextLogger = new AwsLambdaLogger();
tls.DEFAULT_MIN_VERSION = 'TLSv1.2';

export type ResponseWithBody = Response & {
    data?: any;
    isJson: boolean;
    dataString: string;
};

type Headers = globalThis.Headers;
export type HeadersInit = string[][] | Record<string, string | ReadonlyArray<string>> | Headers;

type RequestInit = globalThis.RequestInit;
export type RequestInfo = string | URL | Request;

type Request = globalThis.Request;
type Response = globalThis.Response;

export type { Headers, RequestInit, Request, Response };

/**
 * Defaults set below:
 *
 * - DEFAULTS
 * - DEFAULT_RETRY_OPTIONS
 * - DEFAULT_RATE_LIMIT_RETRY_OPTIONS
 */

export class HTTPResponseError extends Error {
    public response: ResponseWithBody;
    constructor(response: ResponseWithBody, msg?: string) {
        let errorStr = '';
        let errIsSet = false;
        if (response.isJson && response.data.error) {
            if (!Array.isArray(response.data.error)) {
                if (response.data.error.type) {
                    errorStr += `(${response.data.error.type}): `;
                    errIsSet = true;
                }
                if (response.data.error.code) {
                    errorStr += `(${response.data.error.code}): `;
                    errIsSet = true;
                }
                if (response.data.error.message) {
                    errorStr += `${response.data.error.message}`;
                    errIsSet = true;
                }
                if (typeof response.data.error === 'string') {
                    errorStr += `${response.data.error}`;
                    errIsSet = true;
                }
            }
        }
        if (response.isJson && response.data.errorMessages) {
            if (Array.isArray(response.data.errorMessages)) {
                errorStr += `${response.data.errorMessages.join('; ')}`;
                errIsSet = true;
            }
        }
        if (!errIsSet) {
            errorStr = response.dataString;
        }
        super(`${msg ? `${msg}; ` : ''}${errorStr}; HTTP Error Response: ${response.status} ${response.statusText}`);
        this.response = response;
    }
}

export function isRetryable(status: number) {
    return status === 429 || status >= 500;
}

export class RetryError extends HTTPResponseError {
    constructor(response: ResponseWithBody) {
        super(response, 'Retry Error: Ran out of retry attempts.');
        this.response = response;
    }
}

export type ErrorCallback = (err: any) => boolean;
export type RetryCallback = (err: any, attempt: number) => boolean | number;

interface RetryOptions {
    name?: string;
    attempts: number;
    initialIntervalMs: number;
    mode?: RetryMode;
    factor?: number;
    fastFirst?: boolean;
    maxInterval?: number;
    jitterAdjustment?: number;
    onRejection?: RetryCallback;
}

export const DEFAULT_RETRY_OPTIONS: RetryOptions = {
    name: 'node-fetch-retry',
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
        }

        return true;
    },
};

export const DEFAULT_RATE_LIMIT_RETRY_OPTIONS: RetryOptions = {
    name: 'node-fetch-rate-limit-retry',
    attempts: 1,
    initialIntervalMs: 500,
    onRejection: (error) => {
        if (error instanceof RatelimitError) {
            return error.remainingTimeInRatelimit + 50;
        }

        return false;
    },
};

// Timeout, Retry below is using https://genesys.github.io/mollitia/overview/introduction
export interface RequestOptions {
    logger?: AwsLambdaLogger;
    timeout?: {
        name?: string;
        timeoutMs: number;
        retry?: RetryOptions;
    };
    retry?: RetryOptions;
}

const DEFAULTS: RequestOptions = {
    logger: contextLogger,
    retry: DEFAULT_RETRY_OPTIONS,
    timeout: {
        name: 'node-fetch-timeout',
        timeoutMs: 10000,
    },
};

// RateLimit, CircuitBreaker below is using https://genesys.github.io/mollitia/overview/introduction
export interface FetchContainerOptions {
    rateLimit?: {
        name?: string;
        limitForPeriod: number;
        limitPeriodMs: number;
        retry?: RetryOptions;
    };
    circuitBreaker?: {
        name?: string;
        state?: BreakerState; // Default: closed
        failureRateThreshold?: number; // Default: 50 - Specifies the failure rate threshold in percentage
        slowCallRateThreshold?: number; // Default: 100 - If at least 80% of the iterations are considered as being slow, the circuit is switched to Opened state.
        slowCallDurationThresholdMs?: number; // Default: 60000 - Specifies the duration (in ms) threshold above which calls are considered as slow
        permittedNumberOfCallsInHalfOpenState?: number; // Default: 2 - Specifies the number of permitted calls when the circuit is half open
        halfOpenStateMaxDelayMs?: number; // Default: 0 - Specifies the maximum wait (in ms) in Half Open State, before switching back to open. 0 deactivates this
        slidingWindowSize?: number; // Default: 10 - Specifies the maximum number of calls used to calculate failure and slow call rate percentages
        minimumNumberOfCalls?: number; // Default: 10 -  Specifies the minimum number of calls used to calculate failure and slow call rate percentages
        openStateDelayMs?: number; // Default: 60000 - Specifies the time (in ms) the circuit stay opened before switching to half-open
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

function prepareDefaultOptions(options?: RequestOptions): RequestOptions {
    return merge({}, DEFAULTS, options);
}

function prepareCircuitModules(options: RequestOptions): Module[] {
    const modules: Module[] = [];

    if (options.retry) {
        modules.push(
            new Retry({
                name: options.retry.name || 'node-fetch-retry',
                logger: options.logger,
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
                name: options.timeout.name || 'node-fetch-timeout',
                logger: options.logger,
                delay: options.timeout.timeoutMs,
            }),
        );

        if (options.timeout.retry) {
            modules.push(
                new Retry({
                    name: options.timeout.retry.name || 'node-fetch-timeout-retry',
                    logger: options.logger,
                    attempts: options.timeout.retry.attempts,
                    interval: options.timeout.retry.initialIntervalMs,
                    mode: options.timeout.retry.mode,
                    factor: options.timeout.retry.factor,
                    jitterAdjustment: options.timeout.retry.jitterAdjustment,
                    onRejection: options.timeout.retry.onRejection,
                }),
            );
        }
    }

    return modules;
}

function prepareFetchContainerModules(options: RequestOptions, containerOptions?: FetchContainerOptions): Module[] {
    const modules: Module[] = [];

    if (containerOptions) {
        if (containerOptions.rateLimit) {
            const retryOptions = containerOptions.rateLimit.retry || DEFAULT_RATE_LIMIT_RETRY_OPTIONS;
            modules.push(
                new Retry({
                    name: `${containerOptions.rateLimit.name ?? 'node-fetch-rate-limit'}-retry`,
                    logger: options.logger,
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
                    name: containerOptions.rateLimit.name || 'node-fetch-rate-limit',
                    logger: options.logger,
                    limitPeriod: containerOptions.rateLimit.limitPeriodMs,
                    limitForPeriod: containerOptions.rateLimit.limitForPeriod,
                }),
            );
        }

        if (containerOptions.circuitBreaker) {
            modules.push(
                new SlidingCountBreaker({
                    name: containerOptions.circuitBreaker.name,
                    logger: options.logger,
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

async function doGlobalFetch(url: RequestInfo, init?: RequestInit): Promise<Response> {
    const useInit: RequestInit = merge({}, init, { redirect: 'follow' });
    if ((useInit?.headers as Record<string, string>)?.['Content-Type'] === 'application/json' && typeof useInit.body === 'object') {
        useInit.body = JSON.stringify(useInit.body);
    }
    const response = await global.fetch(url, useInit);

    if (response.ok || response.redirected) {
        // response.status >= 200 && response.status < 300
        return response;
    } else {
        let isJson = false;
        let data: any;
        let dataString: string;
        if (response.headers?.has('Content-Type') && response.headers?.get('Content-Type')?.includes('application/json')) {
            data = await response.text();
            try {
                data = JSON.parse(data);
                isJson = true;
                dataString = JSON.stringify(data);
            } catch (_error) {
                isJson = false;
                dataString = data;
            }
        } else {
            isJson = false;
            dataString = await response.text();
        }
        throw new HTTPResponseError(
            merge(
                {},
                {
                    isJson,
                    data,
                    dataString,
                },
                {
                    status: response.status,
                    statusText: response.statusText,
                    headers: response.headers,
                    url: response.url,
                },
            ) as any,
        );
    }
}

async function doFetch(url: RequestInfo, init: RequestInit, options: RequestOptions): Promise<Response> {
    const circuit = new Circuit({
        name: 'node-fetch-circuit',
        func: doGlobalFetch,
        options: {
            modules: prepareCircuitModules(options),
        },
    });
    options.logger!.debug(`Sending HTTP request "${init.method} ${url}"`);
    let response: Response;
    try {
        response = await circuit.execute(url, init);
    } catch (error) {
        if (error instanceof TimeoutError) {
            options.logger!.error(error, `HTTP request "${init.method} ${url}" timed out (${error.name}) after ${options.timeout!.timeoutMs} ms`);
        } else if (options.retry && error instanceof HTTPResponseError) {
            if (options.retry.onRejection && options.retry.onRejection(error, 1)) {
                options.logger!.error(
                    error,
                    `HTTP request "${init.method} ${url}" retries failed (${options.retry.name}) after ${options.retry.attempts} retries`,
                );
                throw new RetryError(error.response);
            }
        }
        throw error;
    }
    options.logger!.debug(`Received HTTP response "${init.method} ${url}": Response status "${response.status} ${response.statusText}"`);
    return response;
}

export default async function fetch(url: RequestInfo, init?: RequestInit, options?: RequestOptions): Promise<Response> {
    return doFetch(url, prepareDefaultInit(init), prepareDefaultOptions(options));
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
export function generateFetchWithOptions(options: {
    init?: RequestInit;
    requestOptions?: RequestOptions;
    containerOptions?: FetchContainerOptions;
}): (url: RequestInfo, init?: RequestInit, options?: RequestOptions) => Promise<Response> {
    const _init = prepareDefaultInit(options.init);
    const _requestOptions = prepareDefaultOptions(options.requestOptions);
    const circuit = new Circuit({
        name: 'node-fetch-container-circuit',
        options: {
            modules: prepareFetchContainerModules(_requestOptions, options.containerOptions),
        },
    });
    return (url: RequestInfo, init?: RequestInit, requestOptions?: RequestOptions): Promise<Response> => {
        circuit.fn(doFetch);
        const __init = prepareDefaultInit(merge({}, _init, init));
        const __requestOptions = prepareDefaultOptions(merge({}, _requestOptions, requestOptions));
        try {
            return circuit.execute(url, prepareDefaultInit(__init), prepareDefaultOptions(__requestOptions));
        } catch (error) {
            if (error instanceof RatelimitError) {
                __requestOptions.logger!.error(
                    error,
                    `HTTP request "${__init.method} ${url}" rate limited (${error.name}) - more than ${
                        options.containerOptions!.rateLimit?.limitForPeriod
                    } in ${options.containerOptions!.rateLimit?.limitPeriodMs} ms - ${error.remainingTimeInRatelimit} ms left in rate limit`,
                );
            } else if (error instanceof BreakerError) {
                __requestOptions.logger!.error(error, `HTTP request "${__init.method} ${url}" circuit open (${error.name})`);
            }
            throw error;
        }
    };
}
