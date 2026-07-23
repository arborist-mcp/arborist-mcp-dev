# Project Improvement Plan

This checklist captures the current project health review and a proposed
sequence for small, reviewable improvements. Keep items scoped enough that each
completed item can land in its own commit unless two changes are inseparable.

## Current Signals

- Public protocol metadata is healthy: `python scripts/tool_catalog.py --check`
  passes and the checked-in catalog matches the generated manifest.
- Version metadata is healthy: `python scripts/version_consistency.py` passes.
- The gateway facade is now about 300 lines after symbol-query, patch,
  and trace route mixins join the earlier index/VFS/parameter helpers. The
  PyO3 root facade remains a thin registration surface over domain bindings.
- The remaining strategic gaps are deeper Rust module splits, fuller C++
  language-aware resolution, property testing beyond the existing fuzz targets,
  and cancellation that can interrupt individual native parse or query
  operations.
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

- [x] Extract gateway symbol query, patch/validation, and trace route
  handlers into focused mixins without changing public response shapes.

- [x] Make the gateway-suite manifest helper expose the same basic CLI
  ergonomics as the Python-suite manifest helper, including descriptions or
  plan output if useful.
- [x] Reduce duplicated protocol error response construction in
  `python/arborist_mcp/gateway.py`.
- [x] Centralize unexpected-parameter validation across MCP helper modules and
  legacy gateway routes.
- [x] Move gateway resource handling into a focused helper module while keeping
  `gateway.py` as transport glue.
- [x] Move gateway tool-call dispatch helpers into a focused module without
  changing public response shapes.
- [x] Move MCP initialize and initialized handling into a focused helper module
  without changing core loading or response shapes.
- [x] Move batch tool dispatch into a focused helper module without changing
  batch validation or per-tool response shapes.
- [x] Move gateway parameter validation plus index and VFS route adapters into
  focused mixins without changing handlers, error responses, or tool metadata.
- [x] Introduce a shared PyO3 symbol-query context for the repeated
  `workspace_root`, `file_path`, `index_db_path`, and `source` patterns across
  list, read, search, and trace wrappers.
- [x] Extend the shared PyO3 context to patch-validation selector and position
  wrappers while preserving their source/index/VFS dispatch branches.
- [x] Apply shared context structs to the remaining patch AST, index, and VFS
  wrappers where their repeated parameter patterns warrant it.
- [x] Introduce a shared PyO3 source-position helper as the first small step
  toward consolidated wrapper arguments.
- [x] Group PyO3 neighborhood/query/patch context `max_depth` and `max_nodes`
  arguments behind a shared bounds object for internal wrapper calls.
- [x] Split PyO3 public bindings by VFS, index, patch, validation, source-query,
  and symbol-query domains, and add a native registration contract test for all
  gateway-referenced core methods.

### P2: Core Architecture Improvements

- [ ] Continue splitting large Rust surfaces along existing module boundaries:
  `patching.rs`, `symbols.rs`, `model.rs`, and test modules should remain
  focused facades over submodules.
- [ ] Preserve live-VFS and persisted-index parity by adding paired tests when
  changing read/list/search/trace behavior.
- [x] Detect source files added after an index build during health inspection
  and persisted queries so incomplete indexes do not silently appear healthy.
- [x] Cross-check indexed-file metadata against persisted file-state rows so
  damaged counts cannot leak into query results or healthy diagnostics.
- [x] Validate persisted symbol and file-state paths against the indexed
  workspace and supported source types before reading or refreshing them.
- [x] Make current-schema validation cover every persisted column and primary
  key layout, and keep query/inspection connections read-only.
- [x] Add durable SQLite v1-v3-to-v4 migration paths with transactional schema
  updates, persisted direct-call arity metadata, and a fail-closed public
  migration operation.
- [x] Centralize symbol-index migration recommendations behind a focused Rust
  module so future migration actions are not scattered through inspection code.
- [x] Type symbol-index migration recommendation actions internally while
  preserving the current public `none` / `rebuild` / `manual` response shape.
- [x] Route unsupported schema-version recommendations through a single
  decision point so future version-specific migrations can be added in one
  place.
- [x] Add cooperative timeout boundaries for large workspace scans, broad raw
  Tree-sitter queries, and trace/neighborhood expansion.
- [x] Add optional cooperative timeout budgets to workspace scans and persisted
  index rebuild/refresh operations.
- [x] Add optional cooperative timeout budgets to direct trace graph and
  neighborhood expansion while preserving existing call signatures.
- [x] Make raw Tree-sitter query timeout budgets configurable while preserving
  the existing default.
- [x] Add cooperative timeout coverage to persisted index health freshness and
  unindexed-file scans.
- [x] Add benchmark baselines for index rebuild, refresh, trace, list, search,
  and patch validation.

### P3: New Feature Opportunities

- [x] Add watch mode that refreshes registered symbol indexes when files change.
- [x] Expose a registered-index incremental refresh primitive for polling and
  watch integrations.
- [x] Expose a full-workspace incremental refresh operation that reuses the
  existing fingerprint-based rebuild path as the foundation for watch mode.
- [x] Add a fail-closed polling console watch command for a specified persisted
  index, including a one-shot reconciliation mode for CI and supervisor probes.
