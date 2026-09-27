//! Retries must be method-aware, and a timed-out attempt must be cancelled.
//!
//! Regression coverage for SMOODEV-3375: retries were method-blind, so a POST
//! that timed out or got a 429/5xx was re-sent up to twice more — image
//! generation billed three images and returned none, and any non-idempotent
//! POST/PATCH could duplicate its side effect.
//!
//! The cases come from spec/retry-idempotency-corpus.json, shared with the
//! TypeScript, Python, Go and .NET suites. Every case runs against a REAL local
//! server and asserts how many requests the SERVER received: the bug is a side
//! effect executing more than once server-side, and only the server can count
//! that.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use smooai_fetch::client;
use smooai_fetch::error::FetchError;
use smooai_fetch::types::{FetchOptions, Method, RequestInit, RetryOptions, TimeoutOptions};
use smooai_fetch::{is_idempotent_method, is_retry_eligible};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// `include_str!` binds the corpus at compile time, so a corpus change cannot
/// silently miss this suite.
const CORPUS: &str = include_str!("../../../spec/retry-idempotency-corpus.json");

fn corpus() -> Value {
    serde_json::from_str(CORPUS).expect("retry-idempotency corpus must parse")
}

/// Parse a corpus method name case-insensitively. `None` for methods the
/// [`Method`] enum cannot express (TRACE, CONNECT, PURGE): those are skipped
/// here, and the other ports classify them.
fn parse_method(name: &str) -> Option<Method> {
    match name.to_ascii_uppercase().as_str() {
        "GET" => Some(Method::GET),
        "HEAD" => Some(Method::HEAD),
        "OPTIONS" => Some(Method::OPTIONS),
        "PUT" => Some(Method::PUT),
        "DELETE" => Some(Method::DELETE),
        "POST" => Some(Method::POST),
        "PATCH" => Some(Method::PATCH),
        _ => None,
    }
}

// --- test server -------------------------------------------------------------

#[derive(Clone)]
enum Respond {
    Status {
        status: u16,
        headers: Vec<(String, String)>,
    },
    /// Read the request, never answer, and record when the client closes.
    Hang,
}

#[derive(Debug, Clone, Copy)]
struct Connection {
    arrived: Instant,
    closed: Option<Instant>,
}

#[derive(Default)]
struct ServerState {
    requests: AtomicUsize,
    connections: Mutex<Vec<Connection>>,
}

/// A minimal HTTP/1.1 server on a raw TcpListener — raw so a hung request can
/// observe the client closing its socket (read returns 0), which no mock
/// server exposes.
async fn spawn_server(respond: Respond) -> (String, Arc<ServerState>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = Arc::new(ServerState::default());
    let accept_state = state.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            let index = {
                let mut conns = accept_state.connections.lock().unwrap();
                conns.push(Connection {
                    arrived: Instant::now(),
                    closed: None,
                });
                conns.len() - 1
            };
            tokio::spawn(handle(stream, index, respond.clone(), accept_state.clone()));
        }
    });
    (format!("http://{addr}/op"), state)
}

async fn handle(mut stream: TcpStream, index: usize, respond: Respond, state: Arc<ServerState>) {
    // Read the head, then the body by Content-Length.
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_ascii_lowercase();
    let content_length = head
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(0);
    while buf.len() < head_end + content_length {
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
    state.requests.fetch_add(1, Ordering::SeqCst);

    match respond {
        Respond::Status { status, headers } => {
            let mut response =
                format!("HTTP/1.1 {status} Test\r\ncontent-length: 0\r\nconnection: close\r\n");
            for (k, v) in headers {
                response.push_str(&format!("{k}: {v}\r\n"));
            }
            response.push_str("\r\n");
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.shutdown().await;
        }
        Respond::Hang => {
            // Never answer. A client that cancels closes the socket (EOF or
            // reset); one that abandons the attempt leaves it open.
            loop {
                match stream.read(&mut chunk).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => continue,
                }
            }
            state.connections.lock().unwrap()[index].closed = Some(Instant::now());
        }
    }
}

