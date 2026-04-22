//! Integration tests combining multiple features.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Respond, ResponseTemplate};

use smooai_fetch::builder::FetchBuilder;
use smooai_fetch::circuit_breaker::CircuitState;
use smooai_fetch::error::FetchError;
use smooai_fetch::types::{Method, RequestInit, RetryOptions};

#[derive(Deserialize, Clone, Debug, PartialEq)]
struct ApiResponse {
    id: String,
    name: String,
}

/// A responder that returns different responses based on the call count.
struct SequenceResponder {
    call_count: Arc<AtomicU32>,
    responses: Vec<ResponseTemplate>,
}

impl Respond for SequenceResponder {
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
async fn test_retry_with_timeout() {
    let mock_server = MockServer::start().await;
    let call_count = Arc::new(AtomicU32::new(0));

    let responder = SequenceResponder {
        call_count: call_count.clone(),
        responses: vec![
            // First request: slow (will timeout)
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "slow"}))
                .insert_header("content-type", "application/json")
                .set_delay(Duration::from_millis(3000)),
            // Second request: fast (will succeed)
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "fast"}))
                .insert_header("content-type", "application/json"),
        ],
    };

    Mock::given(method("GET"))
        .and(path("/retry-timeout"))
        .respond_with(responder)
        .mount(&mock_server)
        .await;

    let client = FetchBuilder::<ApiResponse>::new()
        .with_timeout(200)
        .with_retry(RetryOptions {
            attempts: 2,
            initial_interval_ms: 50,
            factor: 1.0,
            jitter_adjustment: 0.0,
            max_interval_ms: None,
            fast_first: false,
            on_rejection: None,
        })
        .build();

    let url = format!("{}/retry-timeout", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };

    let response = client.fetch(&url, init).await.unwrap();
    assert!(response.ok);
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_circuit_breaker_with_retry() {
    let mock_server = MockServer::start().await;

    // Always returns 500
    Mock::given(method("GET"))
        .and(path("/cb-retry"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_json(serde_json::json!({"error": "Server error"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let client = FetchBuilder::<serde_json::Value>::new()
        .with_timeout(5000)
        .with_retry(RetryOptions {
            attempts: 1,
            initial_interval_ms: 50,
            factor: 1.0,
            jitter_adjustment: 0.0,
            max_interval_ms: None,
            fast_first: false,
            on_rejection: None,
        })
        .with_circuit_breaker(2, 1, 5000)
        .build();

    let url = format!("{}/cb-retry", mock_server.uri());

    // First request: fails (circuit breaker records failure)
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let result = client.fetch(&url, init).await;
    assert!(result.is_err());

    // Second request: fails (circuit breaker records failure, trips open)
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let result = client.fetch(&url, init).await;
    assert!(result.is_err());

    // Circuit should now be open
    let cb = client.circuit_breaker().unwrap();
    assert_eq!(cb.state().await, CircuitState::Open);

    // Third request should fail immediately with CircuitBreaker error
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let result = client.fetch(&url, init).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        FetchError::CircuitBreaker => {}
        other => panic!("Expected CircuitBreaker error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_rate_limit_with_builder() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rate-limited"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "ok"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let client = FetchBuilder::<ApiResponse>::new()
        .with_timeout(5000)
        .without_retry()
        .with_rate_limit(2, 500)
        .build();

    let url = format!("{}/rate-limited", mock_server.uri());

    // First two should succeed quickly
    let start = std::time::Instant::now();

    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let r1 = client.fetch(&url, init).await.unwrap();
    assert!(r1.ok);

    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let r2 = client.fetch(&url, init).await.unwrap();
    assert!(r2.ok);

    let elapsed_two = start.elapsed();

    // Third should be delayed by rate limiting
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let r3 = client.fetch(&url, init).await.unwrap();
    assert!(r3.ok);

    let elapsed_three = start.elapsed();
    assert!(
        elapsed_three > elapsed_two + Duration::from_millis(250),
        "Third request should be delayed by rate limiter. Two: {:?}, Three: {:?}",
        elapsed_two,
        elapsed_three
    );
}

#[tokio::test]
async fn test_hooks_with_builder() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/hooked"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "original"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let client = FetchBuilder::<ApiResponse>::new()
        .with_timeout(5000)
        .without_retry()
        .with_pre_request_hook(Box::new(move |_url, init| {
            let new_url = format!("{}/hooked", base_url);
            Some((new_url, init.clone()))
        }))
        .with_post_response_success_hook(Box::new(|_url, _init, mut response| {
            response.data = Some(ApiResponse {
                id: "hooked".to_string(),
                name: "modified".to_string(),
            });
            Some(response)
        }))
        .build();

    let url = format!("{}/wrong-path", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };

    let response = client.fetch(&url, init).await.unwrap();
    assert!(response.ok);
    assert_eq!(
        response.data,
        Some(ApiResponse {
            id: "hooked".to_string(),
            name: "modified".to_string()
        })
    );
}

#[tokio::test]
async fn test_full_pipeline_success() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/full"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "full-pipeline"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let client = FetchBuilder::<ApiResponse>::new()
        .with_timeout(5000)
        .with_retry(RetryOptions {
            attempts: 2,
            initial_interval_ms: 100,
            factor: 2.0,
            jitter_adjustment: 0.5,
            max_interval_ms: None,
            fast_first: false,
            on_rejection: None,
        })
        .with_rate_limit(10, 60_000)
        .with_circuit_breaker(5, 2, 30_000)
        .build();

    let url = format!("{}/full", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };

    let response = client.fetch(&url, init).await.unwrap();
    assert!(response.ok);
    assert_eq!(response.status, 200);
    assert_eq!(
        response.data,
        Some(ApiResponse {
            id: "1".to_string(),
            name: "full-pipeline".to_string()
        })
    );
}

#[tokio::test]
async fn test_convenience_fetch_function() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/convenience"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "convenient"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/convenience", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };

    let response = smooai_fetch::fetch::<ApiResponse>(&url, init)
        .await
        .unwrap();
    assert!(response.ok);
    assert_eq!(response.status, 200);
    assert_eq!(
        response.data,
        Some(ApiResponse {
            id: "1".to_string(),
            name: "convenient".to_string()
        })
    );
}

#[tokio::test]
async fn test_error_hook_with_builder() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/error-hook"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(serde_json::json!({"error": "Not found"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let client = FetchBuilder::<serde_json::Value>::new()
        .with_timeout(5000)
        .without_retry()
        .with_post_response_error_hook(Box::new(|url, _init, _err, _response| {
            Some(FetchError::SchemaValidation {
                message: format!("Custom error for URL: {}", url),
            })
        }))
        .build();

    let url = format!("{}/error-hook", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };

    let result = client.fetch(&url, init).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        FetchError::SchemaValidation { message } => {
            assert!(message.contains("Custom error for URL:"));
        }
        other => panic!("Expected SchemaValidation, got {:?}", other),
    }
}

