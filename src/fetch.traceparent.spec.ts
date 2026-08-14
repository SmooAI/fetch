import http from 'node:http';
import type { AddressInfo } from 'node:net';
import { trace } from '@opentelemetry/api';
import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
import { afterAll, beforeAll, beforeEach, describe, expect, it } from 'vitest';
import fetch from './fetch';

/**
 * Trace-context propagation on egress.
 *
 * The gap these guard: api-prime EXTRACTS `traceparent` on ingress, but nothing
 * ever INJECTED it, so every service-to-service call began a new root trace.
 * Measured 2026-08-14 over three hours: 34,961 traces touched one service, 4
 * touched two.
 *
 * Asserted at the WIRE — against a real HTTP server, on the headers it actually
 * received — rather than against a mock. A header we believe we set and the
 * server never sees is the exact failure being fixed.
 */

// `register()` installs the W3C propagator and the AsyncLocalStorage context
// manager, i.e. the shape a real service runs in.
new NodeTracerProvider().register();

const TRACEPARENT = /^00-[0-9a-f]{32}-[0-9a-f]{16}-0[01]$/;

let server: http.Server;
let baseUrl: string;
let received: http.IncomingHttpHeaders[] = [];
let failNext = 0;

beforeAll(async () => {
    server = http.createServer((req, res) => {
        received.push(req.headers);
        const status = failNext-- > 0 ? 503 : 200;
        res.writeHead(status, { 'Content-Type': 'application/json' });
        res.end('{"ok":true}');
    });
    await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
    baseUrl = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
});

afterAll(async () => {
    await new Promise<void>((resolve) => server.close(() => resolve()));
});

beforeEach(() => {
    received = [];
    failNext = 0;
});

describe('traceparent injection on egress', () => {
    it('sends a traceparent carrying the active trace id', async () => {
        const tracer = trace.getTracer('fetch-propagation-test');
        const traceId = await tracer.startActiveSpan('caller', async (span) => {
            await fetch(`${baseUrl}/x`);
            span.end();
            return span.spanContext().traceId;
        });

        expect(received).toHaveLength(1);
        expect(received[0].traceparent).toMatch(TRACEPARENT);
        expect(received[0].traceparent).toContain(traceId);
    });

    it('sends no traceparent when there is no active span', async () => {
        await fetch(`${baseUrl}/x`);

        // No active span yields INVALID_SPAN_CONTEXT — all-zero ids. Injecting that
        // writes a malformed traceparent the downstream service may reject, or worse
        // adopt, poisoning its trace.
        expect(received).toHaveLength(1);
        expect(received[0].traceparent).toBeUndefined();
    });

    it('never overwrites a caller-supplied traceparent', async () => {
        const caller = '00-11111111111111111111111111111111-2222222222222222-01';
        const tracer = trace.getTracer('fetch-propagation-test');
        await tracer.startActiveSpan('caller', async (span) => {
            await fetch(`${baseUrl}/x`, { headers: { traceparent: caller } });
            span.end();
        });

        expect(received[0].traceparent).toBe(caller);
    });

    it('injects on every attempt, so a retry carries a current traceparent', async () => {
        failNext = 1;
        const tracer = trace.getTracer('fetch-propagation-test');
        await tracer.startActiveSpan('caller', async (span) => {
            await fetch(`${baseUrl}/x`);
            span.end();
        });

        // Injecting at the top-level entry instead of the single-request site, or
        // mutating the shared init, would leave the retry without a fresh header.
        expect(received.length).toBeGreaterThan(1);
        for (const headers of received) {
            expect(headers.traceparent).toMatch(TRACEPARENT);
        }
    });
});
