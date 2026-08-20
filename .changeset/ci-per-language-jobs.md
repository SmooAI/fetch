---
'@smooai/fetch': patch
---

CI runs one job per language instead of a single serial `validate` job, so a failure in one port no longer hides the verdict of the other four. Two test-visibility gaps closed alongside it: `vitest --passWithNoTests` is gone (an empty TypeScript suite went green), and the Rust lane now runs `--all-features`, which is what actually compiles `tests/trace_propagation_tests.rs` — three real trace-propagation tests sat behind `#![cfg(feature = "otel")]` and reported "0 passed; ok" to a bare `cargo test`.
