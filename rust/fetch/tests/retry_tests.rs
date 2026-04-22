//! Tests for retry logic.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Respond, ResponseTemplate};

use smooai_fetch::client;
use smooai_fetch::error::FetchError;
use smooai_fetch::retry::{calculate_backoff, is_retryable};
use smooai_fetch::types::{
    FetchOptions, Method, RequestInit, RetryContext, RetryDecision, RetryOptions, TimeoutOptions,
};

#[test]
fn test_is_retryable_status_codes() {
    assert!(is_retryable(429));
    assert!(is_retryable(500));
    assert!(is_retryable(502));
    assert!(is_retryable(503));
    assert!(is_retryable(504));
    assert!(!is_retryable(200));
    assert!(!is_retryable(201));
    assert!(!is_retryable(301));
    assert!(!is_retryable(400));
    assert!(!is_retryable(401));
    assert!(!is_retryable(403));
    assert!(!is_retryable(404));
}

#[test]
fn test_calculate_backoff_no_jitter() {
    let options = RetryOptions {
        attempts: 3,
        initial_interval_ms: 100,
        factor: 2.0,
        jitter_adjustment: 0.0,
        max_interval_ms: None,
        fast_first: false,
        on_rejection: None,
    };

    assert_eq!(calculate_backoff(0, &options), 100); // 100 * 2^0
    assert_eq!(calculate_backoff(1, &options), 200); // 100 * 2^1
    assert_eq!(calculate_backoff(2, &options), 400); // 100 * 2^2
    assert_eq!(calculate_backoff(3, &options), 800); // 100 * 2^3
}

#[test]
fn test_calculate_backoff_with_max_interval() {
    let options = RetryOptions {
        attempts: 5,
        initial_interval_ms: 100,
        factor: 2.0,
        jitter_adjustment: 0.0,
        max_interval_ms: Some(300),
        fast_first: false,
        on_rejection: None,
    };

    assert_eq!(calculate_backoff(0, &options), 100);
    assert_eq!(calculate_backoff(1, &options), 200);
    assert_eq!(calculate_backoff(2, &options), 300); // Capped at 300
    assert_eq!(calculate_backoff(3, &options), 300); // Capped at 300
}

#[test]
fn test_calculate_backoff_with_jitter_is_bounded() {
    let options = RetryOptions {
        attempts: 3,
        initial_interval_ms: 1000,
        factor: 1.0,
        jitter_adjustment: 0.5,
        max_interval_ms: None,
        fast_first: false,
        on_rejection: None,
    };

    for _ in 0..100 {
        let delay = calculate_backoff(0, &options);
        assert!(delay >= 500, "delay {} should be >= 500", delay);
        assert!(delay <= 1500, "delay {} should be <= 1500", delay);
    }
}

/// A responder that returns different responses based on the call count.
struct CountingResponder {
    call_count: Arc<AtomicU32>,
    responses: Vec<ResponseTemplate>,
}

impl Respond for CountingResponder {
    fn respond(&self, _request: &wiremock::Request) -> ResponseTemplate {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let idx = count as usize;
        if idx < self.responses.len() {
            self.responses[idx].clone()
        } else {
            self.responses.last().unwrap().clone()
        }
    }
}

#[tokio::test]
async fn test_retry_succeeds_after_failures() {
    let mock_server = MockServer::start().await;
    let call_count = Arc::new(AtomicU32::new(0));

    let responder = CountingResponder {
        call_count: call_count.clone(),
        responses: vec![
            ResponseTemplate::new(500)
                .set_body_json(serde_json::json!({"error": "Server error"}))
                .insert_header("content-type", "application/json"),
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "success"}))
                .insert_header("content-type", "application/json"),
        ],
    };

    Mock::given(method("GET"))
        .and(path("/retry"))
        .respond_with(responder)
        .mount(&mock_server)
        .await;

    let url = format!("{}/retry", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: Some(RetryOptions {
            attempts: 2,
            initial_interval_ms: 50,
            factor: 1.0,
            jitter_adjustment: 0.0,
            max_interval_ms: None,
            fast_first: false,
            on_rejection: None,
        }),
    };

    let response = client::fetch::<serde_json::Value>(&url, init, Some(options), None, None, None)
        .await
        .unwrap();

    assert!(response.ok);
    assert_eq!(response.status, 200);
    assert_eq!(call_count.load(Ordering::SeqCst), 2); // 1 failure + 1 success
}

