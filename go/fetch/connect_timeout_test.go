package fetch

import (
	"context"
	"testing"
	"time"
)

// TestConnectTimeoutFailsFastOnBlackHole mirrors the Rust
// connect_timeout_tests.rs regression coverage (SMOODEV-2498 / SMOODEV-2481):
// a bounded connect timeout must fail fast on a black-holed connect instead of
// stalling until the (10x larger) whole-request timeout.
//
// 10.255.255.1 is a non-routable RFC1918 address with (almost certainly) no
// host answering, so the SYN is dropped and the connect never establishes.
func TestConnectTimeoutFailsFastOnBlackHole(t *testing.T) {
	const connectTimeout = 500 * time.Millisecond

	client := NewClientBuilder().
		WithConnectTimeout(connectTimeout).
		// Whole-request timeout is 10x the connect timeout: if the connect
		// timeout is NOT honored, this would take ~5s and the elapsed
		// assertion below would fail.
		WithTimeout(5 * time.Second).
		WithNoRetry().
		Build()

	start := time.Now()
	_, err := SimpleGet(context.Background(), client, "http://10.255.255.1:80/anything", nil)
	elapsed := time.Since(start)

	if err == nil {
		t.Fatal("expected connect to fail against a black hole, got nil error")
	}
	// Must fail in roughly the connect window, well under the 5s whole timeout.
	if elapsed >= 3*time.Second {
		t.Fatalf("connect timeout did not fire fast: elapsed %v (connect timeout was %v)", elapsed, connectTimeout)
	}
}

// TestConnectTimeoutUnsetLeavesTransportUntouched verifies the default-OFF
// guarantee: when WithConnectTimeout is not called, the SDK does not construct
// a custom transport, so behavior is byte-identical to before.
func TestConnectTimeoutUnsetLeavesTransportUntouched(t *testing.T) {
	client := NewClientBuilder().WithNoRetry().Build()
	if client.connectTimeout != 0 {
		t.Fatalf("expected connectTimeout 0 when unset, got %v", client.connectTimeout)
	}
	if client.httpClient.Transport != nil {
		t.Fatalf("expected nil (default) transport when connect timeout unset, got %T", client.httpClient.Transport)
	}
}
