//! Configuration types for the smooai-fetch client.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Configuration options for retry behavior.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Method {
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

impl Default for Method {
    fn default() -> Self {
        Method::GET
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
