package fetch

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"
)

// HTTPResponseError is returned when the server responds with a non-2xx status code.
type HTTPResponseError struct {
	// StatusCode is the HTTP status code.
	StatusCode int
	// Status is the HTTP status text (e.g. "404 Not Found").
	Status string
	// Body is the raw response body.
	Body []byte
	// Headers are the response headers.
	Headers map[string][]string
	// RetryAfter is the parsed Retry-After header value, if present.
	RetryAfter time.Duration
	// Message is a human-readable error string extracted from the response body.
	Message string
}

// Error implements the error interface.
func (e *HTTPResponseError) Error() string {
	return e.Message
}

// newHTTPResponseError creates an HTTPResponseError by parsing the response body for
// structured error information, mirroring the TypeScript implementation.
func newHTTPResponseError(statusCode int, status string, body []byte, headers map[string][]string, prefix string) *HTTPResponseError {
	retryAfter := parseRetryAfter(headers)

	errMsg := extractErrorMessage(body)
	if errMsg == "" {
		if len(body) > 0 {
			errMsg = string(body)
		} else {
			errMsg = "Unknown error"
		}
	}

	msg := fmt.Sprintf("%s%s; HTTP Error Response: %s", prefix, errMsg, status)

	return &HTTPResponseError{
		StatusCode: statusCode,
		Status:     status,
		Body:       body,
		Headers:    headers,
		RetryAfter: retryAfter,
		Message:    msg,
	}
}

// extractErrorMessage attempts to extract a human-readable error message from a JSON response body.
// It mirrors the TypeScript constructor logic for HTTPResponseError.
func extractErrorMessage(body []byte) string {
	if len(body) == 0 {
		return ""
	}

	var parsed map[string]any
	if err := json.Unmarshal(body, &parsed); err != nil {
		return ""
	}

	var parts []string
	errIsSet := false

	if errField, ok := parsed["error"]; ok {
		switch e := errField.(type) {
		case map[string]any:
			if t, ok := e["type"].(string); ok {
				parts = append(parts, fmt.Sprintf("(%s): ", t))
				errIsSet = true
			}
			if c, ok := e["code"]; ok {
				parts = append(parts, fmt.Sprintf("(%v): ", c))
				errIsSet = true
			}
			if m, ok := e["message"].(string); ok {
				parts = append(parts, m)
				errIsSet = true
			}
		case string:
			parts = append(parts, e)
			errIsSet = true
		}
	}

	if errMessages, ok := parsed["errorMessages"]; ok {
		if arr, ok := errMessages.([]any); ok {
			var msgs []string
			for _, m := range arr {
				if s, ok := m.(string); ok {
					msgs = append(msgs, s)
				}
			}
			if len(msgs) > 0 {
				parts = append(parts, strings.Join(msgs, "; "))
				errIsSet = true
			}
		}
	}

	if !errIsSet {
		return ""
	}

	return strings.Join(parts, "")
}

// parseRetryAfter extracts and parses the Retry-After header value.
func parseRetryAfter(headers map[string][]string) time.Duration {
	vals, ok := headers["Retry-After"]
	if !ok || len(vals) == 0 {
		return 0
	}
	var seconds int
	if _, err := fmt.Sscanf(vals[0], "%d", &seconds); err == nil && seconds > 0 {
		return time.Duration(seconds) * time.Second
	}
	return 0
}

// RetryError is returned when all retry attempts have been exhausted.
type RetryError struct {
	// Cause is the last error encountered.
	Cause error
	// Attempts is the total number of attempts made.
	Attempts int
}

// Error implements the error interface.
func (e *RetryError) Error() string {
	return fmt.Sprintf("Retry Error: Ran out of retry attempts (%d); %v", e.Attempts, e.Cause)
}

// Unwrap returns the underlying cause.
func (e *RetryError) Unwrap() error {
	return e.Cause
}

// RateLimitError is returned when a request is rejected by the rate limiter.
type RateLimitError struct {
	// RetryAfter is the recommended wait duration before retrying.
	RetryAfter time.Duration
}

// Error implements the error interface.
func (e *RateLimitError) Error() string {
	return fmt.Sprintf("Rate limit exceeded; retry after %v", e.RetryAfter)
}

// CircuitBreakerError is returned when the circuit breaker is open and rejects the request.
type CircuitBreakerError struct {
	// State is the current state of the circuit breaker.
	State CircuitBreakerState
}

// Error implements the error interface.
func (e *CircuitBreakerError) Error() string {
	return "circuit breaker is open"
}

// TimeoutError is returned when a request exceeds the configured timeout.
type TimeoutError struct {
	// Timeout is the configured timeout duration.
	Timeout time.Duration
}

// Error implements the error interface.
func (e *TimeoutError) Error() string {
	return fmt.Sprintf("request timed out after %v", e.Timeout)
}

// SchemaValidationError is returned when response body validation fails.
type SchemaValidationError struct {
	// Errors is the list of validation error messages.
	Errors []string
}

// Error implements the error interface.
func (e *SchemaValidationError) Error() string {
	return fmt.Sprintf("schema validation failed: %s", strings.Join(e.Errors, "; "))
}

// IsRetryable reports whether the given HTTP status code should trigger a retry.
// Status 429 (Too Many Requests) and 5xx (server errors) are retryable.
func IsRetryable(status int) bool {
	return status == 429 || status >= 500
}
