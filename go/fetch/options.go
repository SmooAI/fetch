package fetch

import (
	"net/http"
	"time"
)

// RetryOptions configures retry behavior for failed requests.
type RetryOptions struct {
	// Attempts is the maximum number of retry attempts (0 means no retries).
	Attempts int
	// InitialInterval is the initial delay between retries.
	InitialInterval time.Duration
	// Factor is the multiplier applied to the interval for each retry (exponential backoff).
	Factor float64
	// JitterFraction is the fraction of the current interval to use as random jitter (0.0–1.0).
	JitterFraction float64
	// MaxInterval caps the maximum delay between retries.
	MaxInterval time.Duration
	// OnRejection is called after a failed attempt.
	// Return (true, 0) to retry with normal backoff.
	// Return (true, d) to retry after duration d.
	// Return (false, 0) to stop retrying.
	OnRejection func(err error, attempt int) (shouldRetry bool, retryAfter time.Duration)
}

// TimeoutOptions configures request timeout behavior.
type TimeoutOptions struct {
	// Timeout is the maximum duration to wait for a response.
	Timeout time.Duration
}

// RateLimitOptions configures the sliding-window rate limiter.
type RateLimitOptions struct {
	// MaxRequests is the maximum number of requests allowed per period.
	MaxRequests int
	// Period is the duration of the sliding window.
	Period time.Duration
}

// CircuitBreakerOptions configures the circuit breaker.
type CircuitBreakerOptions struct {
	// MaxRequests is the maximum number of requests allowed in the half-open state.
	// 0 means the circuit breaker allows 1 request.
	MaxRequests uint32
	// Interval is the cyclic period of the closed state for clearing internal Counts.
	// 0 means Counts are never cleared in the closed state.
	Interval time.Duration
	// Timeout is the period of the open state, after which the state changes to half-open.
	// 0 defaults to 60 seconds.
	Timeout time.Duration
	// ReadyToTrip is called with a copy of Counts whenever a request fails in the closed state.
	// If it returns true, the circuit breaker transitions to the open state.
	// nil defaults to tripping after 5 consecutive failures.
	ReadyToTrip func(counts CircuitBreakerCounts) bool
	// OnStateChange is called whenever the state of the circuit breaker changes.
	OnStateChange func(name string, from, to CircuitBreakerState)
	// IsSuccessful determines whether the returned error of a request should count as a success.
	// nil counts all non-nil errors as failures.
	IsSuccessful func(err error) bool
}

// CircuitBreakerState represents the state of a circuit breaker.
type CircuitBreakerState int

const (
	// CircuitBreakerStateClosed means the circuit breaker is closed and requests flow through.
	CircuitBreakerStateClosed CircuitBreakerState = iota
	// CircuitBreakerStateHalfOpen means the circuit breaker allows limited requests to test recovery.
	CircuitBreakerStateHalfOpen
	// CircuitBreakerStateOpen means the circuit breaker is open and requests are rejected.
	CircuitBreakerStateOpen
)

// CircuitBreakerCounts holds the numbers of requests and their successes/failures.
type CircuitBreakerCounts struct {
	Requests             uint32
	TotalSuccesses       uint32
	TotalFailures        uint32
	ConsecutiveSuccesses uint32
	ConsecutiveFailures  uint32
}

// LifecycleHooks provides hooks into the request/response lifecycle.
type LifecycleHooks struct {
	// PreRequest is called before sending the request.
	// It may modify the URL and request. Return the modified URL and request,
	// or return ("", nil) to leave them unchanged.
	PreRequest func(url string, req *http.Request) (newURL string, newReq *http.Request)

	// PostResponseSuccess is called after a successful response (2xx).
	// It may modify the FetchResponse before it is returned.
	PostResponseSuccess func(url string, req *http.Request, resp *FetchResponse[any]) *FetchResponse[any]

	// PostResponseError is called when the request results in an error.
	// It may return a replacement error or nil to keep the original.
	PostResponseError func(url string, req *http.Request, err error, resp *FetchResponse[any]) error
}

// RequestOptions configures per-request behavior.
type RequestOptions struct {
	// Timeout configures request timeout. Overrides client-level timeout.
	Timeout *TimeoutOptions
	// Retry configures retry behavior. Overrides client-level retry.
	Retry *RetryOptions
	// Hooks provides lifecycle hooks for this request.
	Hooks *LifecycleHooks
	// Headers are additional headers to include in the request.
	Headers http.Header
}
