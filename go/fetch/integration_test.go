package fetch

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync/atomic"
	"testing"
	"time"
)

func TestIntegration_RetryOnServerError(t *testing.T) {
	var attempts int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		n := atomic.AddInt32(&attempts, 1)
		if n < 3 {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(500)
			json.NewEncoder(w).Encode(map[string]any{"error": "server error"})
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "1", Name: "recovered"})
	}))
	defer server.Close()

	client := NewClientBuilder().
		WithRetry(&RetryOptions{
			Attempts:        3,
			InitialInterval: 10 * time.Millisecond,
			Factor:          1.0,
			OnRejection: func(err error, attempt int) (bool, time.Duration) {
				if httpErr, ok := err.(*HTTPResponseError); ok {
					return IsRetryable(httpErr.StatusCode), 0
				}
				return true, 0
			},
		}).
		WithNoTimeout().
		Build()

	resp, err := Get[testJSON](context.Background(), client, server.URL, nil)
	if err != nil {
		t.Fatalf("unexpected error after retries: %v", err)
	}
	if resp.Data.Name != "recovered" {
		t.Errorf("expected 'recovered', got %s", resp.Data.Name)
	}
	if atomic.LoadInt32(&attempts) != 3 {
		t.Errorf("expected 3 attempts, got %d", atomic.LoadInt32(&attempts))
	}
}

func TestIntegration_RetryExhausted(t *testing.T) {
	var attempts int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		atomic.AddInt32(&attempts, 1)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(500)
		json.NewEncoder(w).Encode(map[string]any{"error": "persistent failure"})
	}))
	defer server.Close()

	client := NewClientBuilder().
		WithRetry(&RetryOptions{
			Attempts:        2,
			InitialInterval: 10 * time.Millisecond,
			Factor:          1.0,
			OnRejection: func(err error, attempt int) (bool, time.Duration) {
				if httpErr, ok := err.(*HTTPResponseError); ok {
					return IsRetryable(httpErr.StatusCode), 0
				}
				return false, 0
			},
		}).
		WithNoTimeout().
		Build()

	_, err := Get[any](context.Background(), client, server.URL, nil)
	if err == nil {
		t.Fatal("expected error after retry exhaustion")
	}
	retryErr, ok := err.(*RetryError)
	if !ok {
		t.Fatalf("expected *RetryError, got %T: %v", err, err)
	}
	if retryErr.Attempts != 3 {
		t.Errorf("expected 3 total attempts, got %d", retryErr.Attempts)
	}
	if atomic.LoadInt32(&attempts) != 3 {
		t.Errorf("expected 3 server hits, got %d", atomic.LoadInt32(&attempts))
	}
}

func TestIntegration_RetryWithRetryAfterHeader(t *testing.T) {
	var attempts int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		n := atomic.AddInt32(&attempts, 1)
		if n == 1 {
			w.Header().Set("Content-Type", "application/json")
			w.Header().Set("Retry-After", "1") // 1 second
			w.WriteHeader(429)
			json.NewEncoder(w).Encode(map[string]any{"error": "rate limited"})
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "1", Name: "ok"})
	}))
	defer server.Close()

	start := time.Now()
	client := NewClientBuilder().
		WithRetry(&DefaultRetryOptions).
		WithNoTimeout().
		Build()

	resp, err := Get[testJSON](context.Background(), client, server.URL, nil)
	elapsed := time.Since(start)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if resp.Data.Name != "ok" {
		t.Errorf("expected 'ok', got %s", resp.Data.Name)
	}
	// Should have waited at least 1 second due to Retry-After
	if elapsed < 900*time.Millisecond {
		t.Errorf("expected at least ~1s delay for Retry-After, got %v", elapsed)
	}
}

