package fetch

import (
	"sync"
	"time"
)

// SlidingWindowRateLimiter implements a sliding-window rate limiter.
// It allows at most MaxRequests requests within the sliding window Period.
// If a request would exceed the limit, it returns a *RateLimitError with the
// remaining time until the oldest request in the window expires.
type SlidingWindowRateLimiter struct {
	maxRequests int
	period      time.Duration

	mu         sync.Mutex
	timestamps []time.Time
	nowFunc    func() time.Time // for testing
}

// NewSlidingWindowRateLimiter creates a new sliding-window rate limiter.
func NewSlidingWindowRateLimiter(maxRequests int, period time.Duration) *SlidingWindowRateLimiter {
	return &SlidingWindowRateLimiter{
		maxRequests: maxRequests,
		period:      period,
		timestamps:  make([]time.Time, 0, maxRequests),
		nowFunc:     time.Now,
	}
}

// Allow checks whether a new request is allowed. If allowed, it records the
// request timestamp and returns nil. If not allowed, it returns a *RateLimitError
// indicating how long to wait.
func (rl *SlidingWindowRateLimiter) Allow() error {
	rl.mu.Lock()
	defer rl.mu.Unlock()

	now := rl.nowFunc()
	windowStart := now.Add(-rl.period)

	// Remove timestamps outside the window
	validIdx := 0
	for _, ts := range rl.timestamps {
		if ts.After(windowStart) {
			rl.timestamps[validIdx] = ts
			validIdx++
		}
	}
	rl.timestamps = rl.timestamps[:validIdx]

	if len(rl.timestamps) >= rl.maxRequests {
		// The oldest timestamp determines when the next slot opens
		oldest := rl.timestamps[0]
		retryAfter := oldest.Add(rl.period).Sub(now)
		if retryAfter < 0 {
			retryAfter = 0
		}
		return &RateLimitError{RetryAfter: retryAfter}
	}

	rl.timestamps = append(rl.timestamps, now)
	return nil
}

// Reset clears all recorded timestamps, resetting the rate limiter.
func (rl *SlidingWindowRateLimiter) Reset() {
	rl.mu.Lock()
	defer rl.mu.Unlock()
	rl.timestamps = rl.timestamps[:0]
}
