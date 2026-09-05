
This document is the execution-facing companion to [multi-language-support-design.md](multi-language-support-design.md) and [language-gaps.md](language-gaps.md. It sequences the rollout and pins the Git/CI rhythm that agents must follow.

## Sequence

1. **Stage A — close existing-language gaps.** Make Rust/Go/JavaScript/TypeScript/TSX behavior match the conservative slices advertised in the design noteand tool guide; fill regression fixtures for any public-output gaps; then bump per-language `analysis_revision` strings when behavior changes.
2. **Onboarding template.** Keep [language-onboarding.md](language-onboarding.md) current; it is the standard checklist every new language batch follows.
3. **Phase B/C/D — add languages in batches.**
   - Batch B: PHP, Swift, Lua (minimal parse/skeleton/index/dependency(conservative)/trace(conservative)/targeting; validation only when real).
   - Batch C: Ruby, Shell, Bash (same MVP shape).
   - Batch D: Zig, Haskell, Elixir (same MVP shape).
   - Re-evaluate each grammar tree-sitter ABI/quality before each batch; defer a language if grammar is unavailable/incompatible.

Each batch must deliver per language: `docs/tools.md` capability table row, `docs/language-gaps.md` row, fixtures under `crates/arborist-core/tests/fixtures/languages/<lang>/`, adapter/descriptor registration, and catalog/provenance sync. No batch may ship a claimed capability without tests.

## Git and CI cadence (enforced)

**Commits remain small and one-concern each.** Use conventional scopes (`core`, `gateway`, `pyo3`, `symbols`, `vfs`, `query`, `patching`, `tests`, `docs`). Typical units: dependency+lockfile; language-id+registry; semantic module; index extractor; dependency resolver; patch targeting/validation; fixtures/tests; docs/catalog.

**Every ~10–15 commits push once and wait for full CI.** If a feature branch is available, push that branch; do not push directly to `main` unless the repo workflow already does so.

Before pushing a checkpoint:
- `cargo fmt --check` (use `cargo fmt --all` before committing draft changes).
- Rust sanity: `cargo test -p arborist-core --locked`; broader `cargo test --locked` when py/gateway affected.
- Catalog sanity when touched: `python scripts/tool_catalog.py --check` (and regenerate with `python -m arborist_mcp.gateway --dump-tool-catalog` if output changed).
- `git diff --check`.

After pushing:
- Wait for the full CI run (~ten-minute gate) before continuing, unless CI is already red from an independent/reported failure.
- Fix CI failures in one or two small `fix(...)` commits until green; if fixes accumulate to 3+ commits without green, push againand share the failing logs rather than silently continuing.
- Resume the next 10–15-commit slice only after CI is green.
- At the end of every stage/batch, do a `batch-final` checkpoint: all commits pushed, CI green, catalog/docs/provenance in sync,and optional review/merge per the repo existing convention.

## Non-goals

- Do not rewrite generated artifacts (`target/`, `.venv/`, `_arborist_core.*`) or user work-tree changes not related to the language rollout.
- Do not add capability flags until a real adapter slice exists; avoid lying about `PATCH_VALIDATION` or conservative trace coverage.
- Do not duplicate language logic in the PyO3/gateway layer; keep business logic in `arborist-core`.