---
'@smooai/fetch': patch
---

Fix the release pipeline so shipped artifacts carry the version they claim, and give the Go module a resolvable path.

`version:sync` ran _after_ `changeset publish`, mutating manifests in the CI workspace that were never committed — so every git tag shipped stale version constants (`go/fetch/v3.3.10` contained `const Version = "2.1.2"`) and `cargo publish --allow-dirty` existed only to tolerate the dirt. The sync now runs inside `changeset version`, so the bumped manifests land in the release commit; `--allow-dirty` is gone; and `node scripts/sync-versions.mjs --check` runs in CI as a guard that fails loudly on any skew, including a pattern that stopped matching.

The Go module path gains the `/v3` major suffix Go requires above v1. Import `github.com/SmooAI/fetch/go/fetch/v3` (package identifier is still `fetch`); tags through `go/fetch/v3.4.0` predate the suffix and do not resolve. The suffix is derived from `package.json`'s major and is covered by the same guard, so a future major bump cannot leave it behind.
