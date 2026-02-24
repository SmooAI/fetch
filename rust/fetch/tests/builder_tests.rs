//! Tests for the FetchBuilder pattern.

use serde::Deserialize;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use smooai_fetch::builder::FetchBuilder;
use smooai_fetch::defaults;
use smooai_fetch::types::{Method, RequestInit};

#[derive(Deserialize, Clone, Debug, PartialEq)]
struct TestData {
    id: String,
    name: String,
}

#[tokio::test]
async fn test_builder_basic_get() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/data"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "test"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let client = FetchBuilder::<TestData>::new()
        .with_timeout(5000)
        .without_retry()
        .build();

    let url = format!("{}/data", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };

    let response = client.fetch(&url, init).await.unwrap();
    assert!(response.ok);
    assert_eq!(response.status, 200);
    assert_eq!(
        response.data,
        Some(TestData {
            id: "1".to_string(),
            name: "test".to_string()
        })
    );
}

#[tokio::test]
async fn test_builder_with_default_headers() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/headers"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "headers"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let mut default_headers = std::collections::HashMap::new();
    default_headers.insert("Authorization".to_string(), "Bearer token123".to_string());
    default_headers.insert("X-Api-Key".to_string(), "my-key".to_string());

    let client = FetchBuilder::<TestData>::new()
        .with_timeout(5000)
        .without_retry()
        .with_init(RequestInit {
            method: Method::GET,
            headers: default_headers,
            body: None,
        })
        .build();

    let url = format!("{}/headers", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };

    let response = client.fetch(&url, init).await.unwrap();
    assert!(response.ok);
}

#[tokio::test]
async fn test_builder_with_retry() {
    let mock_server = MockServer::start().await;

    // Will fail once then succeed
    Mock::given(method("GET"))
        .and(path("/retry-builder"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "retried"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let client = FetchBuilder::<TestData>::new()
        .with_timeout(5000)
        .with_retry(defaults::default_retry_options())
        .build();

    let url = format!("{}/retry-builder", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };

    let response = client.fetch(&url, init).await.unwrap();
    assert!(response.ok);
}

#[tokio::test]
async fn test_builder_without_retry() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/no-retry"))
        .respond_with(
            ResponseTemplate::new(500)
                .set_body_json(serde_json::json!({"error": "Server error"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let client = FetchBuilder::<serde_json::Value>::new()
        .with_timeout(5000)
        .without_retry()
        .build();

    let url = format!("{}/no-retry", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };

    let result = client.fetch(&url, init).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_builder_with_circuit_breaker() {
    let client = FetchBuilder::<serde_json::Value>::new()
        .with_timeout(5000)
        .without_retry()
        .with_circuit_breaker(3, 2, 5000)
        .build();

    assert!(client.circuit_breaker().is_some());
}

#[tokio::test]
async fn test_builder_with_rate_limit() {
    let client = FetchBuilder::<serde_json::Value>::new()
        .with_timeout(5000)
        .without_retry()
        .with_rate_limit(10, 60_000)
        .build();

    assert!(client.rate_limiter().is_some());
}

#[tokio::test]
async fn test_builder_default_has_retry_and_timeout() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/defaults"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "defaults"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    // Default builder should have retry and timeout already configured
    let client = FetchBuilder::<TestData>::new().build();

    let url = format!("{}/defaults", mock_server.uri());
    let init = RequestInit::default();

    let response = client.fetch(&url, init).await.unwrap();
    assert!(response.ok);
}

#[tokio::test]
async fn test_builder_merge_headers() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/merge"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": "1", "name": "merge"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let mut default_headers = std::collections::HashMap::new();
    default_headers.insert("X-Default".to_string(), "default-value".to_string());

    let client = FetchBuilder::<TestData>::new()
        .with_timeout(5000)
        .without_retry()
        .with_init(RequestInit {
            method: Method::GET,
            headers: default_headers,
            body: None,
        })
        .build();

    let mut request_headers = std::collections::HashMap::new();
    request_headers.insert("X-Request".to_string(), "request-value".to_string());

    let url = format!("{}/merge", mock_server.uri());
    let init = RequestInit {
        method: Method::GET,
        headers: request_headers,
        body: None,
    };

    let response = client.fetch(&url, init).await.unwrap();
    assert!(response.ok);
}

#[tokio::test]
async fn test_builder_post_with_body() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/create"))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(serde_json::json!({"id": "new", "name": "created"}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let client = FetchBuilder::<TestData>::new()
        .with_timeout(5000)
        .without_retry()
        .build();

    let mut headers = std::collections::HashMap::new();
    headers.insert("content-type".to_string(), "application/json".to_string());

    let url = format!("{}/create", mock_server.uri());
    let init = RequestInit {
        method: Method::POST,
        headers,
        body: Some(r#"{"name":"test"}"#.to_string()),
    };

    let response = client.fetch(&url, init).await.unwrap();
    assert!(response.ok);
    assert_eq!(response.status, 201);
    assert_eq!(
        response.data,
        Some(TestData {
            id: "new".to_string(),
            name: "created".to_string()
        })
    );
}
