//! Timeout wrapper using tokio.

use std::future::Future;
use std::time::Duration;

use crate::error::FetchError;

/// Execute a future with a timeout.
///
/// If the future completes before the timeout, its result is returned.
/// If the timeout elapses first, a `FetchError::Timeout` is returned.
pub async fn with_timeout<T, F>(timeout_ms: u64, future: F) -> Result<T, FetchError>
where
    F: Future<Output = Result<T, FetchError>>,
{
    match tokio::time::timeout(Duration::from_millis(timeout_ms), future).await {
        Ok(result) => result,
        Err(_elapsed) => Err(FetchError::Timeout { timeout_ms }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_with_timeout_success() {
        let result = with_timeout(1000, async { Ok::<_, FetchError>(42) }).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_with_timeout_expires() {
        let result = with_timeout(50, async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok::<_, FetchError>(42)
        })
        .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            FetchError::Timeout { timeout_ms } => assert_eq!(timeout_ms, 50),
            other => panic!("Expected Timeout error, got {:?}", other),
        }
    }
}
