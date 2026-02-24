//! Tests for the sliding window rate limiter.

use std::time::{Duration, Instant};

use smooai_fetch::error::FetchError;
use smooai_fetch::rate_limit::SlidingWindowRateLimiter;

#[tokio::test]
async fn test_allows_requests_within_limit() {
    let limiter = SlidingWindowRateLimiter::new(5, 1000);

    for _ in 0..5 {
        assert!(limiter.try_acquire().await.is_ok());
    }
}

#[tokio::test]
async fn test_rejects_requests_over_limit() {
    let limiter = SlidingWindowRateLimiter::new(2, 1000);

    assert!(limiter.try_acquire().await.is_ok());
    assert!(limiter.try_acquire().await.is_ok());

    let result = limiter.try_acquire().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        FetchError::RateLimit { remaining_ms } => {
            assert!(remaining_ms > 0);
            assert!(remaining_ms <= 1000);
        }
        other => panic!("Expected RateLimit error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_allows_after_window_expires() {
    let limiter = SlidingWindowRateLimiter::new(2, 100);

    assert!(limiter.try_acquire().await.is_ok());
    assert!(limiter.try_acquire().await.is_ok());
    assert!(limiter.try_acquire().await.is_err());

    // Wait for window to expire
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Should be allowed again
    assert!(limiter.try_acquire().await.is_ok());
    assert!(limiter.try_acquire().await.is_ok());
}

#[tokio::test]
async fn test_sliding_window_partial_expiry() {
    let limiter = SlidingWindowRateLimiter::new(2, 200);

    // First request
    assert!(limiter.try_acquire().await.is_ok());

    // Wait for half the window
    tokio::time::sleep(Duration::from_millis(110)).await;

    // Second request
    assert!(limiter.try_acquire().await.is_ok());

    // Third request should fail (both still in window)
    assert!(limiter.try_acquire().await.is_err());

    // Wait for the first request to expire (another ~100ms)
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Now the first should have expired, allowing a new one
    assert!(limiter.try_acquire().await.is_ok());
}

#[tokio::test]
async fn test_acquire_blocks_until_slot_available() {
    let limiter = SlidingWindowRateLimiter::new(1, 100);

    assert!(limiter.try_acquire().await.is_ok());

    let start = Instant::now();
    // This should block until the window expires
    limiter.acquire().await.unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() >= 100,
        "Should have waited at least 100ms, waited {:?}",
        elapsed
    );
}

#[tokio::test]
async fn test_concurrent_rate_limiting() {
    let limiter = SlidingWindowRateLimiter::new(3, 500);

    // Acquire all slots
    for _ in 0..3 {
        assert!(limiter.try_acquire().await.is_ok());
    }

    // All slots used, should fail
    assert!(limiter.try_acquire().await.is_err());

    // Wait and try again
    tokio::time::sleep(Duration::from_millis(550)).await;

    // Should be able to acquire again
    for _ in 0..3 {
        assert!(limiter.try_acquire().await.is_ok());
    }
}

#[tokio::test]
async fn test_rate_limit_error_has_correct_remaining_time() {
    let limiter = SlidingWindowRateLimiter::new(1, 500);

    assert!(limiter.try_acquire().await.is_ok());

    // Small delay
    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = limiter.try_acquire().await;
    match result.unwrap_err() {
        FetchError::RateLimit { remaining_ms } => {
            // Should be roughly 400ms remaining (500 - 100)
            assert!(
                remaining_ms > 300 && remaining_ms <= 500,
                "Remaining should be ~400ms, got {}ms",
                remaining_ms
            );
        }
        other => panic!("Expected RateLimit error, got {:?}", other),
    }
}
