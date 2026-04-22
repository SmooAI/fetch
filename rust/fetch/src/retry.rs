//! Retry logic with exponential backoff and jitter.

use std::future::Future;
use std::time::{Duration, Instant};

use rand::Rng;
use tracing;

use crate::error::FetchError;
use crate::types::{RetryContext, RetryDecision, RetryOptions};

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

/// Extract the HTTP status code from an error, if any.
fn status_from_error(err: &FetchError) -> Option<u16> {
    match err {
        FetchError::HttpResponse { status, .. } => Some(*status),
        _ => None,
    }
}

/// Execute a future-returning closure with retry logic.
///
/// The `operation` closure receives the attempt number (0-based) and must return
/// a future that resolves to `Result<T, FetchError>`.
///
/// Retries up to `options.attempts` times (so total calls = 1 + attempts).
///
/// Delay selection order (first match wins):
/// 1. If a `RetryCallback` is registered on `options.on_rejection`, its
///    [`RetryDecision`] is honored (`Retry` overrides, `Skip` skips the sleep,
///    `Abort` bails out, `Default` falls through to the built-in logic).
/// 2. `Retry-After` header on the last error.
/// 3. `fast_first = true` on the very first retry → zero delay.
/// 4. Otherwise, exponential+jitter via [`calculate_backoff`].
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
    let start = Instant::now();

    for attempt in 0..max_attempts {
        match operation(attempt).await {
            Ok(result) => return Ok(result),
            Err(err) => {
                let is_last = attempt + 1 >= max_attempts;

                if !err.is_retryable() {
                    // Non-retryable error, return immediately
                    return Err(err);
                }

                if is_last {
                    // All retries exhausted
                    return Err(FetchError::Retry {
                        attempts: options.attempts,
                        source: Box::new(err),
                    });
                }

                // Consult on_rejection callback before computing default delay.
                // `attempt` here is 0-based for the *just-failed* call, so the
                // retry we are about to perform is `attempt + 1` (1-based).
                let decision = options.on_rejection.as_ref().map(|cb| {
                    let ctx = RetryContext {
                        attempt: attempt + 1,
                        last_error: Some(&err),
                        last_status: status_from_error(&err),
                        elapsed: start.elapsed(),
                    };
                    cb(&ctx)
                });

                let delay_ms: u64 = match decision {
                    Some(RetryDecision::Abort) => {
                        // Surface the error unwrapped (no `Retry` wrapping) so
                        // the caller sees exactly what aborted the loop.
                        return Err(err);
                    }
                    Some(RetryDecision::Skip) => {
                        last_error = Some(err);
                        tracing::debug!(
                            attempt = attempt,
                            "Skipping retry attempt per on_rejection callback"
                        );
                        continue;
                    }
                    Some(RetryDecision::Retry { delay }) => {
                        let ms = duration_to_millis(delay);
                        tracing::debug!(
                            attempt = attempt,
                            delay_ms = ms,
                            "Retrying request per on_rejection callback"
                        );
                        last_error = Some(err);
                        tokio::time::sleep(Duration::from_millis(ms)).await;
                        continue;
                    }
                    Some(RetryDecision::Default) | None => {
                        compute_default_delay(attempt, &err, options)
                    }
                };

                tracing::debug!(
                    attempt = attempt,
                    delay_ms = delay_ms,
                    "Retrying request after error"
                );

                last_error = Some(err);
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
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

/// Compute the default retry delay, honoring `Retry-After` and `fast_first`.
fn compute_default_delay(attempt: u32, err: &FetchError, options: &RetryOptions) -> u64 {
    if let Some(retry_after) = err.retry_after_secs() {
        return retry_after * 1000;
    }
    if options.fast_first && attempt == 0 {
        return 0;
    }
    calculate_backoff(attempt, options)
}

/// Saturating conversion from `Duration` to whole milliseconds, capped at
/// `u64::MAX`.
fn duration_to_millis(d: Duration) -> u64 {
    let millis = d.as_millis();
    if millis > u64::MAX as u128 {
        u64::MAX
    } else {
        millis as u64
    }
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
            fast_first: false,
            on_rejection: None,
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
            fast_first: false,
            on_rejection: None,
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
            fast_first: false,
            on_rejection: None,
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
            fast_first: false,
            on_rejection: None,
        };
        // With jitter=0.5, delay should be between 500 and 1500
        for _ in 0..100 {
            let delay = calculate_backoff(0, &options);
            assert!(delay >= 500, "delay {} should be >= 500", delay);
            assert!(delay <= 1500, "delay {} should be <= 1500", delay);
        }
    }

    #[test]
    fn test_compute_default_delay_fast_first() {
        let options = RetryOptions {
            attempts: 3,
            initial_interval_ms: 5_000,
            factor: 2.0,
            jitter_adjustment: 0.0,
            max_interval_ms: None,
            fast_first: true,
            on_rejection: None,
        };
        let err = FetchError::Timeout { timeout_ms: 1000 };
        // attempt 0 (first retry) with fast_first=true → zero delay
        assert_eq!(compute_default_delay(0, &err, &options), 0);
        // attempt 1 (second retry) reverts to the normal backoff formula
        assert_eq!(compute_default_delay(1, &err, &options), 10_000);
    }

    #[test]
    fn test_compute_default_delay_retry_after_beats_fast_first() {
        let options = RetryOptions {
            attempts: 3,
            initial_interval_ms: 5_000,
            factor: 2.0,
            jitter_adjustment: 0.0,
            max_interval_ms: None,
            fast_first: true,
            on_rejection: None,
        };
        let mut headers = std::collections::HashMap::new();
        headers.insert("retry-after".to_string(), "3".to_string());
        let err = FetchError::HttpResponse {
            status: 429,
            status_text: "Too Many Requests".to_string(),
            message: String::new(),
            headers,
            body: String::new(),
            is_json: false,
        };
        assert_eq!(compute_default_delay(0, &err, &options), 3_000);
    }
}
