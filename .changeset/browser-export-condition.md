---
'@smooai/fetch': minor
---

**Add top-level `browser` export condition**

`@smooai/fetch` already shipped a browser-safe build under `./browser`, but the top-level `.` entry had no `browser` condition in the exports map. Browser bundlers (Vite, webpack with `target: 'web'`, esbuild with `platform: 'browser'`) therefore resolved `import fetch from '@smooai/fetch'` to the Node entry, pulling `@smooai/logger` + `rotating-file-stream` + other Node-only dependencies into the browser bundle.

Adding the `browser` condition on `.` means consumers can now do:

```ts
import fetch from '@smooai/fetch';
```

…and the bundler automatically picks the browser-safe dist when building for a browser target. No aliasing, no explicit `/browser` subpath import required.

Consumers that were aliasing `@smooai/fetch` → `@smooai/fetch/browser/index` as a workaround (e.g. `@smooai/config`'s tsup build) can drop that alias on upgrade.
