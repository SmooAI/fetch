---
'@smooai/fetch': patch
---

Two places where a green result meant nothing.

The Go test lane ran `go test` without `-count=1`. Go's test cache does not invalidate on a fixture read from _outside_ the package directory, and the connect-timeout suite loads `spec/connect-timeout-corpus.json` from the repo root — so a deliberately corrupted corpus still returned `ok (cached)`.

The release workflow gated the PyPI, crates.io, Go-tag and NuGet publishes on `steps.changesets.outputs.published == 'true'` — i.e. on the npm publish succeeding in that same run. A run dying after npm left the follow-up run with no changesets to consume, `published == 'false'`, every remaining step skipped, and a green check for a release that published nothing. Those steps now gate on being a publish run and are individually idempotent, a `concurrency` group stops the workflow racing itself, and a final step fails the run if the released version is not live on npm, PyPI, crates.io and the Go tag.
