package fetch

import (
	"net/http"
	"time"
)

// ClientBuilder provides a fluent API for constructing a configured Client.
type ClientBuilder struct {
	httpClient         *http.Client
	baseHeaders        http.Header
	retryOpts          *RetryOptions
	timeoutOpts        *TimeoutOptions
	rateLimitOpts      *RateLimitOptions
	rateLimitRetryOpts *RateLimitRetryOptions
	circuitBreakerOpts *CircuitBreakerOptions
	circuitBreakerName string
	hooks              *LifecycleHooks
	authProvider       AuthTokenProvider
	authScheme         string
}

// NewClientBuilder creates a new ClientBuilder with default retry and timeout options.
func NewClientBuilder() *ClientBuilder {
	retryOpts := DefaultRetryOptions
	timeoutOpts := DefaultTimeoutOptions
	return &ClientBuilder{
		retryOpts:   &retryOpts,
		timeoutOpts: &timeoutOpts,
	}
}

// WithHTTPClient sets the underlying *http.Client.
func (b *ClientBuilder) WithHTTPClient(c *http.Client) *ClientBuilder {
	b.httpClient = c
	return b
}

// WithBaseHeaders sets headers that are included in every request.
func (b *ClientBuilder) WithBaseHeaders(headers http.Header) *ClientBuilder {
	b.baseHeaders = headers
	return b
}

// WithTimeout sets the request timeout duration.
func (b *ClientBuilder) WithTimeout(timeout time.Duration) *ClientBuilder {
	b.timeoutOpts = &TimeoutOptions{Timeout: timeout}
	return b
}

// WithRetry configures retry behavior. Pass nil to disable retries.
func (b *ClientBuilder) WithRetry(opts *RetryOptions) *ClientBuilder {
	b.retryOpts = opts
	return b
}

// WithRateLimit configures the sliding-window rate limiter.
// Rate-limit rejections are retried using DefaultRateLimitRetryOptions unless
// WithRateLimitRetry is called.
func (b *ClientBuilder) WithRateLimit(maxRequests int, period time.Duration) *ClientBuilder {
	b.rateLimitOpts = &RateLimitOptions{
		MaxRequests: maxRequests,
		Period:      period,
	}
	return b
}

// WithRateLimitRetry configures retry behavior that applies specifically to rate-limit
// rejections (i.e., *RateLimitError returned by the sliding-window rate limiter).
// This mirrors the TypeScript FetchBuilder.withRateLimit(... retryOptions) overload.
// Pass nil to clear the setting (and fall back to the default retry behavior for
// rate-limit errors inside the main retry loop).
func (b *ClientBuilder) WithRateLimitRetry(opts *RateLimitRetryOptions) *ClientBuilder {
	b.rateLimitRetryOpts = opts
	return b
}

// WithCircuitBreaker configures the circuit breaker.
func (b *ClientBuilder) WithCircuitBreaker(name string, opts *CircuitBreakerOptions) *ClientBuilder {
	b.circuitBreakerName = name
	b.circuitBreakerOpts = opts
	return b
}

// WithCircuitBreakerStateChange registers a state-change callback on the
// configured circuit breaker. If WithCircuitBreaker has not been called, a
// fresh CircuitBreakerOptions is created so the callback has somewhere to live.
//
// This exposes the underlying sony/gobreaker `OnStateChange` at the builder
// level (mirrors the SMOODEV-950 onStateChange parity surface).
func (b *ClientBuilder) WithCircuitBreakerStateChange(fn func(name string, from, to CircuitBreakerState)) *ClientBuilder {
	if b.circuitBreakerOpts == nil {
		b.circuitBreakerOpts = &CircuitBreakerOptions{}
	}
	b.circuitBreakerOpts.OnStateChange = fn
	return b
}

// WithHooks sets lifecycle hooks for the client.
func (b *ClientBuilder) WithHooks(hooks *LifecycleHooks) *ClientBuilder {
	b.hooks = hooks
	return b
}

// WithAuthTokenProvider registers a sync-or-async auth token provider that is
// invoked before every request and used to populate the `Authorization`
// header. The provider receives the request context, so it can short-circuit
// on cancellation/timeouts. Mirrors the .NET `AuthTokenProvider` delegate and
// the TypeScript `FetchBuilder.withAuthTokenProvider(...)` method.
//
// Pass an empty string for scheme to default to "Bearer".
func (b *ClientBuilder) WithAuthTokenProvider(provider AuthTokenProvider, scheme string) *ClientBuilder {
	b.authProvider = provider
	if scheme == "" {
		scheme = "Bearer"
	}
	b.authScheme = scheme
	return b
}

// WithNoRetry disables retries.
func (b *ClientBuilder) WithNoRetry() *ClientBuilder {
	b.retryOpts = nil
	return b
}

// WithNoTimeout disables the request timeout.
func (b *ClientBuilder) WithNoTimeout() *ClientBuilder {
	b.timeoutOpts = nil
	return b
}

// WithContainerOptions applies all container-level options (rate limit, rate-limit retry,
// circuit breaker) in a single call. Nil fields leave the corresponding setting untouched.
// This mirrors the TypeScript FetchBuilder.withContainerOptions() API.
func (b *ClientBuilder) WithContainerOptions(opts FetchContainerOptions) *ClientBuilder {
	if opts.RateLimit != nil {
		rl := *opts.RateLimit
		b.rateLimitOpts = &rl
	}
	if opts.RateLimitRetry != nil {
		rlr := *opts.RateLimitRetry
		b.rateLimitRetryOpts = &rlr
	}
	if opts.CircuitBreaker != nil {
		cb := *opts.CircuitBreaker
		b.circuitBreakerOpts = &cb
	}
	return b
}

// Build constructs the Client from the builder configuration.
func (b *ClientBuilder) Build() *Client {
	c := &Client{
		httpClient:     b.httpClient,
		baseHeaders:    b.baseHeaders,
		retry:          b.retryOpts,
		timeout:        b.timeoutOpts,
		rateLimitRetry: b.rateLimitRetryOpts,
		hooks:          b.hooks,
		authProvider:   b.authProvider,
		authScheme:     b.authScheme,
	}
	if c.authScheme == "" {
		c.authScheme = "Bearer"
	}

	if c.httpClient == nil {
		c.httpClient = &http.Client{}
	}

	if c.baseHeaders == nil {
		c.baseHeaders = make(http.Header)
	}

	if b.rateLimitOpts != nil {
		c.rateLimiter = NewSlidingWindowRateLimiter(b.rateLimitOpts.MaxRequests, b.rateLimitOpts.Period)
	}

	if b.circuitBreakerOpts != nil {
		name := b.circuitBreakerName
		if name == "" {
			name = "smooai-fetch-circuit-breaker"
		}
		c.circuitBreaker = NewCircuitBreaker(name, b.circuitBreakerOpts)
	}

	return c
}
