package fetch

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"net"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"
)

// retryIdempotencyCorpus is spec/retry-idempotency-corpus.json, shared with the
// other four ports — see the corpus for why the cases are not inlined here.
type retryIdempotencyCorpus struct {
	IdempotencyKeyHeader string `json:"idempotencyKeyHeader"`
	Methods              struct {
		Idempotent    []string `json:"idempotent"`
		NonIdempotent []string `json:"nonIdempotent"`
	} `json:"methods"`
	Retry struct {
		Attempts          int     `json:"attempts"`
		InitialIntervalMs int     `json:"initialIntervalMs"`
		Factor            float64 `json:"factor"`
		JitterAdjustment  float64 `json:"jitterAdjustment"`
	} `json:"retry"`
	TimeoutMs int                    `json:"timeoutMs"`
	Cases     []retryIdempotencyCase `json:"cases"`
	Abort     struct {
		TimeoutMs    int `json:"timeoutMs"`
		CloseSlackMs int `json:"closeSlackMs"`
	} `json:"timeoutAbort"`
}

type retryIdempotencyCase struct {
	Name               string            `json:"name"`
	Method             string            `json:"method"`
	Body               string            `json:"body"`
	Headers            map[string]string `json:"headers"`
	AllowNonIdempotent bool              `json:"allowNonIdempotent"`
	Respond            struct {
		Status  int               `json:"status"`
		Headers map[string]string `json:"headers"`
		Hang    bool              `json:"hang"`
	} `json:"respond"`
	ExpectedAttempts int `json:"expectedAttempts"`
}

func loadRetryIdempotencyCorpus(t *testing.T) retryIdempotencyCorpus {
	t.Helper()
	raw, err := os.ReadFile("../../spec/retry-idempotency-corpus.json")
	if err != nil {
		t.Fatalf("retry-idempotency corpus must be readable: %v", err)
	}
	var c retryIdempotencyCorpus
	if err := json.Unmarshal(raw, &c); err != nil {
		t.Fatalf("retry-idempotency corpus must parse: %v", err)
	}
	return c
}

// Positive control: a corpus that failed to load would leave every table empty
// and every loop below trivially green.
func TestRetryIdempotencyCorpusLoaded(t *testing.T) {
	c := loadRetryIdempotencyCorpus(t)
	if len(c.Cases) == 0 || len(c.Methods.Idempotent) == 0 || len(c.Methods.NonIdempotent) == 0 {
		t.Fatalf("corpus is missing cases: %+v", c)
	}
	if c.Retry.Attempts == 0 || c.TimeoutMs == 0 || c.Abort.TimeoutMs == 0 || c.Abort.CloseSlackMs == 0 {
		t.Fatalf("corpus is missing knobs: %+v", c)
	}
	if c.IdempotencyKeyHeader != IdempotencyKeyHeader {
		t.Fatalf("corpus idempotency header %q != IdempotencyKeyHeader %q", c.IdempotencyKeyHeader, IdempotencyKeyHeader)
	}
}

func TestIsIdempotentMethodCorpus(t *testing.T) {
	c := loadRetryIdempotencyCorpus(t)
	for _, m := range c.Methods.Idempotent {
		if !IsIdempotentMethod(m) {
			t.Errorf("IsIdempotentMethod(%q) = false, want true", m)
		}
	}
	for _, m := range c.Methods.NonIdempotent {
		if IsIdempotentMethod(m) {
			t.Errorf("IsIdempotentMethod(%q) = true, want false", m)
		}
	}
}

func corpusClient(c retryIdempotencyCorpus, allowNonIdempotent bool) *Client {
	return NewClientBuilder().
		WithRetry(&RetryOptions{
			Attempts:           c.Retry.Attempts,
			InitialInterval:    time.Duration(c.Retry.InitialIntervalMs) * time.Millisecond,
			Factor:             c.Retry.Factor,
			JitterFraction:     c.Retry.JitterAdjustment,
			OnRejection:        DefaultRetryOptions.OnRejection,
			AllowNonIdempotent: allowNonIdempotent,
		}).
		WithTimeout(time.Duration(c.TimeoutMs) * time.Millisecond).
		Build()
}

