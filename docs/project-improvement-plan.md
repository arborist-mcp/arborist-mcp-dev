# Project Improvement Plan

This checklist captures the current project health review and a proposed
sequence for small, reviewable improvements. Keep items scoped enough that each
completed item can land in its own commit unless two changes are inseparable.

## Current Signals

- Public protocol metadata is healthy: `python scripts/tool_catalog.py --check`
  passes and the checked-in catalog matches the generated manifest.
- Version metadata is healthy: `python scripts/version_consistency.py` passes.
- The gateway facade is now about 240 lines after symbol-query, patch,
  and trace route mixins join the earlier index/VFS/parameter helpers. The
  PyO3 root facade remains a thin registration surface over domain bindings.
- Multi-language work now has a Phase 1 registry: descriptors, capabilities,
  extension routing, grammar selection, and supported-language reporting for
  Python, C, and C++ share one source of truth while retaining existing paths.
- Property coverage now extends beyond the original fuzz targets to language
  position/path/VFS invariants, patch-preview unified-diff structure, and
  workspace edit preview diff invariants. Model-layer property coverage now
  includes symbol base-name extraction, kind-rank tiers, point ordering,
  shared validation helpers, semantic-skeleton path/symbol alignment,
  query-capture owner pairing, read/trace and neighborhood/list/search
  context alignment, index-health freshness conservation, evidence-replay
  summary consistency, and trace-validation status/gate pairings.
  Cooperative deadline coverage now includes full-rebuild and incremental
  index persistence transactions.
- There are no explicit `TODO`, `FIXME`, `HACK`, or `XXX` markers in the tracked
  source and docs.

## Priority Checklist

### P0: Keep Existing Contracts Healthy

- [x] Run the documented inner loop and record any failures before larger work:
  `.\scripts\test.ps1 -Suite inner-loop`.
- [x] Add missing regression tests for any reproducible failure found during
  the inner loop. No failures were found in the current run.
- [x] Keep `docs/tool-catalog.json` synchronized whenever tool schemas or
  result schemas change.
- [x] Keep `README.md`, `docs/protocol.md`, and `docs/tools.md` synchronized
  with protocol-facing changes through catalog counts and required protocol
  reference checks in `scripts/tool_catalog.py --check`.

### P1: Small Reliability And Maintainability Fixes

- [x] Reject duplicate entries in the CI profile list so the generated
  GitHub Actions matrix cannot contain duplicate jobs.
- [x] Derive the stable fuzz-manifest check directly from `fuzz/Cargo.toml`
  so newly declared fuzz targets are checked automatically.
- [x] Make `scripts/tool_catalog.py --check` fail when README or the tool guide
  document stale total/category counts.
- [x] Cache Cargo registries and release build output in the cross-platform
  wheel workflow using OS- and lockfile-specific keys.
- [x] Extract gateway symbol query, patch/validation, and trace route
  handlers into focused mixins without changing public response shapes.
- [x] Split gateway symbol routes further into read/search/list mixins
  (`gateway_symbol_read_routes.py`, `gateway_symbol_search_routes.py`,
  `gateway_symbol_list_routes.py`) with a thin composition facade.
- [x] Split gateway patch routes into apply/preview and validation mixins
  (`gateway_patch_apply_routes.py`, `gateway_patch_validation_routes.py`).

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
- [x] Split PyO3 VFS bindings into focused `access`, `edits`, and `lifecycle`
  modules while retaining a registration-only facade, JSON edit validation,
  lifecycle deadlines, and native method compatibility.
- [x] Split PyO3 index bindings into focused `build_refresh`, `maintenance`,
  and `registry` modules while retaining a registration-only facade, shared scan
  limits, registry deadlines, and native method compatibility.
- [x] Split PyO3 patch bindings into focused `ast`, `virtual_patch`, `replay`,
  `diagnostics`, and `commit_validation` modules while retaining a registration-only
  `patch_bindings.rs` facade and the existing native method surface.
- [x] Split PyO3 patch-validation bindings into focused `trace`, `graph`,
  `neighborhood`, and `discovery` modules while retaining a registration-only
  `patch_validation.rs` facade and the existing semantic/position method surface.
- [x] Split PyO3 symbol-read bindings into focused `basic`, `context`,
  `neighborhood`, and `discovery` modules while retaining a registration-only
  `symbol_queries/read.rs` facade and source/index/VFS query parity.
- [x] Split PyO3 symbol-list and symbol-search bindings into focused `basic`,
  `context`, `neighborhood`, and `discovery` modules, retaining registration-only
  facades and the existing filter/source/index/VFS dispatch behavior.
- [x] Split PyO3 symbol-trace bindings into focused `graph` and `neighborhood`
  modules while retaining a registration-only facade and both symbol-path and
  position-based source/index/VFS dispatch behavior.
- [x] Split the PyO3 binding regression suite into focused JSON argument,
  core/index, patch application, patch validation, symbol query, VFS, and
  replay-validation modules while retaining shared setup helpers and exact
  test behavior.

