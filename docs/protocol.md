# Protocol Guide

Arborist exposes two compatible stdio protocols:

- Standard MCP methods: `initialize`, `tools/list`, `tools/call`,
  `resources/list`, and `resources/read`.
- Legacy direct JSON-RPC methods named `arborist/*`.

The gateway accepts one JSON document per line on stdin and writes one JSON-RPC
response per line on stdout. Stdio removes only CR/LF transport terminators;
it does not normalize non-JSON Unicode whitespace around a request. If the host
stdout text encoding rejects a valid non-ASCII response, the gateway retries
the same JSON document with equivalent ASCII `\u` escapes. If stdin decoding
encounters invalid UTF-8 bytes, the gateway emits a `-32700` parse-error
response without initializing the Arborist core, then stops reading the affected
stream because its text decoder cannot be safely re-synchronized.

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
Before a successful response is exposed, the gateway validates the result
against the tool's advertised output schema, including required fields,
property types, bounds, enums, and nested arrays or objects. A malformed
result is returned as an MCP tool error with `isError: true` and does not
include `structuredContent`.
Unknown tool names and malformed `tools/call` envelopes are JSON-RPC `-32602`
errors. Tool argument validation failures, core validation failures, and core
runtime errors are returned as MCP tool results with `isError: true`.

These are two protocol-level error envelopes, not two interchangeable result
shapes: legacy `arborist/*` failures use the JSON-RPC response `error` object
(with a numeric `code` and `message`), while a recognized MCP `tools/call` that
fails returns `isError: true` and a text item in `content`. Callers should inspect
the envelope for the protocol they are using instead of expecting every tool to
return a successful result object such as `{ "error": ... }`.

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
{"jsonrpc":"2.0","id":8,"method":"arborist/migrate_symbol_index","params":{"db_path":"tests/fixtures/symbols.db","timeout_ms":5000}}
{"jsonrpc":"2.0","id":9,"method":"arborist/trace_symbol_graph","params":{"workspace_root":"tests/fixtures","symbol_path":"orchestrate","direction":"both","index_db_path":"tests/fixtures/symbols.db"}}
{"jsonrpc":"2.0","id":10,"method":"arborist/read_symbol","params":{"workspace_root":"tests/fixtures","symbol_path":"helper","index_db_path":"tests/fixtures/symbols.db","timeout_ms":5000}}
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
- JSON-RPC request documents larger than 128 MiB of UTF-8 text.

Programmatic gateway calls that pass nested JSON parameters to Rust also require
strict JSON-derived values, including string object keys, lists rather than
Python tuples, and finite numbers. Direct PyO3 JSON-string arguments for replay,
trace-gated validation, and position edits reject duplicate JSON object keys
before model deserialization. Nested JSON parameters are capped at 128 MiB of
UTF-8 text and 64 levels of container nesting at the protocol boundary.
File-backed source reads are capped at 64 MiB before parsing; inline gateway
source parameters remain capped at 4 MiB, and direct core source overlays are
capped at 64 MiB before parsing.

`arborist/batch` accepts an optional shared `timeout_ms` budget capped at
`300000`. The gateway validates all inner call envelopes and explicit inner
timeouts before execution. Every batch-eligible tool accepts a cooperative
timeout: each inner call receives the smaller of its explicit timeout and the
remaining batch budget, or the remaining budget when it omitted one. A single
blocking step inside an inner tool remains non-preemptible. Expiration uses
JSON-RPC code `-32000` for legacy calls, returns an MCP tool error through
`tools/call`, and never returns a partial result array.

Successful batch results contain one `{ "name": ..., "result": ... }` item per
inner call. The same output-schema validation is applied to each nested result
and its batch envelope. A malformed nested result is rejected before
`structuredContent` is exposed; MCP callers receive `isError: true`, while
legacy callers receive a JSON-RPC `-32000` error.

`get_semantic_skeleton` accepts an optional cooperative `timeout_ms` budget
capped at `300000`. It covers path setup, file-backed source reads, parsing
boundaries, Python query iteration, C/C++ symbol collection, skeleton rendering,
and result validation. A single blocking source read or parse remains
non-preemptible.

`preview_patch_ast_node` and `preview_patch_ast_node_at_position` also accept
an optional cooperative `timeout_ms` budget capped at `300000`. It spans
file-backed source reads, target resolution, replacement preparation, updated
source parsing, syntax and reference validation, commit-gate evaluation, diff
generation, and result validation. A single blocking source read or parse
remains non-preemptible.

