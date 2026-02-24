package fetch

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"
)

// testJSON is a helper type for JSON response payloads in tests.
type testJSON struct {
	ID   string `json:"id"`
	Name string `json:"name"`
}

func TestBasicGETRequest(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			t.Errorf("expected GET, got %s", r.Method)
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "1", Name: "test"})
	}))
	defer server.Close()

	client := NewClientBuilder().WithNoRetry().WithNoTimeout().Build()
	resp, err := Get[testJSON](context.Background(), client, server.URL, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !resp.OK {
		t.Errorf("expected OK response, got status %d", resp.StatusCode)
	}
	if resp.StatusCode != 200 {
		t.Errorf("expected 200, got %d", resp.StatusCode)
	}
	if resp.Data.ID != "1" || resp.Data.Name != "test" {
		t.Errorf("unexpected data: %+v", resp.Data)
	}
	if !resp.IsJSON {
		t.Error("expected IsJSON to be true")
	}
}

func TestBasicPOSTRequest(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			t.Errorf("expected POST, got %s", r.Method)
		}
		var body map[string]string
		json.NewDecoder(r.Body).Decode(&body)
		if body["key"] != "value" {
			t.Errorf("expected body key=value, got %v", body)
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "2", Name: "created"})
	}))
	defer server.Close()

	client := NewClientBuilder().WithNoRetry().WithNoTimeout().Build()
	resp, err := Post[testJSON](context.Background(), client, server.URL, map[string]string{"key": "value"}, &RequestOptions{
		Headers: http.Header{"Content-Type": {"application/json"}},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !resp.OK {
		t.Errorf("expected OK, got status %d", resp.StatusCode)
	}
	if resp.Data.ID != "2" {
		t.Errorf("unexpected data: %+v", resp.Data)
	}
}

func TestHTTPErrorResponse(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(404)
		json.NewEncoder(w).Encode(map[string]any{
			"error": map[string]any{
				"message": "Not found",
				"type":    "NotFoundError",
				"code":    404,
			},
		})
	}))
	defer server.Close()

	client := NewClientBuilder().WithNoRetry().WithNoTimeout().Build()
	_, err := Get[testJSON](context.Background(), client, server.URL, nil)
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	httpErr, ok := err.(*HTTPResponseError)
	if !ok {
		t.Fatalf("expected *HTTPResponseError, got %T: %v", err, err)
	}
	if httpErr.StatusCode != 404 {
		t.Errorf("expected status 404, got %d", httpErr.StatusCode)
	}
	if !strings.Contains(httpErr.Error(), "Not found") {
		t.Errorf("expected error message to contain 'Not found', got: %s", httpErr.Error())
	}
	if !strings.Contains(httpErr.Error(), "NotFoundError") {
		t.Errorf("expected error message to contain 'NotFoundError', got: %s", httpErr.Error())
	}
	if !strings.Contains(httpErr.Error(), "404") {
		t.Errorf("expected error message to contain '404', got: %s", httpErr.Error())
	}
}

func TestHTTPErrorStringError(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(400)
		json.NewEncoder(w).Encode(map[string]any{
			"error": "Bad request message",
		})
	}))
	defer server.Close()

	client := NewClientBuilder().WithNoRetry().WithNoTimeout().Build()
	_, err := Get[any](context.Background(), client, server.URL, nil)
	if err == nil {
		t.Fatal("expected error")
	}
	httpErr, ok := err.(*HTTPResponseError)
	if !ok {
		t.Fatalf("expected *HTTPResponseError, got %T", err)
	}
	if !strings.Contains(httpErr.Error(), "Bad request message") {
		t.Errorf("expected 'Bad request message' in error, got: %s", httpErr.Error())
	}
}

