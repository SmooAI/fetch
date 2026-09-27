"""Retry safety: only idempotent (or explicitly opted-in) requests are retried,
and a timed-out attempt is cancelled rather than abandoned (SMOODEV-3375).

Every case comes from spec/retry-idempotency-corpus.json, shared with the other
four ports -- do not inline cases here. They run against a REAL local server that
counts the requests it receives, because the bug is a side effect executing more
than once server-side, and only the server can count that.
"""

from __future__ import annotations

import asyncio
import json
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import pytest

from smooai_fetch import (
    IDEMPOTENCY_KEY_HEADER,
    FetchOptions,
    RetryError,
    RetryOptions,
    TimeoutOptions,
    fetch,
    is_idempotent_method,
)
from smooai_fetch._retry import is_retry_eligible

CORPUS: dict[str, Any] = json.loads((Path(__file__).parents[2] / "spec" / "retry-idempotency-corpus.json").read_text())


@dataclass
class _Connection:
    arrived_at: float
    closed_at: float | None = None


@dataclass
class _Server:
    """Minimal HTTP/1.1 server: answers each request per `respond`, or never (hang)."""

    respond: dict[str, Any]
    requests: int = 0
    connections: list[_Connection] = field(default_factory=list)
    _server: asyncio.Server | None = None
    _writers: list[asyncio.StreamWriter] = field(default_factory=list)

    async def _handle(self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
        conn = _Connection(arrived_at=time.monotonic())
        self.connections.append(conn)
        self._writers.append(writer)
        try:
            head = await reader.readuntil(b"\r\n\r\n")
            length = 0
            for line in head.decode("latin-1").split("\r\n")[1:]:
                name, _, value = line.partition(":")
                if name.strip().lower() == "content-length":
                    length = int(value.strip())
            if length:
                await reader.readexactly(length)
            self.requests += 1

            if not self.respond.get("hang"):
                extra = "".join(f"{k}: {v}\r\n" for k, v in self.respond.get("headers", {}).items())
                status_line = f"HTTP/1.1 {self.respond['status']} X\r\n"
                writer.write(f"{status_line}Content-Length: 0\r\nConnection: close\r\n{extra}\r\n".encode())
                await writer.drain()
                return

            # Hang: never answer. Block until the CLIENT closes the connection --
            # read() returns b"" at EOF -- and record when that happened.
            while await reader.read(1024):
                pass
            conn.closed_at = time.monotonic()
        except (asyncio.IncompleteReadError, ConnectionError):
            conn.closed_at = time.monotonic()
        finally:
            writer.close()

    async def __aenter__(self) -> _Server:
        self._server = await asyncio.start_server(self._handle, "127.0.0.1", 0)
        return self

    async def __aexit__(self, *_: object) -> None:
        assert self._server is not None
        self._server.close()
        for writer in self._writers:
            writer.close()

    @property
    def url(self) -> str:
        assert self._server is not None
        port = self._server.sockets[0].getsockname()[1]
        return f"http://127.0.0.1:{port}/anything"


def _retry_options(allow_non_idempotent: bool = False) -> RetryOptions:
    knobs = CORPUS["retry"]
    return RetryOptions(
        attempts=knobs["attempts"],
        initial_interval_ms=knobs["initialIntervalMs"],
        factor=knobs["factor"],
        jitter=knobs["jitterAdjustment"],
        allow_non_idempotent=allow_non_idempotent,
    )


def test_corpus_loaded() -> None:
    # Positive control: a corpus that failed to load would parametrize nothing
    # and every assertion below would pass vacuously.
    assert len(CORPUS["cases"]) >= 10
    assert CORPUS["methods"]["idempotent"] and CORPUS["methods"]["nonIdempotent"]
    assert CORPUS["idempotencyKeyHeader"] == IDEMPOTENCY_KEY_HEADER


@pytest.mark.parametrize("method", CORPUS["methods"]["idempotent"])
def test_idempotent_methods(method: str) -> None:
    assert is_idempotent_method(method)


@pytest.mark.parametrize("method", CORPUS["methods"]["nonIdempotent"])
def test_non_idempotent_methods(method: str) -> None:
    assert not is_idempotent_method(method)


@pytest.mark.parametrize("case", CORPUS["cases"], ids=[c["name"] for c in CORPUS["cases"]])
async def test_server_side_attempts(case: dict[str, Any]) -> None:
    headers = case.get("headers", {})
    if any(isinstance(v, str) and v and not v.strip() for v in headers.values()):
        # h11 refuses to put a whitespace-only header value on the wire (it
        # raises LocalProtocolError before connecting), so this case can never
        # reach a server from Python. Assert the rule it pins down directly.
        assert not is_retry_eligible(case["method"], headers, _retry_options(case.get("allowNonIdempotent", False)))
        return
    async with _Server(respond=case["respond"]) as server:
        options = FetchOptions(
            method=case["method"],
            headers=dict(case.get("headers", {})),
            body=case.get("body"),
            retry=_retry_options(case.get("allowNonIdempotent", False)),
            timeout=TimeoutOptions(timeout_ms=CORPUS["timeoutMs"]),
        )
        with pytest.raises(Exception) as exc_info:
            await fetch(server.url, options)

    assert server.requests == case["expectedAttempts"], (
        f"{case['name']}: server received {server.requests} request(s), expected {case['expectedAttempts']}"
    )
    if case["expectedAttempts"] == 1:
        # A request that was not retried surfaces its own error, not the
        # retries-exhausted wrapper.
        assert not isinstance(exc_info.value, RetryError)


async def _wait_until_closed(conn: _Connection, deadline_s: float) -> None:
    end = time.monotonic() + deadline_s
    while conn.closed_at is None and time.monotonic() < end:
        await asyncio.sleep(0.01)


async def test_timed_out_get_closes_first_connection_before_retry() -> None:
    knobs = CORPUS["timeoutAbort"]
    ceiling_s = (knobs["timeoutMs"] + knobs["closeSlackMs"]) / 1000.0
    async with _Server(respond={"hang": True}) as server:
        options = FetchOptions(
            method="GET",
            retry=_retry_options(),
            timeout=TimeoutOptions(timeout_ms=knobs["timeoutMs"]),
        )
        with pytest.raises(Exception):
            await fetch(server.url, options)

        assert len(server.connections) == CORPUS["retry"]["attempts"] + 1
        first, second = server.connections[0], server.connections[1]
        await _wait_until_closed(first, ceiling_s)

    assert first.closed_at is not None, "first attempt's connection was never closed -- the timeout abandoned it"
    assert first.closed_at - first.arrived_at <= ceiling_s
    assert first.closed_at <= second.arrived_at, "retry was sent while the timed-out attempt was still open"


async def test_timed_out_post_closes_its_connection() -> None:
    knobs = CORPUS["timeoutAbort"]
    ceiling_s = (knobs["timeoutMs"] + knobs["closeSlackMs"]) / 1000.0
    async with _Server(respond={"hang": True}) as server:
        options = FetchOptions(
            method="POST",
            body='{"a":1}',
            retry=_retry_options(),
            timeout=TimeoutOptions(timeout_ms=knobs["timeoutMs"]),
        )
        with pytest.raises(Exception):
            await fetch(server.url, options)

        assert len(server.connections) == 1
        conn = server.connections[0]
        await _wait_until_closed(conn, ceiling_s)

    assert conn.closed_at is not None, "timed-out POST's connection was never closed"
    assert conn.closed_at - conn.arrived_at <= ceiling_s


async def test_builder_path_is_gated_too() -> None:
    from smooai_fetch import FetchBuilder

    async with _Server(respond={"status": 503}) as server:
        builder = FetchBuilder().with_retry(_retry_options())
        with pytest.raises(Exception):
            await builder.fetch(server.url, method="POST", body='{"a":1}')
    assert server.requests == 1


async def test_pre_request_hook_can_add_idempotency_key() -> None:
    from smooai_fetch import LifecycleHooks

    def add_key(url: str, kwargs: dict[str, Any]) -> tuple[str, dict[str, Any]]:
        kwargs["headers"] = {**kwargs.get("headers", {}), IDEMPOTENCY_KEY_HEADER: "hook-key"}
        return url, kwargs

    async with _Server(respond={"status": 503}) as server:
        options = FetchOptions(
            method="POST",
            body='{"a":1}',
            retry=_retry_options(),
            hooks=LifecycleHooks(pre_request=add_key),
        )
        with pytest.raises(RetryError):
            await fetch(server.url, options)
    assert server.requests == CORPUS["retry"]["attempts"] + 1
