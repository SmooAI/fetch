package fetch

import "net/http"

// FetchResponse wraps an HTTP response with parsed body data and metadata.
type FetchResponse[T any] struct {
	// StatusCode is the HTTP status code.
	StatusCode int
	// Status is the full status string (e.g. "200 OK").
	Status string
	// Headers are the response headers.
	Headers http.Header
	// Data is the parsed response body. For JSON responses this is the decoded value;
	// for non-JSON it is the zero value of T.
	Data T
	// IsJSON indicates whether the response body was parsed as JSON.
	IsJSON bool
	// BodyRaw is the raw response body bytes.
	BodyRaw []byte
	// OK is true if the status code is in the 2xx range.
	OK bool
}