### P2: Core Architecture Improvements

- [x] Continue splitting large Rust surfaces along existing module boundaries:
  `patching.rs`, `symbols.rs`, `model.rs`, and test modules now remain focused
  facades over dedicated submodules; continue extending the same boundary discipline
  as new core responsibilities are added.
- [x] Extract Python overload alias visibility and module-level rebinding
  tracking into a dedicated semantic helper module while retaining the existing
  overload identity behavior and deadline checks.
- [x] Split `tests/c_symbol_graph` into thematic submodules (`core`,
  `constructors`, `templates`, `methods`, `namespaces`, `overloads`,
  `wrappers`, `std_get`) under `tests/c_symbol_graph/`.
- [x] Split `tests/source_overlay` into thematic submodules under
  `tests/source_overlay/`.
- [x] Split `semantic/c` into `identity` (callable overload identity) and
  `skeleton` (skeleton build, symbol-id anchoring, semantic node lookup)
  submodules under `semantic/c/`.
- [x] Split C++ reference validation's typed/indexed `std::get` receiver helpers
  into `patching/c_validation/references/std_get.rs` while keeping the public
  collection APIs stable.
- [x] Extract C++ local-binding shared types into
  `patching/c_validation/references/types.rs`.
- [x] Extract C++ member-call name encoding helpers into
  `patching/c_validation/references/member_call_names.rs`.
- [x] Extract C/C++ call-arity collection and call-name resolution into
  `patching/c_validation/references/call_arities.rs`.
- [x] Extract C++ type-qualifier and declarator-suffix helpers into
  `patching/c_validation/references/type_qualifiers.rs`.
- [x] Split `patching/c_validation/cpp_wrappers` into focused `extraction`,
  `template_arguments`, and `type_normalization` modules with colocated tests,
  retaining the existing C-validation-scoped wrapper helper surface.
- [x] Extract C/C++ local-definition and reference-name collection into
  `patching/c_validation/references/name_collection.rs`.
- [x] Extract C++ local-binding construction into
  `patching/c_validation/references/bindings.rs`.
- [x] Extract C++ member/wrapper receiver resolution into
  `patching/c_validation/references/receivers.rs`, leaving `references/mod.rs`
  as a thin facade.
- [x] Split `references/receivers` into nested modules: `binding_lookup`
  (visible/addressable/temporary/this), `sequence` (sequence/subscript
  element receivers), `wrappers` (optional/expected/smart-pointer receiver
  helpers), and `dispatcher` (ordered member-receiver selection), keeping
  `receivers/mod.rs` as a thin module/re-export facade.
- [x] Split `symbol_dependency/resolution` into focused nested modules for
  path expansion, type aliases, Python lookup, index construction, template
  fallback, ranking, symbol IDs, graph materialization, and per-reference
  candidate resolution, keeping `resolution.rs` as a thin re-export and
  symbol-ID adapter facade.
- [x] Split `tests/c_symbol_graph/wrappers` into nested modules:
  `expected`, `optional`, `pointers`, and `indexed_get`.
- [x] Share strict JSON loads (reject NaN/Infinity and duplicate keys)
  via `scripts/json_strict.py` for check-profile and gateway smoke scripts.
- [x] Peel trailing pointer reference declarators (`T* const&`) in
  `cpp_top_level_pointer_pointee` so pointer-parameter member calls resolve.
- [x] Split `references/bindings` into nested modules: `auto` (decltype/auto
  constructor and alias bindings) and `declared` (explicit type binding
  construction plus declarator helpers), keeping collection entrypoints in
  `bindings/mod.rs`.
- [x] Split `tests/source_overlay/wrappers` into nested modules matching the
  c_symbol_graph wrappers layout (`expected`, `optional`, `pointers`,
  `indexed_get`).
- [x] Split `tests/c_symbol_graph/std_get` and `tests/source_overlay/std_get`
  into `get_if`, `typed`, and `indexed` submodules.
- [x] Centralize package strict JSON parsing via `jsonrpc.loads_strict` and
  reuse it from gateway core payload decoding and index_watch config/core
  responses.
- [x] Split `references/std_get` into nested modules: `core` (container and
  element helpers), `casts` (pointer/any cast receivers), `typed` (typed
  `std::get` wrappers), and `indexed` (indexed tuple `std::get` wrappers).
- [x] Split `receivers/wrappers` into nested modules: `reference` (weak-pointer
  lock and `std::ref`/`std::cref` factory receivers) and `nested`
  (optional/expected/smart-pointer unwrap receivers), with a thin re-export
  facade in `wrappers/mod.rs`.
- [x] Split `receivers/wrappers/nested.rs` into focused `expected`, `helpers`,
  `optional`, and `smart_pointers` submodules while retaining reference-wrapper
  routing and the existing references-scoped re-export surface on the facade.
