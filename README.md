# Arborist MCP

Arborist MCP is a phase-1 foundation for the architecture described in the draft design doc:

- `crates/arborist-core`: Rust parsing core with Tree-sitter based semantic extraction.
- `crates/arborist-py`: PyO3 bridge that exposes the Rust core to Python.
- `python/arborist_mcp`: MCP-compatible JSON-RPC gateway over stdio.

Arborist currently supports Python and C source files. Language routing is
extension-based; Python files use `.py`, and C routing includes `.c`, `.h`,
`.hpp`, and `.hh`.

## What is implemented

- `get_semantic_skeleton`
- `patch_ast_node`
- `patch_ast_node_at_position`
- `patch_virtual_ast_node`
- `patch_virtual_ast_node_at_position`
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
- `read_symbol`
- `trace_symbol_graph_at_position`
- `trace_symbol_neighborhood_at_position`
- `read_symbol_at_position`
- `read_symbol_context`
- `read_symbol_context_at_position`
- `read_symbol_discovery_context`
- `read_symbol_discovery_context_at_position`
- `read_symbol_neighborhood_context`
- `read_symbol_neighborhood_context_at_position`
- `list_symbols`
- `list_symbols_context`
- `list_symbols_discovery_context`
- `list_symbols_neighborhood_context`
- `search_symbols`
- `search_symbols_context`
- `search_symbols_discovery_context`
- `search_symbols_neighborhood_context`
- `trace_symbol_graph`
- `trace_symbol_neighborhood`
- `replay_patch_evidence_against_trace`
- `validate_patch_commit_with_trace`
- `validate_patch_with_trace_context`
- `validate_patch_with_trace_context_at_position`
- `validate_patch_with_graph_context`
- `validate_patch_with_graph_context_at_position`
- `validate_patch_with_neighborhood_context`
- `validate_patch_with_neighborhood_context_at_position`
- `validate_patch_with_discovery_context`
- `validate_patch_with_discovery_context_at_position`
- `execute_tree_query`
- Python and C language routing based on case-insensitive file extension, including C `.h`, `.hpp`, and `.hh` headers
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
- C symbol graphs now tolerate header declarations plus source definitions sharing the same semantic path, including `.H`/`.C` and `.HPP` header-source families
- C patch validation now follows local `#include` chains when checking accessible symbols
- C trace summaries now prefer symbols from the active local `#include` header/source family when duplicate global names exist
- File-local C `static` symbols now get file-qualified semantic paths so cross-file traces do not collapse them together
- Virtual dry-run patch validation with syntax interception
- Heuristic local symbol validation and bypass support
- Workspace-level symbol graph indexing for Python and C
- Workspace-level symbol listing for Python and C, with the same structured symbol metadata used by skeleton, trace, search, and patch flows
- Workspace-level symbol search for Python and C, with the same structured symbol metadata used by skeleton, trace, and patch flows
- Python trace/index resolution now follows local imported-module aliases such as `import graph_b as gb`, imported symbol aliases such as `from graph_b import helper as h`, and imported submodule aliases such as `from pkg import graph_c as gc`
- SQLite-backed persisted symbol registry
- Incremental rebuilds keyed by persisted file fingerprints
- Session-scoped VFS with disk/virtual state and incremental Tree-sitter edits
- LSP-style buffer session primitives for open/change/close event ingestion
- Session-aware `trace_symbol_graph` for unsaved virtual buffers
- Semantic patching routed through the VFS session before commit
- One-shot skeleton, query, patch, trace-context, position-based read/trace,
  and symbol/list/search analysis requests can analyze an optional `source`
  buffer without first writing it to disk; symbol/list/search overlays use a
  `file_path` anchor to identify which workspace file the unsaved buffer
  should replace
- Session-managed symbol index registrations with commit-time auto-refresh
- File-scoped persisted index refresh for tighter post-commit sync
- Partial SQLite persistence updates for changed or deleted file refreshes
- C file refresh now follows the local `#include` reverse-dependency chain so header edits or deletions can rebuild affected dependents in one pass
- Local C include paths are normalized before dependency tracking, so parent-relative includes such as `#include "../include/wrapper.h"` refresh the right dependents
- Missing system includes such as `#include <stdio.h>` are not treated as local workspace dependencies during refresh
- Workspace path checks normalize `.` and `..` segments before enforcing containment
- Disk-backed read, patch, query, trace, index, and refresh entrypoints, plus one-shot source-backed read, patch, query, trace-context, and position-based read/trace entrypoints, normalize path segments before returning file or database paths
- VFS buffers are keyed by normalized absolute paths, so aliases such as `child/../sample.py` share the same dirty buffer and commit state
- Persisted trace reads reject missing `index_db_path` databases without creating empty SQLite files
- Workspace indexing skips common cache, build, dependency, and virtual-environment directories
- The stdio gateway rejects non-standard JSON constants such as `NaN` and `Infinity`, duplicate JSON object keys, unexpected top-level request params, malformed `did_change` edit payloads, empty semantic selectors, reversed byte/position edit ranges, float request IDs, invalid core JSON payloads, wrong-shaped core JSON payloads, nulls for defaulted string parameters, negative numeric parameters, and non-standard response JSON at the protocol boundary
- Programmatic gateway calls that pass nested JSON parameters to Rust also require strict JSON-derived values, including string object keys, lists rather than Python tuples, and finite numbers
- Direct PyO3 JSON-string arguments for replay, trace-gated validation, and position edits also reject duplicate JSON object keys before model deserialization
- Replay and trace-gated validation inputs also reject blank evidence keys, self-contradictory patch gate state, and trace payloads whose evidence summaries or per-symbol `evidence_key` fields no longer match the underlying symbol identity metadata
- Mixed Rust/Python build via `maturin`

