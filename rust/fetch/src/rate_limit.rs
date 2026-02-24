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
            // Calculate remaining time until the oldest request expires
            let oldest = timestamps.front().unwrap();
            let elapsed = now.duration_since(*oldest);
            let remaining = self.limit_period.saturating_sub(elapsed);
            Err(FetchError::RateLimit {
                remaining_ms: remaining.as_millis() as u64,
            })
        }
    }

    /// Acquire a slot, waiting if necessary until a slot becomes available.
    pub async fn acquire(&self) -> Result<(), FetchError> {
        loop {
            match self.try_acquire().await {
                Ok(()) => return Ok(()),
                Err(FetchError::RateLimit { remaining_ms }) => {
                    tokio::time::sleep(Duration::from_millis(remaining_ms + 1)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
