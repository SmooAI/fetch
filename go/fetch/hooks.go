package fetch

import "net/http"

// PreRequestHook is called before each HTTP request.
// It receives the URL and the prepared *http.Request.
// It may return a modified URL and request, or ("", nil) to leave them unchanged.
type PreRequestHook func(url string, req *http.Request) (newURL string, newReq *http.Request)

// PostResponseSuccessHook is called after a successful (2xx) response.
// It receives the original URL, request, and parsed response.
// It may return a modified response, or nil to keep the original.
type PostResponseSuccessHook func(url string, req *http.Request, resp *FetchResponse[any]) *FetchResponse[any]

// PostResponseErrorHook is called when the request fails.
// It receives the original URL, request, error, and optionally the parsed response.
// It may return a replacement error, or nil to keep the original.
type PostResponseErrorHook func(url string, req *http.Request, err error, resp *FetchResponse[any]) error