## Local setup

```powershell
python -m venv .venv
. .\.venv\Scripts\Activate.ps1
python -m pip install --upgrade pip
python -m pip install "maturin>=1.7,<2.0"
maturin develop --locked
.\scripts\sync-extension.ps1 -SkipBuild
```

Or use the bootstrap script:

```powershell
.\scripts\bootstrap.ps1
```

`bootstrap.ps1` and `sync-extension.ps1` now resolve the repository root themselves, so they can be invoked from outside the repo root without creating or activating the wrong `.venv`. `bootstrap.ps1` reuses the `maturin develop` build when it calls `sync-extension.ps1`, so the native extension only gets rebuilt once per bootstrap run.

`sync-extension.ps1` keeps the repo-local generated gateway extension in sync with the latest Rust build so `python -m arborist_mcp.gateway` works directly from the repository root.
It now rebuilds the debug `arborist-py` extension before copying it into `python/arborist_mcp/`, so re-running the script after Rust changes is enough to refresh the repo-root gateway entrypoint.

On Linux and macOS, the PowerShell helper scripts are optional. The equivalent
manual setup is:

```bash
python3 -m venv .venv
. .venv/bin/activate
python -m pip install --upgrade pip
python -m pip install "maturin>=1.7,<2.0"
maturin develop --locked
python -m arborist_mcp.gateway --help
```

For focused validation without the PowerShell wrappers:

```bash
cargo test --locked
python -m unittest discover -s tests
python -m arborist_mcp.gateway --help
```

Windows is the primary development environment today, and Linux has basic CI
coverage. macOS is expected to work through the same `maturin develop` flow, but
it is not yet part of CI.

Common build failures:

- Python: make sure the active interpreter is Python 3.10 or newer and that
  `python -m pip --version` points inside the virtual environment you intended.
- Rust: install a stable toolchain with `rustup`, then retry `cargo test
  --locked` before rebuilding the Python extension.
- maturin/native extension: rerun `maturin develop --locked` after Rust changes.
  From a repo checkout, `python -m arborist_mcp.gateway --help` only works after
  the native `_arborist_core` module has been built or synced.
- Virtual, shared, or network drives: if file watching, locking, or path
  normalization behaves oddly, retry from a local non-synced path and keep the
  workspace, `.venv`, and Cargo target directory on the same filesystem.
- Slow dependency downloads: prefetch with `cargo fetch --locked`, keep Cargo's
  cache warm, or set your normal Cargo/PyPI mirror configuration before running
  `maturin develop`.

## Installation and release artifacts

The current consumable artifact is a Python package with a PyO3 native extension
and the `arborist-mcp` console script. Source checkouts can build it locally with
`maturin develop --locked`; release builds should produce wheels with:

```bash
python -m pip install "maturin>=1.7,<2.0"
maturin build --locked --release
```

The generated wheel lands under `target/wheels/` and can be installed with
`python -m pip install target/wheels/arborist_mcp-*.whl`. A standalone binary
server is not published yet; when that changes, this README should document the
binary entrypoint separately from the Python package flow.

GitHub Actions also provides a manual `wheels` workflow, and the same workflow
runs for `v*` tags. It builds Windows and Linux wheels and uploads them as run
artifacts so non-developer users can install the package without a local Rust
toolchain on matching platforms.

## Quick check

Run the full local gate:

```powershell
.\scripts\check.ps1
```

The full gate also checks PowerShell script syntax, version consistency, builds
and syncs the local gateway extension, and runs a real `initialize` smoke
request.

`check.ps1` now also supports focused profiles, so CI and local debugging can
run the same named slices instead of maintaining separate ad hoc command sets:

```powershell
.\scripts\check.ps1 -ListProfiles
.\scripts\check.ps1 -Profile sanity
.\scripts\check.ps1 -Profile rust
.\scripts\check.ps1 -Profile gateway-fast
.\scripts\check.ps1 -Profile python-fast
.\scripts\check.ps1 -Profile gateway-native
.\scripts\check.ps1 -Profile python-discovery
.\scripts\check.ps1 -Profile gateway-smoke
.\scripts\check.ps1 -Profile python-native
.\scripts\check.ps1 -Profile sanity,rust
```

The GitHub Actions workflow now uses those same profiles in parallel on
Windows, and its matrix is now derived from the same shared profile helper that
drives `check.ps1 -ListProfiles`, which makes failures easier to localize and
keeps CI job definitions aligned with the local script surface. Quick
pure-Python workflow and gateway regressions now surface through the dedicated
`python-fast` profile without waiting on the native-extension jobs, while the
legacy `python-native` profile remains as a local aggregate over the
finer-grained native checks.

For the everyday inner loop, run the focused test entrypoint:

```powershell
.\scripts\test.ps1
```

`test.ps1` now reads a top-level Python suite manifest from
`scripts/python_suite_manifest.py`, which merges local workflow suites from
`tests/suites.json` with the gateway protocol manifest in
`tests/gateway_protocol/suites.json`. That keeps suite metadata, grouping,
workflow selection, and the legacy `tests.test_gateway_protocol` loader aligned
through one shared graph instead of mixing manifest-driven gateway suites with a
separate blanket discovery pass. The default `inner-loop` selection now runs
Rust plus the `python-fast` group, which keeps the local loop on pure-Python
workflow coverage and stubbed gateway protocol suites until you explicitly ask
for native-extension integration coverage. When a selected suite needs the
synced PyO3 extension, `test.ps1` now builds and syncs it automatically unless
you override that behavior with `-SyncExtension never`.

