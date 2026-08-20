//! Sliding window rate limiter.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::error::FetchError;

/// A sliding window rate limiter that tracks request timestamps.
///
/// It allows at most `limit_for_period` requests within a sliding window
/// of `limit_period_ms` milliseconds. If a request would exceed the limit,
/// a `FetchError::RateLimit` is returned with the remaining time until
/// a slot becomes available.
#[derive(Debug, Clone)]
pub struct SlidingWindowRateLimiter {
    /// Maximum number of requests in the window.
    limit_for_period: u32,
    /// Window duration.
    limit_period: Duration,
    /// Timestamps of recent requests.
    timestamps: Arc<Mutex<VecDeque<Instant>>>,
}

impl SlidingWindowRateLimiter {
    /// Create a new sliding window rate limiter.
    pub fn new(limit_for_period: u32, limit_period_ms: u64) -> Self {
        Self {
            limit_for_period,
            limit_period: Duration::from_millis(limit_period_ms),
            timestamps: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Try to acquire a slot. Returns Ok(()) if allowed, or
    /// FetchError::RateLimit with the remaining time if the limit is exceeded.
    pub async fn try_acquire(&self) -> Result<(), FetchError> {
        let mut timestamps = self.timestamps.lock().await;
        let now = Instant::now();

        // Remove expired timestamps outside the sliding window
        while let Some(front) = timestamps.front() {
            if now.duration_since(*front) >= self.limit_period {
                timestamps.pop_front();
            } else {
                break;
            }
        }

        if timestamps.len() < self.limit_for_period as usize {
            timestamps.push_back(now);
            Ok(())
        } else {
            // Calculate remaining time until the oldest request expires.
            // The pruning loop above already dropped every expired timestamp, so
            // `remaining` here is always > 0 — but `as_millis` TRUNCATES, so a
            // sub-millisecond remainder reported 0. A caller that honors
            // `remaining_ms` then retries immediately and spins on the boundary,
            // and an error saying "0ms left" while refusing the request is a lie
            // about its own state. Round up to the next whole millisecond.
            let oldest = timestamps.front().unwrap();
            let elapsed = now.duration_since(*oldest);
            let remaining = self.limit_period.saturating_sub(elapsed);
            Err(FetchError::RateLimit {
                remaining_ms: (remaining.as_millis() as u64).max(1),
            })
        }
    }

    /// Acquire a slot, waiting if necessary until a slot becomes available.
    pub async fn acquire(&self) -> Result<(), FetchError> {
        loop {
            match self.try_acquire().await {
                Ok(()) => return Ok(()),
                Err(FetchError::RateLimit { remaining_ms }) => {
                    // remaining_ms is already rounded up to a whole millisecond by
                    // try_acquire, so this sleep always clears the window boundary.
                    tokio::time::sleep(Duration::from_millis(remaining_ms)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A rejection that reports "0ms remaining" while refusing the request is a
    /// lie about the limiter's own state, and a caller honoring it spins hot on
    /// the window boundary. `as_millis` truncates, so any sub-millisecond
    /// remainder used to report exactly that.
    #[tokio::test]
    async fn rejection_never_reports_zero_remaining() {
        // A 1ms window makes the sub-millisecond remainder the common case
        // rather than a rare boundary hit.
        let limiter = SlidingWindowRateLimiter::new(1, 1);
        limiter.try_acquire().await.unwrap();
        for _ in 0..200 {
            if let Err(FetchError::RateLimit { remaining_ms }) = limiter.try_acquire().await {
                assert!(
                    remaining_ms > 0,
                    "a rejection must report a positive wait, got {remaining_ms}"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_allows_within_limit() {
        let limiter = SlidingWindowRateLimiter::new(3, 1000);
        assert!(limiter.try_acquire().await.is_ok());
        assert!(limiter.try_acquire().await.is_ok());
        assert!(limiter.try_acquire().await.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiter_rejects_over_limit() {
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
    async fn test_rate_limiter_allows_after_window_expires() {
        let limiter = SlidingWindowRateLimiter::new(2, 100);
        assert!(limiter.try_acquire().await.is_ok());
        assert!(limiter.try_acquire().await.is_ok());

        // Wait for window to expire
        tokio::time::sleep(Duration::from_millis(150)).await;

        assert!(limiter.try_acquire().await.is_ok());
        assert!(limiter.try_acquire().await.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiter_acquire_waits() {
        let limiter = SlidingWindowRateLimiter::new(1, 100);
        assert!(limiter.try_acquire().await.is_ok());

        let start = Instant::now();
        // This should wait until the window expires
        limiter.acquire().await.unwrap();
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() >= 100);
    }
}
