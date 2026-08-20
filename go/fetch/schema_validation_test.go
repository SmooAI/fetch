package fetch

import (
	"context"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"
)

type validationUser struct {
	ID    string `json:"id"`
	Email string `json:"email"`
}

func jsonServer(t *testing.T, body string) *httptest.Server {
	t.Helper()
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte(body))
	}))
	t.Cleanup(srv.Close)
	return srv
}

// A failing validator must surface as *SchemaValidationError. Before RequestOptions.Validate
// existed, that type was defined and documented as "returned when response body validation
// fails" and the library never constructed it — the README said Go gave you one on failure,
// and it never did.
func TestValidateFailureReturnsSchemaValidationError(t *testing.T) {
	srv := jsonServer(t, `{"id":"1","email":"not-an-email"}`)

	_, err := Get[validationUser](context.Background(), NewClientBuilder().WithNoRetry().Build(), srv.URL, &RequestOptions{
		Validate: func(data any) []string {
			user, ok := data.(validationUser)
			if !ok {
				return []string{"unexpected payload type"}
			}
			if user.Email == "not-an-email" {
				return []string{"email is not a valid address"}
			}
			return nil
		},
	})

	var validationErr *SchemaValidationError
	if !errors.As(err, &validationErr) {
		t.Fatalf("expected *SchemaValidationError, got %#v", err)
	}
	if len(validationErr.Errors) != 1 || validationErr.Errors[0] != "email is not a valid address" {
		t.Fatalf("validator messages did not reach the error: %v", validationErr.Errors)
	}
}

func TestValidatePassLeavesTheResponseAlone(t *testing.T) {
	srv := jsonServer(t, `{"id":"1","email":"a@b.com"}`)

	resp, err := Get[validationUser](context.Background(), NewClientBuilder().WithNoRetry().Build(), srv.URL, &RequestOptions{
		Validate: func(any) []string { return nil },
	})
	if err != nil {
		t.Fatalf("expected a passing validator to be transparent, got %v", err)
	}
	if resp.Data.Email != "a@b.com" {
		t.Fatalf("expected the decoded body through, got %+v", resp.Data)
	}
}

// The retry branch for SchemaValidationError in DefaultRetryOptions was dead code
// until the error could actually be produced. A body that violates its contract
// will violate it again, so the request must abort rather than burn the budget.
func TestValidationFailureIsNotRetried(t *testing.T) {
	var attempts int
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		attempts++
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte(`{"id":"1","email":"bad"}`))
	}))
	t.Cleanup(srv.Close)

	_, err := Get[validationUser](context.Background(), NewClientBuilder().WithRetry(&DefaultRetryOptions).Build(), srv.URL, &RequestOptions{
		Validate: func(any) []string { return []string{"bad email"} },
	})

	var validationErr *SchemaValidationError
	if !errors.As(err, &validationErr) {
		t.Fatalf("expected *SchemaValidationError, got %#v", err)
	}
	if attempts != 1 {
		t.Fatalf("validation failure must not be retried: server saw %d attempts", attempts)
	}
}

func TestNoValidatorIsBehaviorIdentical(t *testing.T) {
	srv := jsonServer(t, `{"id":"1","email":"whatever"}`)

	resp, err := Get[validationUser](context.Background(), NewClientBuilder().WithNoRetry().Build(), srv.URL, nil)
	if err != nil {
		t.Fatalf("unset Validate must not change behavior, got %v", err)
	}
	if resp.Data.Email != "whatever" {
		t.Fatalf("expected the decoded body through, got %+v", resp.Data)
	}
}
