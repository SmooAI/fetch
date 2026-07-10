---
'@smooai/fetch': minor
---

Add an optional connect timeout (`connectTimeoutMs` / `FetchBuilder.withConnectTimeout`) that bounds only the connection-establishment phase, so a black-holed connect (e.g. a SYN to a dead pod IP still lingering in a ClusterIP's iptables) fails fast and retry can land on a live endpoint, instead of stalling until the whole-request timeout. Node only (via a lazily-loaded undici `Agent` dispatcher); ignored in browser/worker builds. Default-off — unset preserves the previous behavior exactly. Parity with the Rust `with_connect_timeout` (SmooAI/fetch#88, SMOODEV-2513).