- [x] Split `references/bindings/auto` into nested modules: `constructor`
  (decltype(auto)/auto constructor binding), `alias` (address/reference-alias
  helpers), and `copy` (standard-wrapper copy/alias helpers internal to auto).
- [x] Split `model/tests` into nested modules: `position`, `index`,
  `symbols`, `patch`, `trace`, and `misc` for public model validation coverage.
- [x] Split `tests/persisted_index` into nested modules: `rebuild_refresh`,
  `trace`, `inspect`, and `migration`.
- [x] Split `patching/python_bindings` into nested modules: `types`,
  `path`, `summary`, `imports`, `targets`, `scope`, and `local`.
- [x] Split `tests/patch_bindings` into nested modules: `core`,
  `scope_bindings`, `match_case`, `expr_bindings`, `class_closure`,
  `imports`, `replacement`, and `io_bypass`.
- [x] Split `tests/query_parity` into nested modules: `index`, `list`, `patch`,
  `read`, `search`, and `trace`.
- [x] Split `vfs/tests` into nested modules: `lifecycle`, `edits`, `patch`,
  `cpp_trace`, and `misc`, with shared temp helpers on the facade.
- [x] Move C++ references regression tests into
  `patching/c_validation/references/tests.rs`.
- [x] Preserve live-VFS and persisted-index parity by adding paired tests when
  changing read/list/search/trace behavior.
- [x] Add dirty-VFS vs persisted index `with_source` overlay parity coverage for
  list, search/read, and trace in `tests/query_parity/overlay_parity.rs`.
- [x] Expand dirty-VFS vs index `with_source` parity to read/list context,
  neighborhood context, and search context paths.
- [x] Split `tests/trace_semantics` into nested modules: `core`,
  `bindings`, `class_scope`, `match_case`, and `imports_calls`.
- [x] Split `vfs/tests/cpp_trace` into nested modules: `edits`,
  `constructors`, `this_receivers`, `local_params`, `aliases`,
  `headers`, and `guards`.
- [x] Split `vfs/buffer.rs` into focused `buffer/edits.rs`,
  `buffer/loading.rs`, and `buffer/lifecycle.rs` submodules while preserving
  lifecycle deadlines, atomic edit rollback, clean-buffer refresh, index sync,
  and workspace overlay behavior.
- [x] Split `vfs/patch_context.rs` into `patch_context/apply.rs`,
  `patch_context/validation.rs`, and `patch_context/results.rs` while preserving
  semantic/position dispatch, deadline rollback, trace-context validation, and
  live-VFS overlay behavior.
- [x] Split `tests/patch_replay` into focused `replay` and `context`
  modules while preserving the existing patch-validation regression coverage.
- [x] Split `tests/index_refresh` into focused `dependencies` and `validation`
  modules while preserving refresh and persisted-state regression coverage.
- [x] Split `tests/c_patching` into focused `targets` and `validation`
  modules while preserving C/C++ patching regression coverage.
- [x] Split `model/tests/symbols` into `core`, `context`, and `discovery`
  modules while preserving public model validation coverage.
- [x] Split `tests/source_overlay/std_get/indexed` into `direct`, `expected`,
  and `sequence` modules while preserving indexed overlay trace coverage.
- [x] Split `tests/c_symbol_graph/std_get/indexed` into `direct`, `expected`,
  and `sequence` modules while preserving live/persisted indexed graph coverage.
- [x] Split `tests/c_symbol_graph/wrappers/optional` into `aliases`, `nested`,
  and `references` modules while preserving optional/expected wrapper coverage.
- [x] Split `tests/source_overlay/wrappers/optional` into `aliases`, `nested`,
  and `references` modules while preserving overlay wrapper trace coverage.
- [x] Split `tests/c_symbol_graph/constructors` into `basic`, `aliases`, and
  `templates` modules while preserving constructor graph coverage.
- [x] Split `tests/persisted_index/migration` into `guards`, `legacy`, and
  `inspection` modules while preserving schema migration regression coverage.
- [x] Split `tests/persisted_index/inspect` into `health`, `paths`, and
  `outcomes` modules while preserving persisted-index diagnostics coverage.
- [x] Split `tests/query_parity/read` into `basic`, `context`, `discovery`,
  and `neighborhood` modules while preserving live/index parity coverage.
- [x] Split `tests/query_parity/search` into `basic`, `context`, `discovery`,
  and `neighborhood` modules while preserving search parity coverage.
- [x] Split `tests/query_parity/trace` into `graph`, `position`, and
  `neighborhood` modules while preserving trace parity coverage.

- [x] Split the oversized `query_parity/trace/position` test file into
  per-language modules (`csharp`, `kotlin`, `java`, `go`, `rust`,
  `javascript`, `python`) so incremental compilation and targeted trace
  filtering stay fast while preserving all position trace parity coverage.
- [x] Split `tests/c_symbol_graph/namespaces` into `scope`, `aliases`, and
  `headers` modules while preserving namespace resolution coverage.
- [x] Split `tests/c_symbol_graph/overloads` into `member`, `using`, and
  `functions` modules while preserving overload resolution coverage.
