---
'@smooai/fetch': patch
---

SMOODEV-928: Bump `@smooai/logger` to `^4.1.4` and `@smooai/utils` to `^1.3.3`. Picks up the ESM `__filename` TDZ fix from logger 4.1.4 across the runtime dep graph (utils itself was on logger 3.x prior to 1.3.3). Also drops the deprecated `baseUrl: "./"` from tsconfig (TS 5.9+/6.x emit TS5101 with `ignoreDeprecations: "5.0"`); fetch has no `paths` entries so this is a no-op for type resolution.
