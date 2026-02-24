//! Tests for the core fetch functionality.

use serde::Deserialize;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use smooai_fetch::client;
use smooai_fetch::error::FetchError;
use smooai_fetch::types::{FetchOptions, Method, RequestInit, TimeoutOptions};

#[derive(Deserialize, Clone, Debug, PartialEq)]
struct TestResponse {
    id: String,
    name: String,
}

#[tokio::test]
async fn test_basic_get_request() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/test"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "123", "name": "test"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/test", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: None,
    };

    let response = client::fetch::<TestResponse>(&url, init, Some(options), None, None, None)
        .await
        .unwrap();

    assert!(response.ok);
    assert_eq!(response.status, 200);
    assert!(response.is_json);
    assert_eq!(
        response.data,
        Some(TestResponse {
            id: "123".to_string(),
            name: "test".to_string()
        })
    );
}

#[tokio::test]
async fn test_basic_post_request_with_body() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/submit"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(serde_json::json!({"id": "456", "name": "created"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/submit", mock_server.uri());
    let mut headers = std::collections::HashMap::new();
    headers.insert("content-type".to_string(), "application/json".to_string());

    let init = RequestInit {
        method: Method::POST,
        headers,
        body: Some(r#"{"key":"value"}"#.to_string()),
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: None,
    };

    let response = client::fetch::<TestResponse>(&url, init, Some(options), None, None, None)
        .await
        .unwrap();

    assert!(response.ok);
    assert_eq!(response.status, 201);
    assert_eq!(
        response.data,
        Some(TestResponse {
            id: "456".to_string(),
            name: "created".to_string()
        })
    );
}

#[tokio::test]
async fn test_failed_request_404() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/notfound"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_json(serde_json::json!({"error": "Not found"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/notfound", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: None,
    };

    let result =
        client::fetch::<serde_json::Value>(&url, init, Some(options), None, None, None).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    match &err {
        FetchError::HttpResponse {
            status, message, ..
        } => {
            assert_eq!(*status, 404);
            assert!(message.contains("Not found"));
        }
        other => panic!("Expected HttpResponse error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_error_response_with_type_code_message() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/error"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_json(serde_json::json!({
                    "error": {
                        "type": "ERROR_TYPE",
                        "code": 125,
                        "message": "Error message"
                    }
                }))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/error", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: None,
    };

    let result =
        client::fetch::<serde_json::Value>(&url, init, Some(options), None, None, None).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_string = err.to_string();
    assert!(
        err_string.contains("ERROR_TYPE"),
        "Error should contain type: {}",
        err_string
    );
    assert!(
        err_string.contains("125"),
        "Error should contain code: {}",
        err_string
    );
    assert!(
        err_string.contains("Error message"),
        "Error should contain message: {}",
        err_string
    );
}

#[tokio::test]
async fn test_non_json_response() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/text"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("Hello, World!")
                .insert_header("content-type", "text/plain"),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/text", mock_server.uri());
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
    assert!(!response.is_json);
    assert!(response.data.is_none());
    assert_eq!(response.body, "Hello, World!");
}

#[tokio::test]
async fn test_request_with_custom_headers() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/headers"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "headers-test"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/headers", mock_server.uri());
    let mut headers = std::collections::HashMap::new();
    headers.insert("X-Custom-Header".to_string(), "custom-value".to_string());
    headers.insert("Authorization".to_string(), "Bearer token123".to_string());

    let init = RequestInit {
        method: Method::GET,
        headers,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: None,
    };

    let response = client::fetch::<TestResponse>(&url, init, Some(options), None, None, None)
        .await
        .unwrap();

    assert!(response.ok);
    assert_eq!(response.status, 200);
}

#[tokio::test]
async fn test_schema_validation_error_on_type_mismatch() {
    let mock_server = MockServer::start().await;

    // Server returns id as a number, but TestResponse expects a string
    Mock::given(method("GET"))
        .and(path("/bad-schema"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": 123, "name": "test"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/bad-schema", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: None,
    };

    let result = client::fetch::<TestResponse>(&url, init, Some(options), None, None, None).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        FetchError::SchemaValidation { message } => {
            assert!(
                message.contains("invalid type"),
                "Expected type error: {}",
                message
            );
        }
        other => panic!("Expected SchemaValidation error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_default_method_is_get() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/default-method"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "default"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let url = format!("{}/default-method", mock_server.uri());
    let init = RequestInit::default();
    let options = FetchOptions {
        timeout: Some(TimeoutOptions { timeout_ms: 5000 }),
        retry: None,
    };

    let response = client::fetch::<TestResponse>(&url, init, Some(options), None, None, None)
        .await
        .unwrap();

    assert!(response.ok);
}