- [x] Split `tests/trace_semantics/bindings` into `live` and `persisted`
  modules while preserving Python binding trace parity coverage.
- [x] Split `tests/c_symbol_graph/wrappers/expected` into `member`,
  `smart_pointers`, and `references` modules while preserving expected-wrapper
  graph coverage.
- [x] Split `tests/c_symbol_graph/templates` into `declarations`, `calls`, and
  `cross_file` modules while preserving template graph coverage.
- [x] Split `tests/patch_replay/replay` into `acceptance` and `guards`
  modules while preserving replay validation coverage.
- [x] Split `tests/index_refresh/dependencies` into `basic`, `cpp`, and
  `includes` modules while preserving incremental refresh coverage.
- [x] Split `tests/c_patching/targets` into `c`, `templates`, and `qualified`
  modules while preserving C/C++ patch target coverage.
- [x] Split `tests/source_overlay/core/cpp_receivers` into `direct`, `sequence`,
  and `indexed` modules while preserving overlay receiver coverage.
- [x] Split `tests/c_symbol_graph/core` into nested modules: `expansion`,
  `graph_links`, `type_defs`, `this_receivers`, `local_bindings`,
  `std_receivers`, and `aliases`.
- [x] Split `tests/source_overlay/core` into nested modules: `query_ops`,
  `cpp_receivers`, `aliases`, `multi_overlay`, and `guards`.
- [x] Move the persisted-index schema surface behind an
  `index_schema/mod.rs` facade and focused `schema.rs` implementation module
  without changing crate-visible APIs.
- [x] Extract persisted-index schema migration logic into
  `index_schema/migration.rs`, keeping validation and table-definition helpers
  in the schema module.
- [x] Cache Cargo registries and build output per runner OS in the check
  workflow to reduce repeated compilation across superseding pushes.
- [x] Move the persisted-index state implementation behind a
  `symbol_index_state/mod.rs` facade while preserving its public inspection and
  migration exports plus crate-internal query helpers.
- [x] Isolate persisted source fingerprint calculation in
  `symbol_index_state/fingerprints.rs` so freshness policy has a focused
  implementation boundary.
- [x] Extract persisted-index schema structure checks into
  `index_schema/validation.rs`, leaving low-level SQLite table helpers in the
  schema implementation.
- [x] Isolate persisted-index table creation, column upgrades, primary-key
  checks, and file-path index maintenance in `index_schema/tables.rs`.
- [x] Isolate persisted-index metadata loading, schema-version checks, and
  workspace ownership validation in `index_schema/metadata.rs`.
- [x] Extract persisted-index path validation, unindexed-file discovery, and
  freshness issue collection into `symbol_index_state/paths.rs`.
- [x] Extract persisted-index loading and source-overlay refresh composition
  into `symbol_index_state/loading.rs`, leaving health orchestration in
  `state.rs`.
- [x] Isolate persisted-index freshness gating and indexed-file count
  invariants in `symbol_index_state/freshness.rs`.
- [x] Propagate cooperative workspace-scan deadlines through C/C++ symbol
  collection and Python reference, import, and local-binding traversal paths,
  with focused expired-deadline regression coverage.
- [x] Move persisted-index freshness inspection (fresh/stale/missing/unreadable
  file classification) into the freshness module.
- [x] Move persisted-index migration orchestration into
  `symbol_index_state/migration.rs`, keeping health inspection independent from
  schema upgrade execution.
- [x] Rename the remaining health-check implementation module to
  `symbol_index_state/inspection.rs` so module names reflect their actual
  responsibilities.
- [x] Move the persisted-index store behind an `index_store/mod.rs` facade and
  `core.rs` implementation without changing crate-internal persistence APIs.
- [x] Extract persisted graph-edge consistency validation into
  `index_store/validation.rs`.
- [x] Isolate persisted file-state loading and table row-count helpers in
  `index_store/metadata.rs`.
- [x] Extract persisted symbol loading and row-decoding helpers into
  `index_store/loading.rs`, leaving `core.rs` focused on writes and refreshes.
- [x] Move persisted-index incremental refresh writes into
  `index_store/refresh.rs`, keeping the store facade APIs stable.
- [x] Move SARIF patch-diagnostic export helpers into
  `api_patch_validation/sarif.rs`, keeping the public export stable.
- [x] Move patch-result and trace-result integrity validators into
  `api_patch_validation/result_validation.rs`, keeping crate-visible validators
  stable.
- [x] Isolate C/C++ local-include reverse-index and dependent-refresh
  traversal in `include_graph.rs`, keeping workspace refresh APIs stable.
- [x] Extract byte-offset and row/column conversion helpers into
  `language/positions.rs`, preserving Tree-sitter byte-column semantics.
- [x] Extract Tree-sitter node traversal, identifier, and containment helpers
  into `language/tree.rs`, preserving the existing language facade exports.
