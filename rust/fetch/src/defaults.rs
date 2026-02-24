//! Default configuration constants.

use crate::types::{RetryOptions, TimeoutOptions};

/// Default timeout in milliseconds.
pub const DEFAULT_TIMEOUT_MS: u64 = 10_000;

/// Default number of retry attempts.
pub const DEFAULT_RETRY_ATTEMPTS: u32 = 2;

/// Default initial retry interval in milliseconds.
pub const DEFAULT_RETRY_INITIAL_INTERVAL_MS: u64 = 500;

/// Default exponential backoff factor.
pub const DEFAULT_RETRY_FACTOR: f64 = 2.0;

/// Default jitter adjustment (0.0 to 1.0).
pub const DEFAULT_RETRY_JITTER_ADJUSTMENT: f64 = 0.5;

/// Default circuit breaker failure threshold.
pub const DEFAULT_CB_FAILURE_THRESHOLD: u32 = 5;

/// Default circuit breaker success threshold for half-open -> closed.
pub const DEFAULT_CB_SUCCESS_THRESHOLD: u32 = 2;

/// Default circuit breaker open state delay in milliseconds.
pub const DEFAULT_CB_OPEN_STATE_DELAY_MS: u64 = 30_000;

/// Create default retry options.
pub fn default_retry_options() -> RetryOptions {
    RetryOptions {
        attempts: DEFAULT_RETRY_ATTEMPTS,
        initial_interval_ms: DEFAULT_RETRY_INITIAL_INTERVAL_MS,
        factor: DEFAULT_RETRY_FACTOR,
        jitter_adjustment: DEFAULT_RETRY_JITTER_ADJUSTMENT,
        max_interval_ms: None,
    }
}

/// Create default timeout options.
pub fn default_timeout_options() -> TimeoutOptions {
    TimeoutOptions {
        timeout_ms: DEFAULT_TIMEOUT_MS,
    }
}
