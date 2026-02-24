package fetch

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

func TestPreRequestHook(t *testing.T) {
	var receivedPath string
	var receivedHeader string
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		receivedPath = r.URL.Path
		receivedHeader = r.Header.Get("X-Hook-Header")
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "1", Name: "hooked"})
	}))
	defer server.Close()

	client := NewClientBuilder().
		WithNoRetry().
		WithNoTimeout().
		WithHooks(&LifecycleHooks{
			PreRequest: func(url string, req *http.Request) (string, *http.Request) {
				// Modify the request by adding a header
				req.Header.Set("X-Hook-Header", "hook-value")
				// Modify the URL to add a path
				newURL := url + "/modified"
				newReq, _ := http.NewRequestWithContext(req.Context(), req.Method, newURL, req.Body)
				for k, v := range req.Header {
					newReq.Header[k] = v
				}
				return newURL, newReq
			},
		}).
		Build()

	_, err := Get[testJSON](context.Background(), client, server.URL, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if receivedPath != "/modified" {
		t.Errorf("expected path '/modified', got '%s'", receivedPath)
	}
	if receivedHeader != "hook-value" {
		t.Errorf("expected header 'hook-value', got '%s'", receivedHeader)
	}
}

func TestPreRequestHook_NoModification(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "1", Name: "test"})
	}))
	defer server.Close()

	hookCalled := false
	client := NewClientBuilder().
		WithNoRetry().
		WithNoTimeout().
		WithHooks(&LifecycleHooks{
			PreRequest: func(url string, req *http.Request) (string, *http.Request) {
				hookCalled = true
				return "", nil // no modification
			},
		}).
		Build()

	resp, err := Get[testJSON](context.Background(), client, server.URL, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !hookCalled {
		t.Error("expected pre-request hook to be called")
	}
	if resp.Data.Name != "test" {
		t.Errorf("expected 'test', got %s", resp.Data.Name)
	}
}

func TestPostResponseSuccessHook(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]any{"id": "1", "name": "original"})
	}))
	defer server.Close()

	client := NewClientBuilder().
		WithNoRetry().
		WithNoTimeout().
		WithHooks(&LifecycleHooks{
			PostResponseSuccess: func(url string, req *http.Request, resp *FetchResponse[any]) *FetchResponse[any] {
				// Modify the data
				if data, ok := resp.Data.(map[string]any); ok {
					data["_metadata"] = map[string]any{
						"requestURL":    url,
						"requestMethod": req.Method,
					}
					resp.Data = data
				}
				return resp
			},
		}).
		Build()

	resp, err := Get[map[string]any](context.Background(), client, server.URL, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	data := resp.Data
	if data["id"] != "1" {
		t.Errorf("expected id '1', got %v", data["id"])
	}
	metadata, ok := data["_metadata"].(map[string]any)
	if !ok {
		t.Fatal("expected _metadata in response")
	}
	if metadata["requestMethod"] != "GET" {
		t.Errorf("expected method GET in metadata, got %v", metadata["requestMethod"])
	}
}

func TestPostResponseSuccessHook_NilReturn(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "1", Name: "original"})
	}))
	defer server.Close()

	hookCalled := false
	client := NewClientBuilder().
		WithNoRetry().
		WithNoTimeout().
		WithHooks(&LifecycleHooks{
			PostResponseSuccess: func(url string, req *http.Request, resp *FetchResponse[any]) *FetchResponse[any] {
				hookCalled = true
				return nil // no modification
			},
		}).
		Build()

	resp, err := Get[testJSON](context.Background(), client, server.URL, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !hookCalled {
		t.Error("expected hook to be called")
	}
	if resp.Data.Name != "original" {
		t.Errorf("expected 'original', got %s", resp.Data.Name)
	}
}

