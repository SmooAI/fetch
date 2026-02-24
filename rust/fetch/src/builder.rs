//! Builder pattern for creating configured fetch instances.

use std::sync::Arc;

use serde::de::DeserializeOwned;

use crate::circuit_breaker::CircuitBreaker;
use crate::defaults;
use crate::error::FetchError;
use crate::hooks::{
    LifecycleHooks, PostResponseErrorHook, PostResponseSuccessHook, PreRequestHook,
};
use crate::rate_limit::SlidingWindowRateLimiter;
use crate::response::FetchResponse;
use crate::types::{
    CircuitBreakerOptions, FetchContainerOptions, FetchOptions, RateLimitOptions, RequestInit,
    RetryOptions, TimeoutOptions,
};

/// Builder for creating configured fetch functions with retry, timeout, rate limiting,
/// circuit breaking, and lifecycle hooks.
///
/// # Example
///
/// ```rust,no_run
/// use smooai_fetch::builder::FetchBuilder;
/// use smooai_fetch::types::{RequestInit, Method};
/// use serde::Deserialize;
///
/// #[derive(Deserialize, Clone, Debug)]
/// struct MyResponse {
///     id: String,
///     name: String,
/// }
///
/// # async fn example() {
/// let client = FetchBuilder::<MyResponse>::new()
///     .with_timeout(5000)
///     .with_retry(smooai_fetch::defaults::default_retry_options())
///     .build();
///
/// let init = RequestInit {
///     method: Method::GET,
///     ..Default::default()
/// };
///
/// let response = client.fetch("https://api.example.com/data", init).await;
/// # }
/// ```
pub struct FetchBuilder<T: DeserializeOwned + Clone + Send + 'static> {
    fetch_options: FetchOptions,
    container_options: FetchContainerOptions,
    default_init: Option<RequestInit>,
    hooks: LifecycleHooks<T>,
}

impl<T: DeserializeOwned + Clone + Send + 'static> FetchBuilder<T> {
    /// Create a new FetchBuilder with default settings.
    pub fn new() -> Self {
        Self {
            fetch_options: FetchOptions {
                timeout: Some(defaults::default_timeout_options()),
                retry: Some(defaults::default_retry_options()),
            },
            container_options: FetchContainerOptions::default(),
            default_init: None,
            hooks: LifecycleHooks::default(),
        }
    }

    /// Set the default request init (headers, method, etc.) for all requests.
    pub fn with_init(mut self, init: RequestInit) -> Self {
        self.default_init = Some(init);
        self
    }

    /// Set the request timeout in milliseconds.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.fetch_options.timeout = Some(TimeoutOptions { timeout_ms });
        self
    }

    /// Configure retry behavior.
    pub fn with_retry(mut self, options: RetryOptions) -> Self {
        self.fetch_options.retry = Some(options);
        self
    }

    /// Disable retry (no retries will be attempted).
    pub fn without_retry(mut self) -> Self {
        self.fetch_options.retry = None;
        self
    }

    /// Disable timeout.
    pub fn without_timeout(mut self) -> Self {
        self.fetch_options.timeout = None;
        self
    }

    /// Configure rate limiting.
    pub fn with_rate_limit(mut self, limit_for_period: u32, limit_period_ms: u64) -> Self {
        self.container_options.rate_limit = Some(RateLimitOptions {
            limit_for_period,
            limit_period_ms,
        });
        self
    }

    /// Configure the circuit breaker.
    pub fn with_circuit_breaker(
        mut self,
        failure_threshold: u32,
        success_threshold: u32,
        open_state_delay_ms: u64,
    ) -> Self {
        self.container_options.circuit_breaker = Some(CircuitBreakerOptions {
            failure_threshold,
            success_threshold,
            open_state_delay_ms,
        });
        self
    }

    /// Set container options directly.
    pub fn with_container_options(mut self, options: FetchContainerOptions) -> Self {
        self.container_options = options;
        self
    }

    /// Set a pre-request hook.
    pub fn with_pre_request_hook(mut self, hook: PreRequestHook) -> Self {
        self.hooks.pre_request = Some(hook);
        self
    }

    /// Set a post-response success hook.
    pub fn with_post_response_success_hook(mut self, hook: PostResponseSuccessHook<T>) -> Self {
        self.hooks.post_response_success = Some(hook);
        self
    }

    /// Set a post-response error hook.
    pub fn with_post_response_error_hook(mut self, hook: PostResponseErrorHook<T>) -> Self {
        self.hooks.post_response_error = Some(hook);
        self
    }

    /// Build the configured fetch client.
    pub fn build(self) -> FetchClient<T> {
        let rate_limiter = self
            .container_options
            .rate_limit
            .map(|rl| SlidingWindowRateLimiter::new(rl.limit_for_period, rl.limit_period_ms));

        let circuit_breaker = self.container_options.circuit_breaker.map(|cb| {
            CircuitBreaker::new(
                cb.failure_threshold,
                cb.success_threshold,
                cb.open_state_delay_ms,
            )
        });

        FetchClient {
            fetch_options: self.fetch_options,
            default_init: self.default_init,
            rate_limiter,
            circuit_breaker,
            hooks: Arc::new(self.hooks),
        }
    }
}

