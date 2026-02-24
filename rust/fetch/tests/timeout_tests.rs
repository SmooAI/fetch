//! Tests for timeout functionality.

use std::time::Duration;

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use smooai_fetch::client;
use smooai_fetch::error::FetchError;
use smooai_fetch::types::{FetchOptions, Method, RequestInit, TimeoutOptions};

#[tokio::test]
async fn test_request_completes_before_timeout() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/fast"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"status": "ok"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/fast", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: None,
    };

    let response = client::fetch::<serde_json::Value>(&url, init, Some(options), None, None, None)
        .await
        .unwrap();

    assert!(response.ok);
    assert_eq!(response.status, 200);
}

#[tokio::test]
async fn test_request_times_out() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/slow"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"status": "ok"}))
                .insert_header("content-type", "application/json")
                .set_delay(Duration::from_millis(3000)),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/slow", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 200 }),
        retry: None,
    };

    let result =
        client::fetch::<serde_json::Value>(&url, init, Some(options), None, None, None).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        FetchError::Timeout { timeout_ms } => {
            assert_eq!(timeout_ms, 200);
        }
        other => panic!("Expected Timeout error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_timeout_unit_function() {
    // Test the timeout::with_timeout function directly
    let result = smooai_fetch::timeout::with_timeout(100, async {
        tokio::time::sleep(Duration::from_millis(500)).await;
        Ok::<_, FetchError>(42)
    })
    .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        FetchError::Timeout { timeout_ms } => assert_eq!(timeout_ms, 100),
        other => panic!("Expected Timeout error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_timeout_unit_function_success() {
    let result = smooai_fetch::timeout::with_timeout(1000, async { Ok::<_, FetchError>(42) }).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);
}
