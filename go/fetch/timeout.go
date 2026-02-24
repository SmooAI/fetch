package fetch

import (
	"context"
	"time"
)

// ExecuteWithTimeout runs fn with a timeout derived from the given context.
// If the function does not complete within the timeout, a *TimeoutError is returned.
func ExecuteWithTimeout[T any](ctx context.Context, timeout time.Duration, fn func(ctx context.Context) (T, error)) (T, error) {
	ctx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()

	type result struct {
		val T
		err error
	}

	ch := make(chan result, 1)

	go func() {
		val, err := fn(ctx)
		ch <- result{val, err}
	}()

	select {
	case <-ctx.Done():
		var zero T
		if ctx.Err() == context.DeadlineExceeded {
			return zero, &TimeoutError{Timeout: timeout}
		}
		return zero, ctx.Err()
	case r := <-ch:
		return r.val, r.err
	}
}
