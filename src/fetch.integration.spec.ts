import { describe, it, expect, beforeAll, afterEach, afterAll } from 'vitest';
import { setupServer } from 'msw/node';
import { http, HttpResponse, delay } from 'msw';
import { FetchBuilder, HTTPResponseError, TimeoutError } from './fetch';
import fetch from './fetch';
import { z } from 'zod';

// Define test schema
const TestSchema = z.object({
    id: z.string(),
    name: z.string(),
    age: z.number(),
});

type TestSchema = z.infer<typeof TestSchema>;

interface TestRequestBody {
    name: string;
    age: number;
}

// Setup MSW server
const server = setupServer(
    // Basic GET endpoint
    http.get('https://example.com/api/test', () => {
        return HttpResponse.json(
            {
                id: '1',
                name: 'Test User',
                age: 25,
            },
            {
                headers: {
                    'Content-Type': 'application/json',
                    'X-Custom-Header': 'test-value',
                },
            },
        );
    }),

    // POST endpoint
    http.post('https://example.com/api/test', async ({ request }) => {
        const body = (await request.json()) as TestRequestBody;
        const authHeader = request.headers.get('Authorization');
        if (!authHeader?.startsWith('Bearer ')) {
            return new HttpResponse(null, { status: 401 });
        }
        return HttpResponse.json(
            {
                id: '1',
                name: body.name,
                age: body.age,
            },
            {
                headers: {
                    'Content-Type': 'application/json',
                    'X-Custom-Header': 'test-value',
                },
            },
        );
    }),

    // Network error endpoint
    http.get('https://example.com/api/network-error', () => {
        return HttpResponse.error();
    }),

    // Timeout endpoint - delays response
    http.get('https://example.com/api/timeout', async () => {
        await delay(500);
        return HttpResponse.json({
            id: '1',
            name: 'Timeout Success',
            age: 40,
        });
    }),

    // Error endpoint
    http.get('https://example.com/api/error', () => {
        return new HttpResponse(null, {
            status: 400,
            statusText: 'Bad Request',
        });
    }),
);

describe('Fetch Integration Tests', () => {
    beforeAll(() => {
        server.listen();
    });

    afterEach(() => {
        server.resetHandlers();
    });

    afterAll(() => {
        server.close();
    });

    describe('Basic Requests', () => {
        it('should make a successful GET request', async () => {
            const fetch = new FetchBuilder(TestSchema).build();

            const response = await fetch('https://example.com/api/test');
            expect(response.data).toEqual({
                id: '1',
                name: 'Test User',
                age: 25,
            });
            expect(response.headers.get('Content-Type')).toBe('application/json');
            expect(response.headers.get('X-Custom-Header')).toBe('test-value');
        });

        it('should make a successful POST request with headers', async () => {
            const fetch = new FetchBuilder(TestSchema).build();

            const response = await fetch('https://example.com/api/test', {
                method: 'POST',
                headers: {
                    Authorization: 'Bearer test-token',
                    'X-Custom-Header': 'custom-value',
                },
                body: JSON.stringify({
                    name: 'New User',
                    age: 30,
                }),
            });

            expect(response.data).toEqual({
                id: '1',
                name: 'New User',
                age: 30,
            });
            expect(response.headers.get('Content-Type')).toBe('application/json');
            expect(response.headers.get('X-Custom-Header')).toBe('test-value');
        });

        it('should fail with 401 when missing Authorization header', async () => {
            const fetch = new FetchBuilder(TestSchema).build();

            await expect(
                fetch('https://example.com/api/test', {
                    method: 'POST',
                    body: JSON.stringify({
                        name: 'New User',
                        age: 30,
                    }),
                }),
            ).rejects.toThrow(HTTPResponseError);
        });
    });

    describe('Network Errors', () => {
        it('should handle network errors', async () => {
            const fetch = new FetchBuilder<typeof TestSchema>().build();

            await expect(fetch('https://example.com/api/network-error')).rejects.toThrow();
        });
    });

    describe('Timeout', () => {
        it('should throw TimeoutError when request takes too long', async () => {
            const fetch = new FetchBuilder<typeof TestSchema>().withTimeout(300).build();

            await expect(fetch('https://example.com/api/timeout')).rejects.toThrow(TimeoutError);
        });
    });

    describe('Error Handling', () => {
        it('should throw HTTPResponseError for non-2xx responses', async () => {
            const fetch = new FetchBuilder<typeof TestSchema>().build();

            await expect(fetch('https://example.com/api/error')).rejects.toThrow(HTTPResponseError);
        });
    });

    describe('Schema Validation', () => {
        it('should validate response against schema', async () => {
            const fetch = new FetchBuilder(TestSchema).build();

            const response = await fetch('https://example.com/api/test');
            expect(response.data).toMatchObject({
                id: expect.any(String),
                name: expect.any(String),
                age: expect.any(Number),
            });
        });

        it('should throw validation error for invalid response', async () => {
            // Override the test endpoint to return invalid data
            server.use(
                http.get('https://example.com/api/test', () => {
                    return HttpResponse.json({
                        id: 1, // Should be string
                        name: 'Test User',
                        age: '25', // Should be number
                    });
                }),
            );

            const fetch = new FetchBuilder(TestSchema).build();

            await expect(fetch('https://example.com/api/test')).rejects.toThrow();
        });

        it('should validate response using built-in fetch with schema option', async () => {
            const response = await fetch('https://example.com/api/test', {
                options: {
                    schema: TestSchema,
                },
            });

            expect(response.data).toEqual({
                id: '1',
                name: 'Test User',
                age: 25,
            });
            expect(response.headers.get('Content-Type')).toBe('application/json');
            expect(response.headers.get('X-Custom-Header')).toBe('test-value');
        });

        it('should throw validation error using built-in fetch with invalid schema', async () => {
            // Override the test endpoint to return invalid data
            server.use(
                http.get('https://example.com/api/test', () => {
                    return HttpResponse.json({
                        id: 1, // Should be string
                        name: 'Test User',
                        age: '25', // Should be number
                    });
                }),
            );

            await expect(
                fetch('https://example.com/api/test', {
                    options: {
                        schema: TestSchema,
                    },
                }),
            ).rejects.toThrow();
        });
    });
});
