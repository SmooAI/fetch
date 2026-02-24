package fetch

import (
	"context"

	"github.com/sony/gobreaker/v2"
)

// CircuitBreaker wraps sony/gobreaker to provide circuit-breaker functionality.
type CircuitBreaker struct {
	cb *gobreaker.CircuitBreaker[any]
}

// NewCircuitBreaker creates a new CircuitBreaker with the given options.
func NewCircuitBreaker(name string, opts *CircuitBreakerOptions) *CircuitBreaker {
	settings := gobreaker.Settings{
		Name: name,
	}

	if opts != nil {
		settings.MaxRequests = opts.MaxRequests
		settings.Interval = opts.Interval
		settings.Timeout = opts.Timeout

		if opts.ReadyToTrip != nil {
			settings.ReadyToTrip = func(counts gobreaker.Counts) bool {
				return opts.ReadyToTrip(CircuitBreakerCounts{
					Requests:             counts.Requests,
					TotalSuccesses:       counts.TotalSuccesses,
					TotalFailures:        counts.TotalFailures,
					ConsecutiveSuccesses: counts.ConsecutiveSuccesses,
					ConsecutiveFailures:  counts.ConsecutiveFailures,
				})
			}
		}

		if opts.OnStateChange != nil {
			settings.OnStateChange = func(name string, from, to gobreaker.State) {
				opts.OnStateChange(name, toCircuitBreakerState(from), toCircuitBreakerState(to))
			}
		}

		if opts.IsSuccessful != nil {
			settings.IsSuccessful = opts.IsSuccessful
		}
	}

	return &CircuitBreaker{
		cb: gobreaker.NewCircuitBreaker[any](settings),
	}
}

// Execute runs the given function through the circuit breaker.
// If the circuit breaker is open, a *CircuitBreakerError is returned.
func (cb *CircuitBreaker) Execute(ctx context.Context, fn func(ctx context.Context) (any, error)) (any, error) {
	result, err := cb.cb.Execute(func() (any, error) {
		return fn(ctx)
	})
	if err != nil {
		if err == gobreaker.ErrOpenState || err == gobreaker.ErrTooManyRequests {
			return nil, &CircuitBreakerError{State: CircuitBreakerStateOpen}
		}
		return nil, err
	}
	return result, nil
}

// State returns the current state of the circuit breaker.
func (cb *CircuitBreaker) State() CircuitBreakerState {
	return toCircuitBreakerState(cb.cb.State())
}

// toCircuitBreakerState converts gobreaker.State to our CircuitBreakerState.
func toCircuitBreakerState(s gobreaker.State) CircuitBreakerState {
	switch s {
	case gobreaker.StateOpen:
		return CircuitBreakerStateOpen
	case gobreaker.StateHalfOpen:
		return CircuitBreakerStateHalfOpen
	default:
		return CircuitBreakerStateClosed
	}
}
