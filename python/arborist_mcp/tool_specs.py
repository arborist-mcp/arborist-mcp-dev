from __future__ import annotations

from typing import Any, NamedTuple

class ToolSpec(NamedTuple):
    name: str
    handler: str
    params: tuple[str, ...]
    category: str
    result_schema: str = "object"


class ToolParamSpec(NamedTuple):
    schema: dict[str, Any]
    optional: bool = False
    default: Any = None
    string_max_length: int | None = None
    int_max_value: int | None = None
    source_anchored_optional_tools: frozenset[str] = frozenset()


TOOL_SPECS = (
    ToolSpec("arborist/batch", "_batch", ("calls",), "read", "batch"),
    ToolSpec("arborist/get_semantic_skeleton", "_get_semantic_skeleton", ("file_path", "depth_limit", "source", "expand_nodes"), "read", "semantic_skeleton"),
    ToolSpec("arborist/preview_patch_ast_node", "_preview_patch_ast_node", ("file_path", "semantic_path", "new_code", "source", "bypass_reason"), "read", "patch_preview"),
    ToolSpec("arborist/preview_patch_ast_node_at_position", "_preview_patch_ast_node_at_position", ("file_path", "position", "new_code", "source", "bypass_reason"), "read", "patch_preview"),
    ToolSpec("arborist/patch_ast_node", "_patch_ast_node", ("file_path", "semantic_path", "new_code", "source", "bypass_reason"), "write", "patch_ast_node"),
    ToolSpec("arborist/patch_ast_node_at_position", "_patch_ast_node_at_position", ("file_path", "position", "new_code", "source", "bypass_reason"), "write", "patch_ast_node"),
    ToolSpec("arborist/patch_virtual_ast_node", "_patch_virtual_ast_node", ("file_path", "semantic_path", "new_code", "bypass_reason"), "vfs", "patch_ast_node"),
    ToolSpec("arborist/patch_virtual_ast_node_at_position", "_patch_virtual_ast_node_at_position", ("file_path", "position", "new_code", "bypass_reason"), "vfs", "patch_ast_node"),
    ToolSpec("arborist/register_symbol_index", "_register_symbol_index", ("workspace_root", "db_path"), "index", "registered_symbol_index"),
    ToolSpec("arborist/refresh_symbol_index_for_file", "_refresh_symbol_index_for_file", ("workspace_root", "db_path", "file_path", "max_files", "max_file_bytes"), "index", "symbol_index_stats"),
    ToolSpec("arborist/unregister_symbol_index", "_unregister_symbol_index", ("workspace_root",), "index", "boolean"),
    ToolSpec("arborist/list_symbol_indexes", "_list_symbol_indexes", (), "index", "registered_symbol_index_array"),
    ToolSpec("arborist/inspect_symbol_index", "_inspect_symbol_index", ("db_path",), "index", "symbol_index_health"),
    ToolSpec("arborist/did_open", "_did_open", ("file_path", "source"), "vfs", "virtual_file_snapshot"),
    ToolSpec("arborist/did_change", "_did_change", ("file_path", "edits"), "vfs", "virtual_edit"),
    ToolSpec("arborist/did_close", "_did_close", ("file_path", "persist"), "vfs", "virtual_file_snapshot"),
    ToolSpec("arborist/list_virtual_files", "_list_virtual_files", ("dirty_only",), "vfs", "virtual_file_status_array"),
    ToolSpec("arborist/read_virtual_file", "_read_virtual_file", ("file_path",), "vfs", "virtual_file_snapshot"),
    ToolSpec("arborist/apply_buffer_edit", "_apply_buffer_edit", ("file_path", "start_byte", "old_end_byte", "new_text"), "vfs", "virtual_edit"),
    ToolSpec("arborist/commit_virtual_file", "_commit_virtual_file", ("file_path",), "vfs", "virtual_file_snapshot"),
    ToolSpec("arborist/discard_virtual_file", "_discard_virtual_file", ("file_path",), "vfs", "virtual_file_snapshot"),
    ToolSpec("arborist/rebuild_symbol_index", "_rebuild_symbol_index", ("workspace_root", "db_path", "max_files", "max_file_bytes"), "index", "symbol_index_stats"),
    ToolSpec("arborist/refresh_symbol_index", "_refresh_symbol_index", ("workspace_root", "db_path", "max_files", "max_file_bytes"), "index", "symbol_index_stats"),
    ToolSpec("arborist/trace_symbol_graph", "_trace_symbol_graph", ("workspace_root", "symbol_path", "direction", "index_db_path", "file_path", "source"), "trace", "trace_symbol_graph"),
    ToolSpec("arborist/trace_symbol_neighborhood", "_trace_symbol_neighborhood", ("workspace_root", "symbol_path", "direction", "max_depth", "max_nodes", "index_db_path", "file_path", "source"), "trace", "trace_symbol_neighborhood"),
    ToolSpec("arborist/trace_symbol_graph_at_position", "_trace_symbol_graph_at_position", ("workspace_root", "file_path", "position", "direction", "source", "index_db_path"), "trace", "trace_symbol_graph"),
    ToolSpec("arborist/trace_symbol_neighborhood_at_position", "_trace_symbol_neighborhood_at_position", ("workspace_root", "file_path", "position", "direction", "max_depth", "max_nodes", "source", "index_db_path"), "trace", "trace_symbol_neighborhood"),
    ToolSpec("arborist/read_symbol", "_read_symbol", ("workspace_root", "symbol_path", "index_db_path", "file_path", "source"), "read", "symbol_read"),
    ToolSpec("arborist/read_symbol_at_position", "_read_symbol_at_position", ("workspace_root", "file_path", "position", "source", "index_db_path"), "read", "symbol_read"),
    ToolSpec("arborist/read_symbol_context", "_read_symbol_context", ("workspace_root", "symbol_path", "direction", "index_db_path", "file_path", "source"), "read", "symbol_context"),
    ToolSpec("arborist/read_symbol_context_at_position", "_read_symbol_context_at_position", ("workspace_root", "file_path", "position", "direction", "source", "index_db_path"), "read", "symbol_context"),
    ToolSpec("arborist/read_symbol_neighborhood_context", "_read_symbol_neighborhood_context", ("workspace_root", "symbol_path", "direction", "max_depth", "max_nodes", "index_db_path", "file_path", "source"), "read", "symbol_neighborhood_context"),
    ToolSpec("arborist/read_symbol_neighborhood_context_at_position", "_read_symbol_neighborhood_context_at_position", ("workspace_root", "file_path", "position", "direction", "max_depth", "max_nodes", "source", "index_db_path"), "read", "symbol_neighborhood_context"),
    ToolSpec("arborist/read_symbol_discovery_context", "_read_symbol_discovery_context", ("workspace_root", "symbol_path", "direction", "max_depth", "max_nodes", "index_db_path", "file_path", "source"), "read", "symbol_discovery_context"),
    ToolSpec("arborist/read_symbol_discovery_context_at_position", "_read_symbol_discovery_context_at_position", ("workspace_root", "file_path", "position", "direction", "max_depth", "max_nodes", "source", "index_db_path"), "read", "symbol_discovery_context"),
    ToolSpec("arborist/list_symbols", "_list_symbols", ("workspace_root", "limit", "index_db_path", "file_path_contains", "node_kind", "file_path", "source"), "read", "symbol_list"),
    ToolSpec("arborist/list_symbols_context", "_list_symbols_context", ("workspace_root", "limit", "index_db_path", "file_path_contains", "node_kind", "file_path", "source"), "read", "symbol_list_context"),
    ToolSpec("arborist/list_symbols_neighborhood_context", "_list_symbols_neighborhood_context", ("workspace_root", "limit", "direction", "max_depth", "max_nodes", "index_db_path", "file_path_contains", "node_kind", "file_path", "source"), "read", "symbol_list_neighborhood_context"),
    ToolSpec("arborist/list_symbols_discovery_context", "_list_symbols_discovery_context", ("workspace_root", "limit", "direction", "max_depth", "max_nodes", "index_db_path", "file_path_contains", "node_kind", "file_path", "source"), "read", "symbol_list_discovery_context"),
    ToolSpec("arborist/search_symbols", "_search_symbols", ("workspace_root", "query", "limit", "index_db_path", "file_path_contains", "node_kind", "file_path", "source"), "read", "symbol_search"),
    ToolSpec("arborist/search_symbols_context", "_search_symbols_context", ("workspace_root", "query", "limit", "index_db_path", "file_path_contains", "node_kind", "file_path", "source"), "read", "symbol_search_context"),
    ToolSpec("arborist/search_symbols_neighborhood_context", "_search_symbols_neighborhood_context", ("workspace_root", "query", "limit", "direction", "max_depth", "max_nodes", "index_db_path", "file_path_contains", "node_kind", "file_path", "source"), "read", "symbol_search_neighborhood_context"),
    ToolSpec("arborist/search_symbols_discovery_context", "_search_symbols_discovery_context", ("workspace_root", "query", "limit", "direction", "max_depth", "max_nodes", "index_db_path", "file_path_contains", "node_kind", "file_path", "source"), "read", "symbol_search_discovery_context"),
    ToolSpec("arborist/replay_patch_evidence_against_trace", "_replay_patch_evidence_against_trace", ("patch", "trace"), "trace", "trace_patch_evidence_replay"),
    ToolSpec("arborist/validate_patch_commit_with_trace", "_validate_patch_commit_with_trace", ("patch", "trace"), "trace", "patch_trace_validation"),
    ToolSpec("arborist/validate_patch_with_trace_context", "_validate_patch_with_trace_context", ("workspace_root", "file_path", "semantic_path", "new_code", "source", "bypass_reason", "direction", "index_db_path"), "trace", "trace_backed_patch"),
    ToolSpec("arborist/validate_patch_with_trace_context_at_position", "_validate_patch_with_trace_context_at_position", ("workspace_root", "file_path", "position", "new_code", "source", "bypass_reason", "direction", "index_db_path"), "trace", "trace_backed_patch"),
    ToolSpec("arborist/validate_patch_with_graph_context", "_validate_patch_with_graph_context", ("workspace_root", "file_path", "semantic_path", "new_code", "source", "bypass_reason", "direction", "max_depth", "max_nodes", "index_db_path"), "read", "graph_backed_patch"),
    ToolSpec("arborist/validate_patch_with_graph_context_at_position", "_validate_patch_with_graph_context_at_position", ("workspace_root", "file_path", "position", "new_code", "source", "bypass_reason", "direction", "max_depth", "max_nodes", "index_db_path"), "read", "graph_backed_patch"),
    ToolSpec("arborist/validate_patch_with_neighborhood_context", "_validate_patch_with_neighborhood_context", ("workspace_root", "file_path", "semantic_path", "new_code", "source", "bypass_reason", "direction", "max_depth", "max_nodes", "index_db_path"), "read", "neighborhood_context_patch"),
    ToolSpec("arborist/validate_patch_with_neighborhood_context_at_position", "_validate_patch_with_neighborhood_context_at_position", ("workspace_root", "file_path", "position", "new_code", "source", "bypass_reason", "direction", "max_depth", "max_nodes", "index_db_path"), "read", "neighborhood_context_patch"),
    ToolSpec("arborist/validate_patch_with_discovery_context", "_validate_patch_with_discovery_context", ("workspace_root", "file_path", "semantic_path", "new_code", "source", "bypass_reason", "direction", "max_depth", "max_nodes", "index_db_path"), "read", "discovery_context_patch"),
    ToolSpec("arborist/validate_patch_with_discovery_context_at_position", "_validate_patch_with_discovery_context_at_position", ("workspace_root", "file_path", "position", "new_code", "source", "bypass_reason", "direction", "max_depth", "max_nodes", "index_db_path"), "read", "discovery_context_patch"),
    ToolSpec("arborist/execute_tree_query", "_execute_tree_query", ("file_path", "query", "source", "max_captures"), "read", "query_capture_array"),
)
TOOL_NAMES = tuple(spec.name for spec in TOOL_SPECS)
TOOL_SPECS_BY_NAME = {spec.name: spec for spec in TOOL_SPECS}
TOOL_HANDLERS = {spec.name: spec.handler for spec in TOOL_SPECS}
TOOL_PARAM_NAMES = {spec.name: spec.params for spec in TOOL_SPECS}
TOOL_CATEGORIES = {spec.name: spec.category for spec in TOOL_SPECS}


