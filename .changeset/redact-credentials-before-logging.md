---
'@smooai/fetch': patch
---

SMOODEV-2716: Redact credentials from everything the client logs. `@smooai/logger` performs no redaction of its own, so the `Authorization` header, the raw query string, the full URL in the log message and the request body were all reaching CloudWatch in plaintext on every request. A new internal `redact` module scrubs headers, query strings, URLs (including userinfo passwords) and bodies (url-encoded, JSON string and object forms) before they are handed to the logger; the request sent on the wire is unchanged. The Rust client gets the same treatment for the URL on its `Sending HTTP request` tracing event. Both suites load the shared cases in `spec/redaction-corpus.json`. Python, Go and .NET log nothing about a request and so are unaffected.
