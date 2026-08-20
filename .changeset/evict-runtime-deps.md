---
'@smooai/fetch': patch
---

Drop two runtime dependencies from the published package.

`@faker-js/faker` — a test-data generator — was a **runtime** dependency, imported at module load, solely to build cosmetic names for the internal mollitia modules (`smooai-fetch-retry-blue-cat`). Those names only need to be unique within the process, so they come from a counter now.

`@standard-schema/utils` was declared but never imported by anything in `src/`; it reaches this package transitively through `@smooai/utils`, which declares it itself.

The remaining runtime dependencies (`mollitia`, `lodash.merge`, `@smooai/logger`, `@smooai/utils`, `@standard-schema/spec`) are each load-bearing and stay.
