---
'@smooai/fetch': patch
---

Add explicit `./browser` subpath export so `import fetch from '@smooai/fetch/browser'` resolves without the trailing `/index`. The existing `./browser/*` wildcard doesn't match the bare `./browser` specifier per the Node.js exports spec — the `*` requires at least one character — so consumers previously had to write `@smooai/fetch/browser/index`, which contradicts the documented API. Adds a dedicated entry pointing at `dist/browser/index.{mjs,js,d.ts}`. The wildcard form continues to work for any future browser-side subpaths.
