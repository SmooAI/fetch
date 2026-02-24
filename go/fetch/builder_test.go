package fetch

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"
)

func TestClientBuilder_Defaults(t *testing.T) {
	client := NewClientBuilder().Build()
	if client.retry == nil {
		t.Error("expected default retry options")
	}
	if client.timeout == nil {
		t.Error("expected default timeout options")
	}
	if client.retry.Attempts != 2 {
		t.Errorf("expected 2 retry attempts, got %d", client.retry.Attempts)
	}
	if client.timeout.Timeout != 10*time.Second {
		t.Errorf("expected 10s timeout, got %v", client.timeout.Timeout)
	}
}

func TestClientBuilder_WithTimeout(t *testing.T) {
	client := NewClientBuilder().WithTimeout(30 * time.Second).Build()
	if client.timeout.Timeout != 30*time.Second {
		t.Errorf("expected 30s timeout, got %v", client.timeout.Timeout)
	}
}

func TestClientBuilder_WithRetry(t *testing.T) {
	opts := &RetryOptions{
		Attempts:        5,
		InitialInterval: time.Second,
		Factor:          3.0,
	}
	client := NewClientBuilder().WithRetry(opts).Build()
	if client.retry.Attempts != 5 {
		t.Errorf("expected 5 attempts, got %d", client.retry.Attempts)
	}
	if client.retry.Factor != 3.0 {
		t.Errorf("expected factor 3.0, got %f", client.retry.Factor)
	}
}

func TestClientBuilder_WithNoRetry(t *testing.T) {
	client := NewClientBuilder().WithNoRetry().Build()
	if client.retry != nil {
		t.Error("expected nil retry options")
	}
}

func TestClientBuilder_WithNoTimeout(t *testing.T) {
	client := NewClientBuilder().WithNoTimeout().Build()
	if client.timeout != nil {
		t.Error("expected nil timeout options")
	}
}

func TestClientBuilder_WithRateLimit(t *testing.T) {
	client := NewClientBuilder().WithRateLimit(10, time.Minute).Build()
	if client.rateLimiter == nil {
		t.Fatal("expected rate limiter to be set")
	}
	if client.rateLimiter.maxRequests != 10 {
		t.Errorf("expected max 10 requests, got %d", client.rateLimiter.maxRequests)
	}
	if client.rateLimiter.period != time.Minute {
		t.Errorf("expected 1m period, got %v", client.rateLimiter.period)
	}
}

func TestClientBuilder_WithCircuitBreaker(t *testing.T) {
	client := NewClientBuilder().
		WithCircuitBreaker("my-cb", &CircuitBreakerOptions{
			MaxRequests: 3,
			Timeout:     30 * time.Second,
		}).
		Build()
	if client.circuitBreaker == nil {
		t.Fatal("expected circuit breaker to be set")
	}
}

func TestClientBuilder_WithBaseHeaders(t *testing.T) {
	headers := http.Header{
		"Authorization":   {"Bearer token"},
		"X-Custom-Header": {"value"},
	}
	client := NewClientBuilder().WithBaseHeaders(headers).Build()
	if client.baseHeaders.Get("Authorization") != "Bearer token" {
		t.Errorf("expected Authorization header")
	}
	if client.baseHeaders.Get("X-Custom-Header") != "value" {
		t.Errorf("expected X-Custom-Header header")
	}
}

func TestClientBuilder_WithHTTPClient(t *testing.T) {
	httpClient := &http.Client{Timeout: 5 * time.Second}
	client := NewClientBuilder().WithHTTPClient(httpClient).Build()
	if client.httpClient != httpClient {
		t.Error("expected custom http.Client to be set")
	}
}

func TestClientBuilder_WithHooks(t *testing.T) {
	hooks := &LifecycleHooks{
		PreRequest: func(url string, req *http.Request) (string, *http.Request) {
			return url, req
		},
	}
	client := NewClientBuilder().WithHooks(hooks).Build()
	if client.hooks == nil {
		t.Error("expected hooks to be set")
	}
	if client.hooks.PreRequest == nil {
		t.Error("expected PreRequest hook to be set")
	}
}

func TestClientBuilder_FullChain(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "1", Name: "chain"})
	}))
	defer server.Close()

	client := NewClientBuilder().
		WithTimeout(5 * time.Second).
		WithRetry(&RetryOptions{
			Attempts:        1,
			InitialInterval: 100 * time.Millisecond,
			Factor:          2.0,
		}).
		WithBaseHeaders(http.Header{
			"Authorization": {"Bearer test"},
		}).
		Build()

	resp, err := Get[testJSON](context.Background(), client, server.URL, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if resp.Data.Name != "chain" {
		t.Errorf("expected 'chain', got %s", resp.Data.Name)
	}
}

func TestClientBuilder_RateLimitIntegration(t *testing.T) {
	callCount := 0
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		callCount++
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "1", Name: "rl"})
	}))
	defer server.Close()

	client := NewClientBuilder().
		WithRateLimit(2, 500*time.Millisecond).
		WithNoRetry().
		WithNoTimeout().
		Build()

	// First two should succeed
	for i := 0; i < 2; i++ {
		_, err := Get[testJSON](context.Background(), client, server.URL, nil)
		if err != nil {
			t.Fatalf("request %d: unexpected error: %v", i+1, err)
		}
	}

	// Third should be rate limited
	_, err := Get[testJSON](context.Background(), client, server.URL, nil)
	if err == nil {
		t.Fatal("expected rate limit error")
	}
	_, ok := err.(*RateLimitError)
	if !ok {
		t.Fatalf("expected *RateLimitError, got %T: %v", err, err)
	}
}

func TestClientBuilder_DefaultCircuitBreakerName(t *testing.T) {
	client := NewClientBuilder().
		WithCircuitBreaker("", &CircuitBreakerOptions{
			MaxRequests: 1,
		}).
		Build()
	if client.circuitBreaker == nil {
		t.Fatal("expected circuit breaker to be set even with empty name")
	}
}
