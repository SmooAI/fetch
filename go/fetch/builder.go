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
	circuitBreakerOpts *CircuitBreakerOptions
	circuitBreakerName string
	hooks              *LifecycleHooks
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
func (b *ClientBuilder) WithRateLimit(maxRequests int, period time.Duration) *ClientBuilder {
	b.rateLimitOpts = &RateLimitOptions{
		MaxRequests: maxRequests,
		Period:      period,
	}
	return b
}

// WithCircuitBreaker configures the circuit breaker.
func (b *ClientBuilder) WithCircuitBreaker(name string, opts *CircuitBreakerOptions) *ClientBuilder {
	b.circuitBreakerName = name
	b.circuitBreakerOpts = opts
	return b
}

// WithHooks sets lifecycle hooks for the client.
func (b *ClientBuilder) WithHooks(hooks *LifecycleHooks) *ClientBuilder {
	b.hooks = hooks
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

// Build constructs the Client from the builder configuration.
func (b *ClientBuilder) Build() *Client {
	c := &Client{
		httpClient:  b.httpClient,
		baseHeaders: b.baseHeaders,
		retry:       b.retryOpts,
		timeout:     b.timeoutOpts,
		hooks:       b.hooks,
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
