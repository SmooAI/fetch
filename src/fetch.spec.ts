/* eslint-disable @typescript-eslint/no-explicit-any -- ok*/
import { TimeoutError } from 'mollitia';
import { beforeEach, describe, expect, expectTypeOf, MockedFunction, test, vi } from 'vitest';
import fetch, { FetchBuilder, HTTPResponseError, RequestInfo, Response, RetryError, DEFAULT_RATE_LIMIT_RETRY_OPTIONS, RequestInitWithOptions } from './fetch';
import { ContextHeader } from '@smooai/logger/AwsLambdaLogger';
import sleep from '@smooai/utils/utils/sleep';
import { z } from 'zod';

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
                await vi.runAllTimersAsync();
            })();
            return fakeResponse(false, 429);
        });
        mockFetch.mockImplementationOnce(async () => {
            (async () => {
                await vi.runAllTimersAsync();
            })();
            return fakeResponse(false, 500);
        });
        mockFetch.mockImplementationOnce(async () => {
            (async () => {
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
                await vi.runAllTimersAsync();
            })();
            return fakeResponse(false, 429);
        });
        mockFetch.mockImplementationOnce(async () => {
            (async () => {
                await vi.runAllTimersAsync();
            })();
            return fakeResponse(false, 500);
        });
        mockFetch.mockImplementationOnce(async () => {
            (async () => {
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

        const fetchWithRateLimit = new FetchBuilder().withRateLimit(2, 400, DEFAULT_RATE_LIMIT_RETRY_OPTIONS).build();

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

        const fetchWithRateLimit = new FetchBuilder().withRateLimit(2, 1000, DEFAULT_RATE_LIMIT_RETRY_OPTIONS).build();

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

    describe('Schema validation', () => {
        test('Test successful schema validation', async () => {
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
            const mockData = { id: '123', name: 'test' };
            mockFetch.mockResolvedValue(fakeResponse(true, 200, mockData));

            const schema = z.object({
                id: z.string(),
                name: z.string(),
            });

            const fetchWithSchema = new FetchBuilder<typeof schema>().withSchema(schema).build();

            const response = await fetchWithSchema(URL, {
                method: 'GET',
            });

            expect(response.ok).toBeTruthy();
            expect(response.status).toBe(200);
            expect(response.data).toEqual(mockData);
            expect(schema.safeParse(response.data).success).toBeTruthy();
        });

        test('Test failed schema validation', async () => {
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
            const mockData = { id: 123, name: 'test' }; // id is number, should be string
            mockFetch.mockResolvedValue(fakeResponse(true, 200, mockData));

            const schema = z.object({
                id: z.string(),
                name: z.string(),
            });

            const fetchWithSchema = new FetchBuilder<typeof schema>().withSchema(schema).build();

            let error: Error | undefined;
            try {
                await fetchWithSchema(URL, {
                    method: 'GET',
                });
                throw new Error('Expected schema validation to fail');
            } catch (caughtError) {
                error = caughtError as Error;
            }

            expect(error).toBeDefined();
            expect(error!.message).toContain('Expected string, received number at "id"');
        });

        test('Test multiple schema validation errors', async () => {
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
            const mockData = {
                id: 123, // should be string
                name: 456, // should be string
                age: 'not a number', // should be number
                email: 'invalid-email', // should be valid email
            };
            mockFetch.mockResolvedValue(fakeResponse(true, 200, mockData));

            const schema = z.object({
                id: z.string(),
                name: z.string(),
                age: z.number(),
                email: z.string().email(),
            });

            const fetchWithSchema = new FetchBuilder<typeof schema>().withSchema(schema).build();

            let error: Error | undefined;
            try {
                await fetchWithSchema(URL, {
                    method: 'GET',
                });
                throw new Error('Expected schema validation to fail');
            } catch (caughtError) {
                error = caughtError as Error;
            }

            expect(error).toBeDefined();
            expect(error!.message).toContain('1. Expected string, received number at "id"');
            expect(error!.message).toContain('2. Expected string, received number at "name"');
            expect(error!.message).toContain('3. Expected number, received string at "age"');
            expect(error!.message).toContain('4. Invalid email at "email"');
        });

        test('Test schema validation with nested objects', async () => {
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
            const mockData = {
                user: {
                    id: '123',
                    name: 'test',
                    preferences: {
                        theme: 'dark',
                        notifications: true,
                    },
                },
                timestamp: '2024-03-20T12:00:00Z',
            };
            mockFetch.mockResolvedValue(fakeResponse(true, 200, mockData));

            const schema = z.object({
                user: z.object({
                    id: z.string(),
                    name: z.string(),
                    preferences: z.object({
                        theme: z.string(),
                        notifications: z.boolean(),
                    }),
                }),
                timestamp: z.string().datetime(),
            });

            const fetchWithSchema = new FetchBuilder<typeof schema>().withSchema(schema).build();

            const response = await fetchWithSchema(URL, {
                method: 'GET',
            });

            expect(response.ok).toBeTruthy();
            expect(response.status).toBe(200);
            expect(response.data).toEqual(mockData);
            expect(schema.safeParse(response.data).success).toBeTruthy();
        });

        test('Test schema validation with arrays', async () => {
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
            const mockData = {
                items: [
                    { id: '1', name: 'item 1' },
                    { id: '2', name: 'item 2' },
                ],
                total: 2,
            };
            mockFetch.mockResolvedValue(fakeResponse(true, 200, mockData));

            const schema = z.object({
                items: z.array(
                    z.object({
                        id: z.string(),
                        name: z.string(),
                    }),
                ),
                total: z.number(),
            });

            const fetchWithSchema = new FetchBuilder<typeof schema>().withSchema(schema).build();

            const response = await fetchWithSchema(URL, {
                method: 'GET',
            });

            expect(response.ok).toBeTruthy();
            expect(response.status).toBe(200);
            expect(response.data).toEqual(mockData);
            expect(schema.safeParse(response.data).success).toBeTruthy();
        });

        test('Test schema validation with non-JSON response', async () => {
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
            mockFetch.mockResolvedValue(fakeResponse(true, 200, null, 'plain text response', false));

            const schema = z.object({
                id: z.string(),
                name: z.string(),
            });

            const fetchWithSchema = new FetchBuilder<typeof schema>().withSchema(schema).build();

            const response = await fetchWithSchema(URL, {
                method: 'GET',
            });

            expect(response.ok).toBeTruthy();
            expect(response.status).toBe(200);
            expect(response.isJson).toBeFalsy();
            expect(response.dataString).toBe('plain text response');
            expect(response.data).toBeUndefined();
            expect(schema.safeParse(response.data).success).toBeFalsy();
        });

        test('Test request with predefined headers', async () => {
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
            const mockData = { id: '123', name: 'test' };
            mockFetch.mockResolvedValue(fakeResponse(true, 200, mockData));

            const schema = z.object({
                id: z.string(),
                name: z.string(),
            });

            const predefinedHeaders = {
                'X-Custom-Header': 'custom-value',
                'X-Request-ID': 'req-123',
                'X-Environment': 'test',
            };

            const fetchWithSchema = new FetchBuilder<typeof schema>()
                .withSchema(schema)
                .withInit({
                    headers: predefinedHeaders,
                })
                .build();

            const response = await fetchWithSchema(URL, {
                method: 'GET',
            });

            expect(response.ok).toBeTruthy();
            expect(response.status).toBe(200);
            expect(response.data).toEqual(mockData);

            // Verify headers were sent correctly
            expect(mockFetch.mock.calls[0][1]?.headers).toBeDefined();
            const sentHeaders = mockFetch.mock.calls[0][1]?.headers as Record<string, string>;
            expect(sentHeaders['X-Custom-Header']).toBe('custom-value');
            expect(sentHeaders['X-Request-ID']).toBe('req-123');
            expect(sentHeaders['X-Environment']).toBe('test');
            // Verify context headers are still present
            expect(sentHeaders[ContextHeader.CorrelationId]).toBeDefined();
        });

        test('Test request with authentication', async () => {
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
            const mockData = { id: '123', name: 'test' };
            mockFetch.mockResolvedValue(fakeResponse(true, 200, mockData));

            const schema = z.object({
                id: z.string(),
                name: z.string(),
            });

            const authToken = 'Bearer test-token-123';
            const fetchWithSchema = new FetchBuilder<typeof schema>()
                .withSchema(schema)
                .withInit({
                    headers: {
                        Authorization: authToken,
                    },
                })
                .build();

            const response = await fetchWithSchema(URL, {
                method: 'GET',
            });

            expect(response.ok).toBeTruthy();
            expect(response.status).toBe(200);
            expect(response.data).toEqual(mockData);

            // Verify auth header was sent correctly
            expect(mockFetch.mock.calls[0][1]?.headers).toBeDefined();
            const sentHeaders = mockFetch.mock.calls[0][1]?.headers as Record<string, string>;
            expect(sentHeaders['Authorization']).toBe(authToken);
            // Verify context headers are still present
            expect(sentHeaders[ContextHeader.CorrelationId]).toBeDefined();
        });

        test('Test request with merged headers', async () => {
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
            const mockData = { id: '123', name: 'test' };
            mockFetch.mockResolvedValue(fakeResponse(true, 200, mockData));

            const schema = z.object({
                id: z.string(),
                name: z.string(),
            });

            const predefinedHeaders = {
                'X-Custom-Header': 'custom-value',
                Authorization: 'Bearer test-token-123',
            };

            const fetchWithSchema = new FetchBuilder<typeof schema>()
                .withSchema(schema)
                .withInit({
                    headers: predefinedHeaders,
                })
                .build();

            const requestHeaders = {
                'X-Request-ID': 'req-123',
                'X-Environment': 'test',
            };

            const response = await fetchWithSchema(URL, {
                method: 'GET',
                headers: requestHeaders,
            });

            expect(response.ok).toBeTruthy();
            expect(response.status).toBe(200);
            expect(response.data).toEqual(mockData);

            // Verify all headers were merged correctly
            expect(mockFetch.mock.calls[0][1]?.headers).toBeDefined();
            const sentHeaders = mockFetch.mock.calls[0][1]?.headers as Record<string, string>;
            expect(sentHeaders['X-Custom-Header']).toBe('custom-value');
            expect(sentHeaders['Authorization']).toBe('Bearer test-token-123');
            expect(sentHeaders['X-Request-ID']).toBe('req-123');
            expect(sentHeaders['X-Environment']).toBe('test');
            // Verify context headers are still present
            expect(sentHeaders[ContextHeader.CorrelationId]).toBeDefined();
        });
    });

    describe('Test fetch with different init options', () => {
        test('Test fetch with body and content type', async () => {
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
            const requestBody = { key: 'value' };
            mockFetch.mockResolvedValue(fakeResponse(true, 200));

            const response = await fetch(URL, {
                method: 'POST',
                body: JSON.stringify(requestBody),
                headers: {
                    'Content-Type': 'application/json',
                },
            });

            expect(response.ok).toBeTruthy();
            expect(response.status).toBe(200);

            expect(mockFetch.mock.calls[0][1]?.body).toBe(JSON.stringify(requestBody));
            expect(mockFetch.mock.calls[0][1]?.headers).toBeDefined();
            const headers = mockFetch.mock.calls[0][1]?.headers as Record<string, string>;
            expect(headers['Content-Type']).toBe('application/json');
        });

        test('Test fetch with credentials', async () => {
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
            mockFetch.mockResolvedValue(fakeResponse(true, 200));

            const response = await fetch(URL, {
                method: 'GET',
                credentials: 'include',
            });

            expect(response.ok).toBeTruthy();
            expect(response.status).toBe(200);

            expect(mockFetch.mock.calls[0][1]?.credentials).toBe('include');
        });

        test('Test fetch with mode', async () => {
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
            mockFetch.mockResolvedValue(fakeResponse(true, 200));

            const response = await fetch(URL, {
                method: 'GET',
                mode: 'cors',
            });

            expect(response.ok).toBeTruthy();
            expect(response.status).toBe(200);

            expect(mockFetch.mock.calls[0][1]?.mode).toBe('cors');
        });

        test('Test fetch with redirect', async () => {
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
            mockFetch.mockResolvedValue(fakeResponse(true, 200));

            const response = await fetch(URL, {
                method: 'GET',
                redirect: 'follow',
            });

            expect(response.ok).toBeTruthy();
            expect(response.status).toBe(200);

            expect(mockFetch.mock.calls[0][1]?.redirect).toBe('follow');
        });

        test('Test fetch with referrer', async () => {
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
            mockFetch.mockResolvedValue(fakeResponse(true, 200));

            const response = await fetch(URL, {
                method: 'GET',
                referrer: 'https://example.com',
            });

            expect(response.ok).toBeTruthy();
            expect(response.status).toBe(200);

            expect(mockFetch.mock.calls[0][1]?.referrer).toBe('https://example.com');
        });

        test('Test fetch with signal', async () => {
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
            mockFetch.mockResolvedValue(fakeResponse(true, 200));

            const controller = new AbortController();
            const response = await fetch(URL, {
                method: 'GET',
                signal: controller.signal,
            });

            expect(response.ok).toBeTruthy();
            expect(response.status).toBe(200);

            expect(mockFetch.mock.calls[0][1]?.signal).toBe(controller.signal);
        });

        test('Test fetch with multiple init options combined', async () => {
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInit) => Promise<Response>>;
            const requestBody = { key: 'value' };
            mockFetch.mockResolvedValue(fakeResponse(true, 200));

            const controller = new AbortController();
            const response = await fetch(URL, {
                method: 'POST',
                body: JSON.stringify(requestBody),
                headers: {
                    'Content-Type': 'application/json',
                    'X-Custom-Header': 'custom-value',
                },
                credentials: 'include',
                mode: 'cors',
                redirect: 'follow',
                referrer: 'https://example.com',
                signal: controller.signal,
            });

            expect(response.ok).toBeTruthy();
            expect(response.status).toBe(200);

            const init = mockFetch.mock.calls[0][1];
            expect(init?.body).toBe(JSON.stringify(requestBody));
            expect(init?.headers).toBeDefined();
            const headers = init?.headers as Record<string, string>;
            expect(headers['Content-Type']).toBe('application/json');
            expect(headers['X-Custom-Header']).toBe('custom-value');
            expect(init?.credentials).toBe('include');
            expect(init?.mode).toBe('cors');
            expect(init?.redirect).toBe('follow');
            expect(init?.referrer).toBe('https://example.com');
            expect(init?.signal).toBe(controller.signal);
        });
    });

    describe('Test fetch with init.options settings', () => {
        test('Test fetch with timeout option', async () => {
            vi.useFakeTimers();
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInitWithOptions) => Promise<Response>>;
            mockFetch.mockImplementationOnce(async () => {
                await vi.advanceTimersByTimeAsync(6000);
                return fakeResponse(true, 200);
            });
            mockFetch.mockImplementationOnce(async () => {
                return fakeResponse(true, 200);
            });

            const response = await fetch(URL, {
                method: 'GET',
                options: {
                    timeout: { timeoutMs: 5000 },
                },
            });

            expect(response.ok).toBeTruthy();
            expect(response.status).toBe(200);

            // Verify the timeout was respected
            expect(mockFetch).toBeCalledTimes(2);
        });

        test('Test fetch with schema option', async () => {
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInitWithOptions) => Promise<Response>>;
            const mockData = { id: '123', name: 'test' };
            mockFetch.mockResolvedValue(fakeResponse(true, 200, mockData));

            const schema = z.object({
                id: z.string(),
                name: z.string(),
            });

            const response = await fetch(URL, {
                method: 'GET',
                options: {
                    schema,
                },
            });

            expect(response.ok).toBeTruthy();
            expect(response.status).toBe(200);
            expect(response.data).toEqual(mockData);
            expect(schema.safeParse(response.data).success).toBeTruthy();
        });

        test('Test fetch with multiple options combined', async () => {
            const mockFetch = global.fetch as MockedFunction<(url: RequestInfo, init?: RequestInitWithOptions) => Promise<Response>>;
            const mockData = { id: '123', name: 'test' };
            mockFetch.mockImplementationOnce(async () => {
                await vi.advanceTimersByTimeAsync(7000);
                return fakeResponse(true, 200, mockData);
            });
            mockFetch.mockImplementationOnce(async () => {
                return fakeResponse(true, 200, mockData);
            });

            const schema = z.object({
                id: z.string(),
                name: z.string(),
            });

            const retryOptions = {
                attempts: 3,
                initialIntervalMs: 50,
            };

            const response = await fetch(URL, {
                method: 'GET',
                options: {
                    timeout: { timeoutMs: 5000 },
                    schema,
                    retry: retryOptions,
                },
            });

            expect(response.ok).toBeTruthy();
            expect(response.status).toBe(200);
            expect(response.data).toEqual(mockData);
            expect(schema.safeParse(response.data).success).toBeTruthy();
        });
    });
});