`patch_ast_node`, `patch_ast_node_at_position`, `patch_virtual_ast_node`, and
`patch_virtual_ast_node_at_position` accept the same optional cap for patch
application. The budget covers source setup, target resolution, validation, and
VFS mutation through a final gate immediately before persistence. If it expires
before that gate, any patch mutation is rolled back to the exact prior virtual
file entry, including an existing dirty buffer. Once an atomic write begins, no
later timeout check can turn a persisted change into a timeout response;
registered-index synchronization completes or reports its own error instead.
Blocking source reads and parses remain non-preemptible.

`did_open`, `read_virtual_file`, and `list_virtual_files` accept the same
optional `timeout_ms` cap. Open and read cover path validation, source loading,
disk reads, parsing, clean-buffer refresh, response construction, and result
validation. A failure restores the exact prior virtual entry, or removes an
entry loaded only for the failed request. Listing refreshes loaded files in
normalized path order, checks the budget between files and result items, and
rolls all refreshed entries back when the request fails.

`apply_buffer_edit` and `did_change` accept the same optional cap. Their shared
request budget covers loading and clean-buffer refresh, range or position
validation, source splicing, incremental parsing, syntax diagnostics, result
validation, and a final gate before each virtual mutation. A failure restores
the exact pre-request entry or removes one loaded only for that request;
sequential `did_change` edits are therefore atomic as a batch. A single source
splice, position scan, parse, or tree traversal remains non-preemptible.

`commit_virtual_file`, `discard_virtual_file`, and `did_close` use the same cap.
Commit retains a final gate immediately before persistence. Discard covers the
current disk-source read and parse, result validation, and a final gate before
replacing buffered state. `did_close` follows the commit path when `persist=true`
and the discard path otherwise; a timeout leaves the entry open. Once
persistence or buffer replacement starts, no later deadline check can return a
timeout after state may have changed. A post-persistence index-sync error leaves
the clean entry open so a later commit or close can retry synchronization. A
single blocking read, parse, tree traversal, write, or index operation remains
non-preemptible.

Index registration, unregister, list, rebuild, and refresh tools accept an
optional `timeout_ms` budget capped at `300000`. Registry listing checks the
budget while collecting and validating entries and around deterministic sorting.
Unregister keeps a final gate after path normalization and immediately before
registry mutation; once removal begins, it returns the actual outcome rather
than a late timeout. For scan-backed tools, the core checks the budget during
workspace traversal, per-file indexing, C include dependency expansion, and
before persistence, then fails without writing a new snapshot when the budget
has expired. The direct graph and neighborhood trace tools accept the same
budget for expansion phases; loading an index or parsing a source overlay is
still a non-preemptible boundary.

`inspect_symbol_index` also accepts the optional budget and fails closed when
freshness or unindexed-file scanning exceeds it; its successful health response
shape is unchanged. `migrate_symbol_index` uses the same cap for path/open,
schema and metadata validation, legacy-row loading, persisted-path checks, and
a final gate before schema mutation. A timeout through that gate leaves the
index unchanged. After the schema transaction starts, no further deadline check
runs; the required source rebuild and final health inspection complete and
return their actual outcome. A single SQLite query, source read, schema
transaction, or rebuild persistence step remains non-preemptible.

The four `list_symbols*` tools, the four `search_symbols*` tools, and all eight
`read_symbol*` tools also accept the optional `timeout_ms` budget. For direct
reads, the budget covers workspace or persisted-index loading, symbol or
position resolution, source reads, trace expansion, and neighborhood source
reads where applicable. Result and truncation response shapes are unchanged.

All eight patch-context tools—from `validate_patch_with_trace_context` through
`validate_patch_with_discovery_context`, including their position variants—accept
the same optional budget. Trace-context budgets span file or overlay setup,
patch validation, baseline and updated trace queries, impact calculation, and
trace-backed result validation. Rich-context budgets cover setup, patch
validation, the updated trace, bounded graph or source-context expansion, and
result validation. A single blocking source read or parse remains a
non-preemptible boundary.

`replay_patch_evidence_against_trace`, `validate_patch_commit_with_trace`, and
`export_patch_diagnostics_sarif` also accept an optional cooperative
`timeout_ms` capped at `300000`. Their native budgets begin after strict JSON
deserialization and cover patch/trace validation boundaries, updated-source
parse boundaries, syntax traversal, evidence or diagnostic collection, and
final result validation. A single JSON decode, source parse, model-validation
call, artifact-URI encoding, result construction, or response serialization
remains non-preemptible.

`preview_workspace_position_edits` accepts the same optional budget across file
validation, source reads, sequential edit application, updated-source parsing,
diff generation, syntax diagnostics, and result validation. A single blocking
source read or parse remains non-preemptible, and no file is written.

`execute_tree_query` accepts an optional `timeout_ms` cooperative budget capped
at `300000`; omitting it keeps the existing `500ms` default. The budget is
checked by Tree-sitter progress callbacks and capture collection.