- [x] Extract path normalization and workspace-boundary helpers into
  `language/paths.rs`, preserving cross-platform facade exports.
- [x] Extract Tree-sitter parser construction, language detection, and document
  parsing into `language/parser.rs`, preserving language facade exports.
- [x] Extract Python semantic skeleton and symbol lookup helpers into
  `semantic/python.rs`, preserving crate-visible summary helper exports.
- [x] Extract Tree-sitter query owner resolution into `query/owners.rs`,
  keeping query execution and validation in the facade.
- [x] Apply cooperative deadlines to Tree-sitter parsing and query execution,
  using parser and cursor progress callbacks so native work can be interrupted.
- [x] Reuse one exact `TraceQueryDeadline` across nested in-memory trace,
  list, search, and read-context execution; source-overlay trace, list,
  search, direct-read, context-read, and position-read dispatch; trace-backed
  and virtual trace-backed patch construction; path/index/position trace,
  graph, neighborhood, and discovery patch-validation delegation; graph
  expansion and read-context expansion; and outer live, persisted-index, and override list,
  search, direct or position read, or trace expansion, instead of recreating
  rounded child budgets.
- [x] Extend cooperative deadlines through live, override, persisted-index,
  include-scan, and symbol-dependency loading used by trace and index paths.
- [x] Extract symbol-trace neighborhood traversal into
  `symbol_trace/neighborhood.rs`, preserving timeout and result validation.
- [x] Extract patch-trace replay validation and evidence matching into
  `api_patch_validation/replay.rs`, preserving validation exports.
- [x] Extract persisted-index schema migration execution into
  `index_migration/execute.rs`, keeping migration-plan helpers in the facade.
- [x] Extract Python symbol-index extraction into
  `symbol_extractor/python.rs`, preserving the shared indexing facade.
- [x] Extract C/C++ symbol-index extraction into
  `symbol_extractor/c.rs`, leaving `symbol_extractor.rs` as a dispatch facade.
- [x] Split workspace symbol indexing into live/override and persisted
  incremental modules under `symbol_index_workspace/`, retaining the facade API.
- [x] Extract trace graph expansion into `symbol_trace/graph.rs`, keeping
  timeout handling and neighborhood traversal in focused modules.
- [x] Extract cross-platform atomic file replacement into `language/io.rs`,
  keeping language parsing and path helpers independent of platform APIs.
- [x] Move source reading and atomic-write orchestration into `language/io.rs`,
  leaving `language.rs` as a focused facade over language helpers and I/O.
- [x] Extract symbol-summary candidate selection and origin ranking into
  `symbol_summary/selection.rs`, keeping summary assembly in the facade.
- [x] Extract symbol-search filter normalization and matching into
  `symbol_search/filters.rs`, keeping match scoring in the facade.
- [x] Extract symbol-position node lookup and indexed-candidate selection into
  `symbol_position/selection.rs`, keeping source/semantic dispatch in the facade.
- [x] Extract C/C++ reverse include-index construction into
  `include_graph/reverse.rs`, keeping dependent traversal in the facade.
- [x] Extract source-overlay workspace validation into
  `source_overlay/validation.rs`, keeping overlay construction in the facade.
- [x] Reject duplicate source-overlay paths after normalization instead of
  silently allowing later entries to replace earlier ones.
- [x] Apply case-insensitive duplicate overlay detection on Windows, matching
  the platform filesystem semantics.
- [x] Add a `WorkspaceScanLimits::with_max_file_bytes` builder while preserving
  default file-count and timeout limits.
- [x] Extract Tree-sitter query validation and timeout bounds into
  `query/validation.rs`, keeping execution and capture ownership in focused modules.
- [x] Extract Tree-sitter query execution, timeout enforcement, and capture
  assembly into `query/execution.rs`, keeping the public query API as a facade.
- [x] Extract semantic path, depth, and parent-path helpers into
  `semantic/paths.rs`, preserving semantic facade exports.
- [x] Extract symbol-index migration plan construction into
  `index_migration/plan.rs`, keeping execution in `execute.rs`.
- [x] Keep workspace-scan facade focused on public limits and walker exports,
  with scan tests isolated from traversal and limit implementations.
- [x] Reuse the C/C++ symbol-node collection across Tree-sitter query captures
  instead of rescanning the syntax tree for every capture owner.
- [x] Isolate the C/C++ symbol-node collection entrypoint in
  `semantic/c/symbols.rs`, keeping the semantic facade exports stable.
- [x] Isolate persisted symbol dependency index construction in
  `symbol_dependency/resolution/indexes.rs`, keeping refresh-facing exports
  stable.
- [x] Isolate template-path candidate expansion and fallback parsing in
  `symbol_dependency/resolution/template_paths.rs`, keeping dependency
  resolution exports stable.
- [x] Isolate symbol candidate ranking and scope matching in
  `symbol_dependency/resolution/ranking.rs`, keeping dependency resolution
  exports stable.