func TestIntegration_TimeoutWithRetry(t *testing.T) {
	var attempts int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		n := atomic.AddInt32(&attempts, 1)
		if n == 1 {
			// First request: slow (will timeout)
			time.Sleep(300 * time.Millisecond)
			w.Header().Set("Content-Type", "application/json")
			json.NewEncoder(w).Encode(testJSON{ID: "1", Name: "slow"})
			return
		}
		// Subsequent requests: fast
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "1", Name: "fast"})
	}))
	defer server.Close()

	client := NewClientBuilder().
		WithTimeout(100 * time.Millisecond).
		WithRetry(&RetryOptions{
			Attempts:        2,
			InitialInterval: 10 * time.Millisecond,
			Factor:          1.0,
		}).
		Build()

	resp, err := Get[testJSON](context.Background(), client, server.URL, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if resp.Data.Name != "fast" {
		t.Errorf("expected 'fast', got %s", resp.Data.Name)
	}
}

func TestIntegration_RateLimitWithRetry(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "1", Name: "ok"})
	}))
	defer server.Close()

	client := NewClientBuilder().
		WithRateLimit(2, 300*time.Millisecond).
		WithNoRetry().
		WithNoTimeout().
		Build()

	// Make 2 requests (should succeed)
	for i := 0; i < 2; i++ {
		resp, err := Get[testJSON](context.Background(), client, server.URL, nil)
		if err != nil {
			t.Fatalf("request %d: unexpected error: %v", i+1, err)
		}
		if !resp.OK {
			t.Errorf("request %d: expected OK", i+1)
		}
	}

	// Third request should be rate limited
	_, err := Get[testJSON](context.Background(), client, server.URL, nil)
	if err == nil {
		t.Fatal("expected rate limit error")
	}
	_, ok := err.(*RateLimitError)
	if !ok {
		t.Fatalf("expected *RateLimitError, got %T: %v", err, err)
	}

	// Wait for window to expire and try again
	time.Sleep(350 * time.Millisecond)
	resp, err := Get[testJSON](context.Background(), client, server.URL, nil)
	if err != nil {
		t.Fatalf("unexpected error after rate limit window: %v", err)
	}
	if !resp.OK {
		t.Error("expected OK after rate limit window expired")
	}
}

func TestIntegration_CircuitBreakerWithRetry(t *testing.T) {
	var attempts int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		atomic.AddInt32(&attempts, 1)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(500)
		json.NewEncoder(w).Encode(map[string]any{"error": "server error"})
	}))
	defer server.Close()

	client := NewClientBuilder().
		WithCircuitBreaker("integration-cb", &CircuitBreakerOptions{
			MaxRequests: 1,
			Timeout:     5 * time.Second,
			ReadyToTrip: func(counts CircuitBreakerCounts) bool {
				return counts.ConsecutiveFailures >= 3
			},
		}).
		WithNoRetry().
		WithNoTimeout().
		Build()

	// Make requests until circuit opens
	for i := 0; i < 3; i++ {
		_, err := Get[any](context.Background(), client, server.URL, nil)
		if err == nil {
			t.Fatalf("request %d: expected error", i+1)
		}
	}

	// Next request should get circuit breaker error (not an HTTP error)
	_, err := Get[any](context.Background(), client, server.URL, nil)
	if err == nil {
		t.Fatal("expected circuit breaker error")
	}
	_, ok := err.(*CircuitBreakerError)
	if !ok {
		t.Fatalf("expected *CircuitBreakerError, got %T: %v", err, err)
	}
}

