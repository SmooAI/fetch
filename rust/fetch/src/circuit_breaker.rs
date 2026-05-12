//! Circuit breaker implementation with Closed/Open/HalfOpen states.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::error::FetchError;

/// The state of the circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation; requests are allowed.
    Closed,
    /// Requests are rejected; waiting for the open delay to expire.
    Open,
    /// Trial mode; a limited number of requests are allowed to test recovery.
    HalfOpen,
}

/// Callback invoked when the breaker transitions between states. Receives
/// `(from, to)`. Mirrors the cross-port `on_state_change` / `OnStateChange`
/// callbacks.
///
/// Wrapped in an [`Arc`] so the configured breaker can be cheaply cloned
/// across task boundaries.
pub type CircuitStateChangeCallback = Arc<dyn Fn(CircuitState, CircuitState) + Send + Sync>;

/// Internal mutable state for the circuit breaker.
struct CircuitBreakerState {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
    /// Recent outcomes ring buffer (Some(true) = success, Some(false) = failure).
    /// Only populated when `failure_rate_threshold` is set.
    outcomes: VecDeque<bool>,
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
#[derive(Clone)]
pub struct CircuitBreaker {
    failure_threshold: u32,
    success_threshold: u32,
    open_state_delay: Duration,
    /// Failure rate (0.0–1.0) over the most recent `sliding_window_size` outcomes
    /// that trips the breaker. None = count-based detection (the original behavior).
    failure_rate_threshold: Option<f64>,
    /// Size of the outcome ring buffer used by rate-based detection.
    sliding_window_size: usize,
    on_state_change: Option<CircuitStateChangeCallback>,
    inner: Arc<Mutex<CircuitBreakerState>>,
}

impl std::fmt::Debug for CircuitBreaker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CircuitBreaker")
            .field("failure_threshold", &self.failure_threshold)
            .field("success_threshold", &self.success_threshold)
            .field("open_state_delay", &self.open_state_delay)
            .field("failure_rate_threshold", &self.failure_rate_threshold)
            .field("sliding_window_size", &self.sliding_window_size)
            .field("on_state_change", &self.on_state_change.is_some())
            .finish()
    }
}

impl CircuitBreaker {
    /// Create a new circuit breaker with count-based detection (the historical default).
    pub fn new(failure_threshold: u32, success_threshold: u32, open_state_delay_ms: u64) -> Self {
        Self {
            failure_threshold,
            success_threshold,
            open_state_delay: Duration::from_millis(open_state_delay_ms),
            failure_rate_threshold: None,
            sliding_window_size: 10,
            on_state_change: None,
            inner: Arc::new(Mutex::new(CircuitBreakerState {
                state: CircuitState::Closed,
                failure_count: 0,
                success_count: 0,
                last_failure_time: None,
                outcomes: VecDeque::new(),
            })),
        }
    }

    /// Enable rate-based detection: trip the breaker when the failure ratio over
    /// the most recent `sliding_window_size` outcomes meets or exceeds
    /// `threshold` (0.0–1.0), after at least `failure_threshold` samples have
    /// been observed.
    ///
    /// Returns `self` so this can be chained on construction.
    pub fn with_failure_rate_threshold(
        mut self,
        threshold: f64,
        sliding_window_size: usize,
    ) -> Self {
        self.failure_rate_threshold = Some(threshold);
        self.sliding_window_size = sliding_window_size.max(1);
        self
    }