```powershell
.\scripts\test.ps1 -Suite rust
.\scripts\test.ps1 -Suite python-fast
.\scripts\test.ps1 -Suite python-native
.\scripts\test.ps1 -Suite gateway
.\scripts\test.ps1 -Suite gateway-fast
.\scripts\test.ps1 -Suite gateway-native
.\scripts\test.ps1 -Suite gateway-request-validation
.\scripts\test.ps1 -Suite gateway-symbol-routes
.\scripts\test.ps1 -Suite gateway-execution
.\scripts\test.ps1 -Suite gateway-trace-payloads
.\scripts\test.ps1 -Suite gateway-management-routes
.\scripts\test.ps1 -Suite gateway-runtime
.\scripts\test.ps1 -Suite python
.\scripts\test.ps1 -Suite all
.\scripts\test.ps1 -ListSuites
.\scripts\test.ps1 -Suite rust -RustFilter read_symbol_at_position
.\scripts\test.ps1 -Suite rust,gateway-fast
.\scripts\test.ps1 -Suite gateway-native -SyncExtension always
```

The gateway protocol tests now live under `tests/gateway_protocol/` and remain
available through the legacy `tests.test_gateway_protocol` module, so old
commands still work while targeted modules are easier to run in isolation.
`-ListSuites` now prints the combined Python suite matrix from the manifest
graph, `-RustFilter` forwards a focused filter to `cargo test --locked
<filter>`, `-Suite` accepts multiple suite names when you want one command to
cover a narrow mixed loop, and `-SyncExtension auto|always|never` lets you
trade correctness checks against native-extension rebuild cost when you already
know whether the local binary is fresh.

Or run the underlying commands directly:

```powershell
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
python -m unittest tests.test_gateway_protocol
python -m unittest tests.gateway_protocol.request_validation
python -m unittest discover -s tests
python -m arborist_mcp.gateway --help
python -m arborist_mcp.gateway --version
```

## Standard MCP server

The gateway speaks standard MCP over stdio while preserving the older direct
`arborist/*` JSON-RPC methods. MCP clients should call `initialize`, may send
the standard `notifications/initialized` notification, then call `tools/list`
and `tools/call`.

Minimal Claude Desktop / Cursor-style server configuration:

```json
{
  "mcpServers": {
    "arborist": {
      "command": "python",
      "args": ["-m", "arborist_mcp.gateway"],
      "cwd": "E:/workspace/arborist-mcp"
    }
  }
}
```

If Arborist is installed as a package, the console script is equivalent:

```json
{
  "mcpServers": {
    "arborist": {
      "command": "arborist-mcp",
      "args": []
    }
  }
}
```

