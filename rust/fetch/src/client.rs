//! Core fetch client with full pipeline: hooks, timeout, retry, rate limit, circuit breaker.

use std::collections::HashMap;
use std::time::Duration;

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
use crate::types::{FetchOptions, RateLimitRetryOptions, RequestInit};

/// Acquire a rate-limit slot using a dedicated retry loop.
///
/// Unlike the main retry loop, this is driven by [`FetchError::RateLimit`]
/// rejections (which are not "retryable" per [`FetchError::is_retryable`]),
/// so the standard exponential+jitter backoff in [`crate::retry`] does not
/// apply directly. The honored decision order matches the Go port:
///
/// 1. If the limiter reports `remaining_ms`, use that as the sleep duration
///    (mirrors a `Retry-After` header).
/// 2. Otherwise fall back to [`retry::calculate_backoff`] using `rl_retry`.
/// 3. `fast_first = true` short-circuits the very first delay to zero.
async fn acquire_with_retry(
    limiter: &SlidingWindowRateLimiter,
    rl_retry: &RateLimitRetryOptions,
) -> Result<(), FetchError> {
    let max_attempts = 1 + rl_retry.attempts;
    let mut last_err: Option<FetchError> = None;

    for attempt in 0..max_attempts {
        match limiter.try_acquire().await {
            Ok(()) => return Ok(()),
            Err(FetchError::RateLimit { remaining_ms }) => {
                let err = FetchError::RateLimit { remaining_ms };
                if attempt + 1 >= max_attempts {
                    last_err = Some(err);
                    break;
                }

                // Default decision: sleep for remaining window time, capped by max_interval_ms.
                let mut delay_ms: u64 = if rl_retry.fast_first && attempt == 0 {
                    0
                } else if remaining_ms > 0 {
                    remaining_ms
                } else {
                    retry::calculate_backoff(attempt, rl_retry)
                };

                if let Some(max) = rl_retry.max_interval_ms {
                    delay_ms = delay_ms.min(max);
                }

                last_err = Some(err);
                if delay_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
            }
            Err(other) => return Err(other),
        }
    }

    Err(FetchError::Retry {
        attempts: rl_retry.attempts,
        source: Box::new(last_err.unwrap_or(FetchError::RateLimit { remaining_ms: 0 })),
    })
}

/// Inject W3C trace context (`traceparent`/`tracestate`) into an outbound request.
///
/// # Why this exists
///
/// api-prime already EXTRACTS `traceparent` on ingress, but nothing on the
/// platform ever injected it on egress — so every service-to-service call began a
/// brand new root trace. Measured on 2026-08-14 over a three-hour window:
/// 34,961 traces touched exactly one service, and 4 touched two. Distributed
/// tracing did not work, and no amount of extra spans inside a service could fix
/// it.
///
/// This is the client, which is the correct place for propagation: services are
/// already required to use `@smooai/fetch` over raw HTTP, so wiring it once here
/// covers the fleet.
///
/// # Three guards, each for a reason
///
/// 1. **Optional feature.** With `otel` off this is a no-op and the crate does
///    not link OpenTelemetry — an OSS HTTP client must not force that on anyone.
/// 2. **Valid span contexts only.** An unregistered TracerProvider yields
///    `INVALID_SPAN_CONTEXT` (all-zero ids). Injecting that writes a malformed
///    `traceparent` that a downstream service will either reject or, worse,
///    adopt — poisoning its trace. The sibling logger shipped exactly this bug,
///    where all-zero ids overwrote a real correlation id.
/// 3. **Caller wins.** If the caller already set `traceparent` explicitly, theirs
///    is left alone. A client that silently rewrites an intentional header is
///    worse than one that does nothing.
#[cfg(feature = "otel")]
fn inject_trace_context(
    builder: reqwest::RequestBuilder,
    caller_headers: &std::collections::HashMap<String, String>,
) -> reqwest::RequestBuilder {
    use opentelemetry::trace::TraceContextExt as _;
    use tracing_opentelemetry::OpenTelemetrySpanExt as _;

    if caller_headers
        .keys()
        .any(|k| k.eq_ignore_ascii_case("traceparent"))
    {
        return builder;
    }

    // TWO context homes, and neither falls back to the other.
    //
    // Every SmooAI Rust service carries its span as a `tracing` span picked up by
    // a tracing-opentelemetry layer; that context is reachable ONLY through
    // `Span::current().context()`. `opentelemetry::Context::current()` sees just
    // OTel-native spans. Reading only the latter — which is what this function
    // did first — makes the entire feature a silent no-op in production while
    // passing a test that happens to create an OTel-native span.
    // Read the `tracing` span's context first, then fall back to the OTel-native
    // one.
    //
    // Measured, not assumed: with tracing-opentelemetry 0.33 + opentelemetry
    // 0.32, `Context::current()` DOES see a tracing span, so the fallback alone
    // would work today. This ordering is belt-and-braces — it does not depend on
    // tracing-opentelemetry continuing to mirror into the OTel thread-local, and
    // it costs one extra call.
    let cx = tracing::Span::current().context();
    let cx = if cx.span().span_context().is_valid() {
        cx
    } else {
        opentelemetry::Context::current()
    };

    if !cx.span().span_context().is_valid() {
        return builder;
    }

    // `HashMap<String, String>` implements `Injector` upstream (opentelemetry
    // 0.32 propagation/mod.rs), so there is no carrier type to write and no
    // `opentelemetry-http` dependency to add.
    let mut carrier: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    opentelemetry::global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&cx, &mut carrier);
    });

    let mut builder = builder;
    for (key, value) in carrier {
        // reqwest's `.header()` APPENDS rather than replaces, so a duplicate
        // would send two traceparents. The caller-wins guard above is what keeps
        // that from happening.
        builder = builder.header(key, value);
    }
    builder
}