MCP_PROTOCOL_VERSION = "2025-06-18"
MCP_INITIALIZE_PARAM_NAMES = ("protocolVersion", "capabilities", "clientInfo", "_meta")
MCP_INITIALIZED_PARAM_NAMES = ("_meta",)
MCP_TOOL_LIST_PARAM_NAMES = ("cursor", "_meta")
MCP_TOOL_CALL_PARAM_NAMES = ("name", "arguments", "_meta")
MCP_RESOURCE_LIST_PARAM_NAMES = ("cursor", "_meta")
MCP_RESOURCE_READ_PARAM_NAMES = ("uri", "_meta")
TOOL_CATALOG_RESOURCE_URI = "arborist://tool-catalog"
TOOL_CATALOG_RESOURCE_MIME_TYPE = "application/json"
MCP_INITIALIZE_MARKERS = frozenset(("protocolVersion", "capabilities", "clientInfo"))
_SOURCE_ANCHORED_FILE_PATH_TOOLS = frozenset(
    (
        "arborist/trace_symbol_graph",
        "arborist/trace_symbol_neighborhood",
        "arborist/read_symbol",
        "arborist/read_symbol_context",
        "arborist/read_symbol_neighborhood_context",
        "arborist/read_symbol_discovery_context",
        "arborist/list_symbols",
        "arborist/list_symbols_context",
        "arborist/list_symbols_neighborhood_context",
        "arborist/list_symbols_discovery_context",
        "arborist/search_symbols",
        "arborist/search_symbols_context",
        "arborist/search_symbols_neighborhood_context",
        "arborist/search_symbols_discovery_context",
    )
)
READ_ONLY_CATEGORIES = frozenset(("read", "trace"))
TREE_QUERY_MAX_LENGTH = 64 * 1024
TREE_QUERY_MAX_CAPTURES = 100_000
TEXT_PARAM_MAX_LENGTH = 4 * 1024 * 1024
BYPASS_REASON_MAX_LENGTH = 4 * 1024
MAX_BATCH_CALLS = 32
MAX_GRAPH_DEPTH = 64
MAX_GRAPH_NODES = 10_000
MAX_SYMBOL_LIMIT = 10_000
MAX_WORKSPACE_SCAN_FILES = 200_000
MAX_WORKSPACE_SCAN_FILE_BYTES = 64 * 1024 * 1024
WRITING_TOOLS = frozenset(
    (
        "arborist/patch_ast_node",
        "arborist/patch_ast_node_at_position",
        "arborist/commit_virtual_file",
    )
)
NON_MUTATING_STATE_TOOLS = frozenset(
    (
        "arborist/list_virtual_files",
        "arborist/read_virtual_file",
        "arborist/list_symbol_indexes",
        "arborist/inspect_symbol_index",
    )
)
MUTATING_TOOLS = frozenset(
    spec.name
    for spec in TOOL_SPECS
    if spec.category in {"write", "vfs", "index"}
) - NON_MUTATING_STATE_TOOLS
BATCH_ALLOWED_TOOLS = frozenset(
    spec.name
    for spec in TOOL_SPECS
    if (
        (spec.category in READ_ONLY_CATEGORIES or spec.name in NON_MUTATING_STATE_TOOLS)
        and spec.name != "arborist/batch"
    )
)


