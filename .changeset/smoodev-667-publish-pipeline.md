---
'@smooai/fetch': patch
---

SMOODEV-667: Fix release pipeline so PyPI + crates.io + NuGet actually publish. `pnpm build` produces a Python wheel at the pre-sync version (the Cargo/pyproject bumps happen later, inside `ci:publish`), so the publish step was trying to re-upload the stale wheel and getting rejected. Clean `dist/` before `uv run poe publish` so only the freshly-built version ships. Drop `--locked` from the cargo publish step because sync-versions only updates `Cargo.toml` (not `Cargo.lock`), which would trip `--locked` as soon as crates.io is reached. Net effect: `SmooAI.Fetch` NuGet package publishes for the first time; PyPI advances from the stalled 3.0.0.
