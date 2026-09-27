import { readFileSync } from 'node:fs';
import { createServer, IncomingMessage, Server, ServerResponse } from 'node:http';
import { AddressInfo } from 'node:net';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { TimeoutError } from 'mollitia';
import { afterEach, describe, expect, test } from 'vitest';
import fetch, { FetchBuilder, HTTPResponseError, isIdempotentMethod, RetryError } from './fetch';

/**
 * Retries must never duplicate a side effect (SMOODEV-3375).
 *
 * Every case comes from spec/retry-idempotency-corpus.json, shared with the
 * Python, Rust, Go and .NET suites, and runs against a REAL local server that
 * counts what it received — a mocked fetch can only count what the client
 * attempted, and the bug is what the server executed.
 */
type CorpusCase = {
    name: string;
    method: string;
    body?: string;
    headers?: Record<string, string>;
    allowNonIdempotent?: boolean;
    respond: { status?: number; headers?: Record<string, string>; hang?: boolean };
    expectedAttempts: number;
};

const corpus = JSON.parse(readFileSync(join(dirname(fileURLToPath(import.meta.url)), '..', 'spec', 'retry-idempotency-corpus.json'), 'utf8')) as {
    methods: { idempotent: string[]; nonIdempotent: string[] };
    retry: { attempts: number; initialIntervalMs: number; factor: number; jitterAdjustment: number };
    timeoutMs: number;
    cases: CorpusCase[];
    timeoutAbort: { timeoutMs: number; closeSlackMs: number };
};

type Arrival = { method: string; arrivedAt: number; closedAt?: number };

const servers: Server[] = [];
const hanging: ServerResponse[] = [];

afterEach(async () => {
    for (const res of hanging.splice(0)) res.destroy();
    await Promise.all(servers.splice(0).map((server) => new Promise((resolve) => server.close(resolve))));
});

/** A server that answers every request with `respond`, recording each arrival and when its connection closed. */
async function startServer(respond: CorpusCase['respond']): Promise<{ url: string; arrivals: Arrival[] }> {
    const arrivals: Arrival[] = [];
    const server = createServer((req: IncomingMessage, res: ServerResponse) => {
        const arrival: Arrival = { method: req.method ?? '', arrivedAt: Date.now() };
        arrivals.push(arrival);
        req.socket.once('close', () => {
            arrival.closedAt = Date.now();
        });
        req.resume();
        if (respond.hang) {
            hanging.push(res);
            return;
        }
        res.writeHead(respond.status ?? 200, respond.headers ?? {});
        res.end();
    });
    servers.push(server);
    await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
    return { url: `http://127.0.0.1:${(server.address() as AddressInfo).port}/`, arrivals };
}

const retryOptions = (allowNonIdempotent?: boolean) => ({
    attempts: corpus.retry.attempts,
    initialIntervalMs: corpus.retry.initialIntervalMs,
    factor: corpus.retry.factor,
    jitterAdjustment: corpus.retry.jitterAdjustment,
    ...(allowNonIdempotent ? { allowNonIdempotent } : {}),
});

