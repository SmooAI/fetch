package fetch

import (
	"context"
	"errors"
	"math"
	"math/rand"
	"net/http"
	"strings"
	"sync/atomic"
	"time"
)

// IdempotencyKeyHeader is the request header that makes a non-idempotent
// request safe to retry: a server that honours it deduplicates replays that
// carry the same key. A request with a non-empty value for it is retried even
// when RetryOptions.AllowNonIdempotent is false.
const IdempotencyKeyHeader = "Idempotency-Key"

// IsIdempotentMethod reports whether method is idempotent per RFC 9110 §9.2.2
// (GET, HEAD, OPTIONS, TRACE, PUT, DELETE), compared case-insensitively. POST,
// PATCH, CONNECT and anything unrecognised are not.
func IsIdempotentMethod(method string) bool {
	switch strings.ToUpper(method) {
	case http.MethodGet, http.MethodHead, http.MethodOptions, http.MethodTrace, http.MethodPut, http.MethodDelete:
		return true
	default:
		return false
	}
}

// isRetryEligible reports whether a failed attempt of this request may be
// re-sent: its method is idempotent, the caller opted in via
// AllowNonIdempotent, or it carries a non-empty Idempotency-Key header.
func isRetryEligible(method string, header http.Header, opts *RetryOptions) bool {
	if IsIdempotentMethod(method) {
		return true
	}
	if opts != nil && opts.AllowNonIdempotent {
		return true
	}
	// http.Header.Get canonicalises, so any casing the caller used matches.
	return strings.TrimSpace(header.Get(IdempotencyKeyHeader)) != ""
}

// retryGate carries the retry-eligibility of one Fetch call out of the attempt
// that computed it. It is seeded from the pre-hook view of the request and
// overwritten by each attempt once pre-request hooks and the auth provider
// have run, since either can change the method or add an Idempotency-Key.
// Atomic because a timed-out attempt's goroutine may still be writing it.
type retryGate struct {
	eligible atomic.Bool
	opts     *RetryOptions
}

func (g *retryGate) record(req *http.Request) {
	if g == nil {
		return
	}
	g.eligible.Store(isRetryEligible(req.Method, req.Header, g.opts))
}

// notRetryEligibleError marks a failure of a request that must not be re-sent.
// ExecuteWithRetry unwraps it and returns the underlying error immediately,
// without consulting OnRejection and without the RetryError wrapper.
type notRetryEligibleError struct {
	err error
}

func (e *notRetryEligibleError) Error() string { return e.err.Error() }
func (e *notRetryEligibleError) Unwrap() error { return e.err }

// isPreSendRejection reports whether err was raised before anything reached
// the network (the in-process rate limiter or an open circuit breaker). Such
// a request produced no side effect, so retrying it is safe for any method.
func isPreSendRejection(err error) bool {
	var rl *RateLimitError
	var cb *CircuitBreakerError
	return errors.As(err, &rl) || errors.As(err, &cb)
}

// CalculateBackoff computes the backoff duration for a given attempt using exponential backoff with jitter.
//
// The formula is:  interval = initialInterval * (factor ^ attempt) +/- jitter
//
// Where jitter is a random value in [-jitterFraction*interval, +jitterFraction*interval].
// If maxInterval > 0, the result is capped at maxInterval.
func CalculateBackoff(attempt int, opts RetryOptions) time.Duration {
	if attempt <= 0 {
		return opts.InitialInterval
	}

	factor := opts.Factor
	if factor <= 0 {
		factor = 1.0
	}

	interval := float64(opts.InitialInterval) * math.Pow(factor, float64(attempt))

	if opts.JitterFraction > 0 {
		jitter := interval * opts.JitterFraction
		interval += (rand.Float64()*2 - 1) * jitter
	}

	if interval < 0 {
		interval = float64(opts.InitialInterval)
	}

	d := time.Duration(interval)

	if opts.MaxInterval > 0 && d > opts.MaxInterval {
		d = opts.MaxInterval
	}

	return d
}

// statusCodeFromError pulls an HTTP status code out of known error types, or returns 0.
func statusCodeFromError(err error) int {
	if err == nil {
		return 0
	}
	var httpErr *HTTPResponseError
	if errors.As(err, &httpErr) {
		return httpErr.StatusCode
	}
	return 0
}

// ExecuteWithRetry executes fn with retry logic according to opts.
// It calls fn up to 1 + opts.Attempts times (1 initial + N retries).
// Between attempts it sleeps for the duration dictated by the OnRejection callback
// (when present) or the computed exponential backoff. The context can cancel the retry loop.
//
// When opts.FastFirst is true, the first retry fires with zero delay regardless of the
// configured interval or OnRejection decision (other than RetryAbort / RetrySkip).
func ExecuteWithRetry[T any](ctx context.Context, opts RetryOptions, fn func(ctx context.Context) (T, error)) (T, error) {
	totalAttempts := 1 + opts.Attempts
	var lastErr error
	var zero T
	startedAt := time.Now()

	for attempt := 0; attempt < totalAttempts; attempt++ {
		result, err := fn(ctx)
		if err == nil {
			return result, nil
		}
		if ne, ok := err.(*notRetryEligibleError); ok {
			return zero, ne.err
		}
		lastErr = err

		// If this is the last attempt, don't bother with retry bookkeeping.
		if attempt >= totalAttempts-1 {
			break
		}

		// Default decision is to retry with the built-in backoff.
		decision := RetryDefault
		var customDelay time.Duration

		if opts.OnRejection != nil {
			decision, customDelay = opts.OnRejection(RetryContext{
				Attempt:    attempt + 1,
				LastError:  err,
				LastStatus: statusCodeFromError(err),
				Elapsed:    time.Since(startedAt),
			})
		}

		switch decision {
		case RetryAbort:
			return zero, err
		case RetrySkip:
			// No sleep; immediately proceed to the next attempt.
			continue
		}

		// Determine sleep duration based on the decision + FastFirst.
		var sleepDuration time.Duration
		switch {
		case opts.FastFirst && attempt == 0:
			sleepDuration = 0
		case decision == RetryWithDelay:
			sleepDuration = customDelay
		default:
			sleepDuration = CalculateBackoff(attempt, opts)
		}

		if sleepDuration > 0 {
			select {
			case <-ctx.Done():
				return zero, ctx.Err()
			case <-time.After(sleepDuration):
			}
		} else if ctx.Err() != nil {
			return zero, ctx.Err()
		}
	}

	return zero, &RetryError{
		Cause:    lastErr,
		Attempts: totalAttempts,
	}
}
