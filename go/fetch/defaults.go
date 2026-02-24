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
	OnRejection: func(err error, attempt int) (bool, time.Duration) {
		switch e := err.(type) {
		case *HTTPResponseError:
			if IsRetryable(e.StatusCode) {
				if ra := e.RetryAfter; ra > 0 {
					return true, ra
				}
				return true, 0
			}
			return false, 0
		case *RateLimitError:
			return true, e.RetryAfter
		case *TimeoutError:
			return true, 0
		case *SchemaValidationError:
			return false, 0
		default:
			return true, 0
		}
	},
}

// DefaultRateLimitRetryOptions provides retry defaults for rate-limit rejections.
var DefaultRateLimitRetryOptions = RetryOptions{
	Attempts:        1,
	InitialInterval: 500 * time.Millisecond,
	OnRejection: func(err error, attempt int) (bool, time.Duration) {
		if e, ok := err.(*RateLimitError); ok {
			return true, e.RetryAfter + 50*time.Millisecond
		}
		return false, 0
	},
}

// DefaultTimeoutOptions provides the default request timeout (10 seconds).
var DefaultTimeoutOptions = TimeoutOptions{
	Timeout: 10 * time.Second,
}
