//! Configuration types for the smooai-fetch client.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::FetchError;

/// Context passed to a [`RetryCallback`] before each retry attempt.
///
/// This mirrors the information available to the TypeScript `onRejection`
/// callback in `@smooai/fetch`.
#[derive(Debug)]
pub struct RetryContext<'a> {
    /// 1-based attempt number. `1` means the callback is being consulted
    /// before the first retry (i.e. the initial request has already failed
    /// once).
    pub attempt: u32,
    /// The most recent error, if any.
    pub last_error: Option<&'a FetchError>,
    /// The HTTP status code from the most recent error, if it was an
    /// `HttpResponse` error.
    pub last_status: Option<u16>,
    /// Elapsed time since the retry loop started.
    pub elapsed: Duration,
}

/// Decision returned by a [`RetryCallback`] that controls what the retry loop
/// does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryDecision {
    /// Retry after the given delay, overriding the default exponential+jitter
    /// backoff formula.
    Retry {
        /// Delay to wait before performing the retry.
        delay: Duration,
    },
    /// Skip this retry attempt (no sleep, no request) and move on to the next
    /// attempt in the loop.
    Skip,
    /// Abort retrying entirely and surface the last error to the caller.
    Abort,
    /// Use the built-in exponential+jitter backoff formula (same as if no
    /// callback were registered).
    Default,
}

/// Callback invoked before each retry attempt. Returning a [`RetryDecision`]
/// lets the caller override the default exponential+jitter backoff behavior.
///
/// The callback is wrapped in an [`Arc`] so it can be cheaply cloned across
/// task boundaries and stored inside [`RetryOptions`] (which is `Clone`).
pub type RetryCallback = Arc<dyn Fn(&RetryContext) -> RetryDecision + Send + Sync>;

/// Configuration options for retry behavior.
#[derive(Clone)]
pub struct RetryOptions {
    /// Number of retry attempts (not counting the initial request).
    pub attempts: u32,
    /// Initial delay between retries in milliseconds.
    pub initial_interval_ms: u64,
    /// Factor to multiply the interval by for each retry (exponential backoff).
    pub factor: f64,
    /// Amount of random jitter to add to retry delays (0.0 to 1.0).
    pub jitter_adjustment: f64,
    /// Maximum delay between retries in milliseconds. None means no cap.
    pub max_interval_ms: Option<u64>,
    /// When `true`, the first retry fires immediately with zero delay
    /// regardless of the exponential backoff formula. Subsequent retries use
    /// the normal backoff. Defaults to `false`.
    pub fast_first: bool,
    /// Optional callback consulted before each retry attempt. When present the
    /// callback can override the default delay, skip the attempt, or abort
    /// retrying entirely.
    pub on_rejection: Option<RetryCallback>,
}

impl std::fmt::Debug for RetryOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetryOptions")
            .field("attempts", &self.attempts)
            .field("initial_interval_ms", &self.initial_interval_ms)
            .field("factor", &self.factor)
            .field("jitter_adjustment", &self.jitter_adjustment)
            .field("max_interval_ms", &self.max_interval_ms)
            .field("fast_first", &self.fast_first)
            .field("on_rejection", &self.on_rejection.is_some())
            .finish()
    }
}

/// Configuration options for request timeout.
#[derive(Debug, Clone)]
pub struct TimeoutOptions {
    /// Timeout duration in milliseconds.
    pub timeout_ms: u64,
}

/// Configuration options for rate limiting using a sliding window.
#[derive(Debug, Clone)]
pub struct RateLimitOptions {
    /// Maximum number of requests allowed in the period.
    pub limit_for_period: u32,
    /// Duration of the rate limit period in milliseconds.
    pub limit_period_ms: u64,
}

/// Configuration options for the circuit breaker.
#[derive(Debug, Clone)]
pub struct CircuitBreakerOptions {
    /// Failure count threshold to trip the circuit open.
    pub failure_threshold: u32,
    /// Number of successes required to close the circuit from half-open.
    pub success_threshold: u32,
    /// Time to stay in open state before transitioning to half-open, in milliseconds.
    pub open_state_delay_ms: u64,
}

/// Container-level options for rate limiting and circuit breaking.
#[derive(Debug, Clone, Default)]
pub struct FetchContainerOptions {
    /// Rate limiting configuration.
    pub rate_limit: Option<RateLimitOptions>,
    /// Circuit breaker configuration.
    pub circuit_breaker: Option<CircuitBreakerOptions>,
}

/// Options that apply to individual fetch requests.
#[derive(Debug, Clone, Default)]
pub struct FetchOptions {
    /// Timeout configuration.
    pub timeout: Option<TimeoutOptions>,
    /// Retry configuration.
    pub retry: Option<RetryOptions>,
}

/// HTTP method type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Method {
    #[default]
    GET,
    POST,
    PUT,
    PATCH,
    DELETE,
    HEAD,
    OPTIONS,
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Method::GET => write!(f, "GET"),
            Method::POST => write!(f, "POST"),
            Method::PUT => write!(f, "PUT"),
            Method::PATCH => write!(f, "PATCH"),
            Method::DELETE => write!(f, "DELETE"),
            Method::HEAD => write!(f, "HEAD"),
            Method::OPTIONS => write!(f, "OPTIONS"),
        }
    }
}

impl Method {
    /// Convert to reqwest::Method.
    pub fn to_reqwest(&self) -> reqwest::Method {
        match self {
            Method::GET => reqwest::Method::GET,
            Method::POST => reqwest::Method::POST,
            Method::PUT => reqwest::Method::PUT,
            Method::PATCH => reqwest::Method::PATCH,
            Method::DELETE => reqwest::Method::DELETE,
            Method::HEAD => reqwest::Method::HEAD,
            Method::OPTIONS => reqwest::Method::OPTIONS,
        }
    }
}

/// Request initialization options, analogous to the JS `RequestInit`.
#[derive(Debug, Clone, Default)]
pub struct RequestInit {
    /// HTTP method.
    pub method: Method,
    /// Request headers.
    pub headers: HashMap<String, String>,
    /// Request body (serialized as a string).
    pub body: Option<String>,
}
