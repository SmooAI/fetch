//! SmooAI Fetch Client for Rust.
//!
//! A resilient HTTP fetch client with retries, timeouts, rate limiting,
//! and circuit breaking.
//!
//! # Features
//!
//! - **Retry**: Exponential backoff with jitter, Retry-After header support
//! - **Timeout**: Per-request timeout using tokio
//! - **Rate Limiting**: Sliding window rate limiter
//! - **Circuit Breaker**: Closed/Open/HalfOpen state machine
//! - **Lifecycle Hooks**: Pre-request and post-response hooks
//! - **Builder Pattern**: Fluent API for configuration
//!
//! # Example
//!
//! ```rust,no_run
//! use smooai_fetch::builder::FetchBuilder;
//! use smooai_fetch::types::{RequestInit, Method};
//! use serde::Deserialize;
//!
//! #[derive(Deserialize, Clone, Debug)]
//! struct ApiResponse {
//!     id: String,
//!     name: String,
//! }
//!
//! # async fn example() {
//! let client = FetchBuilder::<ApiResponse>::new()
//!     .with_timeout(5000)
//!     .with_retry(smooai_fetch::defaults::default_retry_options())
//!     .with_rate_limit(10, 60_000)
//!     .build();
//!
//! let init = RequestInit {
//!     method: Method::GET,
//!     ..Default::default()
//! };
//!
//! let response = client.fetch("https://api.example.com/data", init).await.unwrap();
//! println!("Got: {:?}", response.data);
//! # }
//! ```

pub mod builder;
pub mod circuit_breaker;
pub mod client;
pub mod defaults;
pub mod error;
pub mod hooks;
pub mod rate_limit;
mod redact;
pub mod response;
pub mod retry;
pub mod timeout;
pub mod types;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// Re-export commonly used items at crate root
pub use builder::{FetchBuilder, FetchClient};
pub use circuit_breaker::{CircuitBreaker, CircuitState, CircuitStateChangeCallback};
pub use error::FetchError;
pub use rate_limit::SlidingWindowRateLimiter;
pub use response::FetchResponse;
pub use types::{
    AuthTokenFuture, AuthTokenProvider, FetchContainerOptions, FetchOptions, Method,
    RateLimitRetryOptions, RequestInit, RetryCallback, RetryContext, RetryDecision, RetryOptions,
};

/// Convenience function: perform a single fetch with default options.
///
/// This is the simplest way to make a request. For configured clients
/// with retry, rate limiting, etc., use [`FetchBuilder`].
pub async fn fetch<T: serde::de::DeserializeOwned + Clone + Send + 'static>(
    url: &str,
    init: RequestInit,
) -> Result<FetchResponse<T>, FetchError> {
    let options = FetchOptions {
        connect_timeout_ms: None,
        timeout: Some(defaults::default_timeout_options()),
        retry: Some(defaults::default_retry_options()),
    };
    client::fetch::<T>(url, init, Some(options), None, None, None, None).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// package.json is the single source of truth for the version across all
    /// five ports; `scripts/sync-versions.mjs` stamps it into Cargo.toml.
    ///
    /// This used to be `assert_eq!(VERSION, "2.1.2")`. Asserting a literal made
    /// the test agree with whatever the crate happened to say, so it stayed
    /// green for the entire time Cargo.toml was pinned at 2.1.2 while
    /// package.json shipped 3.4.0 — the drift it was supposed to catch was the
    /// thing it locked in.
    #[test]
    fn version_matches_package_json() {
        const PACKAGE_JSON: &str = include_str!("../../../package.json");
        let pkg: serde_json::Value =
            serde_json::from_str(PACKAGE_JSON).expect("package.json must parse");
        let expected = pkg["version"].as_str().expect("package.json has a version");
        assert_eq!(
            VERSION, expected,
            "Cargo.toml is out of sync with package.json — run `node scripts/sync-versions.mjs`"
        );
    }
}
