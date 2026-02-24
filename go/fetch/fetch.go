// Package fetch provides a resilient HTTP client with retries, timeouts,
// rate limiting, and circuit breaking.
//
// It mirrors the API of the @smooai/fetch TypeScript library, adapted for
// Go idioms. The main entry points are the Fetch, Get, Post, Put, Patch,
// and Delete generic functions, combined with the Client (configured via
// NewClientBuilder).
//
// Example:
//
//	client := fetch.NewClientBuilder().
//	    WithTimeout(10 * time.Second).
//	    WithRetry(&fetch.DefaultRetryOptions).
//	    WithRateLimit(10, time.Minute).
//	    Build()
//
//	type User struct {
//	    ID   string `json:"id"`
//	    Name string `json:"name"`
//	}
//
//	resp, err := fetch.Get[User](ctx, client, "https://api.example.com/users/1", nil)
//	if err != nil {
//	    log.Fatal(err)
//	}
//	fmt.Println(resp.Data.Name)
package fetch
