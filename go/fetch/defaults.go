package fetch

import "time"

// DefaultRetryOptions provides sensible retry defaults matching the TypeScript implementation.
// 2 retry attempts with jittered exponential backoff, retrying on 429 and 5xx responses.
var DefaultRetryOptions = RetryOptions{
	Attempts:        2,
	InitialInterval: 500 * time.Millisecond,
	Factor:          2.0,
	JitterFraction:  0.5,
	MaxInterval:     0, // no cap
	OnRejection: func(rc RetryContext) (RetryDecision, time.Duration) {
		switch e := rc.LastError.(type) {
		case *HTTPResponseError:
			if IsRetryable(e.StatusCode) {
				if ra := e.RetryAfter; ra > 0 {
					return RetryWithDelay, ra
				}
				return RetryDefault, 0
			}
			return RetryAbort, 0
		case *RateLimitError:
			return RetryWithDelay, e.RetryAfter
		case *TimeoutError:
			return RetryDefault, 0
		case *SchemaValidationError:
			return RetryAbort, 0
		default:
			return RetryDefault, 0
		}
	},
}

// DefaultRateLimitRetryOptions provides retry defaults for rate-limit rejections.
var DefaultRateLimitRetryOptions = RateLimitRetryOptions{
	Attempts:        1,
	InitialInterval: 500 * time.Millisecond,
	OnRejection: func(rc RetryContext) (RetryDecision, time.Duration) {
		if e, ok := rc.LastError.(*RateLimitError); ok {
			return RetryWithDelay, e.RetryAfter + 50*time.Millisecond
		}
		return RetryAbort, 0
	},
}

// DefaultTimeoutOptions provides the default request timeout (10 seconds).
var DefaultTimeoutOptions = TimeoutOptions{
	Timeout: 10 * time.Second,
}
