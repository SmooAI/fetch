/* eslint-disable @typescript-eslint/no-explicit-any -- ok*/
import { TimeoutError } from 'mollitia';
import { beforeEach, describe, expect, expectTypeOf, MockedFunction, test, vi } from 'vitest';
import fetch, { generateFetchWithOptions, HTTPResponseError, RequestInfo, Response, RetryError } from './fetch';
import { ContextHeader } from '@smooai/logger/AwsLambdaLogger';
import sleep from '@smooai/utils/utils/sleep';

const URL = 'https://smoo.ai';

const JSON_HEADERS = new Headers({ 'Content-Type': 'application/json' });
const NON_JSON_HEADERS = new Headers({});

function fakeResponse(ok: boolean, status: number, json: any = {}, text = '', isJson = true): Response {
    const responseText = text || JSON.stringify(json);
    return {
        ok,
        status,
        headers: isJson ? JSON_HEADERS : NON_JSON_HEADERS,
        json: async () => {
            return json;
        },
        text: async () => {
            return responseText;
        },
    } as Response;
}

describe('Test fetch', () => {
    beforeEach(() => {
        vi.resetAllMocks();
        vi.useRealTimers();
        vi.stubGlobal('fetch', vi.fn());
    });

    test('Test basic request', async () => {
        const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
        mockFetch.mockResolvedValue(fakeResponse(true, 200));

        const response = await fetch(URL, {
            method: 'GET',
        });

        expect(response.ok).toBeTruthy();
        expect(response.status).toBe(200);

        expect(mockFetch.mock.calls.length).toBe(1);
        expect(mockFetch.mock.calls[0][0]).toBe(URL);
        expect(mockFetch.mock.calls[0][1]).toBeDefined();
        expect(mockFetch.mock.calls[0][1]!.method).toBe('GET');
        expect(mockFetch.mock.calls[0][1]!.headers).toBeDefined();
        expect(Object.keys(mockFetch.mock.calls[0][1]!.headers!)).toEqual([ContextHeader.CorrelationId]);
        expectTypeOf<string>(mockFetch.mock.calls[0][1]!.headers! as any[ContextHeader.CorrelationId]).toBeString();
    });

    test('Test failed request', async () => {
        const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
        mockFetch.mockResolvedValue(fakeResponse(false, 404));

        let error: Error;
        try {
            await fetch(URL, {
                method: 'GET',
            });
        } catch (caughtError) {
            error = caughtError as Error;
        }

        expect(error!).toBeInstanceOf(HTTPResponseError);
        const httpError = error! as HTTPResponseError;
        expect(httpError.response.ok).toBeFalsy();
        expect(httpError.response.status).toBe(404);

        expect(mockFetch.mock.calls.length).toBe(1);
        expect(mockFetch.mock.calls[0][0]).toBe(URL);
        expect(mockFetch.mock.calls[0][1]).toBeDefined();
        expect(mockFetch.mock.calls[0][1]!.method).toBe('GET');
        expect(mockFetch.mock.calls[0][1]!.headers).toBeDefined();
        expect(Object.keys(mockFetch.mock.calls[0][1]!.headers!)).toEqual([ContextHeader.CorrelationId]);
        expectTypeOf<string>(mockFetch.mock.calls[0][1]!.headers! as any[ContextHeader.CorrelationId]).toBeString();
    });

    test('Test request retries failed', async () => {
        vi.useFakeTimers();
        const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;

        mockFetch.mockImplementationOnce(async () => {
            (async () => {
                // This is kinda tricky but it seems to work to run out the retry interval timer.
                await vi.runAllTimersAsync();
            })();
            return fakeResponse(false, 429);
        });
        mockFetch.mockImplementationOnce(async () => {
            (async () => {
                // This is kinda tricky but it seems to work to run out the retry interval timer.
                await vi.runAllTimersAsync();
            })();
            return fakeResponse(false, 500);
        });
        mockFetch.mockImplementationOnce(async () => {
            (async () => {
                // This is kinda tricky but it seems to work to run out the retry interval timer.
                await vi.runAllTimersAsync();
            })();
            return fakeResponse(false, 501);
        });

        let error: Error;
        try {
            await fetch(URL, {
                method: 'GET',
            });
        } catch (caughtError) {
            error = caughtError as Error;
        }

        expect(error!).toBeInstanceOf(RetryError);
        const retryError = error! as RetryError;
        expect(retryError.response.ok).toBeFalsy();
        expect(retryError.response.status).toBe(501);

        expect(mockFetch.mock.calls.length).toBe(3);
        for (let i = 0; i < 3; i++) {
            expect(mockFetch.mock.calls[i][0]).toBe(URL);
            expect(mockFetch.mock.calls[i][1]).toBeDefined();
            expect(mockFetch.mock.calls[i][1]!.method).toBe('GET');
            expect(mockFetch.mock.calls[i][1]!.headers).toBeDefined();
            expect(Object.keys(mockFetch.mock.calls[i][1]!.headers!)).toEqual([ContextHeader.CorrelationId]);
            expectTypeOf<string>(mockFetch.mock.calls[i][1]!.headers! as any[ContextHeader.CorrelationId]).toBeString();
        }
    });

    test('Test request retries succeeded', async () => {
        vi.useFakeTimers();
        const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;

        mockFetch.mockImplementationOnce(async () => {
            (async () => {
                // This is kinda tricky but it seems to work to run out the retry interval timer.
                await vi.runAllTimersAsync();
            })();
            return fakeResponse(false, 429);
        });
        mockFetch.mockImplementationOnce(async () => {
            (async () => {
                // This is kinda tricky but it seems to work to run out the retry interval timer.
                await vi.runAllTimersAsync();
            })();
            return fakeResponse(false, 500);
        });
        mockFetch.mockImplementationOnce(async () => {
            (async () => {
                // This is kinda tricky but it seems to work to run out the retry interval timer.
                await vi.runAllTimersAsync();
            })();
            return fakeResponse(true, 200);
        });

        let error: Error;
        try {
            const response = await fetch(URL, {
                method: 'GET',
            });

            expect(response!.ok).toBeTruthy();
            expect(response!.status).toBe(200);
        } catch (caughtError) {
            error = caughtError as Error;
        }

        expect(error!).toBeUndefined();

        expect(mockFetch.mock.calls.length).toBe(3);
        for (let i = 0; i < 3; i++) {
            expect(mockFetch.mock.calls[i][0]).toBe(URL);
            expect(mockFetch.mock.calls[i][1]).toBeDefined();
            expect(mockFetch.mock.calls[i][1]!.method).toBe('GET');
            expect(mockFetch.mock.calls[i][1]!.headers).toBeDefined();
            expect(Object.keys(mockFetch.mock.calls[i][1]!.headers!)).toEqual([ContextHeader.CorrelationId]);
            expectTypeOf<string>(mockFetch.mock.calls[i][1]!.headers! as any[ContextHeader.CorrelationId]).toBeString();
        }
    });

    test('Test timeout', async () => {
        vi.useFakeTimers();
        const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;

        mockFetch.mockImplementation(async () => {
            await vi.advanceTimersByTimeAsync(12000);
            return fakeResponse(true, 200);
        });

        let error: Error;
        try {
            await fetch(URL, {
                method: 'GET',
            });
        } catch (caughtError) {
            error = caughtError as Error;
        }

        expect(error!).toBeInstanceOf(TimeoutError);

        expect(mockFetch.mock.calls.length).toBe(3);
        for (let i = 0; i < 3; i++) {
            expect(mockFetch.mock.calls[i][0]).toBe(URL);
            expect(mockFetch.mock.calls[i][1]).toBeDefined();
            expect(mockFetch.mock.calls[i][1]!.method).toBe('GET');
            expect(mockFetch.mock.calls[i][1]!.headers).toBeDefined();
            expect(Object.keys(mockFetch.mock.calls[i][1]!.headers!)).toEqual([ContextHeader.CorrelationId]);
            expectTypeOf<string>(mockFetch.mock.calls[i][1]!.headers! as any[ContextHeader.CorrelationId]).toBeString();
        }
    });

    test('Test timeout then succeed', async () => {
        vi.useFakeTimers();
        const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;

        mockFetch.mockImplementationOnce(async () => {
            await vi.advanceTimersByTimeAsync(11000);
            return fakeResponse(true, 200);
        });
        mockFetch.mockImplementationOnce(async () => {
            return fakeResponse(true, 200);
        });

        let error: Error;
        try {
            const response = await fetch(URL, {
                method: 'GET',
            });

            expect(response!.ok).toBeTruthy();
            expect(response!.status).toBe(200);
        } catch (caughtError) {
            error = caughtError as Error;
        }

        expect(error!).toBeUndefined();

        expect(mockFetch.mock.calls.length).toBe(2);
        for (let i = 0; i < 2; i++) {
            expect(mockFetch.mock.calls[i][0]).toBe(URL);
            expect(mockFetch.mock.calls[i][1]).toBeDefined();
            expect(mockFetch.mock.calls[i][1]!.method).toBe('GET');
            expect(mockFetch.mock.calls[i][1]!.headers).toBeDefined();
            expect(Object.keys(mockFetch.mock.calls[i][1]!.headers!)).toEqual([ContextHeader.CorrelationId]);
            expectTypeOf<string>(mockFetch.mock.calls[i][1]!.headers! as any[ContextHeader.CorrelationId]).toBeString();
        }
    });

    test('Test rate limit exceeded', async () => {
        const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
        mockFetch.mockImplementation(async () => {
            await sleep(100);
            return fakeResponse(true, 200);
        });

        const fetchWithRateLimit = generateFetchWithOptions({
            containerOptions: {
                rateLimit: {
                    limitForPeriod: 2,
                    limitPeriodMs: 400,
                },
            },
        });

        let error: Error;
        try {
            const start = performance.now();
            const response1 = await fetchWithRateLimit(URL, {
                method: 'GET',
            });
            expect(response1!.ok).toBeTruthy();
            expect(response1!.status).toBe(200);
            const response1end = performance.now();
            expect(response1end - start).toBeGreaterThanOrEqual(100);
            expect(response1end - start).toBeLessThan(150);

            const response2 = await fetchWithRateLimit(URL, {
                method: 'GET',
            });
            expect(response2!.ok).toBeTruthy();
            expect(response2!.status).toBe(200);
            const response2end = performance.now();
            expect(response2end - start).toBeGreaterThanOrEqual(200);
            expect(response2end - start).toBeLessThan(250);

            await fetchWithRateLimit(URL, {
                method: 'GET',
            });
            const end = performance.now();
            expect(end - start).toBeGreaterThanOrEqual(400);
        } catch (caughtError) {
            error = caughtError as Error;
        }

        expect(error!).toBeUndefined();

        expect(mockFetch.mock.calls.length).toBe(3);
        for (let i = 0; i < 3; i++) {
            expect(mockFetch.mock.calls[i][0]).toBe(URL);
            expect(mockFetch.mock.calls[i][1]).toBeDefined();
            expect(mockFetch.mock.calls[i][1]!.method).toBe('GET');
            expect(mockFetch.mock.calls[i][1]!.headers).toBeDefined();
            expect(Object.keys(mockFetch.mock.calls[i][1]!.headers!)).toEqual([ContextHeader.CorrelationId]);
            expectTypeOf<string>(mockFetch.mock.calls[i][1]!.headers! as any[ContextHeader.CorrelationId]).toBeString();
        }
    });

    test('Test rate limit not exceeded', async () => {
        vi.useFakeTimers();
        const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;

        mockFetch.mockImplementation(async () => {
            await vi.advanceTimersByTimeAsync(600);
            return fakeResponse(true, 200);
        });

        const fetchWithRateLimit = generateFetchWithOptions({
            containerOptions: {
                rateLimit: {
                    limitForPeriod: 2,
                    limitPeriodMs: 1000,
                },
            },
        });

        let error: Error;
        try {
            const response1 = await fetchWithRateLimit(URL, {
                method: 'GET',
            });
            const response2 = await fetchWithRateLimit(URL, {
                method: 'GET',
            });
            const response3 = await fetchWithRateLimit(URL, {
                method: 'GET',
            });

            expect(response1!.ok).toBeTruthy();
            expect(response1!.status).toBe(200);
            expect(response2!.ok).toBeTruthy();
            expect(response2!.status).toBe(200);
            expect(response3!.ok).toBeTruthy();
            expect(response3!.status).toBe(200);
        } catch (caughtError) {
            error = caughtError as Error;
        }

        expect(error!).toBeUndefined();

        expect(mockFetch.mock.calls.length).toBe(3);
        for (let i = 0; i < 3; i++) {
            expect(mockFetch.mock.calls[i][0]).toBe(URL);
            expect(mockFetch.mock.calls[i][1]).toBeDefined();
            expect(mockFetch.mock.calls[i][1]!.method).toBe('GET');
            expect(mockFetch.mock.calls[i][1]!.headers).toBeDefined();
            expect(Object.keys(mockFetch.mock.calls[i][1]!.headers!)).toEqual([ContextHeader.CorrelationId]);
            expectTypeOf<string>(mockFetch.mock.calls[i][1]!.headers! as any[ContextHeader.CorrelationId]).toBeString();
        }
    });

    test('Test error in response', async () => {
        const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
        mockFetch.mockResolvedValue(
            fakeResponse(false, 400, {
                error: {
                    message: 'Error message 123',
                    type: 'ERROR 124',
                    code: 125,
                },
            }),
        );

        let error: Error;
        try {
            await fetch(URL, {
                method: 'GET',
            });
        } catch (caughtError) {
            error = caughtError as Error;
        }

        expect(error!).toBeInstanceOf(HTTPResponseError);
        let httpError = error! as HTTPResponseError;
        expect(httpError.response.ok).toBeFalsy();
        expect(httpError.response.status).toBe(400);
        expect(httpError.message).toContain('Error message 123');
        expect(httpError.message).toContain('ERROR 124');
        expect(httpError.message).toContain('125');

        error = undefined as any;
        mockFetch.mockResolvedValue(
            fakeResponse(false, 400, {
                error: 'Error message 126',
            }),
        );

        try {
            await fetch(URL, {
                method: 'GET',
            });
        } catch (caughtError) {
            error = caughtError as Error;
        }

        expect(error!).toBeInstanceOf(HTTPResponseError);
        httpError = error! as HTTPResponseError;
        expect(httpError.response.ok).toBeFalsy();
        expect(httpError.response.status).toBe(400);
        expect(httpError.message).toContain('Error message 126');

        error = undefined as any;
        mockFetch.mockResolvedValue(fakeResponse(false, 400, null, 'Error message 127', false));

        try {
            await fetch(URL, {
                method: 'GET',
            });
        } catch (caughtError) {
            error = caughtError as Error;
        }

        expect(error!).toBeInstanceOf(HTTPResponseError);
        httpError = error! as HTTPResponseError;
        expect(httpError.response.ok).toBeFalsy();
        expect(httpError.response.status).toBe(400);
        expect(httpError.message).toContain('Error message 127');
    });
});