func TestHTTPErrorNonJSON(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(500)
		fmt.Fprint(w, "Internal Server Error text")
	}))
	defer server.Close()

	client := NewClientBuilder().WithNoRetry().WithNoTimeout().Build()
	_, err := Get[any](context.Background(), client, server.URL, nil)
	if err == nil {
		t.Fatal("expected error")
	}
	httpErr, ok := err.(*HTTPResponseError)
	if !ok {
		t.Fatalf("expected *HTTPResponseError, got %T", err)
	}
	if !strings.Contains(httpErr.Error(), "Internal Server Error text") {
		t.Errorf("expected body text in error, got: %s", httpErr.Error())
	}
}

func TestNonJSONSuccessResponse(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/plain")
		fmt.Fprint(w, "hello world")
	}))
	defer server.Close()

	client := NewClientBuilder().WithNoRetry().WithNoTimeout().Build()
	resp, err := Get[any](context.Background(), client, server.URL, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if resp.IsJSON {
		t.Error("expected IsJSON to be false")
	}
	if string(resp.BodyRaw) != "hello world" {
		t.Errorf("expected body 'hello world', got %s", string(resp.BodyRaw))
	}
}

func TestPUTRequest(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPut {
			t.Errorf("expected PUT, got %s", r.Method)
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "3", Name: "updated"})
	}))
	defer server.Close()

	client := NewClientBuilder().WithNoRetry().WithNoTimeout().Build()
	resp, err := Put[testJSON](context.Background(), client, server.URL, map[string]string{"name": "updated"}, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if resp.Data.Name != "updated" {
		t.Errorf("expected Name 'updated', got %s", resp.Data.Name)
	}
}

func TestPATCHRequest(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPatch {
			t.Errorf("expected PATCH, got %s", r.Method)
		}
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "4", Name: "patched"})
	}))
	defer server.Close()

	client := NewClientBuilder().WithNoRetry().WithNoTimeout().Build()
	resp, err := Patch[testJSON](context.Background(), client, server.URL, nil, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if resp.Data.Name != "patched" {
		t.Errorf("expected Name 'patched', got %s", resp.Data.Name)
	}
}

func TestDELETERequest(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodDelete {
			t.Errorf("expected DELETE, got %s", r.Method)
		}
		w.WriteHeader(204)
	}))
	defer server.Close()

	client := NewClientBuilder().WithNoRetry().WithNoTimeout().Build()
	resp, err := Delete[any](context.Background(), client, server.URL, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if resp.StatusCode != 204 {
		t.Errorf("expected 204, got %d", resp.StatusCode)
	}
}

func TestBaseHeaders(t *testing.T) {
	var receivedHeaders http.Header
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		receivedHeaders = r.Header
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "1", Name: "test"})
	}))
	defer server.Close()

	client := NewClientBuilder().
		WithBaseHeaders(http.Header{
			"X-Custom-Header": {"custom-value"},
			"Authorization":   {"Bearer test-token"},
		}).
		WithNoRetry().
		WithNoTimeout().
		Build()

	_, err := Get[testJSON](context.Background(), client, server.URL, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if receivedHeaders.Get("X-Custom-Header") != "custom-value" {
		t.Errorf("expected X-Custom-Header 'custom-value', got '%s'", receivedHeaders.Get("X-Custom-Header"))
	}
	if receivedHeaders.Get("Authorization") != "Bearer test-token" {
		t.Errorf("expected Authorization 'Bearer test-token', got '%s'", receivedHeaders.Get("Authorization"))
	}
}

func TestPerRequestHeaders(t *testing.T) {
	var receivedHeaders http.Header
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		receivedHeaders = r.Header
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "1", Name: "test"})
	}))
	defer server.Close()

	client := NewClientBuilder().
		WithBaseHeaders(http.Header{
			"X-Base-Header": {"base-value"},
		}).
		WithNoRetry().
		WithNoTimeout().
		Build()

	_, err := Get[testJSON](context.Background(), client, server.URL, &RequestOptions{
		Headers: http.Header{
			"X-Request-Header": {"request-value"},
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if receivedHeaders.Get("X-Base-Header") != "base-value" {
		t.Errorf("expected X-Base-Header 'base-value', got '%s'", receivedHeaders.Get("X-Base-Header"))
	}
	if receivedHeaders.Get("X-Request-Header") != "request-value" {
		t.Errorf("expected X-Request-Header 'request-value', got '%s'", receivedHeaders.Get("X-Request-Header"))
	}
}

func TestSimpleGetAndPost(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{"method": r.Method})
	}))
	defer server.Close()

	client := NewClientBuilder().WithNoRetry().WithNoTimeout().Build()

	resp, err := SimpleGet(context.Background(), client, server.URL, nil)
	if err != nil {
		t.Fatalf("SimpleGet error: %v", err)
	}
	if !resp.OK {
		t.Error("expected OK")
	}

	resp, err = SimplePost(context.Background(), client, server.URL, nil, nil)
	if err != nil {
		t.Fatalf("SimplePost error: %v", err)
	}
	if !resp.OK {
		t.Error("expected OK")
	}
}

