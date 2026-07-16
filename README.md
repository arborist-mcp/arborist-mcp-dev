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

Arborist currently supports Python, C, and C++ source files. Language routing
is extension-based:

- Python: `.py`, `.pyi`
- C grammar: `.c`, `.h`
- C++ grammar: `.cc`, `.cpp`, `.cxx`, `.c++`, `.hpp`, `.hh`, `.hxx`, `.h++`

C++ files use the dedicated Tree-sitter C++ grammar. C-family indexing,
tracing, query ownership, and patch targets support free functions in named
namespaces plus named methods declared or defined in class bodies, with
qualified semantic paths such as `outer::Class::method`. Class definitions are
also indexed with their namespace and enclosing-class scope. Named class methods
defined outside the class are also matched to their declarations. Explicit
constructors and destructors are supported as `Class::Class` and
`Class::~Class`; defaulted/deleted methods are indexed with their full
declaration signatures. Named function and class-method templates are indexed
and traced with their template declaration text. Explicit function template
specializations have distinct paths such as `increment<int>`. Template
parameter binding, class/method specializations, and overload-aware symbol
identities remain a follow-up. Basic operator methods use paths such as `Class::operator+` and
`Class::operator bool`. C++ `using` aliases are indexed with namespace and
class scope, for example `api::Size` and `api::Config::Count`. See the [tool
guide](docs/tools.md#language-support) for the current scope. C++20 concept
definitions and named enum definitions are also indexed by qualified name, such
as `api::Incrementable` and `api::Status`.

## Implemented Tool Families

The MCP catalog currently returns 54 tools:

- Read tools: 27, including batch reads, semantic skeletons, patch previews, raw Tree-sitter queries,
  symbol reads, symbol list/search, and graph-backed read bundles.
- Write tools: 2, `patch_ast_node` and `patch_ast_node_at_position`.
- VFS tools: 10, including open/change/close, virtual patching, byte edits, commit/discard,
  and virtual reads.
- Index tools: 7, covering register, unregister, list, inspect, rebuild,
  workspace refresh, and file refresh for persisted symbol indexes.
- Trace tools: 8, covering graph/neighborhood traces plus trace-backed replay and validation.

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
python scripts\gateway_smoke.py --require-core
python -m arborist_mcp.gateway --help
```

Or run:

```powershell
.\scripts\bootstrap.ps1
```

## Index Watch

Use the polling watch command to keep one persisted index synchronized without
rewriting it while it is healthy:

```powershell
arborist-index-watch --workspace-root . --db-path .\symbols.db
arborist-index-watch --workspace-root . --db-path .\symbols.db --once
```

The watcher refreshes missing indexes and current-schema freshness issues
through the incremental workspace refresh path. It exits without writing when
inspection requires manual intervention, such as an unsupported or foreign
SQLite schema.

On Linux or macOS:

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

## Quick Validation

For the normal local loop:

```powershell
.\scripts\test.ps1 -Suite inner-loop
```

Useful suite variants:

```powershell
.\scripts\test.ps1 -Suite python-fast
.\scripts\test.ps1 -Suite python-native
.\scripts\test.ps1 -Suite python
.\scripts\test.ps1 -Suite rust,inner-loop -ShowPlan
python scripts/python_suite_manifest.py
```

For the full gate:

```powershell
.\scripts\check.ps1
```

Useful profile variants:

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
python scripts\gateway_smoke.py --launcher console --require-core
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
- MCP resources expose the generated tool catalog snapshot for clients that
  prefer resource reads over `tools/list`.
- Semantic patching with structured binding decisions, commit gates, bypass
  auditing, and trace-backed replay validation.
- Session-scoped VFS with open/change/close, virtual patching, commit/discard,
  and incremental Tree-sitter edits.
- Python/C workspace symbol graph indexing, listing, searching, reading,
  tracing, and bounded neighborhood context.
- SQLite-backed persisted symbol indexes with schema-version checks, health
  inspection, response schema versioning, stale/missing/unreadable/unindexed
  file diagnostics, bounded workspace scans, optional per-file byte limits,
  partial refresh, and fail-closed handling for damaged or unrelated databases.
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
- Extending C++ semantic support beyond named template declarations to template
  parameter binding, specializations, and overload-aware symbol identities.
- Adding registered-index watch mode, benchmarks, fuzz/property tests, and deeper runtime
  controls such as operation timeouts/cancellation for very large workspaces.
