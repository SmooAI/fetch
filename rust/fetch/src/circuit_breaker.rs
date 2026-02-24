//! Circuit breaker implementation with Closed/Open/HalfOpen states.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::error::FetchError;

/// The state of the circuit breaker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation; requests are allowed.
    Closed,
    /// Requests are rejected; waiting for the open delay to expire.
    Open,
    /// Trial mode; a limited number of requests are allowed to test recovery.
    HalfOpen,
}

/// Internal mutable state for the circuit breaker.
#[derive(Debug)]
struct CircuitBreakerState {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
}

/// A circuit breaker that transitions between Closed, Open, and HalfOpen states.
///
/// - **Closed**: All requests pass through. Failures increment the counter.
///   When `failure_threshold` is reached, transitions to Open.
/// - **Open**: All requests are rejected with `FetchError::CircuitBreaker`.
///   After `open_state_delay_ms`, transitions to HalfOpen.
/// - **HalfOpen**: A limited number of requests pass through.
///   If they succeed (reaching `success_threshold`), transitions back to Closed.
///   If any fail, transitions back to Open.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    failure_threshold: u32,
    success_threshold: u32,
    open_state_delay: Duration,
    inner: Arc<Mutex<CircuitBreakerState>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker.
    pub fn new(failure_threshold: u32, success_threshold: u32, open_state_delay_ms: u64) -> Self {
        Self {
            failure_threshold,
            success_threshold,
            open_state_delay: Duration::from_millis(open_state_delay_ms),
            inner: Arc::new(Mutex::new(CircuitBreakerState {
                state: CircuitState::Closed,
                failure_count: 0,
                success_count: 0,
                last_failure_time: None,
            })),
        }
    }

    /// Get the current state of the circuit breaker.
    pub async fn state(&self) -> CircuitState {
        let inner = self.inner.lock().await;
        inner.state.clone()
    }

    /// Check if a request is allowed to proceed. If the circuit is open
    /// and the delay has elapsed, it transitions to half-open.
    ///
    /// Returns Ok(()) if the request is allowed, or FetchError::CircuitBreaker
    /// if the circuit is open.
    pub async fn check(&self) -> Result<(), FetchError> {
        let mut inner = self.inner.lock().await;

        match inner.state {
            CircuitState::Closed => Ok(()),
            CircuitState::Open => {
                // Check if the open delay has elapsed
                if let Some(last_failure) = inner.last_failure_time {
                    if last_failure.elapsed() >= self.open_state_delay {
                        // Transition to HalfOpen
                        inner.state = CircuitState::HalfOpen;
                        inner.success_count = 0;
                        tracing::info!("Circuit breaker transitioning from Open to HalfOpen");
                        Ok(())
                    } else {
                        Err(FetchError::CircuitBreaker)
                    }
                } else {
                    Err(FetchError::CircuitBreaker)
                }
            }
            CircuitState::HalfOpen => Ok(()),
        }
    }

    /// Record a successful request.
    pub async fn record_success(&self) {
        let mut inner = self.inner.lock().await;

        match inner.state {
            CircuitState::HalfOpen => {
                inner.success_count += 1;
                if inner.success_count >= self.success_threshold {
                    inner.state = CircuitState::Closed;
                    inner.failure_count = 0;
                    inner.success_count = 0;
                    tracing::info!("Circuit breaker transitioning from HalfOpen to Closed");
                }
            }
            CircuitState::Closed => {
                // Reset failure count on success in closed state
                inner.failure_count = 0;
            }
            CircuitState::Open => {
                // Should not happen, but ignore
            }
        }
    }

    /// Record a failed request.
    pub async fn record_failure(&self) {
        let mut inner = self.inner.lock().await;

        match inner.state {
            CircuitState::Closed => {
                inner.failure_count += 1;
                if inner.failure_count >= self.failure_threshold {
                    inner.state = CircuitState::Open;
                    inner.last_failure_time = Some(Instant::now());
                    tracing::warn!(
                        failure_count = inner.failure_count,
                        "Circuit breaker transitioning from Closed to Open"
                    );
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open goes back to open
                inner.state = CircuitState::Open;
                inner.last_failure_time = Some(Instant::now());
                inner.success_count = 0;
                tracing::warn!("Circuit breaker transitioning from HalfOpen to Open");
            }
            CircuitState::Open => {
                inner.last_failure_time = Some(Instant::now());
            }
        }
    }

    /// Reset the circuit breaker to the closed state.
    pub async fn reset(&self) {
        let mut inner = self.inner.lock().await;
        inner.state = CircuitState::Closed;
        inner.failure_count = 0;
        inner.success_count = 0;
        inner.last_failure_time = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initial_state_is_closed() {
        let cb = CircuitBreaker::new(3, 2, 1000);
        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_stays_closed_below_threshold() {
        let cb = CircuitBreaker::new(3, 2, 1000);
        cb.record_failure().await;
        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Closed);
        assert!(cb.check().await.is_ok());
    }

    #[tokio::test]
    async fn test_opens_at_threshold() {
        let cb = CircuitBreaker::new(3, 2, 1000);
        cb.record_failure().await;
        cb.record_failure().await;
        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Open);
        assert!(cb.check().await.is_err());
    }

    #[tokio::test]
    async fn test_half_open_after_delay() {
        let cb = CircuitBreaker::new(2, 1, 50);
        cb.record_failure().await;
        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Open);

        tokio::time::sleep(Duration::from_millis(100)).await;

        assert!(cb.check().await.is_ok());
        assert_eq!(cb.state().await, CircuitState::HalfOpen);
    }

    #[tokio::test]
    async fn test_closes_after_success_in_half_open() {
        let cb = CircuitBreaker::new(2, 1, 50);
        cb.record_failure().await;
        cb.record_failure().await;

        tokio::time::sleep(Duration::from_millis(100)).await;
        cb.check().await.unwrap();

        cb.record_success().await;
        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_reopens_on_failure_in_half_open() {
        let cb = CircuitBreaker::new(2, 2, 50);
        cb.record_failure().await;
        cb.record_failure().await;

        tokio::time::sleep(Duration::from_millis(100)).await;
        cb.check().await.unwrap();

        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Open);
    }

    #[tokio::test]
    async fn test_reset() {
        let cb = CircuitBreaker::new(2, 2, 1000);
        cb.record_failure().await;
        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Open);

        cb.reset().await;
        assert_eq!(cb.state().await, CircuitState::Closed);
        assert!(cb.check().await.is_ok());
    }

    #[tokio::test]
    async fn test_success_resets_failure_count_in_closed() {
        let cb = CircuitBreaker::new(3, 2, 1000);
        cb.record_failure().await;
        cb.record_failure().await;
        cb.record_success().await;
        // Failure count was reset, so one more failure should not trip
        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Closed);
    }
}
