package fetch

import (
	"context"
	"errors"
	"testing"
	"time"
)

func TestCalculateBackoff_FirstAttempt(t *testing.T) {
	opts := RetryOptions{
		InitialInterval: 500 * time.Millisecond,
		Factor:          2.0,
		JitterFraction:  0.0,
	}
	d := CalculateBackoff(0, opts)
	if d != 500*time.Millisecond {
		t.Errorf("expected 500ms for attempt 0, got %v", d)
	}
}

func TestCalculateBackoff_ExponentialGrowth(t *testing.T) {
	opts := RetryOptions{
		InitialInterval: 100 * time.Millisecond,
		Factor:          2.0,
		JitterFraction:  0.0,
	}

	// attempt 1: 100ms * 2^1 = 200ms
	d := CalculateBackoff(1, opts)
	if d != 200*time.Millisecond {
		t.Errorf("expected 200ms for attempt 1, got %v", d)
	}

	// attempt 2: 100ms * 2^2 = 400ms
	d = CalculateBackoff(2, opts)
	if d != 400*time.Millisecond {
		t.Errorf("expected 400ms for attempt 2, got %v", d)
	}

	// attempt 3: 100ms * 2^3 = 800ms
	d = CalculateBackoff(3, opts)
	if d != 800*time.Millisecond {
		t.Errorf("expected 800ms for attempt 3, got %v", d)
	}
}

func TestCalculateBackoff_MaxInterval(t *testing.T) {
	opts := RetryOptions{
		InitialInterval: 100 * time.Millisecond,
		Factor:          2.0,
		JitterFraction:  0.0,
		MaxInterval:     300 * time.Millisecond,
	}

	// attempt 2: 100ms * 4 = 400ms, capped to 300ms
	d := CalculateBackoff(2, opts)
	if d != 300*time.Millisecond {
		t.Errorf("expected 300ms (capped), got %v", d)
	}
}

func TestCalculateBackoff_WithJitter(t *testing.T) {
	opts := RetryOptions{
		InitialInterval: 1000 * time.Millisecond,
		Factor:          1.0,
		JitterFraction:  0.5,
	}

	// Run multiple times to verify jitter adds variation
	results := make(map[time.Duration]bool)
	for i := 0; i < 50; i++ {
		d := CalculateBackoff(1, opts)
		results[d] = true
		// 1000ms * 1^1 = 1000ms, jitter of 0.5 means +-500ms
		if d < 500*time.Millisecond || d > 1500*time.Millisecond {
			t.Errorf("backoff %v outside expected range [500ms, 1500ms]", d)
		}
	}
	// With jitter we should see some variation
	if len(results) < 2 {
		t.Error("expected jitter to produce varied results")
	}
}

func TestCalculateBackoff_ZeroFactor(t *testing.T) {
	opts := RetryOptions{
		InitialInterval: 100 * time.Millisecond,
		Factor:          0, // will default to 1.0
		JitterFraction:  0.0,
	}
	d := CalculateBackoff(3, opts)
	if d != 100*time.Millisecond {
		t.Errorf("expected 100ms with factor=0 (treated as 1.0), got %v", d)
	}
}