- [x] Isolate C/C++/Python symbol ID assignment in
  `symbol_dependency/resolution/symbol_ids.rs`, keeping the public assignment
  entrypoint stable.
- [x] Isolate dependency/reference graph materialization in
  `symbol_dependency/resolution/graph.rs`, preserving dependency ordering,
  reverse-reference construction, deadline checks, and public result shapes.
- [x] Isolate per-symbol and per-reference candidate resolution in
  `symbol_dependency/resolution/references.rs`, preserving C/C++ overload,
  receiver, include, alias, template, and Python module-hint behavior.
- [x] Isolate VFS patch result/context assembly in
  `vfs/patch_context/results.rs`, keeping patch entrypoints and result
  validation behavior stable.
- [x] Move C/C++ AST symbol collection traversal into
  `semantic/c/symbols.rs`, keeping semantic-path and callable helper exports
  stable.
- [x] Extract workspace scan limits, deadlines, and source-size validation into
  `workspace_scan/limits.rs`, preserving public scan configuration exports.
- [x] Move workspace traversal, skip-directory policy, and source collection
  into `workspace_scan/walker.rs`, keeping `workspace_scan.rs` as a facade.
- [x] Isolate VFS symbol-index registration, refresh, and status metadata in
  `vfs/indexes.rs`, keeping file editing and lifecycle operations focused.
- [x] Isolate VFS virtual-file status aggregation in `vfs/status.rs` and add
  regression coverage for checked index metadata ordering.
- [x] Isolate VFS virtual-file status aggregation in `vfs/status.rs` and add
  regression coverage for checked index metadata ordering.
- [x] Enforce `max_file_bytes` against symlink targets during workspace scans,
  with cross-platform symlink regression coverage where supported.
- [x] Reuse resolved symlink target metadata during workspace scans to avoid
  duplicate filesystem metadata calls.
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
- [x] Add shared batch deadlines and optional timeout coverage to the remaining
  index registry list/unregister tools so every public tool exposes a bounded
  cooperative budget.
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
- [x] Normalize expanded decorated Python method replacements before re-indenting
  them, preventing decorator/definition indentation drift, and explicitly reject
  mismatched decorator-definition indentation before the commit gate.
- [x] Assign distinct Python overload declaration and implementation IDs, and
  reject ambiguous semantic-path reads, traces, expansions, and patches with
  actionable candidate IDs.
- [x] Recognize `typing` and `typing_extensions` overload decorators imported
  under direct or module aliases declared before the decorated definition and
  not rebound by a later binding, including imports, loop targets, assignments,
  deletes, match captures, and other top-level control-flow events, when assigning
  Python overload identities across skeletons and live or persisted indexes;
  reject arbitrary qualified decorators such as `custom.overload`.
- [x] Check Python overload discovery against semantic and workspace-scan deadlines
  throughout top-level import, rebinding, nested pattern traversal, and decorator
  classification.
- [x] Preserve patch-operation deadlines through Python position targeting and
  patched-symbol identity resolution, including overload-alias collection.
- [x] Apply the remaining trace-backed patch-validation timeout budget to both
  semantic-target selection and patch application across workspace and
  persisted-index graph, trace, neighborhood, and discovery contexts.
- [x] Split persisted-index loading's strict SQLite row and JSON decoding helpers
  from query orchestration, keeping validation errors and existing loading tests
  intact while reducing the loading module's scope.

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
- [x] Reuse prepared live and persisted symbol graphs in the high-cardinality
  C++ auto-constructor receiver regression, preserving every expected edge
  assertion while reducing the cached exact-test runtime from roughly 356
  seconds to 14 seconds locally.
- [x] Reuse the same prepared graphs in the C++ `std::get_if` pointer-binding
  regression, retaining all live/persisted edge assertions while reducing its
  cached exact-test runtime from roughly 66 seconds to 13 seconds locally.
- [x] Reuse prepared graphs across the complete typed C++ `std::get` test
  module, preserving positive and negative trace assertions while reducing the
  six-test module runtime from roughly 70 seconds to 17 seconds locally.
- [x] Reuse prepared graphs across expected-member and nested optional/expected
  wrapper regressions, preserving all edge assertions while reducing their
  cached module runtimes from roughly 59 to 5 seconds and 148 to 13 seconds.
- [x] Group indexed `std::expected` tuple access regression tests behind a
  dedicated module boundary for clearer future category-level splits.
- [x] Group direct indexed tuple access regression tests behind a dedicated
  module boundary for clearer receiver-focused maintenance.
- [x] Group indexed sequence tuple access regression tests behind a dedicated
  module boundary for clearer sequence-specific maintenance.
- [x] Upgrade first-party GitHub Actions to Node.js 24-compatible major
  versions across check and wheel workflows, removing Node.js 20 runtime
  deprecation warnings on GitHub-hosted runners.
- [x] Guard all first-party Action references in check and wheel workflows
  with one Node.js 24 major-version contract test, while keeping cache behavior
  assertions independent from the selected Action release.
