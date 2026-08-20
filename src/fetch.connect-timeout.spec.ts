import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { TimeoutError } from 'mollitia';
import { describe, expect, test } from 'vitest';
import fetch, { FetchBuilder } from './fetch';

/**
 * A bounded connect timeout must fail fast on a black-holed connect instead of
 * stalling until the (much larger) whole-request timeout.
 *
 * The knobs come from spec/connect-timeout-corpus.json, shared with the Python,
 * Rust, Go and .NET suites — see the corpus for why they are not inlined here.
 */
const corpus = JSON.parse(readFileSync(join(dirname(fileURLToPath(import.meta.url)), '..', 'spec', 'connect-timeout-corpus.json'), 'utf8')) as {
    blackHoleUrl: string;
    connectTimeoutMs: number;
    wholeRequestTimeoutMs: number;
    maxElapsedMs: number;
};

const TEST_TIMEOUT_MS = corpus.wholeRequestTimeoutMs * 2;

describe('connect timeout', () => {
    // Positive control: a corpus that failed to load would leave every knob
    // undefined and the elapsed assertions trivially satisfiable.
    test('loaded the shared corpus', () => {
        expect(corpus.blackHoleUrl).toMatch(/^http/);
        expect(corpus.connectTimeoutMs).toBeGreaterThan(0);
        expect(corpus.maxElapsedMs).toBeGreaterThan(corpus.connectTimeoutMs);
        expect(corpus.wholeRequestTimeoutMs).toBeGreaterThan(corpus.maxElapsedMs);
    });

    test(
        'fails fast on a black-holed connect, well under the whole-request timeout',
        async () => {
            const start = Date.now();
            // Retry disabled so the connect window isn't multiplied across attempts.
            const promise = fetch(corpus.blackHoleUrl, {
                options: {
                    connectTimeoutMs: corpus.connectTimeoutMs,
                    timeout: { timeoutMs: corpus.wholeRequestTimeoutMs },
                    retry: { attempts: 0, initialIntervalMs: 0 },
                },
            });

            await expect(promise).rejects.toThrow();
            // A connect timeout surfaces as an undici request error, NOT the mollitia
            // whole-request TimeoutError (which would mean the connect timeout never fired).
            await expect(promise).rejects.not.toBeInstanceOf(TimeoutError);

            expect(Date.now() - start).toBeLessThan(corpus.maxElapsedMs);
        },
        TEST_TIMEOUT_MS,
    );

    test(
        'FetchBuilder.withConnectTimeout wires the same fast-fail path',
        async () => {
            const client = new FetchBuilder()
                .withConnectTimeout(corpus.connectTimeoutMs)
                .withTimeout(corpus.wholeRequestTimeoutMs)
                .withRetry({ attempts: 0, initialIntervalMs: 0 })
                .build();

            const start = Date.now();
            await expect(client(corpus.blackHoleUrl)).rejects.toThrow();
            expect(Date.now() - start).toBeLessThan(corpus.maxElapsedMs);
        },
        TEST_TIMEOUT_MS,
    );

    test(
        'unset connect timeout is behavior-identical (still reaches the black hole and fails)',
        async () => {
            // Without a connect timeout the connect still fails on a black hole, just
            // bounded by the whole-request timeout instead. Proves the default path
            // does not attach a dispatcher / change behavior.
            await expect(
                fetch(corpus.blackHoleUrl, {
                    options: { timeout: { timeoutMs: 800 }, retry: { attempts: 0, initialIntervalMs: 0 } },
                }),
            ).rejects.toThrow();
        },
        TEST_TIMEOUT_MS,
    );
});
