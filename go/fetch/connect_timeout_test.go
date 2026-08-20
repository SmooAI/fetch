package fetch

import (
	"context"
	"encoding/json"
	"os"
	"testing"
	"time"
)

// connectTimeoutCorpus is spec/connect-timeout-corpus.json, shared with the
// other four ports — see the corpus for why these knobs are not inlined here.
type connectTimeoutCorpus struct {
	BlackHoleURL          string `json:"blackHoleUrl"`
	ConnectTimeoutMs      int    `json:"connectTimeoutMs"`
	WholeRequestTimeoutMs int    `json:"wholeRequestTimeoutMs"`
	MaxElapsedMs          int    `json:"maxElapsedMs"`
}

func loadConnectTimeoutCorpus(t *testing.T) connectTimeoutCorpus {
	t.Helper()
	raw, err := os.ReadFile("../../spec/connect-timeout-corpus.json")
	if err != nil {
		t.Fatalf("connect-timeout corpus must be readable: %v", err)
	}
	var c connectTimeoutCorpus
	if err := json.Unmarshal(raw, &c); err != nil {
		t.Fatalf("connect-timeout corpus must parse: %v", err)
	}
	if c.BlackHoleURL == "" || c.ConnectTimeoutMs == 0 || c.MaxElapsedMs == 0 {
		t.Fatalf("connect-timeout corpus is missing knobs: %+v", c)
	}
	return c
}

// TestConnectTimeoutFailsFastOnBlackHole mirrors the Rust
// connect_timeout_tests.rs regression coverage (SMOODEV-2498 / SMOODEV-2481):
// a bounded connect timeout must fail fast on a black-holed connect instead of
// stalling until the (10x larger) whole-request timeout.
//
// 10.255.255.1 is a non-routable RFC1918 address with (almost certainly) no
// host answering, so the SYN is dropped and the connect never establishes.
func TestConnectTimeoutFailsFastOnBlackHole(t *testing.T) {
	corpus := loadConnectTimeoutCorpus(t)
	connectTimeout := time.Duration(corpus.ConnectTimeoutMs) * time.Millisecond

	client := NewClientBuilder().
		WithConnectTimeout(connectTimeout).
		WithTimeout(time.Duration(corpus.WholeRequestTimeoutMs) * time.Millisecond).
		WithNoRetry().
		Build()

	start := time.Now()
	_, err := SimpleGet(context.Background(), client, corpus.BlackHoleURL, nil)
	elapsed := time.Since(start)

	if err == nil {
		t.Fatal("expected connect to fail against a black hole, got nil error")
	}
	// Must fail in roughly the connect window, well under the whole timeout.
	if elapsed >= time.Duration(corpus.MaxElapsedMs)*time.Millisecond {
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
