import { TimeoutError } from 'mollitia';
import { describe, expect, test } from 'vitest';
import fetch, { FetchBuilder } from './fetch';

/**
 * A bounded connect timeout must fail fast on a black-holed connect instead of
 * stalling until the (much larger) whole-request timeout.
 *
 * Regression coverage mirroring the Rust port (SMOODEV-2513 / SMOODEV-2498):
 * api-prime's ~16s stalls were fresh SYNs to dead pod IPs still lingering in a
 * ClusterIP's iptables. Without a connect timeout, undici waits the full
 * whole-request timeout; with one, the connect fails in ~the configured window
 * and retry can land on a live pod.
 *
 * 10.255.255.1 is a non-routable RFC1918 address with (almost certainly) no host
 * answering: the SYN is dropped/black-holed so the connect never establishes.
 */
const BLACK_HOLE_URL = 'http://10.255.255.1:80/anything';

describe('connect timeout', () => {
    test('fails fast on a black-holed connect, well under the whole-request timeout', async () => {
        const start = Date.now();
        // Whole-request timeout is 10x the connect timeout — if the connect timeout
        // is NOT honored, this would run ~5s and blow the elapsed assertion.
        // Retry disabled so the connect window isn't multiplied across attempts.
        const promise = fetch(BLACK_HOLE_URL, {
            options: {
                connectTimeoutMs: 500,
                timeout: { timeoutMs: 5000 },
                retry: { attempts: 0, initialIntervalMs: 0 },
            },
        });

        await expect(promise).rejects.toThrow();
        // A connect timeout surfaces as an undici request error, NOT the mollitia
        // whole-request TimeoutError (which would mean the connect timeout never fired).
        await expect(promise).rejects.not.toBeInstanceOf(TimeoutError);

        const elapsed = Date.now() - start;
        expect(elapsed).toBeLessThan(3000);
    }, 10000);

    test('FetchBuilder.withConnectTimeout wires the same fast-fail path', async () => {
        const client = new FetchBuilder().withConnectTimeout(500).withTimeout(5000).withRetry({ attempts: 0, initialIntervalMs: 0 }).build();

        const start = Date.now();
        await expect(client(BLACK_HOLE_URL)).rejects.toThrow();
        expect(Date.now() - start).toBeLessThan(3000);
    }, 10000);

    test('unset connect timeout is behavior-identical (still reaches the black hole and fails)', async () => {
        // Without a connect timeout the connect still fails on a black hole, just
        // bounded by the whole-request timeout instead. Proves the default path
        // does not attach a dispatcher / change behavior.
        await expect(
            fetch(BLACK_HOLE_URL, {
                options: { timeout: { timeoutMs: 800 }, retry: { attempts: 0, initialIntervalMs: 0 } },
            }),
        ).rejects.toThrow();
    }, 10000);
});
