# Arborist MCP

Arborist MCP is a mixed Rust + Python workspace for semantic code analysis,
patch validation, persisted symbol indexing, and a lightweight stdio MCP
gateway.

Current layers:

- `crates/arborist-core`: Rust parsing core with Tree-sitter based semantic
  extraction, symbol graph indexing, patch validation, VFS state, and SQLite
  persistence.
- `crates/arborist-py`: PyO3 bridge that exposes the Rust core to Python as
  `_arborist_core`.
- `python/arborist_mcp`: MCP-compatible JSON-RPC gateway over stdio.

## Documentation

- [Development guide](docs/development.md): setup, checks, CI profiles, test
  suites, build artifacts, and common failures.
- [Protocol guide](docs/protocol.md): MCP usage, legacy JSON-RPC compatibility,
  tool catalog generation, and protocol validation.
- [Tool guide](docs/tools.md): supported tool families, source overlays, patch
  preview, symbol indexes, trace/context workflows, and C/C++ status.
- [Generated tool catalog](docs/tool-catalog.json): exact `tools/list` snapshot.

## Language Support

Arborist currently supports Python and C source files. Language routing is
extension-based:

- Python: `.py`
- C grammar: `.c`, `.h`, `.hpp`, `.hh`

`.hpp` and `.hh` files are routed through the C grammar today. They are useful
for C-like header/source families, but this is not full C++ support yet. See the
[tool guide](docs/tools.md#language-support) for the current C++ caveat.

## Implemented Tool Families

The MCP catalog currently exposes 52 tools:

- Read tools: semantic skeletons, patch previews, raw Tree-sitter queries,
  symbol reads, symbol list/search, and graph-backed read bundles.
- Write tools: `patch_ast_node` and `patch_ast_node_at_position`.
- VFS tools: open/change/close, virtual patching, byte edits, commit/discard,
  and virtual reads.
- Index tools: register, unregister, list, inspect, rebuild, and file refresh
  for persisted symbol indexes.
- Trace tools: graph/neighborhood traces plus trace-backed replay and validation.

Use `python -m arborist_mcp.gateway --dump-tool-catalog` or read
[`docs/tool-catalog.json`](docs/tool-catalog.json) for exact names, input
schemas, output schemas, defaults, and categories.

## Quick Start

On Windows:

```powershell
python -m venv .venv
. .\.venv\Scripts\Activate.ps1
python -m pip install --upgrade pip
python -m pip install "maturin>=1.7,<2.0"
maturin develop --locked
.\scripts\sync-extension.ps1 -SkipBuild
python -m arborist_mcp.gateway --help
```

Or run:

```powershell
.\scripts\bootstrap.ps1
```

On Linux or macOS:

```bash
python3 -m venv .venv
. .venv/bin/activate
python -m pip install --upgrade pip
python -m pip install "maturin>=1.7,<2.0"
maturin develop --locked
python -m arborist_mcp.gateway --help
```

## Quick Validation

For the normal local loop:

```powershell
.\scripts\test.ps1 -Suite inner-loop
```

For the full gate:

```powershell
.\scripts\check.ps1
```

Useful direct commands:

```powershell
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
python scripts\tool_catalog.py --check
python -m unittest tests.gateway_protocol.request_validation
python -m arborist_mcp.gateway --help
```

See the [development guide](docs/development.md) for profiles, suite names,
native-extension sync behavior, CI coverage, and release wheel builds.

## MCP Usage

Minimal MCP server configuration:

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

If Arborist is installed as a package, `arborist-mcp` is equivalent:

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

Legacy direct `arborist/*` JSON-RPC calls remain supported over the same
newline-delimited stdio transport. See the [protocol guide](docs/protocol.md)
for response shapes, error behavior, and examples.

## Core Capabilities

- Semantic skeletons with stable selectors, symbol IDs, signatures, byte ranges,
  parameters, return types, and docstrings when available.
- One-shot source overlays for unsaved-file analysis, including persisted-index
  read/trace/list/search overlays when `index_db_path` is supplied.
- Patch preview tools that return validation plus unified diff without writing
  to disk.
- Semantic patching with structured binding decisions, commit gates, bypass
  auditing, and trace-backed replay validation.
- Session-scoped VFS with open/change/close, virtual patching, commit/discard,
  and incremental Tree-sitter edits.
- Python/C workspace symbol graph indexing, listing, searching, reading,
  tracing, and bounded neighborhood context.
- SQLite-backed persisted symbol indexes with schema-version checks, health
  inspection, partial refresh, and fail-closed handling for damaged or unrelated
  databases.
- C include-family tracing and patch disambiguation for header/source projects,
  including duplicate globals and file-local `static` symbols.

## Current Status

Phase 1 is complete for the Python/C read path. The current Phase 2 foundation
includes patch validation, trace-backed validation, VFS-backed editor flows,
persisted indexes, source overlays, index health inspection, and generated MCP
tool schemas.

Remaining larger work includes:

- Splitting large Rust modules such as `lib.rs`, `symbols.rs`, and `model.rs`.
- Reducing PyO3 wrapper repetition with parameter/context objects.
- Adding a durable migration strategy beyond the current schema-version gate.
- Adding full C++ support via a dedicated grammar instead of routing `.hpp` and
  `.hh` through the C grammar.
- Adding batch operations, watch mode, benchmarks, fuzz/property tests, and
  stronger resource limits for large workspaces or arbitrary Tree-sitter
  queries.