// TestRetryIdempotencyCorpus counts what the SERVER received, because the bug
// is a side effect executing more than once server-side.
func TestRetryIdempotencyCorpus(t *testing.T) {
	c := loadRetryIdempotencyCorpus(t)
	for _, tc := range c.Cases {
		t.Run(tc.Name, func(t *testing.T) {
			var hits atomic.Int32
			release := make(chan struct{})
			srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				hits.Add(1)
				if tc.Respond.Hang {
					select {
					case <-r.Context().Done():
					case <-release:
					}
					return
				}
				for k, v := range tc.Respond.Headers {
					w.Header().Set(k, v)
				}
				w.WriteHeader(tc.Respond.Status)
			}))
			// Release hung handlers before Close, which waits for them.
			defer srv.Close()
			defer close(release)

			headers := http.Header{}
			for k, v := range tc.Headers {
				// Raw map assignment keeps the corpus's casing, so the
				// case-insensitivity case really sends a lowercase key through.
				headers[k] = []string{v}
			}
			var body any
			if tc.Body != "" {
				body = tc.Body
			}

			_, err := Fetch[any](context.Background(), corpusClient(c, tc.AllowNonIdempotent), tc.Method, srv.URL, body, &RequestOptions{Headers: headers})
			if err == nil {
				t.Fatal("expected the request to fail")
			}
			if got := int(hits.Load()); got != tc.ExpectedAttempts {
				t.Fatalf("server received %d requests, want %d (err: %v)", got, tc.ExpectedAttempts, err)
			}

			var retryErr *RetryError
			if tc.ExpectedAttempts == 1 && errors.As(err, &retryErr) {
				t.Fatalf("a request that was not retried must surface the underlying error, got %T: %v", err, err)
			}
			if tc.ExpectedAttempts > 1 && !errors.As(err, &retryErr) {
				t.Fatalf("an exhausted retry must surface *RetryError, got %T: %v", err, err)
			}
		})
	}
}

// TestRetryIneligibleSkipsOnRejection pins that the callback is not consulted
// for a request that is not retry-eligible: a custom callback is not an opt-in.
func TestRetryIneligibleSkipsOnRejection(t *testing.T) {
	var hits, consulted atomic.Int32
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		hits.Add(1)
		w.WriteHeader(http.StatusServiceUnavailable)
	}))
	defer srv.Close()

	client := NewClientBuilder().WithRetry(&RetryOptions{
		Attempts:        2,
		InitialInterval: time.Millisecond,
		OnRejection: func(RetryContext) (RetryDecision, time.Duration) {
			consulted.Add(1)
			return RetryDefault, 0
		},
	}).Build()

	_, err := Post[any](context.Background(), client, srv.URL, "{}", nil)
	var httpErr *HTTPResponseError
	if !errors.As(err, &httpErr) || httpErr.StatusCode != http.StatusServiceUnavailable {
		t.Fatalf("expected the 503 unwrapped, got %T: %v", err, err)
	}
	if hits.Load() != 1 || consulted.Load() != 0 {
		t.Fatalf("hits=%d consulted=%d, want 1 and 0", hits.Load(), consulted.Load())
	}
}

// TestRetryEligibilitySeesPreRequestHook: the gate is evaluated on the FINAL
// request, so a hook that adds an Idempotency-Key opts the POST in.
func TestRetryEligibilitySeesPreRequestHook(t *testing.T) {
	var hits atomic.Int32
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		hits.Add(1)
		w.WriteHeader(http.StatusServiceUnavailable)
	}))
	defer srv.Close()

	retry := DefaultRetryOptions
	retry.InitialInterval = time.Millisecond
	retry.JitterFraction = 0
	client := NewClientBuilder().WithRetry(&retry).WithHooks(&LifecycleHooks{
		PreRequest: func(url string, req *http.Request) (string, *http.Request) {
			req.Header.Set(IdempotencyKeyHeader, "from-hook")
			return url, req
		},
	}).Build()

	_, _ = Post[any](context.Background(), client, srv.URL, "{}", nil)
	if got := hits.Load(); got != int32(1+retry.Attempts) {
		t.Fatalf("server received %d requests, want %d", got, 1+retry.Attempts)
	}
}

