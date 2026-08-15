import http from 'node:http';
import type { AddressInfo } from 'node:net';
import { afterAll, beforeAll, expect, it, vi } from 'vitest';

/**
 * `@opentelemetry/api` is an OPTIONAL peer dependency. Simulate it being absent —
 * the import rejects exactly as it does when the package is not installed — and
 * assert the client still works: no injection, no crash.
 */
vi.mock('@opentelemetry/api', () => {
    throw new Error("Cannot find module '@opentelemetry/api'");
});

const fetch = (await import('./fetch')).default;

let server: http.Server;
let baseUrl: string;
const received: http.IncomingHttpHeaders[] = [];

beforeAll(async () => {
    server = http.createServer((req, res) => {
        received.push(req.headers);
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end('{"ok":true}');
    });
    await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
    baseUrl = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
});

afterAll(async () => {
    await new Promise<void>((resolve) => server.close(() => resolve()));
});

it('still fetches, without a traceparent, when @opentelemetry/api is not installed', async () => {
    // Prove the simulated absence is actually in effect for this module registry —
    // otherwise this test would pass for the wrong reason (no active span).
    await expect(import('@opentelemetry/api')).rejects.toThrow();

    const response = await fetch(`${baseUrl}/x`);

    expect(response.ok).toBe(true);
    expect(received).toHaveLength(1);
    expect(received[0].traceparent).toBeUndefined();
});