#[tokio::test]
async fn test_retry_exhausted() {
    let mock_server = MockServer::start().await;
    let call_count = Arc::new(AtomicU32::new(0));

    let responder = CountingResponder {
        call_count: call_count.clone(),
        responses: vec![
            ResponseTemplate::new(500)
                .set_body_json(serde_json::json!({"error": "err1"}))
                .insert_header("content-type", "application/json"),
            ResponseTemplate::new(502)
                .set_body_json(serde_json::json!({"error": "err2"}))
                .insert_header("content-type", "application/json"),
            ResponseTemplate::new(503)
                .set_body_json(serde_json::json!({"error": "err3"}))
                .insert_header("content-type", "application/json"),
        ],
    };

    Mock::given(method("GET"))
        .and(path("/retry-exhaust"))
        .respond_with(responder)
        .mount(&mock_server)
        .await;

    let url = format!("{}/retry-exhaust", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: Some(RetryOptions {
            attempts: 2,
            initial_interval_ms: 50,
            factor: 1.0,
            jitter_adjustment: 0.0,
            max_interval_ms: None,
            fast_first: false,
            on_rejection: None,
        }),
    };

    let result =
        client::fetch::<serde_json::Value>(&url, init, Some(options), None, None, None).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        FetchError::Retry { attempts, source } => {
            assert_eq!(attempts, 2);
            // The source should be the last error (503)
            match *source {
                FetchError::HttpResponse { status, .. } => {
                    assert_eq!(status, 503);
                }
                other => panic!("Expected HttpResponse in source, got {:?}", other),
            }
        }
        other => panic!("Expected Retry error, got {:?}", other),
    }
    assert_eq!(call_count.load(Ordering::SeqCst), 3); // 1 initial + 2 retries
}

#[tokio::test]
async fn test_non_retryable_error_not_retried() {
    let mock_server = MockServer::start().await;
    let call_count = Arc::new(AtomicU32::new(0));

    let responder = CountingResponder {
        call_count: call_count.clone(),
        responses: vec![ResponseTemplate::new(404)
            .set_body_json(serde_json::json!({"error": "Not found"}))
            .insert_header("content-type", "application/json")],
    };

    Mock::given(method("GET"))
        .and(path("/no-retry"))
        .respond_with(responder)
        .mount(&mock_server)
        .await;

    let url = format!("{}/no-retry", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: Some(RetryOptions {
            attempts: 3,
            initial_interval_ms: 50,
            factor: 1.0,
            jitter_adjustment: 0.0,
            max_interval_ms: None,
            fast_first: false,
            on_rejection: None,
        }),
    };

    let result =
        client::fetch::<serde_json::Value>(&url, init, Some(options), None, None, None).await;

    assert!(result.is_err());
    // 404 is not retryable, so only 1 call should be made
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_retry_with_retry_after_header() {
    let mock_server = MockServer::start().await;
    let call_count = Arc::new(AtomicU32::new(0));

    let responder = CountingResponder {
        call_count: call_count.clone(),
        responses: vec![
            ResponseTemplate::new(429)
                .set_body_json(serde_json::json!({"error": "Rate limited"}))
                .insert_header("content-type", "application/json")
                .insert_header("retry-after", "1"),
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"status": "ok"}))
                .insert_header("content-type", "application/json"),
        ],
    };

    Mock::given(method("GET"))
        .and(path("/retry-after"))
        .respond_with(responder)
        .mount(&mock_server)
        .await;

    let url = format!("{}/retry-after", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 10000 }),
        retry: Some(RetryOptions {
            attempts: 1,
            initial_interval_ms: 50,
            factor: 1.0,
            jitter_adjustment: 0.0,
            max_interval_ms: None,
            fast_first: false,
            on_rejection: None,
        }),
    };

    let start = std::time::Instant::now();
    let response = client::fetch::<serde_json::Value>(&url, init, Some(options), None, None, None)
        .await
        .unwrap();

    let elapsed = start.elapsed();
    assert!(response.ok);
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
    // Should have waited at least 1 second due to Retry-After header
    assert!(
        elapsed.as_millis() >= 900,
        "Should have waited for Retry-After, elapsed: {:?}",
        elapsed
    );
}

