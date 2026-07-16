# Tool Guide

This guide summarizes Arborist's tool families and semantic behavior. The exact
MCP schemas are generated from the gateway and checked in at
[`docs/tool-catalog.json`](tool-catalog.json).

As of this revision, `tools/list` returns 54 tools:

- Read tools: 27, including batch reads, semantic skeletons, patch previews, raw Tree-sitter
  queries, symbol reads, symbol list/search, and graph-backed read bundles.
- Write tools: 2, `arborist/patch_ast_node` and
  `arborist/patch_ast_node_at_position`.
- VFS tools: 10, including open/change/close, virtual patching, byte edits,
  commit/discard, and virtual reads.
- Index tools: 7, covering register, unregister, list, inspect, rebuild,
  workspace refresh, and file refresh for symbol indexes.
- Trace tools: 8, covering graph/neighborhood traces plus trace-backed replay
  and validation.

## Language Support

Arborist currently supports Python, C, and C++ source files. Language routing
is based on case-insensitive file extensions:

- Python: `.py`, `.pyi`
- C grammar: `.c`, `.h`
- C++ grammar: `.cc`, `.cpp`, `.cxx`, `.c++`, `.hpp`, `.hh`, `.hxx`, `.h++`

C++ files use the dedicated `tree-sitter-cpp` grammar. C-family symbol
indexing, tracing, raw-query owner metadata, and patch target resolution cover
free functions in named namespaces, named methods declared or defined in class
bodies, and header/source families. Symbols use qualified semantic paths, such
as `outer::inner::function` and `outer::Class::method`; same-scope calls prefer
matching symbols during graph resolution. Named methods defined outside their
class are matched to the same semantic path as their class-body declarations.
Explicit constructors and destructors use `Class::Class` and `Class::~Class`
paths. Defaulted/deleted methods retain their full declaration signature.
Templates and overload-aware symbol identities are not yet modeled and should
not be treated as full C++ semantic support.

## Read And Discovery Tools

`get_semantic_skeleton` returns both `available_paths` and
`available_symbols`. Each symbol includes stable `symbol_id`, `semantic_path`,
optional `scope_path`, `node_kind`, `byte_range`, structured `parameters`,
optional `return_type`, and optional `signature` / `docstring`.

`execute_tree_query` runs raw Tree-sitter queries and returns optional
`owner_symbol_id`, `owner_semantic_path`, and `owner_scope_path` fields when a
capture belongs to a semantic symbol. Results are bounded by `max_captures`
(default `10000`) so broad arbitrary queries fail closed instead of returning
unbounded capture sets. `max_captures` is capped at `100000`, Tree-sitter match
expansion is capped internally, and long-running queries stop after a short
execution timeout. Query text is also
capped at 64 KiB before compilation, which keeps accidental or adversarial raw
Tree-sitter queries from consuming unbounded parser resources. Its MCP
`outputSchema` describes each capture field explicitly, including byte ranges
and start/end points.

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
bounded expansion omitted reachable symbols. `max_depth` is capped at `64`, and
`max_nodes` is capped at `10000` across trace, context, and patch-impact tools.

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

`rebuild_symbol_index` creates a missing persisted SQLite symbol index or
rebuilds an existing valid Arborist index for the same workspace. Existing
non-index databases, incomplete schemas, unsupported schema versions, and
indexes from other workspaces are rejected before any schema initialization or
rewrite. `refresh_symbol_index` incrementally synchronizes the complete
workspace: unchanged files are reused by fingerprint, changed and new files are
reparsed, and deleted files are removed. It is the preferred operation for
polling or watch integrations. `refresh_symbol_index_for_file` reparses one
changed file, removes deleted file state when needed, reuses stored symbols for
unchanged files, and persists a partial SQLite update. Workspace scans are
bounded by `max_files` (default `20000`) on rebuilds and missing-index refresh
fallbacks so unexpectedly large workspaces fail with an actionable limit error
instead of scanning without bound. Rebuild and refresh calls can also provide
`max_file_bytes` to reject oversized source files before indexing reads them;
this optional limit is capped at `67108864`. `max_files` is capped at `200000`;
symbol list/search `limit` values are capped at `10000`.

`arborist-index-watch` is a polling console command for one index database. It
uses `inspect_symbol_index` between refreshes, so healthy indexes do not incur
SQLite writes. `--once` performs one inspect-and-reconcile pass for CI or a
supervisor probe. The command refreshes only a missing index or a current-schema
index with freshness issues; foreign, incomplete, and unsupported schemas are
reported and left unchanged.

`register_symbol_index`, `unregister_symbol_index`, and `list_symbol_indexes`
manage session-scoped index registrations. Registered indexes are refreshed when
a committed file belongs to that workspace.

`inspect_symbol_index` is read-only. It reports whether an index exists, whether
its schema and metadata are healthy, the response schema version, the expected
index schema version, a machine-readable migration recommendation, the stored
workspace root, indexed file/symbol counts, file-state row count, fresh indexed
file count, stale indexed files whose fingerprints no longer match disk,
missing indexed files, unreadable indexed files, source files that are not yet
indexed, and diagnostic issues. Persisted index queries fail closed when a new
workspace source file has not been indexed, preventing silently incomplete
search and trace results. The
migration recommendation is intentionally advisory: Arborist does not rewrite
unrecognized SQLite databases during inspection.

Persisted trace reads and single-file refreshes fail closed on missing indexes,
non-index databases, incomplete schema, missing or unsupported schema versions,
metadata issues, indexed-file count mismatches, incompatible column types,
damaged symbol identity fields, persisted paths outside the indexed workspace,
unsupported persisted source paths, invalid byte ranges, invalid JSON
graph/list columns, or empty persisted file-state paths. These checks avoid
silently initializing or partially
migrating unrelated SQLite databases. Inspection and persisted query loading
use read-only SQLite connections; schema creation and migration helpers are
restricted to explicit index write paths.

## C Graph Behavior

C symbol graphs tolerate header declarations plus source definitions sharing the
same semantic path, including uppercase `.H`/`.C` and `.HPP` families. Duplicate
globals keep distinct file-backed `symbol_id` values.

C patch validation follows local `#include` chains when checking accessible
symbols. Ambiguous C bindings include visible include-family context and exact
candidate `symbol_id` hints.

File-local C `static` symbols get file-qualified semantic paths so cross-file
traces do not collapse them together.
