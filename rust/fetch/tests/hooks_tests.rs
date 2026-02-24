//! Tests for lifecycle hooks.

use serde::Deserialize;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use smooai_fetch::client;
use smooai_fetch::error::FetchError;
use smooai_fetch::hooks::LifecycleHooks;
use smooai_fetch::types::{FetchOptions, Method, RequestInit, TimeoutOptions};

#[derive(Deserialize, Clone, Debug, PartialEq)]
struct TestData {
    id: String,
    name: String,
}

#[tokio::test]
async fn test_pre_request_hook_modifies_url() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/modified"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "modified"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let hooks: LifecycleHooks<TestData> = LifecycleHooks {
        pre_request: Some(Box::new(move |_url, init| {
            let new_url = format!("{}/modified", base_url);
            Some((new_url, init.clone()))
        })),
        ..Default::default()
    };

    let url = format!("{}/original", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: None,
    };

    let response = client::fetch::<TestData>(&url, init, Some(options), None, None, Some(&hooks))
        .await
        .unwrap();

    assert!(response.ok);
    assert_eq!(
        response.data,
        Some(TestData {
            id: "1".to_string(),
            name: "modified".to_string()
        })
    );
}

#[tokio::test]
async fn test_pre_request_hook_adds_headers() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/with-headers"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "headers"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let hooks: LifecycleHooks<TestData> = LifecycleHooks {
        pre_request: Some(Box::new(|url, init| {
            let mut new_init = init.clone();
            new_init
                .headers
                .insert("X-Custom-Header".to_string(), "custom-value".to_string());
            Some((url.to_string(), new_init))
        })),
        ..Default::default()
    };

    let url = format!("{}/with-headers", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: None,
    };

    let response = client::fetch::<TestData>(&url, init, Some(options), None, None, Some(&hooks))
        .await
        .unwrap();

    assert!(response.ok);
}

#[tokio::test]
async fn test_pre_request_hook_returns_none_passes_through() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/passthrough"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "passthrough"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let hooks: LifecycleHooks<TestData> = LifecycleHooks {
        pre_request: Some(Box::new(|_url, _init| None)),
        ..Default::default()
    };

    let url = format!("{}/passthrough", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: None,
    };

    let response = client::fetch::<TestData>(&url, init, Some(options), None, None, Some(&hooks))
        .await
        .unwrap();

    assert!(response.ok);
}

#[tokio::test]
async fn test_post_response_success_hook_modifies_response() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/success-hook"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "original"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let hooks: LifecycleHooks<TestData> = LifecycleHooks {
        post_response_success: Some(Box::new(|_url, _init, mut response| {
            // Modify the data in the response
            response.data = Some(TestData {
                id: "modified".to_string(),
                name: "by-hook".to_string(),
            });
            Some(response)
        })),
        ..Default::default()
    };

    let url = format!("{}/success-hook", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: None,
    };

    let response = client::fetch::<TestData>(&url, init, Some(options), None, None, Some(&hooks))
        .await
        .unwrap();

    assert!(response.ok);
    assert_eq!(
        response.data,
        Some(TestData {
            id: "modified".to_string(),
            name: "by-hook".to_string()
        })
    );
}

#[tokio::test]
async fn test_post_response_success_hook_returns_none_passes_through() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/success-none"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "original"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let hooks: LifecycleHooks<TestData> = LifecycleHooks {
        post_response_success: Some(Box::new(|_url, _init, _response| None)),
        ..Default::default()
    };

    let url = format!("{}/success-none", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: None,
    };

    let response = client::fetch::<TestData>(&url, init, Some(options), None, None, Some(&hooks))
        .await
        .unwrap();

    assert!(response.ok);
    assert_eq!(
        response.data,
        Some(TestData {
            id: "1".to_string(),
            name: "original".to_string()
        })
    );
}

#[tokio::test]
async fn test_post_response_error_hook_replaces_error() {
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

    let hooks: LifecycleHooks<serde_json::Value> = LifecycleHooks {
        post_response_error: Some(Box::new(|url, _init, _err, _response| {
            Some(FetchError::SchemaValidation {
                message: format!("Custom error for {}", url),
            })
        })),
        ..Default::default()
    };

    let url = format!("{}/error-hook", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: None,
    };

    let result =
        client::fetch::<serde_json::Value>(&url, init, Some(options), None, None, Some(&hooks))
            .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        FetchError::SchemaValidation { message } => {
            assert!(message.contains("Custom error for"));
            assert!(message.contains("/error-hook"));
        }
        other => panic!("Expected SchemaValidation error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_post_response_error_hook_returns_none_passes_through() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/error-none"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_json(serde_json::json!({"error": "Server error"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let hooks: LifecycleHooks<serde_json::Value> = LifecycleHooks {
        post_response_error: Some(Box::new(|_url, _init, _err, _response| None)),
        ..Default::default()
    };

    let url = format!("{}/error-none", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: None,
    };

    let result =
        client::fetch::<serde_json::Value>(&url, init, Some(options), None, None, Some(&hooks))
            .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        FetchError::HttpResponse { status, .. } => {
            assert_eq!(status, 500);
        }
        other => panic!("Expected HttpResponse error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_all_hooks_together() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/all-hooks"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "original"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let hooks: LifecycleHooks<TestData> = LifecycleHooks {
        pre_request: Some(Box::new(move |_url, init| {
            let new_url = format!("{}/all-hooks", base_url);
            let mut new_init = init.clone();
            new_init
                .headers
                .insert("X-Hook".to_string(), "pre-request".to_string());
            Some((new_url, new_init))
        })),
        post_response_success: Some(Box::new(|_url, _init, mut response| {
            response.data = Some(TestData {
                id: "hooked".to_string(),
                name: "success".to_string(),
            });
            Some(response)
        })),
        post_response_error: Some(Box::new(|_url, _init, _err, _response| {
            // This should not be called for successful responses
            Some(FetchError::SchemaValidation {
                message: "should-not-happen".to_string(),
            })
        })),
    };

    let url = format!("{}/wrong-path", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: None,
    };

    let response = client::fetch::<TestData>(&url, init, Some(options), None, None, Some(&hooks))
        .await
        .unwrap();

    assert!(response.ok);
    assert_eq!(
        response.data,
        Some(TestData {
            id: "hooked".to_string(),
            name: "success".to_string()
        })
    );
}
