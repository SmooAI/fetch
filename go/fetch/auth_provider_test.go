package fetch

import (
	"context"
	"errors"
	"fmt"
	"net/http"
	"net/http/httptest"
	"sync/atomic"
	"testing"
)

func TestClientBuilder_WithAuthTokenProvider_DefaultScheme(t *testing.T) {
	var captured string
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		captured = r.Header.Get("Authorization")
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"ok":true}`))
	}))
	defer server.Close()

	client := NewClientBuilder().
		WithNoRetry().
		WithAuthTokenProvider(func(_ context.Context) (string, error) {
			return "fresh-token", nil
		}, "").
		Build()

	_, err := SimpleGet(context.Background(), client, server.URL, nil)
	if err != nil {
		t.Fatalf("fetch failed: %v", err)
	}

	if captured != "Bearer fresh-token" {
		t.Errorf("expected 'Bearer fresh-token', got %q", captured)
	}
}

func TestClientBuilder_WithAuthTokenProvider_CustomScheme(t *testing.T) {
	var captured string
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		captured = r.Header.Get("Authorization")
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"ok":true}`))
	}))
	defer server.Close()

	client := NewClientBuilder().
		WithNoRetry().
		WithAuthTokenProvider(func(_ context.Context) (string, error) {
			return "abc", nil
		}, "Token").
		Build()

	_, err := SimpleGet(context.Background(), client, server.URL, nil)
	if err != nil {
		t.Fatalf("fetch failed: %v", err)
	}

	if captured != "Token abc" {
		t.Errorf("expected 'Token abc', got %q", captured)
	}
}

func TestClientBuilder_WithAuthTokenProvider_InvokedPerRequest(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(`{"ok":true}`))
	}))
	defer server.Close()

	var calls int32
	client := NewClientBuilder().
		WithNoRetry().
		WithAuthTokenProvider(func(_ context.Context) (string, error) {
			n := atomic.AddInt32(&calls, 1)
			return fmt.Sprintf("tok-%d", n), nil
		}, "Bearer").
		Build()

	for i := 0; i < 3; i++ {
		if _, err := SimpleGet(context.Background(), client, server.URL, nil); err != nil {
			t.Fatalf("fetch %d failed: %v", i, err)
		}
	}

	if got := atomic.LoadInt32(&calls); got != 3 {
		t.Errorf("expected 3 provider invocations, got %d", got)
	}
}

func TestClientBuilder_WithAuthTokenProvider_PropagatesError(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		t.Error("HTTP request fired despite provider error")
		w.WriteHeader(200)
	}))
	defer server.Close()

	wantErr := errors.New("token-mint-failure")
	client := NewClientBuilder().
		WithNoRetry().
		WithAuthTokenProvider(func(_ context.Context) (string, error) {
			return "", wantErr
		}, "Bearer").
		Build()

	_, err := SimpleGet(context.Background(), client, server.URL, nil)
	if err == nil || !errors.Is(err, wantErr) {
		t.Fatalf("expected wrapped %v, got %v", wantErr, err)
	}
}
