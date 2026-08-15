"""Trace-context propagation on egress.

The gap these guard: api-prime EXTRACTS ``traceparent`` on ingress, but nothing ever
INJECTED it, so every service-to-service call began a new root trace. Measured
2026-08-14 over three hours: 34,961 traces touched one service, 4 touched two.

Asserted at the WIRE — what a real local HTTP server actually received — rather than
against a mock transport. A header we believe we set and the server never sees is the
exact failure being fixed, and respx would happily agree with our own bookkeeping.
"""

import sys
import threading
from collections.abc import Iterator
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import pytest
from opentelemetry import context as otel_context
from opentelemetry import trace

from smooai_fetch import FetchOptions, fetch

TRACE_ID = 0x4BF92F3577B34DA6A3CE929D0E0E4736
SPAN_ID = 0x00F067AA0BA902B7


@pytest.fixture
def wire() -> Iterator[tuple[str, list[dict[str, str]]]]:
    """A real local HTTP server; yields (url, headers-received-per-request)."""
    received: list[dict[str, str]] = []

    class Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler's contract
            received.append({k.lower(): v for k, v in self.headers.items()})
            body = b"{}"
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            _ = self.wfile.write(body)

        def log_message(self, format: str, *args: object) -> None:
            pass  # keep pytest output clean

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}/x", received
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)


@pytest.fixture
def active_span() -> Iterator[None]:
    """Activate a valid (non-recording) span context — no SDK required."""
    span = trace.NonRecordingSpan(
        trace.SpanContext(
            trace_id=TRACE_ID,
            span_id=SPAN_ID,
            is_remote=False,
            trace_flags=trace.TraceFlags(trace.TraceFlags.SAMPLED),
        )
    )
    token = otel_context.attach(trace.set_span_in_context(span))
    try:
        yield
    finally:
        otel_context.detach(token)


async def test_active_span_is_propagated_as_traceparent_header(
    wire: tuple[str, list[dict[str, str]]], active_span: None
) -> None:
    """An active span produces a well-formed traceparent carrying its trace id."""
    url, received = wire

    response = await fetch(url)

    assert response.ok
    assert len(received) == 1
    traceparent = received[0].get("traceparent")
    assert traceparent is not None, "the server received no traceparent header"

    version, trace_id, span_id, flags = traceparent.split("-")
    assert version == "00"
    assert trace_id == format(TRACE_ID, "032x")
    assert span_id == format(SPAN_ID, "016x")
    assert flags == "01"


async def test_no_active_span_means_no_traceparent_header(wire: tuple[str, list[dict[str, str]]]) -> None:
    """No valid span context injects NOTHING — never an all-zero traceparent.

    An absent span yields INVALID_SPAN_CONTEXT (all-zero ids). Injecting that writes a
    malformed traceparent the downstream service may reject, or worse adopt, poisoning
    its trace. The sibling logger shipped exactly this bug.
    """
    url, received = wire

    response = await fetch(url)

    assert response.ok
    assert len(received) == 1
    assert "traceparent" not in received[0]


async def test_missing_opentelemetry_is_a_no_op_not_a_crash(
    wire: tuple[str, list[dict[str, str]]], active_span: None, monkeypatch: pytest.MonkeyPatch
) -> None:
    """``opentelemetry-api`` is an extra: absent, the client still works and injects nothing.

    A ``None`` entry in ``sys.modules`` makes ``import`` raise ImportError, which is what a
    consumer who installed plain ``smooai-fetch`` sees. An active span is deliberately in
    scope so the only reason nothing is injected is the missing dependency.
    """
    url, received = wire
    for name in list(sys.modules):
        if name == "opentelemetry" or name.startswith("opentelemetry."):
            monkeypatch.setitem(sys.modules, name, None)

    response = await fetch(url)

    assert response.ok
    assert len(received) == 1
    assert "traceparent" not in received[0]


async def test_caller_traceparent_is_never_overwritten(
    wire: tuple[str, list[dict[str, str]]], active_span: None
) -> None:
    """An explicitly-set traceparent survives untouched, active span or not."""
    url, received = wire
    caller = "00-11111111111111111111111111111111-2222222222222222-01"

    response = await fetch(url, FetchOptions(headers={"traceparent": caller}))

    assert response.ok
    assert len(received) == 1
    assert received[0].get("traceparent") == caller
