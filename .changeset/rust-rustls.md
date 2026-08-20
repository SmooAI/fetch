---
'@smooai/fetch': patch
---

The Rust crate `smooai-fetch` now uses rustls instead of native-tls. `reqwest`'s `default-tls` pulled in `openssl-sys`, which needs a system OpenSSL and its headers at build time — a cross-compile and container-image hazard for a library consumed across the platform. `openssl-sys` and `native-tls` are gone from the lockfile. `http2` and `charset` are re-enabled explicitly, because disabling reqwest's default features would otherwise have silently downgraded every consumer to HTTP/1.1.
