# Arborist MCP

Arborist MCP is a phase-1 foundation for the architecture described in the draft design doc:

- `crates/arborist-core`: Rust parsing core with Tree-sitter based semantic extraction.
- `crates/arborist-py`: PyO3 bridge that exposes the Rust core to Python.
- `python/arborist_mcp`: thin JSON-RPC gateway over stdio.

## What is implemented

- `get_semantic_skeleton`
- `patch_ast_node`
- `patch_virtual_ast_node`
- `register_symbol_index`
- `refresh_symbol_index_for_file`
- `unregister_symbol_index`
- `list_symbol_indexes`
- `did_open`
- `did_change`
- `did_close`
- `list_virtual_files`
- `read_virtual_file`
- `apply_buffer_edit`
- `commit_virtual_file`
- `discard_virtual_file`
- `rebuild_symbol_index`
- `trace_symbol_graph`
- `replay_patch_evidence_against_trace`
- `validate_patch_commit_with_trace`
- `validate_patch_with_trace_context`
- `execute_tree_query`
- Python and C language routing based on file extension
- Selective semantic skeleton expansion via `expand_nodes`
- Semantic skeleton responses now include `available_symbols` metadata with stable selectors, scope paths, node kinds, byte ranges, signatures, structured parameters/return types, and docstrings when available
- Trace results now expose stable `symbol_id` values so duplicate globals can be targeted precisely
- Patch results now expose `resolved_symbol_id`, and C patch targeting accepts precise `symbol_id` selectors
- Patch validation now returns structured `resolved_identifiers` / `ambiguous_identifiers` feedback for C bindings
- Patch validation now also returns structured resolved binding metadata for Python names, including module symbols, parameters, local assignments, and local, relative, or `from ... import <module> as ...` aliases when Arborist can identify them
- Patch validation now also emits a unified `binding_decisions` audit stream for
  resolved, ambiguous, and unresolved references
- Patch validation now emits a structured `commit_gate` report that explains
  whether the patch was allowed, rejected, or allowed only through an explicit
  bypass
- The commit gate now records per-binding `evidence_invariants`, showing whether
  candidate evidence keys passed, blocked, or failed the write gate
- Symbol summaries now carry optional `signature` data across trace and validation feedback
- Symbol summaries now also carry optional `scope_path`, structured `parameters`, optional `return_type`, and optional `docstring` metadata across trace and validation feedback
- Symbol summaries now also carry `byte_range` evidence so callers can jump back to the exact source span
- Trace and patch binding candidates now expose `origin_type` evidence such as `local_file`,
  `include_header`, or `companion_source`
- Trace summaries and patch binding candidates now share a stable `evidence_key`
  built from symbol identity, source span, origin, and signature evidence
- Trace results now expose an `evidence_keys` summary for the traced symbol,
  callers, and callees so patch evidence can be replayed against one stable
  set of graph keys
- C ambiguity feedback now explains why a binding is ambiguous and includes a
  structured `disambiguation_context` with visible include families, candidate
  include families, and precise candidate `symbol_id` hints for repair loops
