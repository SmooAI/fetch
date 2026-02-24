package fetch

import (
	"testing"
	"time"
)

func TestRateLimiter_AllowsUpToMax(t *testing.T) {
	rl := NewSlidingWindowRateLimiter(3, time.Second)
	for i := 0; i < 3; i++ {
		if err := rl.Allow(); err != nil {
			t.Fatalf("request %d should be allowed: %v", i+1, err)
		}
	}
}

func TestRateLimiter_BlocksOverMax(t *testing.T) {
	rl := NewSlidingWindowRateLimiter(2, time.Second)
	if err := rl.Allow(); err != nil {
		t.Fatalf("request 1 should be allowed: %v", err)
	}
	if err := rl.Allow(); err != nil {
		t.Fatalf("request 2 should be allowed: %v", err)
	}
	err := rl.Allow()
	if err == nil {
		t.Fatal("request 3 should be rejected")
	}
	rateLimitErr, ok := err.(*RateLimitError)
	if !ok {
		t.Fatalf("expected *RateLimitError, got %T", err)
	}
	if rateLimitErr.RetryAfter <= 0 {
		t.Errorf("expected positive RetryAfter, got %v", rateLimitErr.RetryAfter)
	}
}

func TestRateLimiter_SlidingWindow(t *testing.T) {
	now := time.Now()
	rl := NewSlidingWindowRateLimiter(2, 200*time.Millisecond)
	rl.nowFunc = func() time.Time { return now }

	// Fill the window
	rl.Allow()
	rl.Allow()

	// Should be blocked
	err := rl.Allow()
	if err == nil {
		t.Fatal("should be blocked")
	}

	// Advance time past the window
	now = now.Add(250 * time.Millisecond)

	// Should be allowed again
	if err := rl.Allow(); err != nil {
		t.Fatalf("should be allowed after window expires: %v", err)
	}
}

func TestRateLimiter_SlidingWindowPartialExpiry(t *testing.T) {
	now := time.Now()
	rl := NewSlidingWindowRateLimiter(2, 200*time.Millisecond)
	rl.nowFunc = func() time.Time { return now }

	// First request at t=0
	rl.Allow()

	// Second request at t=100ms
	now = now.Add(100 * time.Millisecond)
	rl.Allow()

	// Third request at t=100ms should be blocked
	err := rl.Allow()
	if err == nil {
		t.Fatal("should be blocked")
	}

	// Advance to t=210ms (first request expired, second still active)
	now = now.Add(110 * time.Millisecond)

	// Should allow one more (first request expired)
	if err := rl.Allow(); err != nil {
		t.Fatalf("should be allowed after partial expiry: %v", err)
	}

	// Should block again (second request still in window, plus the new one)
	err = rl.Allow()
	if err == nil {
		t.Fatal("should be blocked again")
	}
}

func TestRateLimiter_Reset(t *testing.T) {
	rl := NewSlidingWindowRateLimiter(1, time.Second)
	rl.Allow()

	// Should be blocked
	if err := rl.Allow(); err == nil {
		t.Fatal("should be blocked")
	}

	// Reset
	rl.Reset()

	// Should be allowed after reset
	if err := rl.Allow(); err != nil {
		t.Fatalf("should be allowed after reset: %v", err)
	}
}

func TestRateLimiter_RetryAfterValue(t *testing.T) {
	now := time.Now()
	rl := NewSlidingWindowRateLimiter(1, 500*time.Millisecond)
	rl.nowFunc = func() time.Time { return now }

	rl.Allow()

	// Advance 200ms
	now = now.Add(200 * time.Millisecond)

	err := rl.Allow()
	if err == nil {
		t.Fatal("should be blocked")
	}
	rateLimitErr := err.(*RateLimitError)
	// RetryAfter should be approximately 300ms (500ms - 200ms elapsed)
	if rateLimitErr.RetryAfter < 250*time.Millisecond || rateLimitErr.RetryAfter > 350*time.Millisecond {
		t.Errorf("expected RetryAfter ~300ms, got %v", rateLimitErr.RetryAfter)
	}
}

func TestRateLimiter_ConcurrentAccess(t *testing.T) {
	rl := NewSlidingWindowRateLimiter(100, time.Second)
	done := make(chan bool, 200)

	for i := 0; i < 200; i++ {
		go func() {
			rl.Allow()
			done <- true
		}()
	}

	for i := 0; i < 200; i++ {
		<-done
	}
	// If we got here without a race condition panic, the test passes
}

func TestRateLimiter_SingleRequest(t *testing.T) {
	rl := NewSlidingWindowRateLimiter(1, 100*time.Millisecond)
	if err := rl.Allow(); err != nil {
		t.Fatalf("first request should be allowed: %v", err)
	}
	if err := rl.Allow(); err == nil {
		t.Fatal("second request should be blocked")
	}

	// Wait for window to expire
	time.Sleep(150 * time.Millisecond)

	if err := rl.Allow(); err != nil {
		t.Fatalf("request after window should be allowed: %v", err)
	}
}
