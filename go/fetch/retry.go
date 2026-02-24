package fetch

import (
	"context"
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

// ExecuteWithRetry executes fn with retry logic according to opts.
// It calls fn up to 1 + opts.Attempts times (1 initial + N retries).
// Between attempts it sleeps for either the duration returned by OnRejection
// or the computed backoff. The context can cancel the retry loop.
func ExecuteWithRetry[T any](ctx context.Context, opts RetryOptions, fn func(ctx context.Context) (T, error)) (T, error) {
	totalAttempts := 1 + opts.Attempts
	var lastErr error
	var zero T

	for attempt := 0; attempt < totalAttempts; attempt++ {
		result, err := fn(ctx)
		if err == nil {
			return result, nil
		}
		lastErr = err

		// If this is the last attempt, don't bother checking retry logic
		if attempt >= totalAttempts-1 {
			break
		}

		shouldRetry := true
		var retryAfter time.Duration

		if opts.OnRejection != nil {
			shouldRetry, retryAfter = opts.OnRejection(err, attempt+1)
		}

		if !shouldRetry {
			return zero, err
		}

		// Determine sleep duration
		sleepDuration := retryAfter
		if sleepDuration <= 0 {
			sleepDuration = CalculateBackoff(attempt, opts)
		}

		select {
		case <-ctx.Done():
			return zero, ctx.Err()
		case <-time.After(sleepDuration):
		}
	}

	return zero, &RetryError{
		Cause:    lastErr,
		Attempts: totalAttempts,
	}
}