// --- tests -------------------------------------------------------------------

/// Positive control: a corpus that failed to load (or lost its cases) would
/// make every loop below vacuously pass.
#[test]
fn corpus_loaded() {
    let c = corpus();
    assert!(c["cases"].as_array().is_some_and(|a| a.len() >= 10));
    assert!(c["methods"]["idempotent"]
        .as_array()
        .is_some_and(|a| !a.is_empty()));
    assert!(c["methods"]["nonIdempotent"]
        .as_array()
        .is_some_and(|a| !a.is_empty()));
    assert!(c["timeoutAbort"]["timeoutMs"].as_u64().is_some());
}

#[test]
fn classifies_corpus_methods() {
    let c = corpus();
    let mut checked = 0;
    for (group, expected) in [("idempotent", true), ("nonIdempotent", false)] {
        for name in c["methods"][group].as_array().unwrap() {
            let name = name.as_str().unwrap();
            // TRACE, CONNECT and PURGE are not representable by `Method`; the
            // other ports classify them.
            let Some(method) = parse_method(name) else {
                continue;
            };
            assert_eq!(
                is_idempotent_method(&method),
                expected,
                "{name} should be idempotent={expected}"
            );
            // A bare request of that method is retry-eligible iff idempotent.
            let init = RequestInit {
                method,
                ..Default::default()
            };
            assert_eq!(is_retry_eligible(&init, &RetryOptions::default()), expected);
            checked += 1;
        }
    }
    assert!(checked >= 10, "only {checked} methods were classified");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn corpus_cases_hit_the_server_the_expected_number_of_times() {
    let c = corpus();
    let retry = &c["retry"];
    let timeout_ms = c["timeoutMs"].as_u64().unwrap();
    let cases = c["cases"].as_array().unwrap();
    let mut failures = Vec::new();

    for case in cases {
        let name = case["name"].as_str().unwrap();
        let respond = if case["respond"]["hang"].as_bool() == Some(true) {
            Respond::Hang
        } else {
            let headers = case["respond"]["headers"]
                .as_object()
                .map(|h| {
                    h.iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
                        .collect()
                })
                .unwrap_or_default();
            Respond::Status {
                status: case["respond"]["status"].as_u64().unwrap() as u16,
                headers,
            }
        };
        let (url, state) = spawn_server(respond).await;

        let headers: HashMap<String, String> = case["headers"]
            .as_object()
            .map(|h| {
                h.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let init = RequestInit {
            method: parse_method(case["method"].as_str().unwrap()).expect("server method"),
            headers,
            body: case["body"].as_str().map(str::to_string),
        };
        let options = FetchOptions {
            connect_timeout_ms: None,
            timeout: Some(TimeoutOptions { timeout_ms }),
            retry: Some(RetryOptions {
                attempts: retry["attempts"].as_u64().unwrap() as u32,
                initial_interval_ms: retry["initialIntervalMs"].as_u64().unwrap(),
                factor: retry["factor"].as_f64().unwrap(),
                jitter_adjustment: retry["jitterAdjustment"].as_f64().unwrap(),
                allow_non_idempotent: case["allowNonIdempotent"].as_bool().unwrap_or(false),
                ..Default::default()
            }),
        };

        let result =
            client::fetch::<Value>(&url, init, Some(options), None, None, None, None).await;
        let expected = case["expectedAttempts"].as_u64().unwrap() as usize;
        let got = state.requests.load(Ordering::SeqCst);

        let mut problems = Vec::new();
        if got != expected {
            problems.push(format!(
                "server received {got} request(s), expected {expected}"
            ));
        }
        match &result {
            Err(FetchError::Retry { .. }) if expected == 1 => problems.push(format!(
                "a request that was not retried must surface the underlying error, got {result:?}"
            )),
            Err(_) => {}
            Ok(r) => problems.push(format!("expected an error, got status {}", r.status)),
        }
        if !problems.is_empty() {
            failures.push(format!("{name}: {}", problems.join("; ")));
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} corpus cases failed:\n  {}",
        failures.len(),
        cases.len(),
        failures.join("\n  ")
    );
}

/// Poll until connection `index` has closed, or `deadline` passes.
async fn wait_for_close(
    state: &ServerState,
    index: usize,
    deadline: Instant,
) -> Option<Connection> {
    loop {
        let conn = state.connections.lock().unwrap().get(index).copied();
        if let Some(conn) = conn {
            if conn.closed.is_some() {
                return Some(conn);
            }
        }
        if Instant::now() >= deadline {
            return conn;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn timed_out_attempt_closes_its_connection_before_the_retry() {
    let c = corpus();
    let timeout_ms = c["timeoutAbort"]["timeoutMs"].as_u64().unwrap();
    let slack_ms = c["timeoutAbort"]["closeSlackMs"].as_u64().unwrap();
    let (url, state) = spawn_server(Respond::Hang).await;

    let options = FetchOptions {
        connect_timeout_ms: None,
        timeout: Some(TimeoutOptions { timeout_ms }),
        retry: Some(RetryOptions {
            attempts: 1,
            // Wider than the corpus cases' 1ms so "closed before the retry
            // arrived" is not decided by which server task the scheduler wakes
            // first. An abandoned attempt stays open for the whole test, so
            // this cannot mask a missing cancel.
            initial_interval_ms: 100,
            factor: 1.0,
            jitter_adjustment: 0.0,
            ..Default::default()
        }),
    };
    let init = RequestInit {
        method: Method::GET,
        ..Default::default()
    };
    let result = client::fetch::<Value>(&url, init, Some(options), None, None, None, None).await;
    assert!(
        matches!(result, Err(FetchError::Retry { .. })),
        "expected the GET to time out twice, got {result:?}"
    );

    let first_arrived = state.connections.lock().unwrap()[0].arrived;
    let deadline = first_arrived + Duration::from_millis(timeout_ms + slack_ms);
    let first = wait_for_close(&state, 0, deadline).await.unwrap();
    let closed = first
        .closed
        .expect("the timed-out attempt's connection was never closed — the request was abandoned, not cancelled");
    assert!(
        closed - first.arrived <= Duration::from_millis(timeout_ms + slack_ms),
        "first connection closed {:?} after arriving",
        closed - first.arrived
    );

    let conns = state.connections.lock().unwrap().clone();
    assert_eq!(conns.len(), 2, "a retried GET makes two attempts");
    assert!(
        closed <= conns[1].arrived,
        "first attempt must be cancelled before the retry is sent"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn timed_out_post_closes_its_connection_and_is_not_retried() {
    let c = corpus();
    let timeout_ms = c["timeoutAbort"]["timeoutMs"].as_u64().unwrap();
    let slack_ms = c["timeoutAbort"]["closeSlackMs"].as_u64().unwrap();
    let (url, state) = spawn_server(Respond::Hang).await;

    let options = FetchOptions {
        connect_timeout_ms: None,
        timeout: Some(TimeoutOptions { timeout_ms }),
        retry: Some(RetryOptions::default()),
    };
    let init = RequestInit {
        method: Method::POST,
        body: Some("{}".to_string()),
        ..Default::default()
    };
    let result = client::fetch::<Value>(&url, init, Some(options), None, None, None, None).await;
    assert!(
        matches!(result, Err(FetchError::Timeout { .. })),
        "a POST that times out surfaces the timeout unwrapped, got {result:?}"
    );

    let first_arrived = state.connections.lock().unwrap()[0].arrived;
    let deadline = first_arrived + Duration::from_millis(timeout_ms + slack_ms);
    let first = wait_for_close(&state, 0, deadline).await.unwrap();
    assert!(
        first.closed.is_some(),
        "the timed-out POST's connection was never closed"
    );
    // Give a (wrong) retry the chance to arrive before counting.
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    assert_eq!(state.requests.load(Ordering::SeqCst), 1);
}