Minimal MCP messages:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"example-client","version":"0.1.0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"arborist/get_semantic_skeleton","arguments":{"file_path":"tests/fixtures/sample.py","depth_limit":2}}}
```

`tools/list` is generated from the gateway's tool catalog and is the source of
truth for tool names, JSON input schemas, output schemas, defaults, and
categories. It currently returns 49 tools:

- Read tools: 24, including semantic skeletons, raw Tree-sitter queries, symbol reads, symbol list/search, and graph-backed read bundles.
- Write tools: 2, `arborist/patch_ast_node` and `arborist/patch_ast_node_at_position`.
- VFS tools: 10, including open/change/close, virtual patching, byte edits, commit/discard, and virtual reads.
- Index tools: 5, covering register, unregister, list, rebuild, and file refresh for symbol indexes.
- Trace tools: 8, covering graph/neighborhood traces plus trace-backed replay and validation.

Successful `tools/call` responses return the raw Arborist result as JSON text in
`content[0].text` and as structured JSON under `structuredContent.result`.
Unknown tool names and malformed `tools/call` envelopes are JSON-RPC `-32602`
errors. Tool argument validation failures, core validation failures, and core
runtime errors are returned as MCP tool results with `isError: true`, so clients
can display the error without tearing down the MCP session.

For debugging or documentation generation, `python -m arborist_mcp.gateway
--dump-tool-catalog` prints the exact generated MCP tool catalog as formatted
JSON. The repository also checks in the current generated snapshot at
[`docs/tool-catalog.json`](docs/tool-catalog.json).

## Legacy JSON-RPC compatibility

Existing custom callers can continue invoking `arborist/*` methods directly over
the same newline-delimited stdio transport. The legacy `initialize` request with
empty params still returns the historical `capabilities.tools` name list.

## Example legacy JSON-RPC messages

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"jsonrpc":"2.0","id":2,"method":"arborist/get_semantic_skeleton","params":{"file_path":"tests/fixtures/sample.py","depth_limit":2,"expand_nodes":["top_level"]}}
{"jsonrpc":"2.0","id":3,"method":"arborist/patch_ast_node","params":{"file_path":"tests/fixtures/sample.py","semantic_path":"top_level","new_code":"def top_level(value: int) -> int:\n    return value + 2\n"}}
{"jsonrpc":"2.0","id":38,"method":"arborist/patch_ast_node_at_position","params":{"file_path":"tests/fixtures/sample.py","position":{"row":0,"column":4},"new_code":"def top_level(value: int) -> int:\n    return value + 2\n"}}
{"jsonrpc":"2.0","id":4,"method":"arborist/register_symbol_index","params":{"workspace_root":"tests/fixtures","db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":5,"method":"arborist/list_symbol_indexes","params":{}}
{"jsonrpc":"2.0","id":6,"method":"arborist/did_open","params":{"file_path":"tests/fixtures/sample.py","source":"def top_level(value: int) -> int:\n    return value + 10\n"}}
{"jsonrpc":"2.0","id":7,"method":"arborist/did_change","params":{"file_path":"tests/fixtures/sample.py","edits":[{"start":{"row":1,"column":19},"end":{"row":1,"column":21},"new_text":"11"}]}}
{"jsonrpc":"2.0","id":8,"method":"arborist/list_virtual_files","params":{"dirty_only":true}}
{"jsonrpc":"2.0","id":9,"method":"arborist/did_close","params":{"file_path":"tests/fixtures/sample.py","persist":false}}
{"jsonrpc":"2.0","id":10,"method":"arborist/refresh_symbol_index_for_file","params":{"workspace_root":"tests/fixtures","db_path":"tests/fixtures/symbols.db","file_path":"tests/fixtures/graph_b.py"}}
{"jsonrpc":"2.0","id":11,"method":"arborist/patch_virtual_ast_node","params":{"file_path":"tests/fixtures/sample.py","semantic_path":"top_level","new_code":"def top_level(value: int) -> int:\n    return value + 3\n"}}
{"jsonrpc":"2.0","id":39,"method":"arborist/patch_virtual_ast_node_at_position","params":{"file_path":"tests/fixtures/sample.py","position":{"row":0,"column":4},"new_code":"def top_level(value: int) -> int:\n    return value + 3\n"}}
{"jsonrpc":"2.0","id":12,"method":"arborist/commit_virtual_file","params":{"file_path":"tests/fixtures/sample.py"}}
{"jsonrpc":"2.0","id":13,"method":"arborist/trace_symbol_graph","params":{"workspace_root":"tests/fixtures","symbol_path":"orchestrate","direction":"both","index_db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":44,"method":"arborist/trace_symbol_graph_at_position","params":{"workspace_root":"tests/fixtures","file_path":"tests/fixtures/graph_b.py","position":{"row":0,"column":5},"direction":"callers","index_db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":14,"method":"arborist/read_symbol","params":{"workspace_root":"tests/fixtures","symbol_path":"helper","index_db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":15,"method":"arborist/read_symbol_at_position","params":{"workspace_root":"tests/fixtures","file_path":"tests/fixtures/graph_b.py","position":{"row":0,"column":5},"index_db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":16,"method":"arborist/read_symbol_context","params":{"workspace_root":"tests/fixtures","symbol_path":"helper","direction":"callers","index_db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":17,"method":"arborist/read_symbol_context_at_position","params":{"workspace_root":"tests/fixtures","file_path":"tests/fixtures/graph_b.py","position":{"row":0,"column":5},"direction":"callers","index_db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":18,"method":"arborist/trace_symbol_neighborhood","params":{"workspace_root":"tests/fixtures","symbol_path":"helper","direction":"callers","max_depth":2,"max_nodes":32,"index_db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":45,"method":"arborist/trace_symbol_neighborhood_at_position","params":{"workspace_root":"tests/fixtures","file_path":"tests/fixtures/graph_b.py","position":{"row":0,"column":5},"direction":"callers","max_depth":2,"max_nodes":32,"index_db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":19,"method":"arborist/read_symbol_neighborhood_context","params":{"workspace_root":"tests/fixtures","symbol_path":"helper","direction":"callers","max_depth":2,"max_nodes":32,"index_db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":20,"method":"arborist/read_symbol_neighborhood_context_at_position","params":{"workspace_root":"tests/fixtures","file_path":"tests/fixtures/graph_b.py","position":{"row":0,"column":5},"direction":"callers","max_depth":2,"max_nodes":32,"index_db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":21,"method":"arborist/read_symbol_discovery_context","params":{"workspace_root":"tests/fixtures","symbol_path":"helper","direction":"callers","max_depth":2,"max_nodes":32,"index_db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":22,"method":"arborist/read_symbol_discovery_context_at_position","params":{"workspace_root":"tests/fixtures","file_path":"tests/fixtures/graph_b.py","position":{"row":0,"column":5},"direction":"callers","max_depth":2,"max_nodes":32,"index_db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":23,"method":"arborist/list_symbols","params":{"workspace_root":"tests/fixtures","limit":20,"index_db_path":"tests/fixtures/symbols.db","file_path_contains":"graph","node_kind":"function_definition"}}
{"jsonrpc":"2.0","id":24,"method":"arborist/list_symbols_context","params":{"workspace_root":"tests/fixtures","limit":20,"index_db_path":"tests/fixtures/symbols.db","file_path_contains":"graph","node_kind":"function_definition"}}
{"jsonrpc":"2.0","id":25,"method":"arborist/list_symbols_discovery_context","params":{"workspace_root":"tests/fixtures","limit":20,"direction":"callers","max_depth":2,"max_nodes":32,"index_db_path":"tests/fixtures/symbols.db","file_path_contains":"graph","node_kind":"function_definition"}}
{"jsonrpc":"2.0","id":26,"method":"arborist/list_symbols_neighborhood_context","params":{"workspace_root":"tests/fixtures","limit":20,"direction":"callers","max_depth":2,"max_nodes":32,"index_db_path":"tests/fixtures/symbols.db","file_path_contains":"graph","node_kind":"function_definition"}}
{"jsonrpc":"2.0","id":27,"method":"arborist/search_symbols","params":{"workspace_root":"tests/fixtures","query":"helper","limit":5,"index_db_path":"tests/fixtures/symbols.db","file_path_contains":"graph","node_kind":"function_definition"}}
{"jsonrpc":"2.0","id":28,"method":"arborist/search_symbols_context","params":{"workspace_root":"tests/fixtures","query":"helper","limit":5,"index_db_path":"tests/fixtures/symbols.db","file_path_contains":"graph","node_kind":"function_definition"}}
{"jsonrpc":"2.0","id":29,"method":"arborist/search_symbols_discovery_context","params":{"workspace_root":"tests/fixtures","query":"helper","limit":5,"direction":"callers","max_depth":2,"max_nodes":32,"index_db_path":"tests/fixtures/symbols.db","file_path_contains":"graph","node_kind":"function_definition"}}
{"jsonrpc":"2.0","id":30,"method":"arborist/search_symbols_neighborhood_context","params":{"workspace_root":"tests/fixtures","query":"helper","limit":5,"direction":"callers","max_depth":2,"max_nodes":32,"index_db_path":"tests/fixtures/symbols.db","file_path_contains":"graph","node_kind":"function_definition"}}
{"jsonrpc":"2.0","id":31,"method":"arborist/replay_patch_evidence_against_trace","params":{"patch":{"...":"patch result JSON"},"trace":{"...":"trace result JSON"}}}
{"jsonrpc":"2.0","id":32,"method":"arborist/validate_patch_commit_with_trace","params":{"patch":{"...":"patch result JSON"},"trace":{"...":"trace result JSON"}}}
{"jsonrpc":"2.0","id":33,"method":"arborist/validate_patch_with_trace_context","params":{"workspace_root":"tests/fixtures","file_path":"tests/fixtures/caller.c","semantic_path":"orchestrate","new_code":"int orchestrate(int value) {\n    return helper(value);\n}\n","direction":"both"}}
{"jsonrpc":"2.0","id":40,"method":"arborist/validate_patch_with_trace_context_at_position","params":{"workspace_root":"tests/fixtures","file_path":"tests/fixtures/caller.c","position":{"row":2,"column":4},"new_code":"int orchestrate(int value) {\n    return helper(value);\n}\n","direction":"both"}}
{"jsonrpc":"2.0","id":34,"method":"arborist/validate_patch_with_graph_context","params":{"workspace_root":"tests/fixtures","file_path":"tests/fixtures/graph_a.py","semantic_path":"orchestrate","new_code":"def orchestrate(value: int) -> int:\n    return helper(value)\n","direction":"both","max_depth":2,"max_nodes":32}}
{"jsonrpc":"2.0","id":41,"method":"arborist/validate_patch_with_graph_context_at_position","params":{"workspace_root":"tests/fixtures","file_path":"tests/fixtures/graph_a.py","position":{"row":3,"column":5},"new_code":"def orchestrate(value: int) -> int:\n    return helper(value)\n","direction":"both","max_depth":2,"max_nodes":32}}
{"jsonrpc":"2.0","id":35,"method":"arborist/validate_patch_with_neighborhood_context","params":{"workspace_root":"tests/fixtures","file_path":"tests/fixtures/graph_a.py","semantic_path":"orchestrate","new_code":"def orchestrate(value: int) -> int:\n    return helper(value)\n","direction":"both","max_depth":2,"max_nodes":32}}
{"jsonrpc":"2.0","id":42,"method":"arborist/validate_patch_with_neighborhood_context_at_position","params":{"workspace_root":"tests/fixtures","file_path":"tests/fixtures/graph_a.py","position":{"row":3,"column":5},"new_code":"def orchestrate(value: int) -> int:\n    return helper(value)\n","direction":"both","max_depth":2,"max_nodes":32}}
{"jsonrpc":"2.0","id":36,"method":"arborist/validate_patch_with_discovery_context","params":{"workspace_root":"tests/fixtures","file_path":"tests/fixtures/graph_a.py","semantic_path":"orchestrate","new_code":"def orchestrate(value: int) -> int:\n    return helper(value)\n","direction":"both","max_depth":2,"max_nodes":32}}
{"jsonrpc":"2.0","id":43,"method":"arborist/validate_patch_with_discovery_context_at_position","params":{"workspace_root":"tests/fixtures","file_path":"tests/fixtures/graph_a.py","position":{"row":3,"column":5},"new_code":"def orchestrate(value: int) -> int:\n    return helper(value)\n","direction":"both","max_depth":2,"max_nodes":32}}
{"jsonrpc":"2.0","id":37,"method":"arborist/execute_tree_query","params":{"file_path":"tests/fixtures/sample.py","query":"(function_definition name: (identifier) @name)"}}
```

For one-shot analysis and validation, `get_semantic_skeleton`,
`execute_tree_query`, `patch_ast_node`, `patch_ast_node_at_position`,
`validate_patch_with_trace_context`, `validate_patch_with_trace_context_at_position`,
`validate_patch_with_graph_context`, `validate_patch_with_graph_context_at_position`,
`validate_patch_with_neighborhood_context`,
`validate_patch_with_neighborhood_context_at_position`,
and `validate_patch_with_discovery_context`,
`validate_patch_with_discovery_context_at_position`
accept an optional `source` string. When supplied, Arborist parses and validates
that buffer for the requested `file_path` without creating or overwriting the
file on disk. Use the VFS methods (`did_open`, `did_change`,
`patch_virtual_ast_node`, `patch_virtual_ast_node_at_position`,
`commit_virtual_file`, and `discard_virtual_file`) when the caller wants a
longer-lived editor session that can be committed later.

For C, `patch_ast_node` and `patch_virtual_ast_node` accept either a plain selector such as `helper` or a precise `symbol_id` such as `E:/repo/include/zeta.h::helper`. When a file contains both a forward declaration and a definition for the same symbol, patch targeting now prefers the definition by default.
`patch_ast_node_at_position` and `patch_virtual_ast_node_at_position` bring that same replacement flow to cursor-driven clients. Given `file_path + position`, Arborist resolves the enclosing semantic symbol first, then runs the existing patch validation path using the resolved selector. For Python, the cursor resolves to a stable `semantic_path`; for C, it resolves to the exact `symbol_id`, so declaration and definition sites remain distinct.

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
`validate_patch_commit_with_trace` builds on that replay check and returns a single `allowed/status/reason` decision, making it the first optional strong gate for trace-backed semantic writes. Blocked replay evidence is accepted only when the patch gate itself was explicitly allowed with a bypass reason.
`validate_patch_with_trace_context` removes the manual orchestration step entirely: it runs patch validation, traces the patched symbol against the workspace with the updated file held in-memory after the patch gate accepts it, and returns the patch result plus the trace-backed validation decision in one call. If the optional `source` parameter is supplied, that buffer is used for both patch validation and the trace overlay, so clients can validate unsaved files before writing them to disk. If syntax validation or the patch gate rejects the patch first, tracing is skipped and `trace_error` explains why.
`validate_patch_with_trace_context_at_position` is the cursor-driven entrypoint for that same repair loop. It starts from `file_path + position`, resolves the exact semantic target under the cursor, and then returns the same patch, trace, and replay-backed validation payload as the selector-based variant.
`validate_patch_with_graph_context` pushes that workflow one step further for agents that need impact analysis immediately after a safe patch candidate is found: after the patch gate accepts the edit, Arborist returns the same patch result and trace-backed validation, plus a bounded `trace_symbol_neighborhood` expansion of the patched symbol using the in-memory post-patch source. Callers can tune `max_depth` and `max_nodes` to trade detail for speed, and the same optional `source` parameter lets the whole flow run against unsaved buffers before anything is written to disk.
`validate_patch_with_graph_context_at_position` applies that same cursor-first resolution to the graph-context variant, so editors can jump from a caret location straight into patch validation plus bounded impact analysis.
`validate_patch_with_neighborhood_context` pushes that same workflow into an immediately consumable agent context payload: after the patch gate accepts the edit, Arborist returns the patch result, trace-backed validation, and a `neighborhood_context` bundle whose `neighborhood` graph matches `trace_symbol_neighborhood` while `reads` carries aligned per-node source snippets. That lets callers inspect the patched symbol's reachable neighborhood without issuing follow-up `read_symbol` calls for each graph node. Like the graph-context endpoint, it supports `direction`, `max_depth`, `max_nodes`, and the optional unsaved-buffer `source` overlay.
`validate_patch_with_neighborhood_context_at_position` brings that same thicker response shape to cursor-driven workflows by resolving the patch target from `position` before running validation and neighborhood expansion.
`validate_patch_with_discovery_context` adds the remaining direct-read step to that repair workflow. After the patch gate accepts the edit, Arborist returns the patch result, trace-backed validation, the exact patched root read under `read`, and the full `neighborhood_context` bundle for the same root symbol. That lets agents evaluate a candidate patch, inspect the rewritten symbol body, and inspect bounded caller/callee context from one response without a follow-up `read_symbol` call. Like the graph and neighborhood-context endpoints, it supports `direction`, `max_depth`, `max_nodes`, and the optional unsaved-buffer `source` overlay.
`validate_patch_with_discovery_context_at_position` completes that cursor-first family: given `file_path + position`, it resolves the enclosing symbol and then returns the same patch result, root read, trace, and bounded neighborhood context as the selector-based discovery flow.
`execute_tree_query` now also returns optional `owner_symbol_id`, `owner_semantic_path`, and `owner_scope_path` fields when a capture belongs to a semantic symbol. That lets a raw Tree-sitter query jump directly into later trace or patch calls without rediscovering the owning selector from source text alone.

`trace_symbol_graph` accepts either a plain semantic path such as `orchestrate` or a precise `symbol_id` such as `E:/repo/include/zeta.h::helper` when duplicate C globals need exact targeting.

When `index_db_path` is omitted, `trace_symbol_graph` now resolves against the active VFS session first, so unsaved `did_open` / `did_change` / `patch_virtual_ast_node` edits are reflected immediately without touching disk.

The selector-based symbol reads (`trace_symbol_graph`, `trace_symbol_neighborhood`, `read_symbol`, `read_symbol_context`, `read_symbol_neighborhood_context`, and `read_symbol_discovery_context`) plus the workspace-wide `list_symbols*` and `search_symbols*` families also accept one-shot unsaved `source` overlays when callers provide the workspace `file_path` that buffer should stand in for. Those overlay requests reject `index_db_path`, since Arborist cannot safely combine a persisted index snapshot with an in-memory replacement buffer for the same workspace read.

`trace_symbol_graph_at_position` brings that same raw caller/callee trace to cursor-first clients. Given `file_path + position`, it resolves the exact symbol under the cursor and then returns the same `trace_symbol_graph` payload, so editors can jump straight from a caret location into graph analysis without first reconstructing a `semantic_path` or `symbol_id`.

`trace_symbol_neighborhood` expands that one-hop trace into a bounded multi-hop graph for agent planning. It returns the traced root symbol plus de-duplicated `nodes` and directed `edges`, following callers, callees, or both up to `max_depth` hops and capping expansion at `max_nodes`. When `truncated` is true, Arborist found more reachable symbols than it was allowed to materialize. Like the other symbol graph reads, it accepts either `semantic_path` or precise `symbol_id`, and it respects live VFS buffers whenever `index_db_path` is omitted.

`trace_symbol_neighborhood_at_position` applies that same cursor-first resolution to the bounded graph variant. Given `file_path + position`, it resolves the exact symbol at the caret and then returns the same `trace_symbol_neighborhood` payload, including `truncated`, `nodes`, and `edges`.

`read_symbol` bridges discovery and action directly: given a `semantic_path` or precise `symbol_id`, it returns the structured symbol summary plus the exact source snippet and start/end points for that symbol. Like the other discovery flows, it can read from the persisted index or the live VFS-backed workspace when `index_db_path` is omitted.

`read_symbol_at_position` adds the editor-facing entrypoint for that same direct read. Given a `file_path` plus `position: {row, column}`, it resolves the enclosing semantic symbol first and then returns the same `read_symbol` payload. This lets callers jump straight from a cursor location into a stable symbol read without reconstructing a `semantic_path` up front. Like the path-based read, it can target the persisted index or the live VFS-backed workspace when `index_db_path` is omitted.

`read_symbol_context` packages the next step after discovery into one call: it returns that same direct source read under `read` plus a `trace_symbol_graph` result under `trace`, using one shared symbol resolution pass. This lets agents fetch the exact symbol body and its callers/callees together without orchestrating separate requests. Like `read_symbol` and `trace_symbol_graph`, it accepts either a semantic path or precise `symbol_id`, supports `direction`, and respects live VFS buffers whenever `index_db_path` is omitted.

`read_symbol_context_at_position` is the cursor-driven variant of that thicker read. It starts from `file_path + position`, resolves the exact symbol under the cursor, and then returns the same `read_symbol_context` payload. This is useful when an editor or agent begins with a caret position instead of a previously discovered selector.

`read_symbol_neighborhood_context` removes the remaining N+1 fetch step after graph expansion. It returns the same bounded `trace_symbol_neighborhood` result under `neighborhood` plus an aligned `reads` array whose entries line up positionally with `neighborhood.nodes`, so agents can inspect each reachable symbol body immediately without issuing separate `read_symbol` calls per node. Like the underlying neighborhood read, it accepts either a semantic path or precise `symbol_id`, supports `direction`, `max_depth`, and `max_nodes`, and respects live VFS buffers whenever `index_db_path` is omitted.

`read_symbol_neighborhood_context_at_position` brings that same bounded neighborhood bundle to cursor-driven workflows. Given a file position, it resolves the exact symbol at that location and returns the same `read_symbol_neighborhood_context` result shape, including aligned neighborhood reads.

`read_symbol_discovery_context` makes the single-symbol path as thick as the list, search, and patch discovery flows. It returns the same direct symbol snippet under `read`, the same `trace_symbol_graph` result under `trace`, and the same bounded `read_symbol_neighborhood_context` bundle under `neighborhood_context`. That lets agents inspect the exact symbol body, its immediate caller/callee graph, and aligned bounded neighborhood reads from one response without stitching together follow-up calls. Like the underlying trace and neighborhood reads, it accepts either a semantic path or precise `symbol_id`, supports `direction`, `max_depth`, and `max_nodes`, and respects live VFS buffers whenever `index_db_path` is omitted.

`read_symbol_discovery_context_at_position` closes that loop for editors and cursor-first agents. Given `file_path + position`, it resolves the exact symbol under the cursor and returns the same combined direct read, immediate trace, and bounded neighborhood context payload as `read_symbol_discovery_context`.

`list_symbols` gives agents a stable workspace-wide symbol inventory before they decide whether they need fuzzy search, trace, or patch work. It lists the same structured symbol summaries used elsewhere, reports `total_symbols` plus `truncated`, supports optional `file_path_contains` and `node_kind` narrowing filters, and respects active dirty VFS buffers when `index_db_path` is omitted.

`list_symbols_context` removes the follow-up read loop after a bounded workspace listing. It returns the same `list_symbols` payload under `list` plus an aligned `reads` array whose entries line up positionally with `list.symbols`, so agents can inspect the exact source snippet for each listed symbol immediately without issuing separate `read_symbol` calls. Like `list_symbols`, it supports optional `file_path_contains` and `node_kind` filters and respects live VFS buffers whenever `index_db_path` is omitted.

`list_symbols_discovery_context` folds the whole bounded inventory workflow into one call. It returns the same `list_symbols` payload under `list`, the aligned direct source snippets under `reads`, and the aligned bounded neighborhood bundles under `contexts`. That lets agents enumerate a workspace slice, inspect exact symbol bodies, and understand local caller/callee structure without stitching together separate context and neighborhood requests. It supports `direction`, `max_depth`, `max_nodes`, optional `file_path_contains` and `node_kind` filters, and live VFS buffers whenever `index_db_path` is omitted.

`list_symbols_neighborhood_context` makes the bounded inventory path as thick as the search path. It returns the same `list_symbols` payload under `list` plus an aligned `contexts` array whose entries line up positionally with `list.symbols`; each entry is a full `read_symbol_neighborhood_context` bundle for the corresponding listed symbol. That lets agents enumerate a workspace slice, inspect exact symbol bodies, and understand bounded local caller/callee neighborhoods without issuing separate graph reads per listed symbol. Like the underlying neighborhood read, it supports `direction`, `max_depth`, `max_nodes`, optional `file_path_contains` and `node_kind` filters, and live VFS buffers whenever `index_db_path` is omitted.

`search_symbols` gives agents a lightweight discovery step before trace or patch work. It searches the workspace or a persisted symbol index for case-insensitive matches across stable symbol fields such as `symbol_id`, `semantic_path`, `file_path`, `signature`, parameters, return type, and docstring, then returns the same structured symbol metadata shape used elsewhere plus `total_matches`, `truncated`, and per-result `match_details` metadata that records the matched symbol id, ranking score, and matched fields. Optional `file_path_contains` and `node_kind` params let callers narrow the candidate set before ranking. When `index_db_path` is omitted, `search_symbols` also respects active dirty VFS buffers inside the workspace.

`search_symbols_context` removes the follow-up read loop after fuzzy discovery. It returns the same `search_symbols` payload under `search` plus an aligned `reads` array whose entries line up positionally with `search.matches`, so agents can inspect the exact source snippet for each ranked hit immediately without issuing separate `read_symbol` calls. Like `search_symbols`, it supports optional `file_path_contains` and `node_kind` filters and respects live VFS buffers whenever `index_db_path` is omitted.

`search_symbols_discovery_context` packages the full ranked discovery workflow into one call. It returns the same `search_symbols` payload under `search`, the aligned direct source snippets under `reads`, and the aligned bounded neighborhood bundles under `contexts`. That lets agents search, inspect exact symbol bodies, and understand local caller/callee structure for each ranked candidate without manually composing separate context and neighborhood requests. Like the underlying neighborhood read, it supports `direction`, `max_depth`, `max_nodes`, optional `file_path_contains` and `node_kind` filters, and live VFS buffers whenever `index_db_path` is omitted.

`search_symbols_neighborhood_context` pushes that same discovery flow into immediate local graph inspection. It returns the same `search_symbols` payload under `search` plus an aligned `contexts` array whose entries line up positionally with `search.matches`; each entry is a full `read_symbol_neighborhood_context` bundle for the corresponding hit. That lets agents search, inspect the exact symbol body, and understand a bounded caller/callee neighborhood for each ranked candidate without issuing separate graph reads per match. Like the underlying neighborhood read, it supports `direction`, `max_depth`, `max_nodes`, optional `file_path_contains` and `node_kind` filters, and live VFS buffers whenever `index_db_path` is omitted.

The stdio gateway accepts one JSON document per line. Both standard MCP methods
and legacy direct `arborist/*` JSON-RPC methods use that transport.

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
- Python `expand_nodes` selectors can expand nested symbols even when those symbols are deeper than the default `depth_limit`
- Skeleton discovery now returns structured symbol metadata, including scope, docstring, and input/output signature context, so read-path exploration can hand precise selectors straight into trace and patch flows
- `did_open` accepts editor buffer contents without forcing a disk write first
- `did_change` applies ordered line/column edits atomically onto the current virtual buffer
- `did_close` can discard or persist the session buffer and unload it from memory
- `trace_symbol_graph` now prefers dirty VirtualState buffers over disk when no persisted index is requested
- `patch_ast_node` uses the same VFS session machinery and commits on success
- `patch_virtual_ast_node` keeps the validated patch in `VirtualState` until an explicit commit
- One-shot skeleton, query, patch, trace-context, and position-based
  read/trace requests accept optional `source` buffers for unsaved-file
  analysis without mutating disk
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
- Workspace containment checks now normalize `.` and `..` path segments before comparing paths, so refresh and trace-backed validation requests cannot escape a workspace through lexical path tricks
- Disk-backed file entrypoints and one-shot source-backed read, patch, query,
  trace-context, and position-based read/trace entrypoints normalize paths
  before reading or writing, so response payloads and evidence keys do not
  preserve caller-supplied `.` or `..` aliases
- VFS operations normalize file identities before opening, editing, listing, closing, or committing buffers, so path aliases share one session entry instead of creating parallel dirty state
- Persisted trace requests with a missing `index_db_path` now fail closed without creating an empty SQLite database
- Persisted trace reads now fail closed on missing or corrupt symbol index metadata, symbol identity fields, file-state paths, byte ranges, or JSON graph/list columns instead of silently defaulting damaged values
- Persisted trace reads and single-file refreshes also reject existing non-index, incomplete-schema, metadata-incomplete, or type-incompatible SQLite databases without initializing or partially migrating Arborist tables as a side effect
- Single-file index refreshes now reject existing symbol databases that were built for a different workspace, avoiding mixed-workspace persisted graph state
- Workspace indexing, single-file refreshes, and live VFS trace overlays skip generated/cache/dependency directories such as `.pytest_cache`, `.mypy_cache`, `.ruff_cache`, `.tox`, `.venv`, `__pycache__`, `venv`, `node_modules`, `target`, `dist`, and `build`
- C trace/index rebuild flows now handle `header declaration + source definition` pairs without symbol-key collisions, including uppercase `.H`/`.C` and `.HPP` sibling and companion files
- Duplicate C globals now keep distinct graph edges via stable include-family/file-backed `symbol_id` values, and persisted traces can target those IDs directly
- C patch targeting now understands those precise `symbol_id` selectors too, and same-file declaration/definition name collisions prefer the definition node during replacement
- C unresolved-symbol interception now recognizes declarations brought in by local headers referenced via `#include "..."` 
- C trace summaries now rank same-name globals by local include visibility so a caller that includes `zeta.h` prefers `zeta.c` over unrelated duplicate definitions
- C trace/index rebuild flows now keep file-local `static` helpers distinct via file-qualified semantic paths such as `path/to/file.c::helper`

The symbol store is intentionally SQLite-backed for now. It moves the project toward the architecture doc's persistent registry shape while keeping setup simple before introducing LMDB-specific layout and memory-mapped optimizations. Rebuilds now persist per-file fingerprints so unchanged files can be reused on subsequent index refreshes.
