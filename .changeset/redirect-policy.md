---
'@smooai/fetch': minor
---

Make redirect handling configurable in all five languages

Redirects were followed unconditionally everywhere, and TypeScript went further:
`merge({}, init, { redirect: 'follow' })` put the literal last, so a caller
passing `redirect: 'manual'` had it **silently overwritten**. Python hardcoded
`follow_redirects=True` into the httpx kwargs; Rust, Go and .NET set nothing and
inherited platform defaults that follow up to 10 hops.

That is a security gap, not just an ergonomic one. A caller who resolves a
hostname and checks it against an SSRF allowlist has that guard defeated by a 302
to an internal address, because the check was performed on the original host. And
RFC 8461 forbids following redirects when fetching an MTA-STS policy.

- **TypeScript** — `redirect` is honoured (defaults first, caller last)
- **Python** — `FetchOptions.follow_redirects`
- **Rust** — `RequestInit.follow_redirects: Option<bool>` (`None` inherits, so a
  client-level default survives a per-request `..Default::default()`)
- **Go** — `ClientBuilder.WithFollowRedirects`, applied to a caller-supplied
  `*http.Client` too
- **.NET** — `SmooFetchOptions.FollowRedirects` / `WithFollowRedirects`

Honouring the option was not sufficient on its own: in TS, Rust, Go and .NET a
3xx is neither "ok" nor "redirected", so it was raised as an error and the option
was undone a line later. Each now returns a deliberately-unfollowed 3xx as an
ordinary response. Defaults are unchanged — everything still follows unless a
caller says otherwise.
