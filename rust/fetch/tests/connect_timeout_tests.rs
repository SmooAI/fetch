//! Test that a bounded connect timeout fails fast on a black-holed connect
//! instead of stalling until the (much larger) whole-request timeout.
//!
//! Regression coverage for SMOODEV-2498 / SMOODEV-2481: api-prime's ~16s stalls
//! were fresh SYNs to dead pod IPs still lingering in a ClusterIP's iptables.
//! Without a connect timeout, reqwest waits the full whole-request timeout; with
//! one, the connect fails in ~the configured window and retry lands on a live pod.

use std::time::{Duration, Instant};

use serde_json::Value;
use smooai_fetch::client;
use smooai_fetch::error::FetchError;
use smooai_fetch::types::{FetchOptions, Method, RequestInit, TimeoutOptions};

#[tokio::test]
async fn connect_timeout_fails_fast_on_black_hole() {
    // 10.255.255.1 is a non-routable RFC1918 address with (almost certainly) no
    // host answering: the SYN is dropped/black-holed, so the connect never
    // establishes and would otherwise hang until the whole-request timeout.
    let url = "http://10.255.255.1:80/anything";

    let connect_timeout_ms = 500;
    // Whole-request timeout is 10x the connect timeout — if the connect timeout
    // is NOT honored, this test would take ~10s and the elapsed assertion fails.
    let options = FetchOptions {
        connect_timeout_ms: Some(connect_timeout_ms),
        timeout: Some(TimeoutOptions { timeout_ms: 5_000 }),
        retry: None,
    };

    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };

    let start = Instant::now();
    let result = client::fetch::<Value>(url, init, Some(options), None, None, None, None).await;
    let elapsed = start.elapsed();

    // The connect must fail (not succeed against a black hole).
    assert!(result.is_err(), "expected connect to fail, got {result:?}");
    // reqwest surfaces a connect timeout as a request error, not our whole-request
    // Timeout variant (which would mean the connect timeout never fired).
    match result.unwrap_err() {
        FetchError::Request(_) => {}
        other => panic!("expected FetchError::Request from connect timeout, got {other:?}"),
    }
    // Must fail in roughly the connect window, well under the 5s whole timeout.
    assert!(
        elapsed < Duration::from_secs(3),
        "connect timeout did not fire fast: elapsed {elapsed:?} (connect_timeout was {connect_timeout_ms}ms)"
    );
}