impl<T: DeserializeOwned + Clone + Send + 'static> Default for FetchBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// A configured fetch client that encapsulates retry, timeout, rate limiting,
/// circuit breaking, and lifecycle hooks.
pub struct FetchClient<T: DeserializeOwned + Clone + Send + 'static> {
    fetch_options: FetchOptions,
    default_init: Option<RequestInit>,
    rate_limiter: Option<SlidingWindowRateLimiter>,
    circuit_breaker: Option<CircuitBreaker>,
    hooks: Arc<LifecycleHooks<T>>,
}

impl<T: DeserializeOwned + Clone + Send + 'static> FetchClient<T> {
    /// Execute a fetch request with the configured pipeline.
    pub async fn fetch(
        &self,
        url: &str,
        init: RequestInit,
    ) -> Result<FetchResponse<T>, FetchError> {
        // Merge default init with per-request init
        let merged_init = self.merge_init(init);

        crate::client::fetch::<T>(
            url,
            merged_init,
            Some(self.fetch_options.clone()),
            self.rate_limiter.as_ref(),
            self.circuit_breaker.as_ref(),
            Some(self.hooks.as_ref()),
        )
        .await
    }

    /// Execute a fetch request with per-request option overrides.
    pub async fn fetch_with_options(
        &self,
        url: &str,
        init: RequestInit,
        options: FetchOptions,
    ) -> Result<FetchResponse<T>, FetchError> {
        let merged_init = self.merge_init(init);

        crate::client::fetch::<T>(
            url,
            merged_init,
            Some(options),
            self.rate_limiter.as_ref(),
            self.circuit_breaker.as_ref(),
            Some(self.hooks.as_ref()),
        )
        .await
    }

    /// Get a reference to the circuit breaker, if configured.
    pub fn circuit_breaker(&self) -> Option<&CircuitBreaker> {
        self.circuit_breaker.as_ref()
    }

    /// Get a reference to the rate limiter, if configured.
    pub fn rate_limiter(&self) -> Option<&SlidingWindowRateLimiter> {
        self.rate_limiter.as_ref()
    }

    /// Merge default init with per-request init. Per-request values take precedence.
    fn merge_init(&self, init: RequestInit) -> RequestInit {
        match &self.default_init {
            Some(default) => {
                let mut merged_headers = default.headers.clone();
                merged_headers.extend(init.headers);
                RequestInit {
                    method: init.method,
                    headers: merged_headers,
                    body: init.body.or_else(|| default.body.clone()),
                }
            }
            None => init,
        }
    }
}