- C symbol graphs now tolerate header declarations plus source definitions sharing the same semantic path
- C patch validation now follows local `#include` chains when checking accessible symbols
- C trace summaries now prefer symbols from the active local `#include` header/source family when duplicate global names exist
- File-local C `static` symbols now get file-qualified semantic paths so cross-file traces do not collapse them together
- Virtual dry-run patch validation with syntax interception
- Heuristic local symbol validation and bypass support
- Workspace-level symbol graph indexing for Python and C
- Python trace/index resolution now follows local imported-module aliases such as `import graph_b as gb`, imported symbol aliases such as `from graph_b import helper as h`, and imported submodule aliases such as `from pkg import graph_c as gc`
- SQLite-backed persisted symbol registry
- Incremental rebuilds keyed by persisted file fingerprints
- Session-scoped VFS with disk/virtual state and incremental Tree-sitter edits
- LSP-style buffer session primitives for open/change/close event ingestion
- Session-aware `trace_symbol_graph` for unsaved virtual buffers
- Semantic patching routed through the VFS session before commit
- Session-managed symbol index registrations with commit-time auto-refresh
- File-scoped persisted index refresh for tighter post-commit sync
- Partial SQLite persistence updates for changed or deleted file refreshes
- C file refresh now follows the local `#include` reverse-dependency chain so header edits or deletions can rebuild affected dependents in one pass
- Local C include paths are normalized before dependency tracking, so parent-relative includes such as `#include "../include/wrapper.h"` refresh the right dependents
- Missing system includes such as `#include <stdio.h>` are not treated as local workspace dependencies during refresh
- Workspace path checks normalize `.` and `..` segments before enforcing containment
- Disk-backed read, patch, query, trace, index, and refresh entrypoints normalize path segments before returning file or database paths
- VFS buffers are keyed by normalized absolute paths, so aliases such as `child/../sample.py` share the same dirty buffer and commit state
- Persisted trace reads reject missing `index_db_path` databases without creating empty SQLite files
- Workspace indexing skips common cache, build, dependency, and virtual-environment directories
- The stdio gateway rejects non-standard JSON constants such as `NaN` and `Infinity`, malformed `did_change` edit payloads, and negative numeric parameters before forwarding requests to the Rust core
- Mixed Rust/Python build via `maturin`

## Local setup

```powershell
python -m venv .venv
. .\.venv\Scripts\Activate.ps1
python -m pip install --upgrade pip
python -m pip install maturin
maturin develop
.\scripts\sync-extension.ps1
```

Or use the bootstrap script:

```powershell
.\scripts\bootstrap.ps1
```

`bootstrap.ps1` and `sync-extension.ps1` now resolve the repository root themselves, so they can be invoked from outside the repo root without creating or activating the wrong `.venv`. `bootstrap.ps1` reuses the `maturin develop` build when it calls `sync-extension.ps1`, so the native extension only gets rebuilt once per bootstrap run.

`sync-extension.ps1` keeps the checked-in local gateway extension in sync with the latest Rust build so `python -m arborist_mcp.gateway` works directly from the repository root.
It now rebuilds the debug `arborist-py` extension before copying it into `python/arborist_mcp/`, so re-running the script after Rust changes is enough to refresh the repo-root gateway entrypoint.

## Quick check

```powershell
cargo test
python -m arborist_mcp.gateway --help
```