- [x] Split gateway context direction and node-bound validation regressions
  into a dedicated mixin while preserving the request-validation suite identity,
  native-extension requirements, and all 125 discovered tests.
- [x] Split gateway version, tool-catalog, and suite-manifest contract tests
  into a dedicated metadata mixin while preserving all request-validation test
  identities and full Python discovery coverage.
- [x] Split gateway source/index-path compatibility and position-entrypoint
  validation into a dedicated mixin, reducing the root request-validation
  module by more than 800 lines without changing its 125 discovered tests.
- [x] Split gateway timeout-bound validation across semantic, index, patch,
  VFS, trace, and search entrypoints into a dedicated mixin while preserving
  all suite and discovery results.
- [x] Split gateway edit, byte-budget, and workspace-preview payload
  validation into a dedicated mixin, reducing the root request-validation
  module to roughly one thousand lines with no discovery changes.
- [x] Split gateway scalar type, integer-bound, and string validation into a
  dedicated parameter mixin, reducing the root request-validation module to
  roughly 550 lines while retaining all 125 tests.
- [x] Split gateway trace direction and neighborhood-bound validation into
  a dedicated mixin, leaving the root request-validation module focused on
  JSON-RPC envelope and top-level parameter handling at under 400 lines.
- [x] Replace dynamic symbol-route native-test extraction with explicit
  shared fixture and live-test mixins, preserving the 11 pure and 17 native
  tests while reducing the main route module by more than one thousand lines.
- [x] Split the remaining pure symbol-route cases into read/search and
  patch/context mixins, leaving the suite metadata entrypoint at fewer than
  sixty lines while preserving all route-forwarding assertions.
- [x] Split gateway lazy-core, stdio, one-shot CLI, JSON framing, and response
  serialization regressions into a dedicated runtime transport mixin while
  preserving all 68 runtime tests.
- [x] Split gateway initialization, MCP capability, tool-schema, and resource
  catalog contract tests into a dedicated runtime catalog mixin while
  preserving the complete runtime suite.
- [x] Split gateway tools-call, batch, core-payload, and tree-query regressions
  into dedicated runtime mixins, leaving the suite metadata entrypoint at
  roughly thirty lines while preserving all 68 runtime tests.
- [x] Deep-copy generated tool input and output schemas so caller mutation of
  one catalog response cannot corrupt global templates or later MCP tools/list
  results.
- [x] Expand generated schema cloning to break internal mutable aliases, so
  mutating one property in a single tool descriptor cannot silently alter a
  sibling property that reused the same schema template.
- [x] Split common and semantic/symbol/trace result schemas out of the
  1,100-line gateway schema registry, reducing the aggregation module to about
  725 lines while preserving its public imports and byte-identical catalog.
- [x] Finish the result-schema domain split for patching, queries, VFS, and
  indexes, leaving a roughly 160-line registry while preserving all public
  names, shared-schema identities, batch variants, and catalog bytes.
- [x] Split tool parameter schemas and shared spec models out of the
  547-line tool registry, reducing it to roughly 155 lines while preserving
  public names, pickle identity, mutable-schema aliases, and catalog bytes.
- [x] Include unreadable-file counts in index-watch health summaries so
  emitted reconciliation and CI diagnostics cover every persisted-index file
  freshness category.
- [x] Extract static tool declarations and their derived lookup tables into a
  lightweight definitions module, reducing the compatibility registry to under
  one hundred lines without changing object identity or catalog output.
- [x] Guard literal gateway handler defaults, shared route-helper defaults, and
  index-watch scan defaults against the generated tool manifest so runtime and
  advertised parameter behavior cannot drift silently.
- [x] Extract semantic-skeleton and raw tree-query handlers into a focused
  source-query route mixin, reducing the gateway facade below three hundred
  lines while preserving handler identity, validation, and response behavior.
- [x] Centralize shared native-core timeout invocation and strict JSON payload
  decoding in a dedicated helper mixin, removing the cross-domain dependency on
  symbol routes and reducing the gateway facade to roughly 260 lines.
- [x] Move source/file-path compatibility and workspace write-boundary checks
  into the parameter-validation mixin, keeping route-facing error behavior
  unchanged while reducing the gateway facade to roughly 240 lines.
- [x] Add an `arborist-index-watch --version` entrypoint that reports the
  shared package version before required watch-target validation, matching the
  main gateway CLI's installation-diagnostic ergonomics.
- [x] Extract index-watch argparse types, limits, and parser construction into
  a cold-importable CLI-arguments module, reducing the runtime module below
  five hundred lines while preserving its existing helper imports.
- [x] Extract index-watch configuration parsing, target models, strict payload
  decoding, and deterministic target ordering into a cold-importable support
  module, reducing the runtime module below four hundred lines while preserving
  its established import and pickle identities.
- [x] Extract index reconciliation, health summaries, multi-target polling, and
  check-mode coordination into a CLI-independent runtime module, reducing the
  console facade below two hundred fifty lines while preserving its established
  callable API, facade monkeypatch seams, protocol metadata, and pickle paths.
