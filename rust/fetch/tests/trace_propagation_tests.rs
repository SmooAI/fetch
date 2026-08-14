//! Trace-context propagation on egress (feature `otel`).
//!
//! The gap these guard: api-prime EXTRACTS `traceparent` on ingress, but nothing
//! ever INJECTED it, so every service-to-service call began a new root trace.
//! Measured 2026-08-14 over three hours: 34,961 traces touched one service, 4
//! touched two.
//!
//! Asserted at the WIRE — what the server actually received — rather than by
//! inspecting our own builder. A header we believe we set and the server never
//! sees is the exact failure being fixed.
#![cfg(feature = "otel")]

use opentelemetry::trace::TraceContextExt;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use std::collections::HashMap;
use tracing_opentelemetry::OpenTelemetrySpanExt as _;
use tracing_subscriber::layer::SubscriberExt as _;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

use smooai_fetch::client;
use smooai_fetch::types::{Method as FetchMethod, RequestInit};

fn init_from(server: &MockServer, headers: HashMap<String, String>) -> (String, RequestInit) {
    (
        format!("{}/x", server.uri()),
        RequestInit {
            method: FetchMethod::GET,
            headers,
            ..Default::default()
        },
    )
}

async fn captured_header(server: &MockServer, name: &str) -> Option<String> {
    let requests = server.received_requests().await.expect("recording enabled");
    requests
        .first()
        .and_then(|r: &Request| r.headers.get(name))
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}

#[tokio::test]
async fn an_active_span_is_propagated_as_a_traceparent_header() {
    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
    let provider = SdkTracerProvider::builder().build();
    let tracer = provider.tracer("fetch-propagation-test");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;

    // Production shape: a `tracing` span picked up by a tracing-opentelemetry
    // layer — NOT an OTel-native span. An earlier version of this test used the
    // native form and passed against an implementation that read only
    // `Context::current()`, which sees nothing in any real SmooAI service. The
    // feature would have shipped as a silent no-op.
    let subscriber =
        tracing_subscriber::registry().with(tracing_opentelemetry::layer().with_tracer(tracer));
    let (url, init) = init_from(&server, HashMap::new());

    let expected_trace_id = {
        let _sub = tracing::subscriber::set_default(subscriber);
        let span = tracing::info_span!("caller");
        let entered = span.enter();
        let id = span.context().span().span_context().trace_id().to_string();
        let _ = client::fetch::<serde_json::Value>(&url, init, None, None, None, None, None).await;
        drop(entered);
        id
    };

    let traceparent = captured_header(&server, "traceparent")
        .await
        .expect("the server received a traceparent header");
    assert!(
        traceparent.contains(&expected_trace_id),
        "traceparent {traceparent} must carry the caller's trace id {expected_trace_id}"
    );
}

#[tokio::test]
async fn no_active_span_means_no_header() {
    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;

    let (url, init) = init_from(&server, HashMap::new());
    let _ = client::fetch::<serde_json::Value>(&url, init, None, None, None, None, None).await;

    // An unregistered/absent span yields INVALID_SPAN_CONTEXT — all-zero ids.
    // Injecting that writes a malformed traceparent the downstream service may
    // reject, or worse adopt, poisoning its trace. The sibling logger shipped
    // exactly this bug, where all-zero ids overwrote a real correlation id.
    assert_eq!(captured_header(&server, "traceparent").await, None);
}

#[tokio::test]
async fn an_explicit_caller_header_is_never_overwritten() {
    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
    let provider = SdkTracerProvider::builder().build();
    let tracer = provider.tracer("fetch-propagation-test");

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;

    const CALLER: &str = "00-11111111111111111111111111111111-2222222222222222-01";
    let mut headers = HashMap::new();
    headers.insert("traceparent".to_string(), CALLER.to_string());

    let subscriber =
        tracing_subscriber::registry().with(tracing_opentelemetry::layer().with_tracer(tracer));
    let (url, init) = init_from(&server, headers);
    {
        let _sub = tracing::subscriber::set_default(subscriber);
        let span = tracing::info_span!("caller");
        let _entered = span.enter();
        let _ = client::fetch::<serde_json::Value>(&url, init, None, None, None, None, None).await;
    }

    // A client that silently rewrites an intentional header is worse than one
    // that does nothing.
    assert_eq!(
        captured_header(&server, "traceparent").await.as_deref(),
        Some(CALLER)
    );
}
