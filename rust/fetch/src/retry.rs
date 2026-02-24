//! Retry logic with exponential backoff and jitter.

use std::future::Future;

use rand::Rng;
use tracing;

use crate::error::FetchError;
use crate::types::RetryOptions;

/// Determine if a given status code is retryable.
/// 429 (Too Many Requests) and 5xx are retryable.
pub fn is_retryable(status: u16) -> bool {
    status == 429 || status >= 500
}

/// Calculate the backoff delay for a given attempt using exponential backoff with jitter.
///
/// - `attempt`: zero-based attempt index (0 = first retry)
/// - `options`: retry configuration
///
/// Returns the delay in milliseconds.
pub fn calculate_backoff(attempt: u32, options: &RetryOptions) -> u64 {
    let base = options.initial_interval_ms as f64;
    let factor = options.factor;

    // Exponential: base * factor^attempt
    let exponential = base * factor.powi(attempt as i32);

    // Apply max interval cap if set
    let capped = match options.max_interval_ms {
        Some(max) => exponential.min(max as f64),
        None => exponential,
    };

    // Apply jitter: random value in [capped * (1 - jitter), capped * (1 + jitter)]
    let jitter = options.jitter_adjustment;
    if jitter <= 0.0 {
        return capped.max(0.0) as u64;
    }
    let mut rng = rand::thread_rng();
    let jitter_factor = 1.0 + rng.gen_range(-jitter..jitter);
    (capped * jitter_factor).max(0.0) as u64
}

/// Execute a future-returning closure with retry logic.
///
/// The `operation` closure receives the attempt number (0-based) and must return
/// a future that resolves to `Result<T, FetchError>`.
///
/// Retries up to `options.attempts` times (so total calls = 1 + attempts).
/// If a Retry-After header is present, that value is used instead of the calculated backoff.
///
/// Returns the successful result or the last error after all retries are exhausted.
pub async fn execute_with_retry<T, F, Fut>(
    options: &RetryOptions,
    operation: F,
) -> Result<T, FetchError>
where
    F: Fn(u32) -> Fut,
    Fut: Future<Output = Result<T, FetchError>>,
{
    let max_attempts = 1 + options.attempts; // initial + retries
    let mut last_error: Option<FetchError> = None;

    for attempt in 0..max_attempts {
        match operation(attempt).await {
            Ok(result) => return Ok(result),
            Err(err) => {
                let is_last = attempt + 1 >= max_attempts;

                if is_last || !err.is_retryable() {
                    if !err.is_retryable() {
                        // Non-retryable error, return immediately
                        return Err(err);
                    }
                    // All retries exhausted
                    return Err(FetchError::Retry {
                        attempts: options.attempts,
                        source: Box::new(err),
                    });
                }

                // Calculate delay, respecting Retry-After header
                let delay_ms = if let Some(retry_after) = err.retry_after_secs() {
                    retry_after * 1000
                } else {
                    calculate_backoff(attempt, options)
                };

                tracing::debug!(
                    attempt = attempt,
                    delay_ms = delay_ms,
                    "Retrying request after error"
                );

                last_error = Some(err);
                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
            }
        }
    }

    // Should not reach here, but just in case
    Err(last_error.unwrap_or(FetchError::Retry {
        attempts: options.attempts,
        source: Box::new(FetchError::SchemaValidation {
            message: "Unknown retry failure".to_string(),
        }),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable() {
        assert!(is_retryable(429));
        assert!(is_retryable(500));
        assert!(is_retryable(502));
        assert!(is_retryable(503));
        assert!(!is_retryable(200));
        assert!(!is_retryable(400));
        assert!(!is_retryable(404));
    }

    #[test]
    fn test_calculate_backoff_attempt_0() {
        let options = RetryOptions {
            attempts: 3,
            initial_interval_ms: 500,
            factor: 2.0,
            jitter_adjustment: 0.0, // No jitter for deterministic test
            max_interval_ms: None,
        };
        let delay = calculate_backoff(0, &options);
        // base * factor^0 = 500 * 1 = 500
        assert_eq!(delay, 500);
    }

    #[test]
    fn test_calculate_backoff_attempt_1() {
        let options = RetryOptions {
            attempts: 3,
            initial_interval_ms: 500,
            factor: 2.0,
            jitter_adjustment: 0.0,
            max_interval_ms: None,
        };
        let delay = calculate_backoff(1, &options);
        // base * factor^1 = 500 * 2 = 1000
        assert_eq!(delay, 1000);
    }

    #[test]
    fn test_calculate_backoff_with_max_interval() {
        let options = RetryOptions {
            attempts: 3,
            initial_interval_ms: 500,
            factor: 2.0,
            jitter_adjustment: 0.0,
            max_interval_ms: Some(800),
        };
        let delay = calculate_backoff(2, &options);
        // base * factor^2 = 500 * 4 = 2000, capped at 800
        assert_eq!(delay, 800);
    }

    #[test]
    fn test_calculate_backoff_with_jitter_range() {
        let options = RetryOptions {
            attempts: 3,
            initial_interval_ms: 1000,
            factor: 1.0,
            jitter_adjustment: 0.5,
            max_interval_ms: None,
        };
        // With jitter=0.5, delay should be between 500 and 1500
        for _ in 0..100 {
            let delay = calculate_backoff(0, &options);
            assert!(delay >= 500, "delay {} should be >= 500", delay);
            assert!(delay <= 1500, "delay {} should be <= 1500", delay);
        }
    }
}
