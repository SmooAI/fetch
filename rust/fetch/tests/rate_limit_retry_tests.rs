//! Tests for `FetchContainerOptions::rate_limit_retry` / `FetchBuilder::with_rate_limit_retry`.

use std::time::Instant;

use serde::Deserialize;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use smooai_fetch::builder::FetchBuilder;
use smooai_fetch::types::{Method, RequestInit, RetryOptions};

#[derive(Deserialize, Clone, Debug, PartialEq)]
struct TestData {
    ok: bool,
}

/// When a rate-limit retry policy is configured, a transient rate-limit rejection
/// is retried inside a dedicated inner loop and the request eventually succeeds.
#[tokio::test]
async fn test_rate_limit_retry_recovers() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"ok": true}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let rl_retry = RetryOptions {
        attempts: 3,
        initial_interval_ms: 10,
        factor: 1.0,
        jitter_adjustment: 0.0,
        max_interval_ms: Some(50),
        fast_first: true,
        on_rejection: None,
    };

    // limit=1 over a 100ms window. The first request burns the slot.
    let client = FetchBuilder::<TestData>::new()
        .without_retry()
        .with_rate_limit(1, 100)
        .with_rate_limit_retry(rl_retry)
        .build();

    let url = format!("{}/data", mock_server.uri());
    let make = || RequestInit {
        method: Method::GET,
        ..Default::default()
    };

    // Burn the slot.
    let first = client.fetch(&url, make()).await.unwrap();
    assert!(first.ok);

    // The next request should be rate-limited, then retry once the window
    // expires (≤100ms) and succeed.
    let start = Instant::now();
    let second = client.fetch(&url, make()).await.unwrap();
    let elapsed = start.elapsed();
    assert!(second.ok);
    // Should have waited *some* amount of time but well under 1s.
    assert!(elapsed.as_millis() < 1_000);
}

/// When the rate-limit retry budget is exhausted, the caller gets a `Retry`
/// error wrapping the underlying rate-limit rejection.
#[tokio::test]
async fn test_rate_limit_retry_exhausts() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"ok": true}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    // Aggressively short delays so the test stays fast — limit=1 over a 5s
    // window guarantees we never escape the rate limit within the budget.
    let rl_retry = RetryOptions {
        attempts: 2,
        initial_interval_ms: 5,
        factor: 1.0,
        jitter_adjustment: 0.0,
        max_interval_ms: Some(10),
        fast_first: false,
        on_rejection: None,
    };

    let client = FetchBuilder::<TestData>::new()
        .without_retry()
        .with_rate_limit(1, 5_000)
        .with_rate_limit_retry(rl_retry)
        .build();

    let url = format!("{}/data", mock_server.uri());
    let make = || RequestInit {
        method: Method::GET,
        ..Default::default()
    };

    // Burn the slot.
    let first = client.fetch(&url, make()).await.unwrap();
    assert!(first.ok);

    // The retry budget runs out before the 5s window expires.
    let result = client.fetch(&url, make()).await;
    assert!(
        result.is_err(),
        "expected exhausted retry to error, got {:?}",
        result
    );
}
