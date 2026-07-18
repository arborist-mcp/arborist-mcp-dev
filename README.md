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
- C++ grammar: `.cc`, `.cpp`, `.cxx`, `.c++`, `.tpp`, `.tcc`, `.ipp`, `.inl`,
  `.hpp`, `.hh`, `.hxx`, `.h++`

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
specializations have distinct paths such as `increment<int>` and `Box<int>::value`.
Non-type template parameters are treated as local bindings during patch validation
and reference tracing. C++ callable `semantic_path` values remain overload-set
paths, while exact `symbol_id` values include normalized parameter types and
member qualifiers, such as `api::convert(int)`, `api::convert(double)`, and
`api::Counter::value() const`. Basic operator methods use paths such as
`Class::operator+` and `Class::operator bool` with the same exact-ID convention.
C++ graph resolution filters direct function calls by argument count before
choosing an overload; defaulted and variadic parameters are considered when
matching candidates. Namespace-qualified calls such as `api::convert(value)`
are resolved relative to enclosing namespaces before overload filtering.
Explicit template calls such as `convert<int>(value)` are resolved through the
same direct-call graph path.
Direct C++ type constructions such as `Counter(value)`, `Counter{value}`, and
`new api::Counter(value)` resolve to the matching constructor overload by
argument count. Template constructions such as `api::Box<int>{value}` fall
back to the primary class template when an explicit specialization is not
indexed; this applies to `new api::Box<int>(value)` as well.
Namespace aliases are expanded for direct qualified calls, so an alias such as
`namespace vendor = detail;` resolves `vendor::convert(value)` to `detail`;
alias chains are expanded transitively.
Qualified calls through `using api::function;` declarations resolve to
the imported callables rather than the declaration symbols themselves; local
and imported overloads remain part of the same argument-count-filtered set.
Unqualified direct calls also resolve through scoped `using api::function;`
declarations before global fallback candidates are considered.
Direct unqualified C++ calls also honor `using namespace vendor;` imports from
the enclosing namespace scopes before falling back to global candidates, including
namespace-alias targets such as `using namespace alias;` when the alias is
declared earlier in the same source file.
C++ `using` aliases and declarations are indexed with namespace and class scope,
for example `api::Size`, `api::Config::Count`, and `api::convert`. Namespace
aliases are indexed at their definition scope, for example `api::vendor`. See the [tool
guide](docs/tools.md#language-support) for the current scope. C++20 concept
definitions, named enum definitions and members, and named struct/union definitions are
also indexed by qualified name, such as `api::Incrementable`, `api::Status`,
`api::Status::ready`, `api::Counter`, and `api::Counter::Storage`. C definitions such as `struct
Packet { ... };`, `union Payload { ... };`, and named enum members are indexed without a `typedef`
alias. C++ anonymous-namespace members use file-anchored identities so symbols
with the same name in separate translation units remain isolated. `extern "C"`
function declarations and definitions are indexed through their linkage wrapper.
Declarations in `#if`/`#else` branches are also indexed without evaluating
preprocessor conditions.
Inline friend functions, including function templates, are indexed in their
enclosing namespace rather than as class methods.
Explicit class and function template instantiations are indexed with their
specialized paths, such as `api::Vector<int>` and `api::increment<int>`.

## Implemented Tool Families

The MCP catalog currently returns 58 tools:

- Read tools: 29, including batch reads, semantic skeletons, patch previews,
  bounded raw Tree-sitter queries with cooperative timeout budgets, symbol
  reads, symbol list/search, and graph-backed read bundles.
- Write tools: 2, `patch_ast_node` and `patch_ast_node_at_position`.
- VFS tools: 10, including open/change/close, virtual patching, byte edits, commit/discard,
  and virtual reads.
- Index tools: 9, covering register, unregister, list, inspect, migrate,
  rebuild, workspace refresh, and file refresh for persisted symbol indexes.
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
arborist-index-watch --workspace-root . --db-path .\symbols.db --once --timeout-ms 5000
arborist-index-watch --workspace-root . --db-path .\symbols.db --once --dry-run
arborist-index-watch --workspace-root . --db-path .\symbols.db --check
```

The watcher refreshes missing indexes and current-schema freshness issues
through the incremental workspace refresh path. It exits without writing when
inspection requires manual intervention, such as an unsupported or foreign
SQLite schema. `--timeout-ms` bounds health freshness reads and workspace
reconciliation scans as well as refresh indexing work.
`--dry-run` reports `would_refresh` or `would_migrate` without changing an
index. `--check` performs that no-write inspection once and returns a nonzero
exit status unless every target is healthy, which is useful for CI and
deployment checks.

To watch several registered workspace/index pairs, provide a JSON manifest:

```json
{
  "indexes": [
    {"workspace_root": "./workspace-a", "db_path": "./indexes/a.db"},
    {"workspace_root": "./workspace-b", "db_path": "./indexes/b.db"}
  ]
}
```

Run it with `arborist-index-watch --config .\watch.json`. Relative paths in
the manifest are resolved from the manifest directory. Each target is
inspected and reconciled in deterministic workspace order; an unsupported or
foreign index stops the command without rewriting it. Duplicate workspace or
database paths are rejected before the first refresh.

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
  tracing, bounded neighborhood context, and optional cooperative budgets for
  direct trace expansion.
- SQLite-backed persisted symbol indexes with transactional v1-v3-to-v4 schema
  migration plus source reindexing, health inspection, response schema versioning,
  stale/missing/unreadable/unindexed file diagnostics, bounded workspace scans,
  optional per-file byte limits and cooperative time budgets, partial refresh,
  and fail-closed handling for damaged or unrelated databases.
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
- Extending C++ semantic support beyond overload-aware callable identities to
  fuller language-aware overload resolution and remaining grammar coverage.
- Adding benchmarks, fuzz/property tests, and broader cancellation boundaries
  for very large operations.
