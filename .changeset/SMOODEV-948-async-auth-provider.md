---
'@smooai/fetch': patch
---

SMOODEV-948: Async auth-token provider across TS, Python, Rust, Go. Adds a first-class hook that's invoked before every request to mint / refresh an auth token (sync or async), with the resulting `Authorization` header injected using a configurable scheme (default `Bearer`). Mirrors the existing .NET `AuthTokenProvider` delegate.
