package fetch

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net"
	"net/http"
	"strings"
	"time"

	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/propagation"
	"go.opentelemetry.io/otel/trace"
)

// Client is a resilient HTTP client with built-in retry, timeout,
// rate limiting, and circuit breaker support.
type Client struct {
	httpClient     *http.Client
	baseHeaders    http.Header
	retry          *RetryOptions
	timeout        *TimeoutOptions
	connectTimeout time.Duration
	rateLimiter    *SlidingWindowRateLimiter
	rateLimitRetry *RateLimitRetryOptions
	circuitBreaker *CircuitBreaker
	hooks          *LifecycleHooks
	authProvider   AuthTokenProvider
	authScheme     string
}

// transportWithConnectTimeout clones the default transport (preserving proxy,
// TLS, and keep-alive settings) and bounds only the connection-establishment
// phase via the dialer's Timeout. A connect that never completes (e.g. a
// black-holed SYN to a dead pod IP still lingering in a ClusterIP's iptables)
// then fails in ~connectTimeout instead of stalling until the whole-request
// timeout. Slow-but-alive handlers are unaffected.
func transportWithConnectTimeout(connectTimeout time.Duration) *http.Transport {
	tr := http.DefaultTransport.(*http.Transport).Clone()
	tr.DialContext = (&net.Dialer{Timeout: connectTimeout}).DialContext
	return tr
}

// NewClient creates a new Client with default settings (retry + timeout).
// Use NewClientBuilder for advanced configuration.
func NewClient() *Client {
	retryOpts := DefaultRetryOptions
	timeoutOpts := DefaultTimeoutOptions
	return &Client{
		httpClient:  &http.Client{},
		baseHeaders: make(http.Header),
		retry:       &retryOpts,
		timeout:     &timeoutOpts,
	}
}

// Fetch performs an HTTP request and returns a typed FetchResponse.
// The response body is automatically decoded as JSON if the Content-Type header
// indicates JSON. T is the expected response body type for JSON responses.
func Fetch[T any](ctx context.Context, client *Client, method, url string, body any, opts *RequestOptions) (*FetchResponse[T], error) {
	if client == nil {
		client = NewClient()
	}

	// Determine effective retry and timeout options
	retryOpts := client.retry
	timeoutOpts := client.timeout
	var hooks *LifecycleHooks
	var extraHeaders http.Header

	if opts != nil {
		if opts.Retry != nil {
			retryOpts = opts.Retry
		}
		if opts.Timeout != nil {
			timeoutOpts = opts.Timeout
		}
		if opts.Hooks != nil {
			hooks = opts.Hooks
		}
		if opts.Headers != nil {
			extraHeaders = opts.Headers
		}
	}

	if hooks == nil {
		hooks = client.hooks
	}

	// Build the core request-execute function
	doRequest := func(ctx context.Context) (*FetchResponse[T], error) {
		return executeHTTPRequest[T](ctx, client, method, url, body, extraHeaders, hooks)
	}

	// Wrap with timeout if configured
	if timeoutOpts != nil && timeoutOpts.Timeout > 0 {
		inner := doRequest
		doRequest = func(ctx context.Context) (*FetchResponse[T], error) {
			return ExecuteWithTimeout(ctx, timeoutOpts.Timeout, func(ctx context.Context) (*FetchResponse[T], error) {
				return inner(ctx)
			})
		}
	}

	// Wrap with rate limiter if configured. When WithRateLimitRetry has been set,
	// rate-limit rejections are retried within a dedicated inner loop so they don't
	// consume the main retry budget — matching the TypeScript container-options
	// semantics where rateLimit.retry is distinct from the request-level retry.
	if client.rateLimiter != nil {
		inner := doRequest
		rateLimitRetryOpts := client.rateLimitRetry
		doRequest = func(ctx context.Context) (*FetchResponse[T], error) {
			gated := func(ctx context.Context) (*FetchResponse[T], error) {
				if err := client.rateLimiter.Allow(); err != nil {
					return nil, err
				}
				return inner(ctx)
			}
			if rateLimitRetryOpts != nil && rateLimitRetryOpts.Attempts > 0 {
				return ExecuteWithRetry(ctx, *rateLimitRetryOpts, gated)
			}
			return gated(ctx)
		}
	}

	// Wrap with circuit breaker if configured
	if client.circuitBreaker != nil {
		inner := doRequest
		doRequest = func(ctx context.Context) (*FetchResponse[T], error) {
			result, err := client.circuitBreaker.Execute(ctx, func(ctx context.Context) (any, error) {
				return inner(ctx)
			})
			if err != nil {
				return nil, err
			}
			return result.(*FetchResponse[T]), nil
		}
	}

	// Wrap with retry if configured
	if retryOpts != nil && retryOpts.Attempts > 0 {
		return ExecuteWithRetry(ctx, *retryOpts, func(ctx context.Context) (*FetchResponse[T], error) {
			return doRequest(ctx)
		})
	}

	return doRequest(ctx)
}

