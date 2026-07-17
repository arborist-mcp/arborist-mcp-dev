# Protocol Guide

Arborist exposes two compatible stdio protocols:

- Standard MCP methods: `initialize`, `tools/list`, `tools/call`,
  `resources/list`, and `resources/read`.
- Legacy direct JSON-RPC methods named `arborist/*`.

The gateway accepts one JSON document per line on stdin and writes one JSON-RPC
response per line on stdout.

## Standard MCP

MCP clients should call `initialize`, may send
`notifications/initialized`, then call `tools/list` / `tools/call` and optional
resource methods.

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
{"jsonrpc":"2.0","id":4,"method":"resources/read","params":{"uri":"arborist://tool-catalog"}}
```

Successful `tools/call` responses return the raw Arborist result as JSON text in
`content[0].text` and as structured JSON under `structuredContent.result`.
Unknown tool names and malformed `tools/call` envelopes are JSON-RPC `-32602`
errors. Tool argument validation failures, core validation failures, and core
runtime errors are returned as MCP tool results with `isError: true`.

## Tool Catalog

`tools/list` is generated from the gateway's tool catalog and is the source of
truth for tool names, JSON input schemas, output schemas, defaults, and
categories. The generated snapshot is checked in at
[`docs/tool-catalog.json`](tool-catalog.json).

The same generated catalog is also exposed as a read-only MCP resource:

```json
{"jsonrpc":"2.0","id":5,"method":"resources/list","params":{}}
{"jsonrpc":"2.0","id":6,"method":"resources/read","params":{"uri":"arborist://tool-catalog"}}
```

For debugging or documentation generation:

```bash
python -m arborist_mcp.gateway --dump-tool-catalog
python scripts/tool_catalog.py --check
```

## Legacy JSON-RPC

Existing custom callers can continue invoking `arborist/*` methods directly over
the same newline-delimited stdio transport. The legacy `initialize` request with
empty params still returns the historical `capabilities.tools` name list.

Minimal legacy messages:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"jsonrpc":"2.0","id":2,"method":"arborist/get_semantic_skeleton","params":{"file_path":"tests/fixtures/sample.py","depth_limit":2,"expand_nodes":["top_level"]}}
{"jsonrpc":"2.0","id":3,"method":"arborist/preview_patch_ast_node","params":{"file_path":"tests/fixtures/sample.py","semantic_path":"top_level","new_code":"def top_level(value: int) -> int:\n    return value + 2\n"}}
{"jsonrpc":"2.0","id":4,"method":"arborist/patch_ast_node","params":{"file_path":"tests/fixtures/sample.py","semantic_path":"top_level","new_code":"def top_level(value: int) -> int:\n    return value + 2\n"}}
{"jsonrpc":"2.0","id":5,"method":"arborist/register_symbol_index","params":{"workspace_root":"tests/fixtures","db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":6,"method":"arborist/list_symbol_indexes","params":{}}
{"jsonrpc":"2.0","id":7,"method":"arborist/inspect_symbol_index","params":{"db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":8,"method":"arborist/migrate_symbol_index","params":{"db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":9,"method":"arborist/trace_symbol_graph","params":{"workspace_root":"tests/fixtures","symbol_path":"orchestrate","direction":"both","index_db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":10,"method":"arborist/read_symbol","params":{"workspace_root":"tests/fixtures","symbol_path":"helper","index_db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":11,"method":"arborist/search_symbols","params":{"workspace_root":"tests/fixtures","query":"helper","limit":5,"index_db_path":"tests/fixtures/symbols.db"}}
```

## Request Validation

The gateway rejects malformed protocol input before it reaches the Rust core.
Current protocol-boundary checks include:

- Non-standard JSON constants such as `NaN` and `Infinity`.
- Duplicate JSON object keys.
- Unexpected top-level request params.
- Malformed `did_change` edit payloads.
- Empty semantic selectors.
- Reversed byte or position edit ranges.
- Float request IDs.
- Invalid or wrong-shaped JSON returned by the core.
- Nulls for defaulted string parameters.
- Negative numeric parameters.
- Non-standard response JSON.

Programmatic gateway calls that pass nested JSON parameters to Rust also require
strict JSON-derived values, including string object keys, lists rather than
Python tuples, and finite numbers. Direct PyO3 JSON-string arguments for replay,
trace-gated validation, and position edits reject duplicate JSON object keys
before model deserialization.

Index registration, rebuild, and refresh tools accept an optional `timeout_ms`
budget capped at `300000`. The budget is cooperative: the core checks it during
workspace traversal, per-file indexing, C include dependency expansion, and
before persistence, then fails without writing a new snapshot when the budget
has expired. The direct graph and neighborhood trace tools accept the same
budget for expansion phases; loading an index or parsing a source overlay is
still a non-preemptible boundary.

`execute_tree_query` accepts an optional `timeout_ms` cooperative budget capped
at `300000`; omitting it keeps the existing `500ms` default. The budget is
checked by Tree-sitter progress callbacks and capture collection.