- [x] Add a no-write `--dry-run` index-watch mode that reports planned refresh
  or migration actions for single-index and manifest-based checks.
- [x] Add an index-watch `--check` mode that turns no-write health diagnostics
  into a CI-friendly success or failure exit status.
- [x] Route C++ source and header extensions through `tree-sitter-cpp` while
  preserving C-family free-function and header/source graph behavior.
- [x] Route common C++ template and inline implementation extensions (`.tpp`,
  `.tcc`, `.ipp`, and `.inl`) through workspace scans and persisted indexes.
- [x] Model named-namespace free functions, class definitions, and named class
  methods, including class out-of-line definitions plus explicit/defaulted/deleted
  constructors/destructors, in C++ skeletons, indexes, traces, patch targets,
  and raw-query owner metadata.
- [x] Model named function and class-method templates in C++ skeletons,
  indexes, traces, and raw-query owner metadata while preserving template
  declaration text.
- [x] Model basic C++ operator and conversion methods with stable operator-name
  paths and overload-aware callable identities.
- [x] Extend C++ semantic support beyond non-type template parameter binding and
  explicit function/class/method specializations to overload-aware callable
  identities across skeletons, indexes, traces, patches, and raw-query owner
  metadata.
- [x] Resolve direct, unqualified C++ calls against overload candidates by
  argument count in live and persisted symbol graphs.
- [x] Resolve namespace-qualified C++ calls through enclosing namespaces before
  filtering overloads in live and persisted symbol graphs.
- [x] Trace explicit C++ template calls through the existing direct-call graph
  resolution path in live and persisted indexes.
- [x] Trace dependent C++ member-template calls such as
  `this->template method<T>(...)` through enclosing-class overload resolution
  in live and persisted indexes.
- [x] Prefer indexed explicit C++ function and member-template specializations
  for explicit calls, with primary-template fallback when no specialization is
  indexed.
- [x] Respect lvalue `this` receivers when selecting C++ `&`, `const &`, and
  `&&` member overloads across workspace, persisted-index, and VFS queries.
- [x] Recognize `std::move(*this)` as an explicit C++ rvalue self receiver and
  select matching `&&` member overloads across workspace, persisted-index, and
  VFS queries without guessing arbitrary object types.
- [x] Recognize explicit `static_cast<T&&>(*this)` C++ self receivers for the
  same rvalue member-overload selection across workspace, persisted-index, and
  VFS queries.
- [x] Expand C++ namespace aliases for direct qualified calls in live and
  persisted symbol graphs.
- [x] Resolve direct qualified C++ calls through `using` declarations to their
  imported callables in live and persisted symbol graphs.
- [x] Resolve direct unqualified C++ calls through scoped `using` declarations
  in live and persisted symbol graphs.
- [x] Resolve direct unqualified C++ calls through scoped `using namespace`
  imports in live and persisted symbol graphs.
- [x] Expand namespace aliases used as scoped C++ `using namespace` import
  targets before direct-call overload filtering.
- [x] Verify explicit C++ class/method specializations across skeletons, live
  and persisted traces, and semantic patch targets.
- [x] Treat non-type C++ template parameters as local bindings during patch
  validation and reference tracing.
- [x] Add symbol rename or guided multi-file edit previews using the existing
  symbol graph and patch validation machinery.
- [x] Add direct caller/callee change summaries and distinct affected-symbol
  counts for live and persisted trace-backed patch validation.
- [x] Add optional SARIF 2.1.0 diagnostics export for patch validation CI integrations.

### P4: Testing And Hardening

- [x] Add generated invariant coverage for path normalization, byte/position
  conversion, edit ordering, and VFS commit/discard idempotence, including
  multi-byte edit cases.
- [x] Add fuzz targets for JSON request validation, Tree-sitter query limits,
  patch replacement boundaries, and persisted-index loading; type-check every
  fuzz manifest in the full local validation profile.
- [x] Add benchmark regression thresholds once local benchmark variance is
  understood.
- [x] Add cross-platform smoke coverage for repo-root gateway startup and
  package-installed gateway startup.

## Suggested Commit Sequence

1. `docs(project): add improvement plan`
2. `test(gateway): improve gateway suite manifest coverage`
3. `refactor(gateway): extract resource handlers`
4. `refactor(gateway): extract tool dispatch helpers`
5. `refactor(gateway): extract lifecycle handlers`
6. `refactor(gateway): share mcp param validation`
7. `refactor(gateway): extract batch tool dispatch`
8. `refactor(gateway): use shared error responses`
9. `refactor(pyo3): consolidate shared wrapper arguments`
10. `refactor(pyo3): group context bounds`
11. `perf(scripts): broaden benchmark workflows`
12. `ci(gateway): smoke installed console script`
13. `refactor(index): centralize migration recommendations`
14. `refactor(index): type migration recommendation actions`
15. `refactor(index): route schema version migration actions`
16. `feat(index): add schema migration scaffolding`
17. `test(scripts): add benchmark threshold coverage`
18. `feat(index): add watch-mode refresh loop`
19. `feat(core): add cpp grammar support`
20. `test(core): harden language helper invariants`

The first four items are intentionally low-risk and give quick maintainability
wins before deeper Rust and protocol work.