func TestPostResponseErrorHook(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(404)
		json.NewEncoder(w).Encode(map[string]any{"error": "Not found"})
	}))
	defer server.Close()

	client := NewClientBuilder().
		WithNoRetry().
		WithNoTimeout().
		WithHooks(&LifecycleHooks{
			PostResponseError: func(url string, req *http.Request, err error, resp *FetchResponse[any]) error {
				if httpErr, ok := err.(*HTTPResponseError); ok {
					return errors.New("Custom error: " + url + " returned " + http.StatusText(httpErr.StatusCode))
				}
				return err
			},
		}).
		Build()

	_, err := Get[any](context.Background(), client, server.URL, nil)
	if err == nil {
		t.Fatal("expected error")
	}
	if !strings.Contains(err.Error(), "Custom error:") {
		t.Errorf("expected custom error message, got: %s", err.Error())
	}
	if !strings.Contains(err.Error(), "Not Found") {
		t.Errorf("expected 'Not Found' in error, got: %s", err.Error())
	}
}

func TestPostResponseErrorHook_NilReturn(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(500)
		json.NewEncoder(w).Encode(map[string]any{"error": "server error"})
	}))
	defer server.Close()

	hookCalled := false
	client := NewClientBuilder().
		WithNoRetry().
		WithNoTimeout().
		WithHooks(&LifecycleHooks{
			PostResponseError: func(url string, req *http.Request, err error, resp *FetchResponse[any]) error {
				hookCalled = true
				return nil // keep original error
			},
		}).
		Build()

	_, err := Get[any](context.Background(), client, server.URL, nil)
	if err == nil {
		t.Fatal("expected error")
	}
	if !hookCalled {
		t.Error("expected hook to be called")
	}
	// Should get the original HTTPResponseError
	_, ok := err.(*HTTPResponseError)
	if !ok {
		t.Fatalf("expected *HTTPResponseError, got %T: %v", err, err)
	}
}

func TestAllHooksCombined(t *testing.T) {
	var receivedHeader string
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		receivedHeader = r.Header.Get("X-Pre-Request")
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]any{"id": "1", "name": "test"})
	}))
	defer server.Close()

	client := NewClientBuilder().
		WithNoRetry().
		WithNoTimeout().
		WithHooks(&LifecycleHooks{
			PreRequest: func(url string, req *http.Request) (string, *http.Request) {
				req.Header.Set("X-Pre-Request", "pre-value")
				return url, req
			},
			PostResponseSuccess: func(url string, req *http.Request, resp *FetchResponse[any]) *FetchResponse[any] {
				if data, ok := resp.Data.(map[string]any); ok {
					data["processed"] = true
					resp.Data = data
				}
				return resp
			},
			PostResponseError: func(url string, req *http.Request, err error, resp *FetchResponse[any]) error {
				// Should not be called for success
				t.Error("PostResponseError should not be called for success")
				return err
			},
		}).
		Build()

	resp, err := Get[map[string]any](context.Background(), client, server.URL, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if receivedHeader != "pre-value" {
		t.Errorf("expected pre-request header, got '%s'", receivedHeader)
	}
	if resp.Data["processed"] != true {
		t.Error("expected processed=true in response data")
	}
}

func TestPerRequestHooksOverrideClientHooks(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "1", Name: "test"})
	}))
	defer server.Close()

	clientHookCalled := false
	requestHookCalled := false

	client := NewClientBuilder().
		WithNoRetry().
		WithNoTimeout().
		WithHooks(&LifecycleHooks{
			PreRequest: func(url string, req *http.Request) (string, *http.Request) {
				clientHookCalled = true
				return "", nil
			},
		}).
		Build()

	_, err := Get[testJSON](context.Background(), client, server.URL, &RequestOptions{
		Hooks: &LifecycleHooks{
			PreRequest: func(url string, req *http.Request) (string, *http.Request) {
				requestHookCalled = true
				return "", nil
			},
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	if clientHookCalled {
		t.Error("client-level hook should not be called when request-level hook is provided")
	}
	if !requestHookCalled {
		t.Error("request-level hook should be called")
	}
}