// abortServer is a raw TCP server that reads each request's head and never
// answers, recording when each request arrived and when its connection was
// closed by the client (read returns EOF). Raw sockets, not net/http, so
// "closed" means the peer really closed the TCP connection.
type abortServer struct {
	ln       net.Listener
	mu       sync.Mutex
	arrivals []time.Time
	closes   []time.Time // indexed like arrivals; zero until closed
	closed   chan int
}

func newAbortServer(t *testing.T) *abortServer {
	t.Helper()
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("listen: %v", err)
	}
	s := &abortServer{ln: ln, closed: make(chan int, 16)}
	go func() {
		for {
			conn, err := ln.Accept()
			if err != nil {
				return
			}
			go s.serve(conn)
		}
	}()
	t.Cleanup(func() { _ = ln.Close() })
	return s
}

func (s *abortServer) serve(conn net.Conn) {
	defer conn.Close()
	r := bufio.NewReader(conn)
	// Read the request head; the body (if any) is drained by the EOF loop.
	for {
		line, err := r.ReadString('\n')
		if err != nil {
			return
		}
		if strings.TrimRight(line, "\r\n") == "" {
			break
		}
	}
	s.mu.Lock()
	idx := len(s.arrivals)
	s.arrivals = append(s.arrivals, time.Now())
	s.closes = append(s.closes, time.Time{})
	s.mu.Unlock()

	buf := make([]byte, 512)
	for {
		if _, err := r.Read(buf); err != nil {
			break
		}
	}
	s.mu.Lock()
	s.closes[idx] = time.Now()
	s.mu.Unlock()
	s.closed <- idx
}

func (s *abortServer) snapshot() ([]time.Time, []time.Time) {
	s.mu.Lock()
	defer s.mu.Unlock()
	return append([]time.Time(nil), s.arrivals...), append([]time.Time(nil), s.closes...)
}

// TestTimeoutClosesTheAttemptConnection: a timed-out attempt must be cancelled,
// not abandoned — its connection closes, so the server can stop work — and a
// retry must never overlap the attempt it replaces.
func TestTimeoutClosesTheAttemptConnection(t *testing.T) {
	c := loadRetryIdempotencyCorpus(t)
	timeout := time.Duration(c.Abort.TimeoutMs) * time.Millisecond
	bound := timeout + time.Duration(c.Abort.CloseSlackMs)*time.Millisecond

	for _, method := range []string{http.MethodGet, http.MethodPost} {
		t.Run(method, func(t *testing.T) {
			srv := newAbortServer(t)
			client := NewClientBuilder().
				WithRetry(&RetryOptions{Attempts: 1, InitialInterval: time.Millisecond, OnRejection: DefaultRetryOptions.OnRejection}).
				WithTimeout(timeout).
				Build()

			_, err := Fetch[any](context.Background(), client, method, "http://"+srv.ln.Addr().String()+"/hang", nil, nil)
			var timeoutErr *TimeoutError
			if !errors.As(err, &timeoutErr) {
				t.Fatalf("expected a *TimeoutError, got %T: %v", err, err)
			}

			deadline := time.After(bound)
		wait:
			for {
				select {
				case idx := <-srv.closed:
					if idx == 0 {
						break wait
					}
				case <-deadline:
					break wait
				}
			}
			arrivals, closes := srv.snapshot()
			if len(arrivals) == 0 {
				t.Fatal("the server never saw the first attempt")
			}
			if closes[0].IsZero() {
				t.Fatalf("the first attempt's connection was still open %v after it arrived", bound)
			}
			if lag := closes[0].Sub(arrivals[0]); lag > bound {
				t.Fatalf("the first attempt's connection closed %v after it arrived, want <= %v", lag, bound)
			}

			switch method {
			case http.MethodGet:
				if len(arrivals) != 2 {
					t.Fatalf("GET: server saw %d attempts, want 2", len(arrivals))
				}
				if !closes[0].Before(arrivals[1]) {
					t.Fatalf("GET: the first connection closed at %v, AFTER the retry arrived at %v", closes[0], arrivals[1])
				}
			case http.MethodPost:
				if len(arrivals) != 1 {
					t.Fatalf("POST: server saw %d attempts, want 1", len(arrivals))
				}
			}
		})
	}
}