/// No-op when the `otel` feature is off — the crate does not link OpenTelemetry.
#[cfg(not(feature = "otel"))]
fn inject_trace_context(
    builder: reqwest::RequestBuilder,
    _caller_headers: &std::collections::HashMap<String, String>,
) -> reqwest::RequestBuilder {
    builder
}

/// Perform a single HTTP request (no retry, no timeout wrapper).
async fn do_single_request<T: DeserializeOwned>(
    url: &str,
    init: &RequestInit,
    connect_timeout_ms: Option<u64>,
) -> Result<FetchResponse<T>, FetchError> {
    let client = match connect_timeout_ms {
        Some(ms) => reqwest::Client::builder()
            .connect_timeout(Duration::from_millis(ms))
            .build()?,
        None => reqwest::Client::new(),
    };

    let mut request_builder = client.request(init.method.to_reqwest(), url);

    // Set headers
    for (key, value) in &init.headers {
        request_builder = request_builder.header(key, value);
    }

    // Continue the caller's trace across the hop. Applied AFTER the caller's own
    // headers so an explicitly-set `traceparent` still wins (see the function).
    request_builder = inject_trace_context(request_builder, &init.headers);

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
///    4b. Actual HTTP request
/// 5. Post-response hooks (success or error)
///
/// # Type Parameters
/// - `T`: The expected response body type, must implement `DeserializeOwned`.
pub async fn fetch<T: DeserializeOwned + Clone + Send + 'static>(
    url: &str,
    init: RequestInit,
    options: Option<FetchOptions>,
    rate_limiter: Option<&SlidingWindowRateLimiter>,
    rate_limit_retry: Option<&RateLimitRetryOptions>,
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

    // SMOODEV-2716: a URL carries credentials in its userinfo password and its
    // query params, and the tracing subscriber does no redaction of its own.
    tracing::debug!(
        method = %init.method,
        url = %crate::redact::redact_url(&url),
        "Sending HTTP request"
    );

    // 2. Rate limit check
    //
    // When `rate_limit_retry` is configured AND a rate limiter is active, run
    // `try_acquire()` inside a dedicated retry loop so rate-limit rejections do
    // not consume the main retry budget. Otherwise fall back to the existing
    // `acquire()` behavior (which blocks until a slot is available).
    if let Some(limiter) = rate_limiter {
        match rate_limit_retry {
            Some(rl_retry) if rl_retry.attempts > 0 => {
                acquire_with_retry(limiter, rl_retry).await?;
            }
            _ => {
                limiter.acquire().await?;
            }
        }
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
    let connect_timeout_ms = opts.connect_timeout_ms;

    let operation = |_attempt: u32| {
        let url = url_clone.clone();
        let init = init_clone.clone();
        async move {
            timeout::with_timeout(
                timeout_ms,
                do_single_request::<T>(&url, &init, connect_timeout_ms),
            )
            .await
        }
    };

    // 4. Execute with retry (or just once if no retry options)
    let result = if let Some(ref retry_opts) = opts.retry {
        retry::execute_with_retry(retry_opts, operation).await
    } else {
        // No retry, just execute once with timeout
        timeout::with_timeout(
            timeout_ms,
            do_single_request::<T>(&url, &init, connect_timeout_ms),
        )
        .await
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
