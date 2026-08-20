---
'@smooai/fetch': minor
---

The Go port gains a real response-validation entry point: `RequestOptions.Validate func(data any) []string`. Returning messages fails the request with a `*SchemaValidationError` carrying them, which `DefaultRetryOptions` already treats as non-retryable. Until now `SchemaValidationError` was a type the package defined, documented as "returned when response body validation fails", and never constructed — and the README's language matrix promised callers would get one. Go has no Standard Schema equivalent, so this is the seam rather than a bundled validator. Leaving `Validate` unset is behavior-identical.
