# Arborist MCP

Arborist MCP is a semantic code analysis and editing toolkit exposed as a lightweight stdio MCP server. It combines a Rust parsing core with a Python gateway to support multi-language symbol extraction, patch validation, virtual file state, and persisted symbol indexing.

## Highlights

- **Semantic skeletons** with stable selectors, symbol IDs, signatures, byte ranges, parameters, return types, and docstrings when available.

- **Patch preview and validation** that returns a unified diff without writing to disk, plus targeted semantic patching with binding decisions and commit gates.

- **Virtual file state (VFS)** for unsaved-file analysis, including one-shot source overlays and session-scoped edits.

- **Workspace symbol indexing** with SQLite-backed persistence, refresh, migration, search, list, read, and graph tracing/neighborhood context.

- **Multi-language support** built on Tree-sitter parsing and an extension-routed adapter registry; see the [tool guide](docs/tools.md) for the exact per-language capability matrix.

- **MCP first**: tools, schemas, and the checked-in catalog snapshot are generated from the gateway manifest, and a stdio transport serves both MCP and legacy JSON-RPC calls.

## Architecture

- `crates/arborist-core` -- Rust business logic: parsing, semantic skeleton generation, AST patching, virtual file state, symbol indexing, trace validation, and SQLite-backed persisted indexes.

- `crates/arborist-py` -- PyO3 bridge that exposes the Rust core to Python as `_arborist_core`; kept as a thin adapter.

- `python/arborist_mcp` -- stdio JSON-RPC/MCP gateway, tool manifest, schemas, and the MCP server entrypoint.

## Supported Languages

Arborist routes source files by file extension. It currently supports Python, C, C++, JavaScript, TypeScript/TSX, Rust, Go, Java, C#, and Kotlin; capability depth varies by language. The exact extension lists, trace coverage, and patching behavior are documented in the [tool guide](docs/tools.md).

## Quick Start

On Windows:

```powershell
python -m venv .venv
. .\.venv\Scripts\Activate.ps1
python -m pip install --upgrade pip
python -m pip install "maturin>=1.7,<2.0"
maturin develop --locked
.\scripts\sync-extension.ps1 -SkipBuild
python scripts\gateway_smoke.py --require-core
python -m arborist_mcp.gateway --help
```

Or run the bootstrap helper:

```powershell
.\scripts\bootstrap.ps1
```

On Linux/macOS:

```bash
python3 -m venv .venv
. .venv/bin/activate
python -m pip install --upgrade pip
python -m pip install "maturin>=1.7,<2.0"
maturin develop --locked
python -m pip install .
python scripts/gateway_smoke.py --require-core
python -m arborist_mcp.gateway --help
```

## Run as an MCP Server

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

See the [protocol guide](docs/protocol.md) for response shapes, error behavior, and legacy JSON-RPC compatibility.

## Index Watch

The `arborist-index-watch` CLI keeps persisted indexes synchronized without rewriting healthy indexes:

```powershell
arborist-index-watch --workspace-root . --db-path .\symbols.db --once
arborist-index-watch --workspace-root . --db-path .\symbols.db --check
```

Use `--config .\watch.json` to watch multiple workspace/index pairs. See the [development guide](docs/development.md) for flags, health summaries, and CI behavior.

## Tool Catalog

The generated [tool catalog](docs/tool-catalog.json) lists every MCP tool. As of this revision, `tools/list` returns 58 tools:

- Read tools: 29, including semantic skeletons, symbol reads, patch previews, and graph-backed read bundles.
- Write tools: 2, `arborist/patch_ast_node` and `arborist/patch_ast_node_at_position`.
- VFS tools: 10, including open/change/close, virtual patching, and commit/discard.
- Index tools: 9, covering register, list, inspect, migrate, rebuild, and symbol-index refresh.
- Trace tools: 8, covering graph/neighborhood traces plus trace-backed replay and validation.

## Development

For the normal local loop:

```powershell
.\scripts\test.ps1 -Suite inner-loop
```

Targeted Python and native gateway suites keep the full gate faster to iterate:

```powershell
.\scripts\test.ps1 -Suite python
.\scripts\test.ps1 -Suite python-fast
.\scripts\test.ps1 -Suite python-native
.\scripts\test.ps1 -Suite rust,inner-loop -ShowPlan
```

Use `python scripts/python_suite_manifest.py` to inspect how those suite groups are built.

For the full gate:

```powershell
.\scripts\check.ps1
```

Focused check profiles:

```powershell
.\scripts\check.ps1 -Profile python-fast
.\scripts\check.ps1 -Profile gateway-fast
.\scripts\check.ps1 -Profile gateway-native
.\scripts\check.ps1 -Profile python-discovery
.\scripts\check.ps1 -Profile gateway-smoke
.\scripts\check.ps1 -Profile python-native
.\scripts\check.ps1 -Profile full,python-native -ShowPlan
```

Useful direct commands:

```powershell
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
python scripts\tool_catalog.py --check
python scripts\gateway_smoke.py --require-core
python -m arborist_mcp.gateway --help
```

See the [development guide](docs/development.md) for the full validation matrix, suite variants, and known issues.

## Documentation

- [Development guide](docs/development.md) -- setup, validation, CI profiles, benchmarks, build artifacts, and common failures.
- [Protocol guide](docs/protocol.md) -- MCP usage, tool catalog generation, request validation, and legacy JSON-RPC compatibility.
- [Tool guide](docs/tools.md) -- tool families, source overlays, patch preview, symbol indexes, trace/context workflows, and per-language capability details.
- [Generated tool catalog](docs/tool-catalog.json) -- exact `tools/list` snapshot; schemas, defaults, and categories.
- [AGENTS.md](AGENTS.md) -- repository guidance for AI coding agents working in this codebase.
