package fetch

import (
	"context"
	"testing"
	"time"
)

func TestExecuteWithTimeout_Success(t *testing.T) {
	result, err := ExecuteWithTimeout(context.Background(), time.Second, func(ctx context.Context) (string, error) {
		return "done", nil
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "done" {
		t.Errorf("expected 'done', got %s", result)
	}
}

func TestExecuteWithTimeout_Timeout(t *testing.T) {
	_, err := ExecuteWithTimeout(context.Background(), 50*time.Millisecond, func(ctx context.Context) (string, error) {
		select {
		case <-ctx.Done():
			return "", ctx.Err()
		case <-time.After(5 * time.Second):
			return "late", nil
		}
	})
	if err == nil {
		t.Fatal("expected timeout error")
	}
	timeoutErr, ok := err.(*TimeoutError)
	if !ok {
		t.Fatalf("expected *TimeoutError, got %T: %v", err, err)
	}
	if timeoutErr.Timeout != 50*time.Millisecond {
		t.Errorf("expected timeout of 50ms, got %v", timeoutErr.Timeout)
	}
}

func TestExecuteWithTimeout_ContextAlreadyCancelled(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	_, err := ExecuteWithTimeout(ctx, time.Second, func(ctx context.Context) (string, error) {
		return "never", nil
	})
	if err == nil {
		t.Fatal("expected error from cancelled context")
	}
}

func TestExecuteWithTimeout_CompletesJustBeforeDeadline(t *testing.T) {
	result, err := ExecuteWithTimeout(context.Background(), 500*time.Millisecond, func(ctx context.Context) (string, error) {
		time.Sleep(50 * time.Millisecond)
		return "fast", nil
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "fast" {
		t.Errorf("expected 'fast', got %s", result)
	}
}

func TestExecuteWithTimeout_FunctionReturnsError(t *testing.T) {
	_, err := ExecuteWithTimeout(context.Background(), time.Second, func(ctx context.Context) (string, error) {
		return "", &HTTPResponseError{StatusCode: 500, Message: "server error"}
	})
	if err == nil {
		t.Fatal("expected error")
	}
	httpErr, ok := err.(*HTTPResponseError)
	if !ok {
		t.Fatalf("expected *HTTPResponseError, got %T", err)
	}
	if httpErr.StatusCode != 500 {
		t.Errorf("expected status 500, got %d", httpErr.StatusCode)
	}
}

func TestTimeoutIntegrationWithFetch(t *testing.T) {
	// Use the timeout integration through the Client
	client := NewClientBuilder().
		WithTimeout(100 * time.Millisecond).
		WithNoRetry().
		Build()

	// We can't easily simulate a slow server in a unit test without httptest,
	// but we can verify the client was configured correctly.
	if client.timeout == nil {
		t.Fatal("expected timeout to be set")
	}
	if client.timeout.Timeout != 100*time.Millisecond {
		t.Errorf("expected 100ms timeout, got %v", client.timeout.Timeout)
	}
}
