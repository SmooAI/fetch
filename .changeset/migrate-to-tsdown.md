---
'@smooai/fetch': patch
---

Migrate build tooling from tsup to tsdown — faster, oxc-based, drop-in replacement. The `esbuild-plugin-alias` shim used to swap `@smooai/logger` Node entries for browser variants is replaced with `@rollup/plugin-alias` (rolldown-compatible). Output extensions shift from `.js`/`.mjs`/`.d.ts` to `.cjs`/`.mjs`/`.d.cts`/`.d.mts` (tsdown defaults); the `exports` map is updated to match. No public API change.