// --- fast_first ---------------------------------------------------------

#[tokio::test]
async fn test_fast_first_skips_initial_delay() {
    let mock_server = MockServer::start().await;
    let call_count = Arc::new(AtomicU32::new(0));

    // Fail once, then succeed. With fast_first, the sole retry should fire
    // immediately (well under the 2s configured backoff).
    let responder = CountingResponder {
        call_count: call_count.clone(),
        responses: vec![
            ResponseTemplate::new(500)
                .set_body_json(serde_json::json!({"error": "boom"}))
                .insert_header("content-type", "application/json"),
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"ok": true}))
                .insert_header("content-type", "application/json"),
        ],
    };

    Mock::given(method("GET"))
        .and(path("/fast-first"))
        .respond_with(responder)
        .mount(&mock_server)
        .await;

    let url = format!("{}/fast-first", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: Some(RetryOptions {
            attempts: 1,
            initial_interval_ms: 2_000, // deliberately large
            factor: 2.0,
            jitter_adjustment: 0.0,
            max_interval_ms: None,
            fast_first: true,
            on_rejection: None,
        }),
    };

    let start = std::time::Instant::now();
    let response = client::fetch::<serde_json::Value>(&url, init, Some(options), None, None, None)
        .await
        .unwrap();
    let elapsed = start.elapsed();

    assert!(response.ok);
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
    // Without fast_first we would sleep ~2000ms; with it, the retry should be
    // well under 500ms even under heavy CI load.
    assert!(
        elapsed.as_millis() < 500,
        "fast_first should eliminate the 2s backoff; elapsed was {:?}",
        elapsed
    );
}

// --- on_rejection -------------------------------------------------------

fn failing_mock_server_responder(call_count: Arc<AtomicU32>) -> CountingResponder {
    CountingResponder {
        call_count,
        responses: vec![
            ResponseTemplate::new(500)
                .set_body_json(serde_json::json!({"error": "boom"}))
                .insert_header("content-type", "application/json"),
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"ok": true}))
                .insert_header("content-type", "application/json"),
        ],
    }
}

#[tokio::test]
async fn test_on_rejection_retry_decision_overrides_delay() {
    let mock_server = MockServer::start().await;
    let call_count = Arc::new(AtomicU32::new(0));

    Mock::given(method("GET"))
        .and(path("/on-rejection-retry"))
        .respond_with(failing_mock_server_responder(call_count.clone()))
        .mount(&mock_server)
        .await;

    let seen_attempts = Arc::new(AtomicU32::new(0));
    let seen_attempts_cb = seen_attempts.clone();

    let callback: smooai_fetch::types::RetryCallback = Arc::new(move |ctx: &RetryContext| {
        seen_attempts_cb.store(ctx.attempt, Ordering::SeqCst);
        // Sanity-check we got a retryable 5xx through last_status.
        assert_eq!(ctx.last_status, Some(500));
        RetryDecision::Retry {
            delay: std::time::Duration::from_millis(17),
        }
    });

    let url = format!("{}/on-rejection-retry", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: Some(RetryOptions {
            attempts: 1,
            initial_interval_ms: 5_000,
            factor: 2.0,
            jitter_adjustment: 0.0,
            max_interval_ms: None,
            fast_first: false,
            on_rejection: Some(callback),
        }),
    };

    let start = std::time::Instant::now();
    let response = client::fetch::<serde_json::Value>(&url, init, Some(options), None, None, None)
        .await
        .unwrap();
    let elapsed = start.elapsed();

    assert!(response.ok);
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
    assert_eq!(seen_attempts.load(Ordering::SeqCst), 1);
    // ~17ms delay; allow generous upper bound but confirm we did not sleep the
    // configured 5s.
    assert!(
        elapsed.as_millis() >= 10 && elapsed.as_millis() < 1_000,
        "callback delay was not honored; elapsed: {:?}",
        elapsed
    );
}

