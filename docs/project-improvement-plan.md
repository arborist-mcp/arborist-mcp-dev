# Project Improvement Plan

This checklist captures the current project health review and a proposed
sequence for small, reviewable improvements. Keep items scoped enough that each
completed item can land in its own commit unless two changes are inseparable.

## Current Signals

- Public protocol metadata is healthy: `python scripts/tool_catalog.py --check`
  passes and the checked-in catalog matches the generated manifest.
- Version metadata is healthy: `python scripts/version_consistency.py` passes.
- The largest production files are still large enough to slow review:
  `python/arborist_mcp/gateway.py` is now about 1686 lines after the first
  protocol-helper splits, and
  `crates/arborist-py/src/lib.rs` is about 990 lines.
- The README already names the main strategic gaps: Rust module splits, PyO3
  wrapper repetition, durable schema migrations, full C++ grammar support,
  watch mode, benchmarks, fuzz/property tests, and cancellation controls.
- There are no explicit `TODO`, `FIXME`, `HACK`, or `XXX` markers in the tracked
  source and docs.

## Priority Checklist

### P0: Keep Existing Contracts Healthy

- [x] Run the documented inner loop and record any failures before larger work:
  `.\scripts\test.ps1 -Suite inner-loop`.
- [x] Add missing regression tests for any reproducible failure found during
  the inner loop. No failures were found in the current run.
- [ ] Keep `docs/tool-catalog.json` synchronized whenever tool schemas or
  result schemas change.
- [ ] Keep `README.md`, `docs/protocol.md`, and `docs/tools.md` synchronized
  with any protocol-facing change.

### P1: Small Reliability And Maintainability Fixes

- [x] Make the gateway-suite manifest helper expose the same basic CLI
  ergonomics as the Python-suite manifest helper, including descriptions or
  plan output if useful.
- [ ] Reduce duplicated protocol error response construction in
  `python/arborist_mcp/gateway.py`.
- [x] Move gateway resource handling into a focused helper module while keeping
  `gateway.py` as transport glue.
- [x] Move gateway tool-call dispatch helpers into a focused module without
  changing public response shapes.
- [x] Move MCP initialize and initialized handling into a focused helper module
  without changing core loading or response shapes.
- [ ] Introduce shared PyO3 argument/context structs where wrappers repeatedly
  validate the same `workspace_root`, `file_path`, `index_db_path`, `source`,
  `symbol_path`, `direction`, `max_depth`, and `max_nodes` patterns.

### P2: Core Architecture Improvements

- [ ] Continue splitting large Rust surfaces along existing module boundaries:
  `patching.rs`, `symbols.rs`, `model.rs`, and test modules should remain
  focused facades over submodules.
- [ ] Preserve live-VFS and persisted-index parity by adding paired tests when
  changing read/list/search/trace behavior.
- [ ] Add a durable SQLite migration path beyond the current fail-closed schema
  version gate.
- [ ] Add timeout/cancellation boundaries for large workspace scans, broad raw
  Tree-sitter queries, and trace/neighborhood expansion.
- [ ] Add benchmark baselines for index rebuild, refresh, trace, list, search,
  and patch validation.

### P3: New Feature Opportunities

- [ ] Add watch mode that refreshes registered symbol indexes when files change.
- [ ] Add full C++ support with `tree-sitter-cpp` instead of routing `.hpp` and
  `.hh` through the C grammar.
- [ ] Add symbol rename or guided multi-file edit previews using the existing
  symbol graph and patch validation machinery.
- [ ] Add richer impact summaries for trace-backed patch validation, including
  changed callers/callees and bounded affected symbol counts.
- [ ] Add optional SARIF or JSON diagnostics export for CI integrations.

### P4: Testing And Hardening

- [ ] Add property tests for path normalization, byte/position conversion, edit
  ordering, and VFS commit/discard idempotence.
- [ ] Add fuzz targets for JSON request validation, Tree-sitter query limits,
  patch replacement boundaries, and persisted-index loading.
- [ ] Add benchmark regression thresholds once local benchmark variance is
  understood.
- [ ] Add cross-platform smoke coverage for repo-root gateway startup and
  package-installed gateway startup.

## Suggested Commit Sequence

1. `docs(project): add improvement plan`
2. `test(gateway): improve gateway suite manifest coverage`
3. `refactor(gateway): extract resource handlers`
4. `refactor(gateway): extract tool dispatch helpers`
5. `refactor(gateway): extract lifecycle handlers`
6. `refactor(pyo3): consolidate shared wrapper arguments`
7. `feat(index): add schema migration scaffolding`
8. `feat(index): add watch-mode refresh loop`
9. `feat(core): add cpp grammar support`

The first four items are intentionally low-risk and give quick maintainability
wins before deeper Rust and protocol work.
