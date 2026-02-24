//! Tests for the circuit breaker.

use std::time::Duration;

use smooai_fetch::circuit_breaker::{CircuitBreaker, CircuitState};
use smooai_fetch::error::FetchError;

#[tokio::test]
async fn test_initial_state_is_closed() {
    let cb = CircuitBreaker::new(3, 2, 1000);
    assert_eq!(cb.state().await, CircuitState::Closed);
}

#[tokio::test]
async fn test_allows_requests_in_closed_state() {
    let cb = CircuitBreaker::new(3, 2, 1000);
    assert!(cb.check().await.is_ok());
}

#[tokio::test]
async fn test_stays_closed_below_failure_threshold() {
    let cb = CircuitBreaker::new(3, 2, 1000);
    cb.record_failure().await;
    cb.record_failure().await;
    assert_eq!(cb.state().await, CircuitState::Closed);
    assert!(cb.check().await.is_ok());
}

#[tokio::test]
async fn test_opens_at_failure_threshold() {
    let cb = CircuitBreaker::new(3, 2, 1000);
    cb.record_failure().await;
    cb.record_failure().await;
    cb.record_failure().await;
    assert_eq!(cb.state().await, CircuitState::Open);
}

#[tokio::test]
async fn test_rejects_in_open_state() {
    let cb = CircuitBreaker::new(2, 1, 10000);
    cb.record_failure().await;
    cb.record_failure().await;

    let result = cb.check().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        FetchError::CircuitBreaker => {}
        other => panic!("Expected CircuitBreaker error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_transitions_to_half_open_after_delay() {
    let cb = CircuitBreaker::new(2, 1, 50);
    cb.record_failure().await;
    cb.record_failure().await;
    assert_eq!(cb.state().await, CircuitState::Open);

    // Wait for the delay
    tokio::time::sleep(Duration::from_millis(100)).await;

    // check() should transition to HalfOpen
    assert!(cb.check().await.is_ok());
    assert_eq!(cb.state().await, CircuitState::HalfOpen);
}

#[tokio::test]
async fn test_closes_after_success_in_half_open() {
    let cb = CircuitBreaker::new(2, 2, 50);
    cb.record_failure().await;
    cb.record_failure().await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    cb.check().await.unwrap(); // Transition to HalfOpen

    cb.record_success().await;
    assert_eq!(cb.state().await, CircuitState::HalfOpen); // Need 2 successes

    cb.record_success().await;
    assert_eq!(cb.state().await, CircuitState::Closed);
}

#[tokio::test]
async fn test_reopens_on_failure_in_half_open() {
    let cb = CircuitBreaker::new(2, 3, 50);
    cb.record_failure().await;
    cb.record_failure().await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    cb.check().await.unwrap();
    assert_eq!(cb.state().await, CircuitState::HalfOpen);

    cb.record_failure().await;
    assert_eq!(cb.state().await, CircuitState::Open);
}

#[tokio::test]
async fn test_success_resets_failure_count_in_closed() {
    let cb = CircuitBreaker::new(3, 2, 1000);

    cb.record_failure().await;
    cb.record_failure().await;
    // 2 failures, one away from threshold

    cb.record_success().await;
    // Success should reset failure count

    cb.record_failure().await;
    // Only 1 failure since reset, should still be closed
    assert_eq!(cb.state().await, CircuitState::Closed);
}

#[tokio::test]
async fn test_reset() {
    let cb = CircuitBreaker::new(2, 2, 10000);
    cb.record_failure().await;
    cb.record_failure().await;
    assert_eq!(cb.state().await, CircuitState::Open);

    cb.reset().await;
    assert_eq!(cb.state().await, CircuitState::Closed);
    assert!(cb.check().await.is_ok());
}

#[tokio::test]
async fn test_full_lifecycle() {
    let cb = CircuitBreaker::new(2, 1, 50);

    // Start closed
    assert_eq!(cb.state().await, CircuitState::Closed);
    assert!(cb.check().await.is_ok());

    // Trip open
    cb.record_failure().await;
    cb.record_failure().await;
    assert_eq!(cb.state().await, CircuitState::Open);
    assert!(cb.check().await.is_err());

    // Wait for half-open
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(cb.check().await.is_ok());
    assert_eq!(cb.state().await, CircuitState::HalfOpen);

    // Succeed to close
    cb.record_success().await;
    assert_eq!(cb.state().await, CircuitState::Closed);

    // Verify requests work again
    assert!(cb.check().await.is_ok());
}

#[tokio::test]
async fn test_multiple_half_open_cycles() {
    let cb = CircuitBreaker::new(2, 1, 50);

    // Cycle 1: trip open, go half-open, fail again
    cb.record_failure().await;
    cb.record_failure().await;
    assert_eq!(cb.state().await, CircuitState::Open);

    tokio::time::sleep(Duration::from_millis(100)).await;
    cb.check().await.unwrap();
    assert_eq!(cb.state().await, CircuitState::HalfOpen);

    cb.record_failure().await;
    assert_eq!(cb.state().await, CircuitState::Open);

    // Cycle 2: go half-open again, succeed this time
    tokio::time::sleep(Duration::from_millis(100)).await;
    cb.check().await.unwrap();
    assert_eq!(cb.state().await, CircuitState::HalfOpen);

    cb.record_success().await;
    assert_eq!(cb.state().await, CircuitState::Closed);
}
