# Tool Guide

This guide summarizes Arborist's tool families and semantic behavior. The exact
MCP schemas are generated from the gateway and checked in at
[`docs/tool-catalog.json`](tool-catalog.json).

As of this revision, `tools/list` returns 52 tools:

- Read tools: 26, including semantic skeletons, patch previews, raw Tree-sitter
  queries, symbol reads, symbol list/search, and graph-backed read bundles.
- Write tools: 2, `arborist/patch_ast_node` and
  `arborist/patch_ast_node_at_position`.
- VFS tools: 10, including open/change/close, virtual patching, byte edits,
  commit/discard, and virtual reads.
- Index tools: 6, covering register, unregister, list, inspect, rebuild, and
  file refresh for symbol indexes.
- Trace tools: 8, covering graph/neighborhood traces plus trace-backed replay
  and validation.

## Language Support

Arborist currently supports Python and C source files. Language routing is based
on case-insensitive file extensions:

- Python: `.py`
- C grammar: `.c`, `.h`, `.hpp`, `.hh`

Important C++ caveat: `.hpp` and `.hh` are currently parsed with the C grammar.
This supports C-like header/source families, but it is not full C++ semantic
support. Real C++ projects may expose grammar and symbol-resolution gaps until a
dedicated `tree-sitter-cpp` integration is added.

## Read And Discovery Tools

`get_semantic_skeleton` returns both `available_paths` and
`available_symbols`. Each symbol includes stable `symbol_id`, `semantic_path`,
optional `scope_path`, `node_kind`, `byte_range`, structured `parameters`,
optional `return_type`, and optional `signature` / `docstring`.

`execute_tree_query` runs raw Tree-sitter queries and returns optional
`owner_symbol_id`, `owner_semantic_path`, and `owner_scope_path` fields when a
capture belongs to a semantic symbol. Results are bounded by `max_captures`
(default `10000`) so broad arbitrary queries fail closed instead of returning
unbounded capture sets. Its MCP `outputSchema` describes each capture field
explicitly, including byte ranges and start/end points.

`read_symbol` and `read_symbol_at_position` bridge discovery and action by
returning structured symbol metadata plus the exact source snippet and start/end
points.

The `list_symbols*` and `search_symbols*` families use the same structured
symbol shape as skeleton, trace, and patch flows. Search matches are
case-insensitive and can include matched-field metadata for ranking.

## Source Overlays

One-shot skeleton, query, patch, trace-context, and position-based read/trace
requests can analyze an optional `source` buffer without writing it to disk.

Selector-based symbol reads, graph reads, list, and search families also accept
one-shot unsaved `source` overlays when callers provide the workspace
`file_path` that buffer should replace.

When `index_db_path` is supplied with a source overlay, Arborist resolves against
the persisted index plus the in-memory replacement for that one anchored file.
When `index_db_path` is omitted, Arborist resolves against the live workspace and
active VFS buffers.

Use the VFS methods (`did_open`, `did_change`, `patch_virtual_ast_node`,
`patch_virtual_ast_node_at_position`, `commit_virtual_file`, and
`discard_virtual_file`) when the caller wants a longer-lived editor session.
Snapshot and list-status outputs have precise MCP schemas for file path, source,
dirty state, version, and syntax error counts.

## Patch And Preview Tools

`preview_patch_ast_node` and `preview_patch_ast_node_at_position` run the same
semantic patch validation path as normal patching, but they do not write to
disk. They return:

- `patch`: the full patch validation result.
- `unified_diff`: a compact unified diff from original source to preview source.
- `changed`: whether the preview changes source text.

`patch_ast_node` and `patch_ast_node_at_position` perform semantic replacement
with validation. Patch responses include `resolved_symbol_id`, `resolved_path`,
`updated_source`, and `validation`.

For C, patch selectors may be a plain name such as `helper` or a precise
`symbol_id` such as `E:/repo/include/zeta.h::helper`. When a file contains both
a forward declaration and a definition for the same symbol, Arborist prefers the
definition by default.

Patch validation reports:

- `resolved_identifiers`
- `ambiguous_identifiers`
- `binding_decisions`
- `commit_gate`
- `evidence_invariants`

`commit_gate` records whether the patch was allowed, rejected, or allowed only
through an explicit bypass. Bypass reasons must be nonblank.

## Trace And Context Tools

`trace_symbol_graph` accepts either a plain semantic path such as `orchestrate`
or a precise `symbol_id` when duplicate C globals need exact targeting. It
returns the traced symbol, callers, callees, and `evidence_keys`.

`trace_symbol_neighborhood` expands a trace into a bounded graph. Callers can
control `direction`, `max_depth`, and `max_nodes`; `truncated` indicates the
bounded expansion omitted reachable symbols.

`read_symbol_context`, `read_symbol_neighborhood_context`, and
`read_symbol_discovery_context` combine source reads with trace and neighborhood
data to reduce multi-call orchestration.

`validate_patch_with_trace_context` runs patch validation, traces the patched
symbol with the updated file held in memory, and returns the trace-backed
validation decision in one response. If syntax validation or the patch gate
rejects first, tracing is skipped and `trace_error` explains why.

The graph, neighborhood, and discovery context variants add bounded impact
analysis and aligned source snippets for reachable symbols. `*_at_position`
variants resolve the target from `file_path + position` before running the same
workflow.

`replay_patch_evidence_against_trace` compares patch evidence invariants against
trace graph evidence. `validate_patch_commit_with_trace` turns that replay into
a single allowed/status/reason decision.

## Symbol Index Tools

`rebuild_symbol_index` creates or replaces a persisted SQLite symbol index.
`refresh_symbol_index_for_file` reparses one changed file, removes deleted file
state when needed, reuses stored symbols for unchanged files, and persists a
partial SQLite update.

`register_symbol_index`, `unregister_symbol_index`, and `list_symbol_indexes`
manage session-scoped index registrations. Registered indexes are refreshed when
a committed file belongs to that workspace.

`inspect_symbol_index` is read-only. It reports whether an index exists, whether
its schema and metadata are healthy, the response schema version, the expected
index schema version, the stored workspace root, indexed file/symbol counts,
file-state row count, fresh indexed file count, stale indexed files whose
fingerprints no longer match disk, missing indexed files, unreadable indexed
files, and diagnostic issues.

Persisted trace reads and single-file refreshes fail closed on missing indexes,
non-index databases, incomplete schema, missing or unsupported schema versions,
metadata issues, incompatible column types, damaged symbol identity fields,
invalid byte ranges, invalid JSON graph/list columns, or empty persisted
file-state paths. These checks avoid silently initializing or partially
migrating unrelated SQLite databases.

## C Graph Behavior

C symbol graphs tolerate header declarations plus source definitions sharing the
same semantic path, including uppercase `.H`/`.C` and `.HPP` families. Duplicate
globals keep distinct file-backed `symbol_id` values.

C patch validation follows local `#include` chains when checking accessible
symbols. Ambiguous C bindings include visible include-family context and exact
candidate `symbol_id` hints.

File-local C `static` symbols get file-qualified semantic paths so cross-file
traces do not collapse them together.