func TestIntegration_NonRetryableError(t *testing.T) {
	var attempts int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		atomic.AddInt32(&attempts, 1)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(400) // Not retryable
		json.NewEncoder(w).Encode(map[string]any{"error": "bad request"})
	}))
	defer server.Close()

	client := NewClientBuilder().
		WithRetry(&RetryOptions{
			Attempts:        3,
			InitialInterval: 10 * time.Millisecond,
			OnRejection: func(err error, attempt int) (bool, time.Duration) {
				if httpErr, ok := err.(*HTTPResponseError); ok {
					return IsRetryable(httpErr.StatusCode), 0
				}
				return false, 0
			},
		}).
		WithNoTimeout().
		Build()

	_, err := Get[any](context.Background(), client, server.URL, nil)
	if err == nil {
		t.Fatal("expected error")
	}
	// Should not retry 400 errors
	if atomic.LoadInt32(&attempts) != 1 {
		t.Errorf("expected 1 attempt (no retry for 400), got %d", atomic.LoadInt32(&attempts))
	}
}

func TestIntegration_POSTWithJSONBody(t *testing.T) {
	type CreateRequest struct {
		Name  string `json:"name"`
		Email string `json:"email"`
	}
	type CreateResponse struct {
		ID    string `json:"id"`
		Name  string `json:"name"`
		Email string `json:"email"`
	}

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			t.Errorf("expected POST, got %s", r.Method)
		}
		var req CreateRequest
		json.NewDecoder(r.Body).Decode(&req)
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(CreateResponse{
			ID:    "new-id",
			Name:  req.Name,
			Email: req.Email,
		})
	}))
	defer server.Close()

	client := NewClientBuilder().WithNoRetry().WithNoTimeout().Build()
	resp, err := Post[CreateResponse](context.Background(), client, server.URL, CreateRequest{
		Name:  "Test User",
		Email: "test@example.com",
	}, &RequestOptions{
		Headers: http.Header{"Content-Type": {"application/json"}},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if resp.Data.ID != "new-id" {
		t.Errorf("expected ID 'new-id', got %s", resp.Data.ID)
	}
	if resp.Data.Email != "test@example.com" {
		t.Errorf("expected email 'test@example.com', got %s", resp.Data.Email)
	}
}

func TestIntegration_HooksWithRetry(t *testing.T) {
	var attempts int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		n := atomic.AddInt32(&attempts, 1)
		if n < 2 {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(500)
			json.NewEncoder(w).Encode(map[string]any{"error": "server error"})
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "1", Name: "success"})
	}))
	defer server.Close()

	preRequestCallCount := 0
	client := NewClientBuilder().
		WithRetry(&RetryOptions{
			Attempts:        2,
			InitialInterval: 10 * time.Millisecond,
			Factor:          1.0,
		}).
		WithNoTimeout().
		WithHooks(&LifecycleHooks{
			PreRequest: func(url string, req *http.Request) (string, *http.Request) {
				preRequestCallCount++
				req.Header.Set("X-Attempt", strings.Repeat("*", preRequestCallCount))
				return url, req
			},
		}).
		Build()

	resp, err := Get[testJSON](context.Background(), client, server.URL, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if resp.Data.Name != "success" {
		t.Errorf("expected 'success', got %s", resp.Data.Name)
	}
	// Pre-request hook should be called for each attempt
	if preRequestCallCount != 2 {
		t.Errorf("expected 2 pre-request hook calls, got %d", preRequestCallCount)
	}
}

func TestIntegration_FullPipeline(t *testing.T) {
	var attempts int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		n := atomic.AddInt32(&attempts, 1)
		if n == 1 {
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(503)
			json.NewEncoder(w).Encode(map[string]any{"error": "service unavailable"})
			return
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "1", Name: "full-pipeline"})
	}))
	defer server.Close()

	client := NewClientBuilder().
		WithTimeout(5 * time.Second).
		WithRetry(&RetryOptions{
			Attempts:        2,
			InitialInterval: 10 * time.Millisecond,
			Factor:          1.0,
		}).
		WithRateLimit(10, time.Second).
		WithBaseHeaders(http.Header{
			"X-Client": {"smooai-fetch-go"},
		}).
		WithHooks(&LifecycleHooks{
			PreRequest: func(url string, req *http.Request) (string, *http.Request) {
				req.Header.Set("X-Request-Time", time.Now().Format(time.RFC3339))
				return url, req
			},
			PostResponseSuccess: func(url string, req *http.Request, resp *FetchResponse[any]) *FetchResponse[any] {
				return resp
			},
		}).
		Build()

	resp, err := Get[testJSON](context.Background(), client, server.URL, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !resp.OK {
		t.Error("expected OK response")
	}
	if resp.Data.Name != "full-pipeline" {
		t.Errorf("expected 'full-pipeline', got %s", resp.Data.Name)
	}
	if atomic.LoadInt32(&attempts) != 2 {
		t.Errorf("expected 2 attempts, got %d", atomic.LoadInt32(&attempts))
	}
}

