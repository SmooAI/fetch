---
'@smooai/fetch': patch
---

SMOODEV-2716: Redact credential params (`client_secret`, `client_id`, tokens, etc.) from url-encoded request bodies before they reach any log record. The request/response logging stored the raw form body as an opaque string, so an OAuth `client_credentials` token exchange leaked its `client_secret` to CloudWatch in plaintext. `getRequestBody` now scrubs sensitive `x-www-form-urlencoded` params via the exported `redactFormCredentials`. TypeScript only for now — the Python/Rust/Go/.NET implementations need the same redaction as a parity follow-up.