describe('retry idempotency corpus', () => {
    // Positive control: a corpus that failed to load would leave the case table
    // empty and every per-case assertion vacuously satisfied.
    test('loaded the shared corpus', () => {
        expect(corpus.cases.length).toBeGreaterThan(10);
        expect(corpus.methods.idempotent).toContain('GET');
        expect(corpus.methods.nonIdempotent).toContain('POST');
    });

    test.each(corpus.methods.idempotent)('%s is idempotent', (method) => {
        expect(isIdempotentMethod(method)).toBe(true);
    });

    test.each(corpus.methods.nonIdempotent)('%s is not idempotent', (method) => {
        expect(isIdempotentMethod(method)).toBe(false);
    });

    test.each(corpus.cases.map((c) => [c.name, c] as const))('%s', async (_name, c) => {
        const { url, arrivals } = await startServer(c.respond);

        const promise = fetch(url, {
            method: c.method,
            headers: c.headers,
            body: c.body,
            options: { retry: retryOptions(c.allowNonIdempotent), timeout: { timeoutMs: corpus.timeoutMs } },
        });

        const error = await promise.then(
            () => undefined,
            (e: unknown) => e,
        );
        expect(error).toBeDefined();
        expect(arrivals.length).toBe(c.expectedAttempts);
        // An unretried request surfaces its own error, never "ran out of retries".
        if (c.expectedAttempts === 1) {
            expect(error).not.toBeInstanceOf(RetryError);
            expect(error instanceof HTTPResponseError || error instanceof TimeoutError).toBe(true);
        }
    });

    test('a pre-request hook that adds an Idempotency-Key makes a POST retry-eligible', async () => {
        const { url, arrivals } = await startServer({ status: 503 });
        const client = new FetchBuilder()
            .withRetry(retryOptions())
            .withHooks({
                preRequest: (u, init) => [u, { ...init, headers: { ...(init.headers as Record<string, string>), 'Idempotency-Key': 'from-hook' } }],
            })
            .build();

        await expect(client(url, { method: 'POST', body: '{}' })).rejects.toBeInstanceOf(RetryError);
        expect(arrivals.length).toBe(corpus.retry.attempts + 1);
    });

    test('FetchBuilder.withRetry({ allowNonIdempotent }) keeps the other defaults', async () => {
        const { url, arrivals } = await startServer({ status: 503 });
        const client = new FetchBuilder().withRetry({ allowNonIdempotent: true, initialIntervalMs: 1 }).build();

        await expect(client(url, { method: 'POST', body: '{}' })).rejects.toBeInstanceOf(RetryError);
        // DEFAULT_RETRY_OPTIONS.attempts (2) survived the partial merge.
        expect(arrivals.length).toBe(3);
    });

    test('an ineligible POST never consults onRejection', async () => {
        const { url, arrivals } = await startServer({ status: 503 });
        let consulted = 0;
        await expect(
            fetch(url, {
                method: 'POST',
                options: {
                    retry: {
                        ...retryOptions(),
                        onRejection: () => {
                            consulted++;
                            return true;
                        },
                    },
                },
            }),
        ).rejects.toBeInstanceOf(HTTPResponseError);
        expect(arrivals.length).toBe(1);
        expect(consulted).toBe(0);
    });

    test('a request the caller aborted is not retried', async () => {
        const { url, arrivals } = await startServer({ hang: true });
        const controller = new AbortController();
        const promise = fetch(url, { method: 'GET', signal: controller.signal, options: { retry: retryOptions(), timeout: { timeoutMs: 5_000 } } });
        await expect.poll(() => arrivals.length).toBe(1);
        controller.abort();

        await expect(promise).rejects.toThrow();
        await new Promise((resolve) => setTimeout(resolve, 100));
        expect(arrivals.length).toBe(1);
    });
});

describe('timeouts abort the attempt', () => {
    const { timeoutMs, closeSlackMs } = corpus.timeoutAbort;

    test('a timed-out GET closes its connection before the retry is sent', async () => {
        const { url, arrivals } = await startServer({ hang: true });

        // A 100ms backoff instead of the corpus's 1ms: with 1ms, whether the
        // server observes "first socket closed" or "second request arrived" first
        // is down to event-loop scheduling. An attempt that is abandoned rather
        // than cancelled stays open for the whole test, so the wider gap cannot
        // hide a missing abort.
        await expect(
            fetch(url, {
                method: 'GET',
                options: { retry: { attempts: 1, initialIntervalMs: 100, jitterAdjustment: 0 }, timeout: { timeoutMs } },
            }),
        ).rejects.toBeInstanceOf(TimeoutError);

        expect(arrivals.length).toBe(2);
        const [first, second] = arrivals;
        expect(first.closedAt, "the timed-out attempt's connection was never closed — it was abandoned, not cancelled").toBeDefined();
        expect(first.closedAt! - first.arrivedAt).toBeLessThanOrEqual(timeoutMs + closeSlackMs);
        expect(first.closedAt!).toBeLessThanOrEqual(second.arrivedAt);
    });

    test('a timed-out POST closes its connection and is sent exactly once', async () => {
        const { url, arrivals } = await startServer({ hang: true });

        await expect(fetch(url, { method: 'POST', body: '{}', options: { timeout: { timeoutMs } } })).rejects.toBeInstanceOf(TimeoutError);

        await expect.poll(() => arrivals[0]?.closedAt, { timeout: closeSlackMs }).toBeDefined();
        expect(arrivals[0].closedAt! - arrivals[0].arrivedAt).toBeLessThanOrEqual(timeoutMs + closeSlackMs);
        await new Promise((resolve) => setTimeout(resolve, 200));
        expect(arrivals.length).toBe(1);
    });
});
