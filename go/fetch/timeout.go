package fetch

import (
	"context"
	"time"
)

// attemptUnwindGrace bounds how long a timed-out attempt is given to unwind
// after its context is cancelled. net/http closes the connection synchronously
// on cancellation (microseconds), so this is only ever reached by caller code
// that ignores its context — e.g. a hook or auth provider that blocks — and it
// keeps such code from stretching the timeout indefinitely.
const attemptUnwindGrace = time.Second

// ExecuteWithTimeout runs fn with a timeout derived from the given context.
// If the function does not complete within the timeout, a *TimeoutError is returned.
//
// The timeout CANCELS the attempt rather than abandoning it: fn's context is
// cancelled, which makes net/http close the connection so the server can see
// the request go away, and ExecuteWithTimeout waits (up to attemptUnwindGrace)
// for fn to return before reporting the timeout. A retry that follows therefore
// never overlaps the attempt it replaces.
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
		cancel()
		select {
		case <-ch:
		case <-time.After(attemptUnwindGrace):
		}
		var zero T
		if ctx.Err() == context.DeadlineExceeded {
			return zero, &TimeoutError{Timeout: timeout}
		}
		return zero, ctx.Err()
	case r := <-ch:
		return r.val, r.err
	}
}
