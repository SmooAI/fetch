//! Core fetch client with full pipeline: hooks, timeout, retry, rate limit, circuit breaker.

use std::collections::HashMap;

use serde::de::DeserializeOwned;
use tracing;

use crate::circuit_breaker::CircuitBreaker;
use crate::defaults;
use crate::error::FetchError;
use crate::hooks::LifecycleHooks;
use crate::rate_limit::SlidingWindowRateLimiter;
use crate::response::FetchResponse;
use crate::retry;
use crate::timeout;
use crate::types::{FetchOptions, RequestInit};

/// Perform a single HTTP request (no retry, no timeout wrapper).
async fn do_single_request<T: DeserializeOwned>(
    url: &str,
    init: &RequestInit,
) -> Result<FetchResponse<T>, FetchError> {
    let client = reqwest::Client::new();

    let mut request_builder = client.request(init.method.to_reqwest(), url);

    // Set headers
    for (key, value) in &init.headers {
        request_builder = request_builder.header(key, value);
    }

    // Set body
    if let Some(ref body) = init.body {
        request_builder = request_builder.body(body.clone());
    }

    let response = request_builder.send().await?;

    // Extract response metadata
    let status = response.status().as_u16();
    let status_text = response
        .status()
        .canonical_reason()
        .unwrap_or("")
        .to_string();

    // Extract headers
    let mut headers = HashMap::new();
    for (name, value) in response.headers() {
        if let Ok(v) = value.to_str() {
            headers.insert(name.as_str().to_string(), v.to_string());
        }
    }

    // Determine if response is JSON
    let is_json = headers
        .get("content-type")
        .map(|ct| ct.contains("application/json"))
        .unwrap_or(false);

    // Read body
    let body = response.text().await.unwrap_or_default();

    // Parse data if JSON
    let data: Option<T> = if is_json && !body.is_empty() {
        match serde_json::from_str::<T>(&body) {
            Ok(parsed) => Some(parsed),
            Err(e) => {
                // If the response is OK, schema validation failure is an error
                if (200..300).contains(&(status as u32)) {
                    return Err(FetchError::SchemaValidation {
                        message: e.to_string(),
                    });
                }
                // For error responses, we do not fail on parse errors,
                // just leave data as None
                None
            }
        }
    } else {
        None
    };

    let fetch_response = FetchResponse::new(status, status_text, headers, body, is_json, data);

    if fetch_response.ok {
        Ok(fetch_response)
    } else {
        Err(FetchError::from_response(&fetch_response, None))
    }
}

/// Execute a fetch request with the full resilience pipeline.
///
/// Pipeline order:
/// 1. Pre-request hook
/// 2. Rate limit check (if configured)
/// 3. Circuit breaker check (if configured)
/// 4. Retry wrapper (if configured)
///    4a. Timeout wrapper (if configured)
///        4b. Actual HTTP request
/// 5. Post-response hooks (success or error)
///
/// # Type Parameters
/// - `T`: The expected response body type, must implement `DeserializeOwned`.
pub async fn fetch<T: DeserializeOwned + Clone + Send + 'static>(
    url: &str,
    init: RequestInit,
    options: Option<FetchOptions>,
    rate_limiter: Option<&SlidingWindowRateLimiter>,
    circuit_breaker: Option<&CircuitBreaker>,
    hooks: Option<&LifecycleHooks<T>>,
) -> Result<FetchResponse<T>, FetchError> {
    let opts = options.unwrap_or_default();

    // 1. Apply pre-request hook
    let (url, init) = if let Some(hooks) = hooks {
        if let Some(ref pre_request) = hooks.pre_request {
            match pre_request(url, &init) {
                Some((new_url, new_init)) => (new_url, new_init),
                None => (url.to_string(), init),
            }
        } else {
            (url.to_string(), init)
        }
    } else {
        (url.to_string(), init)
    };

    tracing::debug!(
        method = %init.method,
        url = %url,
        "Sending HTTP request"
    );

    // 2. Rate limit check
    if let Some(limiter) = rate_limiter {
        limiter.acquire().await?;
    }

    // 3. Circuit breaker check
    if let Some(cb) = circuit_breaker {
        cb.check().await?;
    }

    // Build the operation closure for retry
    let url_clone = url.clone();
    let init_clone = init.clone();
    let timeout_ms = opts
        .timeout
        .as_ref()
        .map(|t| t.timeout_ms)
        .unwrap_or(defaults::DEFAULT_TIMEOUT_MS);

    let operation = |_attempt: u32| {
        let url = url_clone.clone();
        let init = init_clone.clone();
        async move { timeout::with_timeout(timeout_ms, do_single_request::<T>(&url, &init)).await }
    };

    // 4. Execute with retry (or just once if no retry options)
    let result = if let Some(ref retry_opts) = opts.retry {
        retry::execute_with_retry(retry_opts, operation).await
    } else {
        // No retry, just execute once with timeout
        timeout::with_timeout(timeout_ms, do_single_request::<T>(&url, &init)).await
    };

    // Record success/failure with circuit breaker
    match &result {
        Ok(_) => {
            if let Some(cb) = circuit_breaker {
                cb.record_success().await;
            }
        }
        Err(_) => {
            if let Some(cb) = circuit_breaker {
                cb.record_failure().await;
            }
        }
    }

    // 5. Apply post-response hooks
    match result {
        Ok(response) => {
            if let Some(hooks) = hooks {
                if let Some(ref post_success) = hooks.post_response_success {
                    match post_success(&url, &init, response.clone()) {
                        Some(modified) => Ok(modified),
                        None => Ok(response),
                    }
                } else {
                    Ok(response)
                }
            } else {
                Ok(response)
            }
        }
        Err(err) => {
            if let Some(hooks) = hooks {
                if let Some(ref post_error) = hooks.post_response_error {
                    match post_error(&url, &init, &err, None) {
                        Some(modified_err) => Err(modified_err),
                        None => Err(err),
                    }
                } else {
                    Err(err)
                }
            } else {
                Err(err)
            }
        }
    }
}