#[tokio::test]
async fn test_on_rejection_abort_stops_retry_loop() {
    let mock_server = MockServer::start().await;
    let call_count = Arc::new(AtomicU32::new(0));

    let responder = CountingResponder {
        call_count: call_count.clone(),
        responses: vec![ResponseTemplate::new(500)
            .set_body_json(serde_json::json!({"error": "boom"}))
            .insert_header("content-type", "application/json")],
    };

    Mock::given(method("GET"))
        .and(path("/on-rejection-abort"))
        .respond_with(responder)
        .mount(&mock_server)
        .await;

    let callback: smooai_fetch::types::RetryCallback =
        Arc::new(|_ctx: &RetryContext| RetryDecision::Abort);

    let url = format!("{}/on-rejection-abort", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: Some(RetryOptions {
            attempts: 3,
            initial_interval_ms: 50,
            factor: 1.0,
            jitter_adjustment: 0.0,
            max_interval_ms: None,
            fast_first: false,
            on_rejection: Some(callback),
        }),
    };

    let result =
        client::fetch::<serde_json::Value>(&url, init, Some(options), None, None, None).await;

    // Abort surfaces the underlying HttpResponse error rather than wrapping in
    // `FetchError::Retry`.
    assert!(matches!(
        result,
        Err(FetchError::HttpResponse { status: 500, .. })
    ));
    // Exactly one HTTP call — no retry was performed.
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_on_rejection_default_falls_through_to_exponential() {
    // With `RetryDecision::Default`, behavior must match a run with no
    // callback registered. Using `initial_interval_ms = 50` we expect the
    // retry to succeed within a reasonable window (comparable to the
    // no-callback path used elsewhere in this file).
    let mock_server = MockServer::start().await;
    let call_count = Arc::new(AtomicU32::new(0));

    Mock::given(method("GET"))
        .and(path("/on-rejection-default"))
        .respond_with(failing_mock_server_responder(call_count.clone()))
        .mount(&mock_server)
        .await;

    let callback: smooai_fetch::types::RetryCallback =
        Arc::new(|_ctx: &RetryContext| RetryDecision::Default);

    let url = format!("{}/on-rejection-default", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: Some(RetryOptions {
            attempts: 1,
            initial_interval_ms: 50,
            factor: 1.0,
            jitter_adjustment: 0.0,
            max_interval_ms: None,
            fast_first: false,
            on_rejection: Some(callback),
        }),
    };

    let response = client::fetch::<serde_json::Value>(&url, init, Some(options), None, None, None)
        .await
        .unwrap();
    assert!(response.ok);
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_on_rejection_skip_consumes_attempt_without_sleep() {
    // Server always fails. With attempts=2 and `RetryDecision::Skip` returned
    // on every call, the loop should burn its 2 retries with no sleeps and
    // surface a Retry error.
    let mock_server = MockServer::start().await;
    let call_count = Arc::new(AtomicU32::new(0));

    let responder = CountingResponder {
        call_count: call_count.clone(),
        responses: vec![ResponseTemplate::new(500)
            .set_body_json(serde_json::json!({"error": "boom"}))
            .insert_header("content-type", "application/json")],
    };

    Mock::given(method("GET"))
        .and(path("/on-rejection-skip"))
        .respond_with(responder)
        .mount(&mock_server)
        .await;

    let callback: smooai_fetch::types::RetryCallback =
        Arc::new(|_ctx: &RetryContext| RetryDecision::Skip);

    let url = format!("{}/on-rejection-skip", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: Some(RetryOptions {
            attempts: 2,
            initial_interval_ms: 5_000, // would be painful if not skipped
            factor: 1.0,
            jitter_adjustment: 0.0,
            max_interval_ms: None,
            fast_first: false,
            on_rejection: Some(callback),
        }),
    };

    let start = std::time::Instant::now();
    let result =
        client::fetch::<serde_json::Value>(&url, init, Some(options), None, None, None).await;
    let elapsed = start.elapsed();

    assert!(matches!(result, Err(FetchError::Retry { .. })));
    assert_eq!(call_count.load(Ordering::SeqCst), 3); // 1 + 2 retries
    assert!(
        elapsed.as_millis() < 1_000,
        "Skip should bypass the 5s backoff; elapsed: {:?}",
        elapsed
    );
}