def tool_spec(tool_name: str) -> ToolSpec:
    return TOOL_SPECS_BY_NAME[tool_name]


def _schema(
    schema_type: str,
    description: str,
    *,
    default: Any = None,
    enum: tuple[str, ...] | None = None,
    minimum: int | None = None,
    maximum: int | None = None,
    min_items: int | None = None,
    max_length: int | None = None,
    allow_empty: bool = False,
) -> dict[str, Any]:
    result: dict[str, Any] = {"type": schema_type, "description": description}
    if default is not None:
        result["default"] = default
    if enum is not None:
        result["enum"] = list(enum)
    if minimum is not None:
        result["minimum"] = minimum
    if maximum is not None:
        result["maximum"] = maximum
    if min_items is not None:
        result["minItems"] = min_items
    if max_length is not None:
        result["maxLength"] = max_length
    if schema_type == "string" and not allow_empty:
        result["minLength"] = 1
    return result


POSITION_SCHEMA = {
    "type": "object",
    "description": "Zero-based Tree-sitter point for position-based lookup or patching.",
    "properties": {
        "row": _schema("integer", "Zero-based row.", minimum=0),
        "column": _schema("integer", "Zero-based column.", minimum=0),
    },
    "required": ["row", "column"],
    "additionalProperties": False,
}
POSITION_EDIT_SCHEMA = {
    "type": "object",
    "description": "LSP-style text edit using zero-based start and end positions.",
    "properties": {
        "start": POSITION_SCHEMA,
        "end": POSITION_SCHEMA,
        "new_text": _schema(
            "string",
            "Replacement text for the range.",
            allow_empty=True,
            max_length=TEXT_PARAM_MAX_LENGTH,
        ),
    },
    "required": ["start", "end", "new_text"],
    "additionalProperties": False,
}
JSON_OBJECT_SCHEMA = {
    "type": "object",
    "description": "JSON object returned by a prior Arborist patch or trace call.",
    "additionalProperties": True,
}
BATCH_CALL_SCHEMA = {
    "type": "object",
    "description": "Read-only Arborist tool call to run inside a batch.",
    "properties": {
        "name": _schema("string", "Arborist tool name to call."),
        "arguments": {
            "type": "object",
            "description": "Arguments for the inner tool call.",
            "additionalProperties": True,
        },
    },
    "required": ["name"],
    "additionalProperties": False,
}
TOOL_PARAM_SPECS = {
    "bypass_reason": ToolParamSpec(
        _schema(
            "string",
            "Required explanation when intentionally bypassing trace-backed commit gates.",
            max_length=BYPASS_REASON_MAX_LENGTH,
        ),
        optional=True,
        string_max_length=BYPASS_REASON_MAX_LENGTH,
    ),
    "calls": ToolParamSpec(
        {
            "type": "array",
            "description": "Read-only Arborist tool calls to execute in order.",
            "items": BATCH_CALL_SCHEMA,
            "minItems": 1,
            "maxItems": MAX_BATCH_CALLS,
        }
    ),
    "db_path": ToolParamSpec(_schema("string", "SQLite symbol-index database path.")),
    "depth_limit": ToolParamSpec(
        _schema(
            "integer",
            "Maximum semantic skeleton expansion depth.",
            default=2,
            minimum=0,
        ),
        optional=True,
        default=2,
    ),
    "direction": ToolParamSpec(
        _schema(
            "string",
            "Graph direction to inspect.",
            default="both",
            enum=("callers", "callees", "both"),
        ),
        optional=True,
        default="both",
    ),
    "dirty_only": ToolParamSpec(
        _schema(
            "boolean",
            "When true, list only virtual files with unsaved changes.",
            default=False,
        ),
        optional=True,
        default=False,
    ),
    "edits": ToolParamSpec(
        {
            "type": "array",
            "description": "Ordered LSP-style position edits to apply to an open virtual file.",
            "items": POSITION_EDIT_SCHEMA,
        }
    ),
    "expand_nodes": ToolParamSpec(
        {
            "type": "array",
            "description": "Semantic selectors to expand in the returned skeleton.",
            "items": _schema("string", "Semantic selector."),
        },
        optional=True,
    ),
    "file_path": ToolParamSpec(
        _schema(
            "string",
            "Source file path. Python (.py, .pyi) and C extensions are supported; .hpp and .hh use the C grammar, not full C++ parsing.",
        ),
        source_anchored_optional_tools=_SOURCE_ANCHORED_FILE_PATH_TOOLS,
    ),
    "file_path_contains": ToolParamSpec(
        _schema(
            "string",
            "Optional substring filter applied to indexed file paths.",
        ),
        optional=True,
    ),
    "index_db_path": ToolParamSpec(
        _schema(
            "string",
            "Optional persisted symbol-index database path.",
        ),
        optional=True,
    ),
    "limit": ToolParamSpec(
        _schema(
            "integer",
            "Maximum number of symbols to return.",
            minimum=0,
            maximum=MAX_SYMBOL_LIMIT,
        ),
        optional=True,
        default={"list": 100, "search": 20},
        int_max_value=MAX_SYMBOL_LIMIT,
    ),
    "max_depth": ToolParamSpec(
        _schema(
            "integer",
            "Maximum graph expansion depth.",
            default=2,
            minimum=0,
            maximum=MAX_GRAPH_DEPTH,
        ),
        optional=True,
        default=2,
        int_max_value=MAX_GRAPH_DEPTH,
    ),
    "max_nodes": ToolParamSpec(
        _schema(
            "integer",
            "Maximum graph node count. Must be greater than zero.",
            default=64,
            minimum=1,
            maximum=MAX_GRAPH_NODES,
        ),
        optional=True,
        default=64,
        int_max_value=MAX_GRAPH_NODES,
    ),
    "max_captures": ToolParamSpec(
        _schema(
            "integer",
            "Maximum Tree-sitter query captures to return. Must be greater than zero.",
            default=10000,
            minimum=1,
            maximum=TREE_QUERY_MAX_CAPTURES,
        ),
        optional=True,
        default=10000,
        int_max_value=TREE_QUERY_MAX_CAPTURES,
    ),
    "max_files": ToolParamSpec(
        _schema(
            "integer",
            "Maximum source files to scan while indexing a workspace. Must be greater than zero.",
            default=20000,
            minimum=1,
            maximum=MAX_WORKSPACE_SCAN_FILES,
        ),
        optional=True,
        default=20000,
        int_max_value=MAX_WORKSPACE_SCAN_FILES,
    ),
    "max_file_bytes": ToolParamSpec(
        _schema(
            "integer",
            "Optional maximum byte size for each source file read while indexing. Must be greater than zero when supplied.",
            minimum=1,
            maximum=MAX_WORKSPACE_SCAN_FILE_BYTES,
        ),
        optional=True,
        int_max_value=MAX_WORKSPACE_SCAN_FILE_BYTES,
    ),
    "new_code": ToolParamSpec(
        _schema(
            "string",
            "Replacement source code for the selected AST node.",
            max_length=TEXT_PARAM_MAX_LENGTH,
        ),
        string_max_length=TEXT_PARAM_MAX_LENGTH,
    ),
    "new_text": ToolParamSpec(
        _schema(
            "string",
            "Replacement text for a byte-range edit.",
            allow_empty=True,
            max_length=TEXT_PARAM_MAX_LENGTH,
        ),
        string_max_length=TEXT_PARAM_MAX_LENGTH,
    ),
    "node_kind": ToolParamSpec(
        _schema("string", "Optional Tree-sitter node-kind filter."),
        optional=True,
    ),
    "old_end_byte": ToolParamSpec(
        _schema(
            "integer",
            "Exclusive end byte of the old range.",
            minimum=0,
        )
    ),
    "patch": ToolParamSpec(JSON_OBJECT_SCHEMA),
    "persist": ToolParamSpec(
        _schema(
            "boolean",
            "When closing a virtual file, commit changes to disk before closing.",
            default=False,
        ),
        optional=True,
        default=False,
    ),
    "position": ToolParamSpec(POSITION_SCHEMA),
    "query": ToolParamSpec(
        _schema(
            "string",
            "Tree-sitter query or symbol search text.",
            max_length=TREE_QUERY_MAX_LENGTH,
        )
    ),
    "semantic_path": ToolParamSpec(_schema("string", "Stable Arborist semantic selector.")),
    "source": ToolParamSpec(
        _schema(
            "string",
            "Optional unsaved source buffer to analyze instead of reading from disk.",
            allow_empty=True,
            max_length=TEXT_PARAM_MAX_LENGTH,
        ),
        optional=True,
        string_max_length=TEXT_PARAM_MAX_LENGTH,
    ),
    "start_byte": ToolParamSpec(
        _schema("integer", "Inclusive start byte for a buffer edit.", minimum=0)
    ),
    "symbol_path": ToolParamSpec(_schema("string", "Stable symbol path or symbol_id selector.")),
    "trace": ToolParamSpec(JSON_OBJECT_SCHEMA),
    "workspace_root": ToolParamSpec(
        _schema(
            "string",
            "Workspace root for index, trace, and symbol operations.",
            default=".",
        ),
        optional=True,
        default=".",
    ),
}
def tool_param_spec(param_name: str) -> ToolParamSpec:
    return TOOL_PARAM_SPECS[param_name]


TOOL_PARAM_SCHEMAS = {
    name: spec.schema for name, spec in TOOL_PARAM_SPECS.items()
}
OPTIONAL_TOOL_PARAMS = frozenset(
    name for name, spec in TOOL_PARAM_SPECS.items() if spec.optional
)
SOURCE_ANCHORED_OPTIONAL_FILE_PATH_TOOLS = frozenset(
    tool_name
    for spec in TOOL_PARAM_SPECS.values()
    for tool_name in spec.source_anchored_optional_tools
)
TOOL_PARAM_DEFAULTS = {
    name: spec.default
    for name, spec in TOOL_PARAM_SPECS.items()
    if spec.default is not None
}
STRING_PARAM_MAX_LENGTHS = {
    name: spec.string_max_length
    for name, spec in TOOL_PARAM_SPECS.items()
    if spec.string_max_length is not None
}