func TestNilClient(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(testJSON{ID: "1", Name: "test"})
	}))
	defer server.Close()

	// Passing nil client should use defaults
	resp, err := Get[testJSON](context.Background(), nil, server.URL, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !resp.OK {
		t.Error("expected OK")
	}
}

func TestStringBody(t *testing.T) {
	var receivedBody string
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		bodyBytes, _ := readAll(r.Body)
		receivedBody = string(bodyBytes)
		w.WriteHeader(200)
	}))
	defer server.Close()

	client := NewClientBuilder().WithNoRetry().WithNoTimeout().Build()
	_, err := Post[any](context.Background(), client, server.URL, "raw string body", nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if receivedBody != "raw string body" {
		t.Errorf("expected 'raw string body', got '%s'", receivedBody)
	}
}

func TestByteBody(t *testing.T) {
	var receivedBody string
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		bodyBytes, _ := readAll(r.Body)
		receivedBody = string(bodyBytes)
		w.WriteHeader(200)
	}))
	defer server.Close()

	client := NewClientBuilder().WithNoRetry().WithNoTimeout().Build()
	_, err := Post[any](context.Background(), client, server.URL, []byte("byte body"), nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if receivedBody != "byte body" {
		t.Errorf("expected 'byte body', got '%s'", receivedBody)
	}
}

func TestReaderBody(t *testing.T) {
	var receivedBody string
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		bodyBytes, _ := readAll(r.Body)
		receivedBody = string(bodyBytes)
		w.WriteHeader(200)
	}))
	defer server.Close()

	client := NewClientBuilder().WithNoRetry().WithNoTimeout().Build()
	_, err := Post[any](context.Background(), client, server.URL, strings.NewReader("reader body"), nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if receivedBody != "reader body" {
		t.Errorf("expected 'reader body', got '%s'", receivedBody)
	}
}

func TestContextCancellation(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		time.Sleep(2 * time.Second)
		w.WriteHeader(200)
	}))
	defer server.Close()

	ctx, cancel := context.WithCancel(context.Background())
	cancel() // cancel immediately

	client := NewClientBuilder().WithNoRetry().WithNoTimeout().Build()
	_, err := Get[any](ctx, client, server.URL, nil)
	if err == nil {
		t.Fatal("expected error from cancelled context")
	}
}

func TestErrorMessages(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(400)
		json.NewEncoder(w).Encode(map[string]any{
			"errorMessages": []string{"Error one", "Error two"},
		})
	}))
	defer server.Close()

	client := NewClientBuilder().WithNoRetry().WithNoTimeout().Build()
	_, err := Get[any](context.Background(), client, server.URL, nil)
	if err == nil {
		t.Fatal("expected error")
	}
	if !strings.Contains(err.Error(), "Error one") || !strings.Contains(err.Error(), "Error two") {
		t.Errorf("expected error messages in output, got: %s", err.Error())
	}
}

// readAll is a simple helper for reading an io.Reader in tests.
func readAll(r interface{ Read([]byte) (int, error) }) ([]byte, error) {
	var buf []byte
	tmp := make([]byte, 1024)
	for {
		n, err := r.Read(tmp)
		buf = append(buf, tmp[:n]...)
		if err != nil {
			break
		}
	}
	return buf, nil
}