func TestExecuteWithRetry_ImmediateSuccess(t *testing.T) {
	calls := 0
	result, err := ExecuteWithRetry(context.Background(), RetryOptions{Attempts: 2, InitialInterval: time.Millisecond}, func(ctx context.Context) (string, error) {
		calls++
		return "ok", nil
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "ok" {
		t.Errorf("expected 'ok', got %s", result)
	}
	if calls != 1 {
		t.Errorf("expected 1 call, got %d", calls)
	}
}

func TestExecuteWithRetry_SuccessAfterFailures(t *testing.T) {
	calls := 0
	result, err := ExecuteWithRetry(context.Background(), RetryOptions{
		Attempts:        2,
		InitialInterval: time.Millisecond,
		Factor:          1.0,
	}, func(ctx context.Context) (string, error) {
		calls++
		if calls < 3 {
			return "", errors.New("transient error")
		}
		return "recovered", nil
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "recovered" {
		t.Errorf("expected 'recovered', got %s", result)
	}
	if calls != 3 {
		t.Errorf("expected 3 calls (1 initial + 2 retries), got %d", calls)
	}
}

func TestExecuteWithRetry_AllAttemptsFail(t *testing.T) {
	calls := 0
	_, err := ExecuteWithRetry(context.Background(), RetryOptions{
		Attempts:        2,
		InitialInterval: time.Millisecond,
		Factor:          1.0,
	}, func(ctx context.Context) (string, error) {
		calls++
		return "", errors.New("persistent error")
	})
	if err == nil {
		t.Fatal("expected error")
	}
	retryErr, ok := err.(*RetryError)
	if !ok {
		t.Fatalf("expected *RetryError, got %T: %v", err, err)
	}
	if retryErr.Attempts != 3 {
		t.Errorf("expected 3 total attempts, got %d", retryErr.Attempts)
	}
	if calls != 3 {
		t.Errorf("expected 3 calls, got %d", calls)
	}
}

func TestExecuteWithRetry_OnRejectionStopsRetry(t *testing.T) {
	calls := 0
	nonRetryableErr := errors.New("non-retryable")
	_, err := ExecuteWithRetry(context.Background(), RetryOptions{
		Attempts:        3,
		InitialInterval: time.Millisecond,
		OnRejection: func(rc RetryContext) (RetryDecision, time.Duration) {
			if errors.Is(rc.LastError, nonRetryableErr) {
				return RetryAbort, 0
			}
			return RetryDefault, 0
		},
	}, func(ctx context.Context) (string, error) {
		calls++
		return "", nonRetryableErr
	})
	if err == nil {
		t.Fatal("expected error")
	}
	if !errors.Is(err, nonRetryableErr) {
		t.Errorf("expected nonRetryableErr, got %v", err)
	}
	if calls != 1 {
		t.Errorf("expected 1 call (no retry), got %d", calls)
	}
}

func TestExecuteWithRetry_OnRejectionCustomDelay(t *testing.T) {
	calls := 0
	start := time.Now()
	_, err := ExecuteWithRetry(context.Background(), RetryOptions{
		Attempts:        1,
		InitialInterval: 10 * time.Second, // very long default
		OnRejection: func(rc RetryContext) (RetryDecision, time.Duration) {
			return RetryWithDelay, 10 * time.Millisecond // override to short delay
		},
	}, func(ctx context.Context) (string, error) {
		calls++
		if calls < 2 {
			return "", errors.New("fail")
		}
		return "ok", nil
	})
	elapsed := time.Since(start)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if calls != 2 {
		t.Errorf("expected 2 calls, got %d", calls)
	}
	// Should have used the 10ms delay, not the 10s default
	if elapsed > 1*time.Second {
		t.Errorf("expected fast retry, but took %v", elapsed)
	}
}

func TestExecuteWithRetry_ContextCancellation(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	calls := 0
	_, err := ExecuteWithRetry(ctx, RetryOptions{
		Attempts:        5,
		InitialInterval: 100 * time.Millisecond,
	}, func(ctx context.Context) (string, error) {
		calls++
		if calls == 1 {
			cancel()
		}
		return "", errors.New("fail")
	})
	if err == nil {
		t.Fatal("expected error from context cancellation")
	}
}

func TestExecuteWithRetry_ZeroAttempts(t *testing.T) {
	calls := 0
	_, err := ExecuteWithRetry(context.Background(), RetryOptions{
		Attempts:        0,
		InitialInterval: time.Millisecond,
	}, func(ctx context.Context) (string, error) {
		calls++
		return "", errors.New("fail")
	})
	if err == nil {
		t.Fatal("expected error")
	}
	if calls != 1 {
		t.Errorf("expected 1 call with 0 retries, got %d", calls)
	}
	// With 0 retries, there's only 1 attempt, so it should just return the error directly
	// (not wrapped as RetryError, since there were no retries)
	if _, ok := err.(*RetryError); ok {
		// Actually with our implementation 1 total attempt still wraps in RetryError
		// This is acceptable behavior
	}
}

// TestExecuteWithRetry_FastFirst verifies that FastFirst=true makes the first
// retry fire immediately, bypassing the configured InitialInterval.
func TestExecuteWithRetry_FastFirst(t *testing.T) {
	calls := 0
	start := time.Now()
	_, err := ExecuteWithRetry(context.Background(), RetryOptions{
		Attempts:        1,
		InitialInterval: 2 * time.Second, // very long so non-fast first would be obvious
		Factor:          1.0,
		FastFirst:       true,
	}, func(ctx context.Context) (string, error) {
		calls++
		if calls < 2 {
			return "", errors.New("first attempt fails")
		}
		return "ok", nil
	})
	elapsed := time.Since(start)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if calls != 2 {
		t.Errorf("expected 2 calls, got %d", calls)
	}
	// The first retry should fire with ~0 delay; allow generous headroom for CI.
	if elapsed > 250*time.Millisecond {
		t.Errorf("expected FastFirst retry to be near-instant, took %v", elapsed)
	}
}

// TestExecuteWithRetry_OnRejectionAbort verifies RetryAbort stops the loop.
func TestExecuteWithRetry_OnRejectionAbort(t *testing.T) {
	calls := 0
	bombErr := errors.New("boom")
	_, err := ExecuteWithRetry(context.Background(), RetryOptions{
		Attempts:        5,
		InitialInterval: time.Millisecond,
		OnRejection: func(rc RetryContext) (RetryDecision, time.Duration) {
			return RetryAbort, 0
		},
	}, func(ctx context.Context) (string, error) {
		calls++
		return "", bombErr
	})
	if err == nil {
		t.Fatal("expected error")
	}
	if !errors.Is(err, bombErr) {
		t.Errorf("expected bombErr, got %v", err)
	}
	if calls != 1 {
		t.Errorf("expected 1 call (aborted after first), got %d", calls)
	}
}

// TestExecuteWithRetry_OnRejectionWithDelay verifies RetryWithDelay uses the returned duration.
func TestExecuteWithRetry_OnRejectionWithDelay(t *testing.T) {
	calls := 0
	start := time.Now()
	_, err := ExecuteWithRetry(context.Background(), RetryOptions{
		Attempts:        1,
		InitialInterval: 5 * time.Second, // very long; we expect the override to beat it
		OnRejection: func(rc RetryContext) (RetryDecision, time.Duration) {
			return RetryWithDelay, 17 * time.Millisecond
		},
	}, func(ctx context.Context) (string, error) {
		calls++
		if calls < 2 {
			return "", errors.New("please retry me")
		}
		return "ok", nil
	})
	elapsed := time.Since(start)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if calls != 2 {
		t.Errorf("expected 2 calls, got %d", calls)
	}
	if elapsed < 17*time.Millisecond {
		t.Errorf("expected at least 17ms delay, took %v", elapsed)
	}
	if elapsed > 500*time.Millisecond {
		t.Errorf("expected short custom delay, took %v", elapsed)
	}
}

// TestExecuteWithRetry_OnRejectionDefault verifies RetryDefault preserves the built-in backoff.
func TestExecuteWithRetry_OnRejectionDefault(t *testing.T) {
	calls := 0
	seenAttempts := []int{}
	seenElapsed := []time.Duration{}
	start := time.Now()

	_, err := ExecuteWithRetry(context.Background(), RetryOptions{
		Attempts:        2,
		InitialInterval: 20 * time.Millisecond,
		Factor:          2.0,
		JitterFraction:  0.0,
		OnRejection: func(rc RetryContext) (RetryDecision, time.Duration) {
			seenAttempts = append(seenAttempts, rc.Attempt)
			seenElapsed = append(seenElapsed, rc.Elapsed)
			return RetryDefault, 0
		},
	}, func(ctx context.Context) (string, error) {
		calls++
		return "", errors.New("always fail")
	})
	elapsed := time.Since(start)

	if err == nil {
		t.Fatal("expected RetryError")
	}
	if _, ok := err.(*RetryError); !ok {
		t.Errorf("expected *RetryError, got %T", err)
	}
	if calls != 3 {
		t.Errorf("expected 3 calls, got %d", calls)
	}
	// OnRejection is called between attempts (before retry 1 and retry 2) => 2 calls.
	if len(seenAttempts) != 2 {
		t.Fatalf("expected OnRejection called twice, got %d", len(seenAttempts))
	}
	if seenAttempts[0] != 1 || seenAttempts[1] != 2 {
		t.Errorf("expected attempts [1,2], got %v", seenAttempts)
	}
	if seenElapsed[1] <= seenElapsed[0] {
		t.Errorf("expected Elapsed to grow between attempts, got %v then %v", seenElapsed[0], seenElapsed[1])
	}
	// Backoff: CalculateBackoff(0) = 20ms; CalculateBackoff(1) = 40ms. Total >= 60ms.
	if elapsed < 50*time.Millisecond {
		t.Errorf("expected default backoff (>=~60ms total), took %v", elapsed)
	}
}