// injectTraceContext writes W3C trace context (`traceparent`/`tracestate`) onto
// an outbound request so a trace survives the service hop.
//
// # Why this exists
//
// api-prime already EXTRACTS `traceparent` on ingress, but nothing on the
// platform ever injected it on egress — so every service-to-service call began a
// brand new root trace. Measured over a three-hour production window: 34,961
// traces touched exactly one service, and 4 touched two. Distributed tracing did
// not work, and no amount of extra spans inside a service could fix it.
//
// The client is the correct place for propagation: services are already required
// to use @smooai/fetch over raw net/http, so wiring it once here covers the fleet.
//
// # Two guards, each for a reason
//
//  1. Caller wins. If the caller already set `traceparent` explicitly, theirs is
//     left alone. A client that silently rewrites an intentional header is worse
//     than one that does nothing.
//  2. Valid span contexts only. With no active span the context carries
//     `trace.SpanContext{}` — all-zero ids. Injecting that writes a malformed
//     `00-000…-00` traceparent that a downstream service will either reject or,
//     worse, adopt — poisoning its trace. The sibling logger shipped exactly this
//     bug, where all-zero ids overwrote a real correlation id. The stock W3C
//     propagator happens to guard this too, so the check is defence in depth
//     against a custom or composite propagator that does not.
//
// With no propagator configured the OTel global default is a no-op composite, so
// a service that has not set one up sends nothing extra either way.
func injectTraceContext(req *http.Request) {
	// http.Header.Get canonicalises, so this catches whatever casing the caller
	// used — the case-insensitive check the header map cannot do on its own.
	if req.Header.Get("traceparent") != "" {
		return
	}

	ctx := req.Context()
	if !trace.SpanContextFromContext(ctx).IsValid() {
		return
	}

	otel.GetTextMapPropagator().Inject(ctx, propagation.HeaderCarrier(req.Header))
}

// executeHTTPRequest performs the actual HTTP request and parses the response.
func executeHTTPRequest[T any](
	ctx context.Context,
	client *Client,
	method, requestURL string,
	body any,
	extraHeaders http.Header,
	hooks *LifecycleHooks,
) (*FetchResponse[T], error) {
	// Prepare body
	var bodyReader io.Reader
	if body != nil {
		switch v := body.(type) {
		case string:
			bodyReader = strings.NewReader(v)
		case []byte:
			bodyReader = bytes.NewReader(v)
		case io.Reader:
			bodyReader = v
		default:
			jsonBytes, err := json.Marshal(body)
			if err != nil {
				return nil, fmt.Errorf("failed to marshal request body: %w", err)
			}
			bodyReader = bytes.NewReader(jsonBytes)
		}
	}

	req, err := http.NewRequestWithContext(ctx, method, requestURL, bodyReader)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	// Apply base headers
	for key, vals := range client.baseHeaders {
		for _, v := range vals {
			req.Header.Set(key, v)
		}
	}

	// Apply extra headers (per-request)
	for key, vals := range extraHeaders {
		for _, v := range vals {
			req.Header.Set(key, v)
		}
	}

	// Apply pre-request hook
	if hooks != nil && hooks.PreRequest != nil {
		newURL, newReq := hooks.PreRequest(requestURL, req)
		if newURL != "" && newReq != nil {
			requestURL = newURL
			req = newReq
		}
	}

	// Continue the caller's trace across the hop. Applied AFTER the caller's own
	// headers and after the pre-request hook (which may replace the whole
	// request) so an explicitly-set `traceparent` still wins. This is the
	// single-request site, inside the retry/timeout/breaker wrappers, so every
	// attempt carries a CURRENT traceparent rather than a stale one.
	injectTraceContext(req)

	// Apply auth-token provider (after the pre-request hook so the hook can
	// adjust the URL/init first; the resulting Authorization header overrides
	// any value the hook may have set).
	if client.authProvider != nil {
		token, err := client.authProvider(ctx)
		if err != nil {
			if hooks != nil && hooks.PostResponseError != nil {
				if replacementErr := hooks.PostResponseError(requestURL, req, err, nil); replacementErr != nil {
					return nil, replacementErr
				}
			}
			return nil, fmt.Errorf("auth token provider failed: %w", err)
		}
		scheme := client.authScheme
		if scheme == "" {
			scheme = "Bearer"
		}
		req.Header.Set("Authorization", fmt.Sprintf("%s %s", scheme, token))
	}

	// Execute the HTTP request
	httpClient := client.httpClient
	if httpClient == nil {
		httpClient = http.DefaultClient
	}

	resp, err := httpClient.Do(req)
	if err != nil {
		// Wrap context deadline exceeded as TimeoutError
		if ctx.Err() == context.DeadlineExceeded {
			timeoutErr := &TimeoutError{Timeout: 0}
			if hooks != nil && hooks.PostResponseError != nil {
				if replacementErr := hooks.PostResponseError(requestURL, req, timeoutErr, nil); replacementErr != nil {
					return nil, replacementErr
				}
			}
			return nil, timeoutErr
		}
		if hooks != nil && hooks.PostResponseError != nil {
			if replacementErr := hooks.PostResponseError(requestURL, req, err, nil); replacementErr != nil {
				return nil, replacementErr
			}
		}
		return nil, err
	}
	defer resp.Body.Close()

	// Read body
	bodyBytes, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response body: %w", err)
	}

	// Parse response
	isOK := resp.StatusCode >= 200 && resp.StatusCode < 300
	isJSON := false
	contentType := resp.Header.Get("Content-Type")
	if strings.Contains(contentType, "application/json") {
		isJSON = true
	}

	fetchResp := &FetchResponse[T]{
		StatusCode: resp.StatusCode,
		Status:     resp.Status,
		Headers:    resp.Header,
		IsJSON:     isJSON,
		BodyRaw:    bodyBytes,
		OK:         isOK,
	}

	// Parse JSON body
	if isJSON && len(bodyBytes) > 0 {
		var data T
		if jsonErr := json.Unmarshal(bodyBytes, &data); jsonErr == nil {
			fetchResp.Data = data
		}
	}

	if !isOK {
		// Build headers map for the error
		headerMap := make(map[string][]string)
		for key, vals := range resp.Header {
			headerMap[key] = vals
		}

		httpErr := newHTTPResponseError(resp.StatusCode, resp.Status, bodyBytes, headerMap, "")
		if hooks != nil && hooks.PostResponseError != nil {
			anyResp := toAnyResponse(fetchResp)
			if replacementErr := hooks.PostResponseError(requestURL, req, httpErr, anyResp); replacementErr != nil {
				return nil, replacementErr
			}
		}
		return nil, httpErr
	}

	// Apply post-response success hook
	if hooks != nil && hooks.PostResponseSuccess != nil {
		anyResp := toAnyResponse(fetchResp)
		modified := hooks.PostResponseSuccess(requestURL, req, anyResp)
		if modified != nil {
			fetchResp = fromAnyResponse[T](modified)
		}
	}

	return fetchResp, nil
}

