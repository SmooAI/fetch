package fetch

import (
	"context"
	"errors"
	"testing"
	"time"
)

func TestCircuitBreaker_ClosedState(t *testing.T) {
	cb := NewCircuitBreaker("test-cb", &CircuitBreakerOptions{
		MaxRequests: 1,
		Timeout:     time.Second,
	})

	if cb.State() != CircuitBreakerStateClosed {
		t.Errorf("expected closed state, got %d", cb.State())
	}

	result, err := cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "success", nil
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "success" {
		t.Errorf("expected 'success', got %v", result)
	}
}

func TestCircuitBreaker_OpensAfterFailures(t *testing.T) {
	cb := NewCircuitBreaker("test-cb-open", &CircuitBreakerOptions{
		MaxRequests: 1,
		Timeout:     60 * time.Second,
		ReadyToTrip: func(counts CircuitBreakerCounts) bool {
			return counts.ConsecutiveFailures >= 3
		},
	})

	testErr := errors.New("test failure")

	// Fail 3 times to trip the breaker
	for i := 0; i < 3; i++ {
		cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
			return nil, testErr
		})
	}

	// Next request should get a CircuitBreakerError
	_, err := cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "should not execute", nil
	})
	if err == nil {
		t.Fatal("expected error from open circuit breaker")
	}
	cbErr, ok := err.(*CircuitBreakerError)
	if !ok {
		t.Fatalf("expected *CircuitBreakerError, got %T: %v", err, err)
	}
	if cbErr.State != CircuitBreakerStateOpen {
		t.Errorf("expected open state in error, got %d", cbErr.State)
	}
}

func TestCircuitBreaker_RecoversToHalfOpen(t *testing.T) {
	stateChanges := make([]CircuitBreakerState, 0)
	cb := NewCircuitBreaker("test-cb-recover", &CircuitBreakerOptions{
		MaxRequests: 1,
		Timeout:     100 * time.Millisecond, // short timeout for testing
		ReadyToTrip: func(counts CircuitBreakerCounts) bool {
			return counts.ConsecutiveFailures >= 2
		},
		OnStateChange: func(name string, from, to CircuitBreakerState) {
			stateChanges = append(stateChanges, to)
		},
	})

	testErr := errors.New("failure")

	// Trip the breaker
	for i := 0; i < 2; i++ {
		cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
			return nil, testErr
		})
	}

	// Verify open
	if cb.State() != CircuitBreakerStateOpen {
		t.Fatalf("expected open state, got %d", cb.State())
	}

	// Wait for timeout to transition to half-open
	time.Sleep(150 * time.Millisecond)

	// The next request should be allowed (half-open allows MaxRequests)
	result, err := cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "recovered", nil
	})
	if err != nil {
		t.Fatalf("unexpected error in half-open state: %v", err)
	}
	if result != "recovered" {
		t.Errorf("expected 'recovered', got %v", result)
	}

	// Should be back to closed
	if cb.State() != CircuitBreakerStateClosed {
		t.Errorf("expected closed state after recovery, got %d", cb.State())
	}
}

func TestCircuitBreaker_IsSuccessful(t *testing.T) {
	acceptableErr := errors.New("acceptable error")

	cb := NewCircuitBreaker("test-cb-success", &CircuitBreakerOptions{
		MaxRequests: 1,
		Timeout:     time.Second,
		ReadyToTrip: func(counts CircuitBreakerCounts) bool {
			return counts.ConsecutiveFailures >= 2
		},
		IsSuccessful: func(err error) bool {
			// Treat "acceptable error" as a success
			return err == acceptableErr
		},
	})

	// These should count as successes, not failures
	for i := 0; i < 5; i++ {
		_, err := cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
			return nil, acceptableErr
		})
		if err != nil && err != acceptableErr {
			t.Fatalf("unexpected error type: %v", err)
		}
	}

	// Circuit should still be closed because IsSuccessful returned true
	if cb.State() != CircuitBreakerStateClosed {
		t.Errorf("expected closed state, got %d", cb.State())
	}
}

func TestCircuitBreaker_NilOptions(t *testing.T) {
	// Should use default gobreaker settings (5 consecutive failures to trip)
	cb := NewCircuitBreaker("test-cb-nil", nil)
	if cb.State() != CircuitBreakerStateClosed {
		t.Errorf("expected closed state, got %d", cb.State())
	}

	result, err := cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return "ok", nil
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "ok" {
		t.Errorf("expected 'ok', got %v", result)
	}
}

func TestCircuitBreaker_PassesThroughErrors(t *testing.T) {
	cb := NewCircuitBreaker("test-cb-passthrough", &CircuitBreakerOptions{
		ReadyToTrip: func(counts CircuitBreakerCounts) bool {
			return false // never trip
		},
	})

	myErr := errors.New("my specific error")
	_, err := cb.Execute(context.Background(), func(ctx context.Context) (any, error) {
		return nil, myErr
	})
	if err != myErr {
		t.Errorf("expected exact error passthrough, got %v", err)
	}
}
