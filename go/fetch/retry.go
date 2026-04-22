package fetch

import (
	"context"
	"errors"
	"math"
	"math/rand"
	"time"
)

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
