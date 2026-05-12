//! Tests for `FetchBuilder::with_auth_provider`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde::Deserialize;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use smooai_fetch::builder::FetchBuilder;
use smooai_fetch::types::{AuthTokenProvider, Method, RequestInit};

#[derive(Deserialize, Clone, Debug)]
struct TestReply {
    ok: bool,
}

#[tokio::test]
async fn test_async_auth_provider_injects_bearer_header() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/data"))
        .and(header("Authorization", "Bearer fresh-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"ok": true}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let provider: AuthTokenProvider =
        Arc::new(|| Box::pin(async move { "fresh-token".to_string() }));

    let client = FetchBuilder::<TestReply>::new()
        .without_retry()
        .with_auth_provider(provider, "Bearer".to_string())
        .build();

    let url = format!("{}/data", mock_server.uri());
    let response = client
        .fetch(
            &url,
            RequestInit {
                method: Method::GET,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(response.ok);
}

#[tokio::test]
async fn test_provider_is_invoked_before_every_request() {
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

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_for_provider = counter.clone();
    let provider: AuthTokenProvider = Arc::new(move || {
        let c = counter_for_provider.clone();
        Box::pin(async move {
            let n = c.fetch_add(1, Ordering::SeqCst);
            format!("tok-{}", n)
        })
    });

    let client = FetchBuilder::<TestReply>::new()
        .without_retry()
        .with_auth_provider(provider, "Bearer".to_string())
        .build();

    let url = format!("{}/data", mock_server.uri());
    for _ in 0..3 {
        let _ = client
            .fetch(
                &url,
                RequestInit {
                    method: Method::GET,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_custom_auth_scheme() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/data"))
        .and(header("Authorization", "Token abc"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"ok": true}))
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let provider: AuthTokenProvider = Arc::new(|| Box::pin(async move { "abc".to_string() }));

    let client = FetchBuilder::<TestReply>::new()
        .without_retry()
        .with_auth_provider(provider, "Token".to_string())
        .build();

    let url = format!("{}/data", mock_server.uri());
    let response = client
        .fetch(
            &url,
            RequestInit {
                method: Method::GET,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(response.ok);
}
