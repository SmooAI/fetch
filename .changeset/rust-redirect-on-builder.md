---
'@smooai/fetch': patch
---

Rust: move the redirect option off `RequestInit` and onto the builder

3.7.0 added `RequestInit.follow_redirects`. Adding a public field to a struct
consumers construct is a **breaking change** in Rust semver, shipped under a
minor — building the SmooAI monorepo against 3.7.0 fails with
`error[E0063]: missing field` in **129 exhaustive constructors** across ~40 crates.

The option is now `FetchBuilder::with_follow_redirects`, matching the shape Go
and .NET already use, and `RequestInit` is back to its 3.6.2 fields. Redirect
policy is per-Client in reqwest anyway, so the builder was the right home from
the start.

`client::fetch` keeps its exact signature; a new `client::fetch_with_redirect_policy`
takes the extra argument, so nothing that compiled against 3.6.2 needs changing.

3.7.0 is yanked from crates.io. The other four languages were unaffected —
Python added a defaulted dataclass field, Go and .NET added builder methods.