    /// Register a state-change callback. Fires whenever the breaker transitions
    /// between Closed / Open / HalfOpen.
    pub fn with_on_state_change(mut self, callback: CircuitStateChangeCallback) -> Self {
        self.on_state_change = Some(callback);
        self
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
                        self.transition(&mut inner, CircuitState::HalfOpen);
                        inner.success_count = 0;
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
        self.push_outcome(&mut inner, true);

        match inner.state {
            CircuitState::HalfOpen => {
                inner.success_count += 1;
                if inner.success_count >= self.success_threshold {
                    self.transition(&mut inner, CircuitState::Closed);
                    inner.failure_count = 0;
                    inner.success_count = 0;
                    inner.outcomes.clear();
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
        self.push_outcome(&mut inner, false);

        match inner.state {
            CircuitState::Closed => {
                inner.failure_count += 1;

                // Rate-based detection takes priority when configured.
                if let Some(rate_threshold) = self.failure_rate_threshold {
                    if inner.outcomes.len() >= self.failure_threshold as usize {
                        let failures = inner.outcomes.iter().filter(|ok| !**ok).count() as f64;
                        let total = inner.outcomes.len() as f64;
                        if total > 0.0 && failures / total >= rate_threshold {
                            self.transition(&mut inner, CircuitState::Open);
                            inner.last_failure_time = Some(Instant::now());
                            tracing::warn!(
                                failure_count = inner.failure_count,
                                rate = failures / total,
                                "Circuit breaker transitioning from Closed to Open (rate-based)"
                            );
                            return;
                        }
                    }
                } else if inner.failure_count >= self.failure_threshold {
                    self.transition(&mut inner, CircuitState::Open);
                    inner.last_failure_time = Some(Instant::now());
                    tracing::warn!(
                        failure_count = inner.failure_count,
                        "Circuit breaker transitioning from Closed to Open"
                    );
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open goes back to open
                self.transition(&mut inner, CircuitState::Open);
                inner.last_failure_time = Some(Instant::now());
                inner.success_count = 0;
            }
            CircuitState::Open => {
                inner.last_failure_time = Some(Instant::now());
            }
        }
    }

    /// Reset the circuit breaker to the closed state.
    pub async fn reset(&self) {
        let mut inner = self.inner.lock().await;
        let prev = inner.state;
        inner.state = CircuitState::Closed;
        inner.failure_count = 0;
        inner.success_count = 0;
        inner.last_failure_time = None;
        inner.outcomes.clear();
        if prev != CircuitState::Closed {
            self.fire_callback(prev, CircuitState::Closed);
        }
    }

    fn transition(&self, inner: &mut CircuitBreakerState, target: CircuitState) {
        if inner.state == target {
            return;
        }
        let previous = inner.state;
        inner.state = target;
        self.fire_callback(previous, target);
    }

    fn fire_callback(&self, from: CircuitState, to: CircuitState) {
        if let Some(ref cb) = self.on_state_change {
            // User callbacks must not break the breaker's internal invariants;
            // panics propagate naturally (consistent with tokio task semantics)
            // but we make no attempt to catch_unwind.
            cb(from, to);
        }
    }

    fn push_outcome(&self, inner: &mut CircuitBreakerState, ok: bool) {
        // Only buffer outcomes when rate-based detection is enabled — saves
        // memory for the common count-based case.
        if self.failure_rate_threshold.is_none() {
            return;
        }
        if inner.outcomes.len() >= self.sliding_window_size {
            inner.outcomes.pop_front();
        }
        inner.outcomes.push_back(ok);
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

    #[tokio::test]
    async fn test_on_state_change_callback_fires_on_transitions() {
        use std::sync::Mutex as StdMutex;
        let log: Arc<StdMutex<Vec<(CircuitState, CircuitState)>>> =
            Arc::new(StdMutex::new(Vec::new()));
        let log_for_cb = log.clone();
        let cb = CircuitBreaker::new(2, 1, 50).with_on_state_change(Arc::new(move |from, to| {
            log_for_cb.lock().unwrap().push((from, to))
        }));

        // Closed → Open
        cb.record_failure().await;
        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Open);

        // Open → HalfOpen
        tokio::time::sleep(Duration::from_millis(100)).await;
        cb.check().await.unwrap();
        assert_eq!(cb.state().await, CircuitState::HalfOpen);

        // HalfOpen → Closed
        cb.record_success().await;
        assert_eq!(cb.state().await, CircuitState::Closed);

        let entries = log.lock().unwrap().clone();
        assert!(entries.contains(&(CircuitState::Closed, CircuitState::Open)));
        assert!(entries.contains(&(CircuitState::Open, CircuitState::HalfOpen)));
        assert!(entries.contains(&(CircuitState::HalfOpen, CircuitState::Closed)));
    }

    #[tokio::test]
    async fn test_rate_based_threshold_trips_breaker() {
        // failure_threshold serves as the minimum sample count. rate=0.7
        // means we trip once 70% of recent samples are failures.
        let cb = CircuitBreaker::new(4, 1, 1000).with_failure_rate_threshold(0.7, 10);

        // 3 successes, 1 failure → 25% → closed.
        cb.record_success().await;
        cb.record_success().await;
        cb.record_success().await;
        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Closed);

        // Add 6 more failures: window 3 ok / 7 fail → 70% → trips.
        for _ in 0..6 {
            cb.record_failure().await;
        }
        assert_eq!(cb.state().await, CircuitState::Open);
    }

    #[tokio::test]
    async fn test_rate_based_respects_min_samples() {
        // Need 5 minimum samples before rate evaluation kicks in.
        let cb = CircuitBreaker::new(5, 1, 1000).with_failure_rate_threshold(0.5, 10);

        // 4 consecutive failures → still closed because rate evaluation is suppressed.
        for _ in 0..4 {
            cb.record_failure().await;
        }
        assert_eq!(cb.state().await, CircuitState::Closed);

        // 5th failure: 5/5 = 100% → trips.
        cb.record_failure().await;
        assert_eq!(cb.state().await, CircuitState::Open);
    }
}
