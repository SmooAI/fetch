package fetch

import (
	"net/http"
	"time"
)

// RetryDecision indicates what the retry loop should do after a failed attempt.
type RetryDecision int

const (
	// RetryDefault keeps the built-in exponential+jitter behavior.
	RetryDefault RetryDecision = iota
	// RetryWithDelay uses the duration returned alongside the decision.
	RetryWithDelay
	// RetryAbort stops retrying and returns the last error.
	RetryAbort
	// RetrySkip skips the current delay entirely and moves to the next attempt.
	RetrySkip
)

// RetryContext is passed to OnRejectionFunc and describes the current retry state.
type RetryContext struct {
	// Attempt is the 1-based attempt number that just failed.
	Attempt int
	// LastError is the error from the last attempt. May be nil.
	LastError error
	// LastStatus is the HTTP status code from the last response, or 0 if no response.
	LastStatus int
	// Elapsed is the total time spent since the retry loop started.
	Elapsed time.Duration
}

// OnRejectionFunc is called after a failed attempt and returns a decision about the next step.
// The returned duration is only consulted when the decision is RetryWithDelay.
type OnRejectionFunc func(ctx RetryContext) (RetryDecision, time.Duration)

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
	// FastFirst, when true, fires the first retry with zero delay.
	FastFirst bool
	// OnRejection is consulted before each retry. If nil, all attempts use RetryDefault.
	OnRejection OnRejectionFunc
}

// RateLimitRetryOptions aliases RetryOptions for rate-limit-specific retry configuration,
// matching the TypeScript container-options shape where rateLimit.retry is a RetryOptions.
type RateLimitRetryOptions = RetryOptions

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

// FetchContainerOptions groups the container-level options (rate limit, rate-limit retry,
// circuit breaker) so they can be applied in a single call via WithContainerOptions,
// matching the ergonomics of the TypeScript FetchBuilder.withContainerOptions() API.
type FetchContainerOptions struct {
	// RateLimit configures the sliding-window rate limiter. nil leaves the current setting untouched.
	RateLimit *RateLimitOptions
	// RateLimitRetry configures retry behavior specifically for rate-limit rejections.
	// nil leaves the current setting untouched.
	RateLimitRetry *RateLimitRetryOptions
	// CircuitBreaker configures the circuit breaker. nil leaves the current setting untouched.
	CircuitBreaker *CircuitBreakerOptions
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