## Example JSON-RPC message

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"jsonrpc":"2.0","id":2,"method":"arborist/get_semantic_skeleton","params":{"file_path":"tests/fixtures/sample.py","depth_limit":2,"expand_nodes":["top_level"]}}
{"jsonrpc":"2.0","id":3,"method":"arborist/patch_ast_node","params":{"file_path":"tests/fixtures/sample.py","semantic_path":"top_level","new_code":"def top_level(value: int) -> int:\n    return value + 2\n"}}
{"jsonrpc":"2.0","id":4,"method":"arborist/register_symbol_index","params":{"workspace_root":"tests/fixtures","db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":5,"method":"arborist/list_symbol_indexes","params":{}}
{"jsonrpc":"2.0","id":6,"method":"arborist/did_open","params":{"file_path":"tests/fixtures/sample.py","source":"def top_level(value: int) -> int:\n    return value + 10\n"}}
{"jsonrpc":"2.0","id":7,"method":"arborist/did_change","params":{"file_path":"tests/fixtures/sample.py","edits":[{"start":{"row":1,"column":19},"end":{"row":1,"column":21},"new_text":"11"}]}}
{"jsonrpc":"2.0","id":8,"method":"arborist/list_virtual_files","params":{"dirty_only":true}}
{"jsonrpc":"2.0","id":9,"method":"arborist/did_close","params":{"file_path":"tests/fixtures/sample.py","persist":false}}
{"jsonrpc":"2.0","id":10,"method":"arborist/refresh_symbol_index_for_file","params":{"workspace_root":"tests/fixtures","db_path":"tests/fixtures/symbols.db","file_path":"tests/fixtures/graph_b.py"}}
{"jsonrpc":"2.0","id":11,"method":"arborist/patch_virtual_ast_node","params":{"file_path":"tests/fixtures/sample.py","semantic_path":"top_level","new_code":"def top_level(value: int) -> int:\n    return value + 3\n"}}
{"jsonrpc":"2.0","id":12,"method":"arborist/commit_virtual_file","params":{"file_path":"tests/fixtures/sample.py"}}
{"jsonrpc":"2.0","id":13,"method":"arborist/trace_symbol_graph","params":{"workspace_root":"tests/fixtures","symbol_path":"orchestrate","direction":"both","index_db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":14,"method":"arborist/replay_patch_evidence_against_trace","params":{"patch":{"...":"patch result JSON"},"trace":{"...":"trace result JSON"}}}
{"jsonrpc":"2.0","id":15,"method":"arborist/validate_patch_commit_with_trace","params":{"patch":{"...":"patch result JSON"},"trace":{"...":"trace result JSON"}}}
{"jsonrpc":"2.0","id":16,"method":"arborist/validate_patch_with_trace_context","params":{"workspace_root":"tests/fixtures","file_path":"tests/fixtures/caller.c","semantic_path":"orchestrate","new_code":"int orchestrate(int value) {\n    return helper(value);\n}\n","direction":"both"}}
{"jsonrpc":"2.0","id":17,"method":"arborist/execute_tree_query","params":{"file_path":"tests/fixtures/sample.py","query":"(function_definition name: (identifier) @name)"}}
```

For C, `patch_ast_node` and `patch_virtual_ast_node` accept either a plain selector such as `helper` or a precise `symbol_id` such as `E:/repo/include/zeta.h::helper`. When a file contains both a forward declaration and a definition for the same symbol, patch targeting now prefers the definition by default.

`get_semantic_skeleton` now returns both `available_paths` and `available_symbols`. Each `available_symbols` item includes the symbol's stable `symbol_id`, `semantic_path`, optional `scope_path`, `node_kind`, `byte_range`, structured `parameters`, optional `return_type`, and optional `signature` / `docstring`, which lets an agent round-trip directly from lightweight exploration into later trace or patch requests without reconstructing selectors from raw text. For Python decorated definitions, `signature` and `byte_range` cover the full decorated source span rather than only the inner `def` / `class` header.
For C, `expand_nodes` accepts either a plain semantic path such as `helper` or the same precise `symbol_id` returned in `available_symbols`.

When patch validation can bind a reference confidently, the patch response includes it under `validation.resolved_identifiers`. Python now reports resolved module, parameter, and local-assignment bindings with semantic metadata, and it can resolve local or relative import aliases such as `gb.helper(...)`, `h(...)`, `from .local_mod import helper2 as h2`, `from ..graph_b import helper as h`, `from pkg import graph_c as gc`, or `from pkg import worker` when `pkg/__init__.py` re-exports that symbol back to the imported workspace symbol. Decorated imported Python symbols preserve the same full decorated `signature` and `byte_range` metadata in these binding candidates. C continues to report resolved declaration/definition candidates. When multiple same-rank candidates remain, the patch is rejected unless bypassed and the competing bindings are returned under `validation.ambiguous_identifiers`.
Patch validation also emits `validation.binding_decisions`, a single audit stream where every checked reference records its `status`, `reason`, `selected_symbol_id`, and candidate evidence. This gives repair loops one stable place to inspect the binding path before deciding whether a patch is safe to commit.
The final patch decision is mirrored in `validation.commit_gate`, which records `allowed`, `status`, `reason`, optional `bypass_reason`, syntax error count, and the blocking binding decisions that prevented a normal commit.
The gate also emits `evidence_invariants`: resolved bindings must provide exactly one selected candidate evidence key, while ambiguous and unresolved bindings are recorded as blocked evidence that can be replayed by future trace checks.
Those binding candidates now also include `signature` when Arborist can recover one, so repair loops can compare callable shapes instead of only names and IDs.
They also include `byte_range`, while `node_kind` already distinguishes cases such as `declaration` vs `function_definition`, so ambiguity handling can point back to the exact evidence span inside the source file.
Binding candidates now also include `origin_type` so callers can distinguish local definitions, included headers, and companion source definitions. Ambiguous C bindings include a `reason` plus `disambiguation_context`, which now reports the visible include-family chain, the candidate include families, and exact candidate `symbol_id` selectors for explainable repair loops.
Trace summaries, the traced root symbol, and patch binding candidates also include `evidence_key`, a stable, human-readable comparison key derived from `symbol_id`, `file_path`, `node_kind`, `origin_type`, `byte_range`, and `signature`. This gives future trace/patch invariant checks a single field to compare before allowing semantic writes.
`trace_symbol_graph` also returns `evidence_keys`, grouping the traced root symbol key plus caller and callee keys. Repair loops can compare `commit_gate.evidence_invariants[*].candidate_evidence_keys` against `trace.evidence_keys.callees` without reconstructing the graph evidence shape client-side.
Trace symbols and validation candidates now also expose `origin_type`, `evidence_key`, optional `scope_path`, structured `parameters`, optional `return_type`, and optional `docstring` fields, so callers can keep using one semantic contract as they move from exploration to graph tracing to patch safety checks.
Python trace/index resolution also follows local import aliases and package re-exports for module-qualified or imported-symbol calls, so `gb.helper(...)`, `h(...)`, and `from pkg import worker` can resolve back to the same underlying workspace symbol when those imports come from local files.
`replay_patch_evidence_against_trace` consumes a patch result plus a trace result and reports whether each patch evidence invariant is `matched`, `blocked`, `missing`, or `failed` against the trace graph keys.
`validate_patch_commit_with_trace` builds on that replay check and returns a single `allowed/status/reason` decision, making it the first optional strong gate for trace-backed semantic writes.
`validate_patch_with_trace_context` removes the manual orchestration step entirely: it runs patch validation, traces the patched symbol against the workspace with the updated file held in-memory, and returns the patch result plus the trace-backed validation decision in one call.
`execute_tree_query` now also returns optional `owner_symbol_id`, `owner_semantic_path`, and `owner_scope_path` fields when a capture belongs to a semantic symbol. That lets a raw Tree-sitter query jump directly into later trace or patch calls without rediscovering the owning selector from source text alone.

`trace_symbol_graph` accepts either a plain semantic path such as `orchestrate` or a precise `symbol_id` such as `E:/repo/include/zeta.h::helper` when duplicate C globals need exact targeting.

When `index_db_path` is omitted, `trace_symbol_graph` now resolves against the active VFS session first, so unsaved `did_open` / `did_change` / `patch_virtual_ast_node` edits are reflected immediately without touching disk.

The stdio gateway currently accepts one JSON document per line. This keeps the environment lightweight while leaving room to swap in a full MCP transport adapter later.

## Current phase status

Phase 1 is complete for the Python/C read path. The current Phase 2 foundation includes:

- semantic-path based node replacement
- virtual in-memory patch validation before disk writes
- syntax interception via Tree-sitter error detection
- heuristic unresolved identifier detection with `bypass_reason`
- persistent SQLite symbol registry with rebuild + load flows
- VFS buffer lifecycle with read/edit/discard/commit semantics
- Incremental reparsing via `Tree::edit()` + parse reuse for buffer edits
- `get_semantic_skeleton` can keep the file mostly skeletal while fully expanding selected semantic paths
- Skeleton discovery now returns structured symbol metadata, including scope, docstring, and input/output signature context, so read-path exploration can hand precise selectors straight into trace and patch flows
- `did_open` accepts editor buffer contents without forcing a disk write first
- `did_change` applies ordered line/column edits onto the current virtual buffer
- `did_close` can discard or persist the session buffer and unload it from memory
- `trace_symbol_graph` now prefers dirty VirtualState buffers over disk when no persisted index is requested
- `patch_ast_node` uses the same VFS session machinery and commits on success
- `patch_virtual_ast_node` keeps the validated patch in `VirtualState` until an explicit commit
- Patch responses now report `resolved_symbol_id`, so callers can round-trip a precise C trace target into a later patch request
- C patch validation now reports structured binding feedback, including resolved `symbol_id` matches and ambiguous same-name candidates
- Python patch validation now reports structured resolved binding feedback for visible module symbols, parameters, local assignments, local or relative aliases, and package `__init__.py` re-exports
- Patch validation now records every checked binding in `binding_decisions`, unifying resolved, ambiguous, and unresolved evidence into one audit trail
- Patch validation now records a structured `commit_gate`, and `applied` is driven by that gate's `allowed` decision
- `commit_gate.evidence_invariants` now records per-binding evidence-key checks as the foundation for trace/patch replay validation
- Trace summaries and validation candidates now include signatures when available, which makes same-name symbol disambiguation more actionable for the LLM
- Trace summaries and validation candidates now also include scope, structured parameters, return types, and optional docstrings when Arborist can recover them
- Python traces now follow local imported-module and imported-symbol aliases instead of treating those calls as opaque bare names
- Trace summaries and validation candidates now include source byte ranges, making it easier to round-trip from feedback back into an exact patch target
- Trace summaries and validation candidates now include `origin_type`, and ambiguous C patch feedback includes a structured `reason` plus `disambiguation_context` with include-family visibility and precise selector hints
- Trace summaries and validation candidates now include a shared `evidence_key` so patch evidence can be compared directly against trace evidence
- `trace_symbol_graph` now returns an `evidence_keys` summary that groups root,
  caller, and callee evidence keys for replay checks
- Registered symbol indexes are automatically rebuilt when a committed file belongs to that workspace
- `refresh_symbol_index_for_file` reparses only the changed file, removes deleted file state when needed, reuses stored symbols for the rest, and persists the refresh as a partial SQLite update instead of a whole-table rewrite
- Persisted symbol rows now retain raw reference names so file refreshes can reconnect callers when a previously missing symbol becomes resolvable
- File refresh now re-resolves only the changed symbols plus their impacted graph neighborhood before writing the updated rows back to SQLite
- C header refresh now expands through the local reverse `#include` DAG, so touching or deleting `wrapper.h` can rebuild dependent files such as `caller.c` during the same partial refresh
- Parent-relative local include paths are normalized before reverse-dependency matching, so `#include "../include/wrapper.h"` links back to the same refreshed header path as `include/wrapper.h`
- Missing angle-bracket system includes are ignored for local reverse-dependency expansion, while missing quote-style local includes are still tracked so deleted headers can invalidate dependents
- Workspace containment checks now normalize `.` and `..` path segments before comparing paths, so refresh requests cannot escape a workspace through lexical path tricks
- Disk-backed file entrypoints normalize paths before reading or writing, so response payloads and evidence keys do not preserve caller-supplied `.` or `..` aliases
- VFS operations normalize file identities before opening, editing, listing, closing, or committing buffers, so path aliases share one session entry instead of creating parallel dirty state
- Persisted trace requests with a missing `index_db_path` now fail closed without creating an empty SQLite database
- Workspace indexing, single-file refreshes, and live VFS trace overlays skip generated/cache/dependency directories such as `.pytest_cache`, `.mypy_cache`, `.ruff_cache`, `.tox`, `.venv`, `__pycache__`, `venv`, `node_modules`, `target`, `dist`, and `build`
- C trace/index rebuild flows now handle `header declaration + source definition` pairs without symbol-key collisions
- Duplicate C globals now keep distinct graph edges via stable include-family/file-backed `symbol_id` values, and persisted traces can target those IDs directly
- C patch targeting now understands those precise `symbol_id` selectors too, and same-file declaration/definition name collisions prefer the definition node during replacement
- C unresolved-symbol interception now recognizes declarations brought in by local headers referenced via `#include "..."` 
- C trace summaries now rank same-name globals by local include visibility so a caller that includes `zeta.h` prefers `zeta.c` over unrelated duplicate definitions
- C trace/index rebuild flows now keep file-local `static` helpers distinct via file-qualified semantic paths such as `path/to/file.c::helper`

The symbol store is intentionally SQLite-backed for now. It moves the project toward the architecture doc's persistent registry shape while keeping setup simple before introducing LMDB-specific layout and memory-mapped optimizations. Rebuilds now persist per-file fingerprints so unchanged files can be reused on subsequent index refreshes.