- [x] Make index-watch fail closed when a migration call returns an unhealthy
  post-migration health payload instead of reporting a successful migration.
- [x] Validate index-watch health payload structure before making reconcile
  decisions, so malformed native responses cannot be treated as healthy.

### P2: Multi-Language Foundation

- [x] Record the multi-language adapter architecture, capability contract,
  persisted-index compatibility rules, and staged JavaScript/TypeScript-first
  delivery plan in `docs/multi-language-support-design.md`.
- [x] Establish the Phase 0 Python/C/C++ contract baseline with the existing
  locked Rust regression suite, including routing, parser safety, symbols,
  VFS/index parity, traces, patches, and SQLite behavior.
- [x] Complete Phase 1 registry migration: route detection, grammar selection,
  and supported-language reporting through descriptors and capabilities while
  preserving `.h` as C by default.
- [x] Complete Phase 2 adapter composition without changing existing public
  protocol behavior.
- [x] Complete Phase 3 structured reference facts and persisted analysis
  provenance before adding the first JavaScript/TypeScript adapter.
- [x] Add JavaScript, TypeScript, and TSX grammar routing, Tree-sitter
  queries, semantic skeletons, and conservative direct-call indexing/tracing.
- [x] Add static local JavaScript/TypeScript import, re-export, and direct
  `require` dependency extraction for transitive incremental refresh.
- [x] Resolve direct calls through statically resolvable local named imports,
  including aliases, while rejecting unresolved imports instead of falling back
  to unrelated workspace symbols.
- [x] Follow static local named re-export chains for direct calls with cycle
  detection and fail-closed unresolved targets; keep star/default/namespace
  resolution and patch flows capability-gated.
- [x] Add a reusable patch-preview contract that exercises every adapter advertising
  patch targeting and validation, covering successful previews and fail-closed
  unresolved-reference rejection.
- [x] Add a reusable VFS overlay parity contract that compares virtual reads with
  persisted-index source overlays for every registered language without mutating disk.
- [x] Add a reusable persisted-index contract that reloads every registered language
  and rejects a stale language analysis revision before reading symbols.
- [x] Add a reusable resolver-safety contract for every traceable language, proving
  direct-call live/persisted parity, unresolved-call rejection, and disabled
  cross-language matching.
- [x] Add JavaScript/TypeScript/TSX ambiguity fixtures and a shared
  live/persisted contract for ambiguous named star re-exports.
- [x] Establish dedicated language fixture directories with direct-call and unresolved-call
  sources for every registered adapter, and consume them from common trace contracts.
- [x] Add a reusable UTF-8 position contract for every registered adapter,
  proving byte-column lookup parity between live and persisted symbol reads.
- [x] Add a reusable parser-deadline contract for every registered adapter,
  proving complex malformed input fails explicitly once its parse budget expires.
- [x] Add a reusable incremental-refresh contract for every registered
  adapter, proving changed source is reindexed without losing its semantic target.
- [x] Add a reusable persisted-symbol stability contract for every registered
  adapter, proving unchanged rebuilds preserve public IDs and symbol source.
- [x] Align C#'s declared file-dependency capability with bounded source-level extraction for explicit aliases/static imports, direct base/interface references, and same-directory namespace imports.
- [x] Align Python's declared file-dependency capability with bounded local `import`/`from ... import ...` resolution for `.py`/`.pyi` modules and packages. This includes package/module initialization edges, wildcard imports, and incremental invalidation of package `__init__.py`.
- [x] Extend Kotlin's bounded file-dependency capability to resolve unique local wildcard package imports, retaining fail-closed behavior when multiple source roots expose the same package.
- [x] Keep Kotlin `.kt`/`.kts` routing and bounded local import dependency resolution aligned, including script files in explicit and wildcard package candidates.
- [x] Extend Java's bounded file-dependency capability to resolve unique local package wildcard imports and static type-member wildcard imports, retaining fail-closed behavior for ambiguous package roots.
- [x] Harden Go's bounded module-import dependency capability to parse candidate production sources, reject syntax-invalid or package-mismatched files, and fail closed on mixed production package directories.
- [x] Harden Go patch binding validation for local module imports: resolve only
  uniquely parsed production packages, fail closed on invalid or mismatched
  package directories, and consume the caller's deadline while scanning them.
- [x] Add proptest-based property coverage for UTF-8 position helpers, path
  normalization idempotence, VFS byte-edit range semantics, and
  position-edit/byte-edit equivalence.
- [x] Add proptest-based patch-preview unified-diff property coverage
  (identity, single/multi-line replacement, insertion/deletion, trailing-newline).
- [x] Add defense-in-depth public output validation to PatchPreviewResult and
  WorkspaceEditPreviewResult rejecting empty-hunk diffs with changed=true.
- [x] Extend cooperative deadline coverage to full-rebuild and incremental
  index persistence transactions.

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
