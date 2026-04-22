---
'@smooai/fetch': minor
---

SMOODEV-627: Close TS→Rust/Go drift on retry options and builder surface. Rust + Go `RetryOptions` now match TS: `on_rejection` / `OnRejection` callback (decisions: Retry with custom delay, Abort, Skip, Default), plus `fast_first` / `FastFirst` for zero-delay first retry. Go also gets `WithRateLimitRetry(opts)` (configurable per-client rate-limit retry) and `WithContainerOptions(FetchContainerOptions)` batch setter mirroring TS's container-options ergonomics. Also gitignore `.smooai-logs/` so the pre-commit hook stops committing ephemeral test logs.