// toAnyResponse converts a typed FetchResponse to FetchResponse[any].
func toAnyResponse[T any](resp *FetchResponse[T]) *FetchResponse[any] {
	return &FetchResponse[any]{
		StatusCode: resp.StatusCode,
		Status:     resp.Status,
		Headers:    resp.Headers,
		Data:       resp.Data,
		IsJSON:     resp.IsJSON,
		BodyRaw:    resp.BodyRaw,
		OK:         resp.OK,
	}
}

// fromAnyResponse converts a FetchResponse[any] back to a typed FetchResponse.
func fromAnyResponse[T any](resp *FetchResponse[any]) *FetchResponse[T] {
	result := &FetchResponse[T]{
		StatusCode: resp.StatusCode,
		Status:     resp.Status,
		Headers:    resp.Headers,
		IsJSON:     resp.IsJSON,
		BodyRaw:    resp.BodyRaw,
		OK:         resp.OK,
	}
	if data, ok := resp.Data.(T); ok {
		result.Data = data
	}
	return result
}

// Get is a convenience function for making GET requests.
func Get[T any](ctx context.Context, client *Client, url string, opts *RequestOptions) (*FetchResponse[T], error) {
	return Fetch[T](ctx, client, http.MethodGet, url, nil, opts)
}

// Post is a convenience function for making POST requests.
func Post[T any](ctx context.Context, client *Client, url string, body any, opts *RequestOptions) (*FetchResponse[T], error) {
	return Fetch[T](ctx, client, http.MethodPost, url, body, opts)
}

// Put is a convenience function for making PUT requests.
func Put[T any](ctx context.Context, client *Client, url string, body any, opts *RequestOptions) (*FetchResponse[T], error) {
	return Fetch[T](ctx, client, http.MethodPut, url, body, opts)
}

// Patch is a convenience function for making PATCH requests.
func Patch[T any](ctx context.Context, client *Client, url string, body any, opts *RequestOptions) (*FetchResponse[T], error) {
	return Fetch[T](ctx, client, http.MethodPatch, url, body, opts)
}

// Delete is a convenience function for making DELETE requests.
func Delete[T any](ctx context.Context, client *Client, url string, opts *RequestOptions) (*FetchResponse[T], error) {
	return Fetch[T](ctx, client, http.MethodDelete, url, nil, opts)
}

// SimpleGet is a convenience function for making a GET request with no typed response.
func SimpleGet(ctx context.Context, client *Client, url string, opts *RequestOptions) (*FetchResponse[any], error) {
	return Fetch[any](ctx, client, http.MethodGet, url, nil, opts)
}

// SimplePost is a convenience function for making a POST request with no typed response.
func SimplePost(ctx context.Context, client *Client, url string, body any, opts *RequestOptions) (*FetchResponse[any], error) {
	return Fetch[any](ctx, client, http.MethodPost, url, body, opts)
}

// fetchWithTimeout creates a function that applies timeout to an HTTP request.
// This is used internally as part of the request pipeline.
func fetchWithTimeout(timeout time.Duration) func(context.Context, func(context.Context) (any, error)) (any, error) {
	return func(ctx context.Context, fn func(context.Context) (any, error)) (any, error) {
		return ExecuteWithTimeout(ctx, timeout, fn)
	}
}