#[tokio::test]
async fn test_circuit_breaker_recovery() {
    let mock_server = MockServer::start().await;
    let call_count = Arc::new(AtomicU32::new(0));

    let responder = SequenceResponder {
        call_count: call_count.clone(),
        responses: vec![
            // First 2 requests fail
            ResponseTemplate::new(500)
                .set_body_json(serde_json::json!({"error": "fail1"}))
                .insert_header("content-type", "application/json"),
            ResponseTemplate::new(500)
                .set_body_json(serde_json::json!({"error": "fail2"}))
                .insert_header("content-type", "application/json"),
            // Then succeed
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "recovered"}))
                .insert_header("content-type", "application/json"),
        ],
    };

    Mock::given(method("GET"))
        .and(path("/recover"))
        .respond_with(responder)
        .mount(&mock_server)
        .await;

    let client = FetchBuilder::<ApiResponse>::new()
        .with_timeout(5000)
        .without_retry()
        .with_circuit_breaker(2, 1, 100) // Fast recovery for testing
        .build();

    let url = format!("{}/recover", mock_server.uri());

    // Fail twice to open the circuit
    for _ in 0..2 {
        let init = RequestInit {
            method: Method::GET,
            ..Default::default()
        };
        let _ = client.fetch(&url, init).await;
    }

    let cb = client.circuit_breaker().unwrap();
    assert_eq!(cb.state().await, CircuitState::Open);

    // Wait for circuit to go half-open
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Next request should succeed and close the circuit
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let response = client.fetch(&url, init).await.unwrap();
    assert!(response.ok);
    assert_eq!(cb.state().await, CircuitState::Closed);
}

#[tokio::test]
async fn test_version_constant() {
    assert_eq!(smooai_fetch::VERSION, "2.1.2");
}
