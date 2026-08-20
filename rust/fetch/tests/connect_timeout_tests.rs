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

/// The knobs are shared with the other four ports — see the corpus itself for
/// why they must not be inlined here. `include_str!` binds it at compile time,
/// so a corpus change cannot silently miss this suite.
const CORPUS: &str = include_str!("../../../spec/connect-timeout-corpus.json");

#[tokio::test]
async fn connect_timeout_fails_fast_on_black_hole() {
    let corpus: Value = serde_json::from_str(CORPUS).expect("connect-timeout corpus must parse");
    let url = corpus["blackHoleUrl"].as_str().expect("blackHoleUrl");
    let connect_timeout_ms = corpus["connectTimeoutMs"]
        .as_u64()
        .expect("connectTimeoutMs");
    let whole_timeout_ms = corpus["wholeRequestTimeoutMs"]
        .as_u64()
        .expect("wholeRequestTimeoutMs");
    let max_elapsed_ms = corpus["maxElapsedMs"].as_u64().expect("maxElapsedMs");

    let options = FetchOptions {
        connect_timeout_ms: Some(connect_timeout_ms),
        timeout: Some(TimeoutOptions {
            timeout_ms: whole_timeout_ms,
        }),
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
    // Must fail in roughly the connect window, well under the whole timeout.
    assert!(
        elapsed < Duration::from_millis(max_elapsed_ms),
        "connect timeout did not fire fast: elapsed {elapsed:?} (connect_timeout was {connect_timeout_ms}ms, whole timeout {whole_timeout_ms}ms)"
    );
}