func TestIntegration_DefaultRetryOptions(t *testing.T) {
	// Verify default retry options work correctly with the error types
	opts := DefaultRetryOptions

	// HTTPResponseError with 429 should retry
	httpErr := &HTTPResponseError{StatusCode: 429}
	shouldRetry, _ := opts.OnRejection(httpErr, 1)
	if !shouldRetry {
		t.Error("429 should be retryable")
	}

	// HTTPResponseError with 500 should retry
	httpErr = &HTTPResponseError{StatusCode: 500}
	shouldRetry, _ = opts.OnRejection(httpErr, 1)
	if !shouldRetry {
		t.Error("500 should be retryable")
	}

	// HTTPResponseError with 400 should NOT retry
	httpErr = &HTTPResponseError{StatusCode: 400}
	shouldRetry, _ = opts.OnRejection(httpErr, 1)
	if shouldRetry {
		t.Error("400 should not be retryable")
	}

	// HTTPResponseError with Retry-After header should return duration
	httpErr = &HTTPResponseError{StatusCode: 429, RetryAfter: 2 * time.Second}
	shouldRetry, retryAfter := opts.OnRejection(httpErr, 1)
	if !shouldRetry {
		t.Error("429 with Retry-After should be retryable")
	}
	if retryAfter != 2*time.Second {
		t.Errorf("expected 2s retry-after, got %v", retryAfter)
	}

	// TimeoutError should retry
	timeoutErr := &TimeoutError{Timeout: time.Second}
	shouldRetry, _ = opts.OnRejection(timeoutErr, 1)
	if !shouldRetry {
		t.Error("TimeoutError should be retryable")
	}

	// RateLimitError should retry
	rateLimitErr := &RateLimitError{RetryAfter: 500 * time.Millisecond}
	shouldRetry, retryAfter = opts.OnRejection(rateLimitErr, 1)
	if !shouldRetry {
		t.Error("RateLimitError should be retryable")
	}
	if retryAfter != 500*time.Millisecond {
		t.Errorf("expected 500ms retry-after, got %v", retryAfter)
	}

	// SchemaValidationError should NOT retry
	schemaErr := &SchemaValidationError{Errors: []string{"invalid"}}
	shouldRetry, _ = opts.OnRejection(schemaErr, 1)
	if shouldRetry {
		t.Error("SchemaValidationError should not be retryable")
	}
}

func TestIntegration_DefaultRateLimitRetryOptions(t *testing.T) {
	opts := DefaultRateLimitRetryOptions

	// RateLimitError should retry with adjusted duration
	rateLimitErr := &RateLimitError{RetryAfter: 200 * time.Millisecond}
	shouldRetry, retryAfter := opts.OnRejection(rateLimitErr, 1)
	if !shouldRetry {
		t.Error("RateLimitError should be retryable")
	}
	if retryAfter != 250*time.Millisecond {
		t.Errorf("expected 250ms (200ms + 50ms), got %v", retryAfter)
	}

	// Non-RateLimitError should NOT retry
	httpErr := &HTTPResponseError{StatusCode: 500}
	shouldRetry, _ = opts.OnRejection(httpErr, 1)
	if shouldRetry {
		t.Error("non-RateLimitError should not be retryable with rate limit retry options")
	}
}
