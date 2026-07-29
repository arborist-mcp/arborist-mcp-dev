from __future__ import annotations

from .tool_spec_models import ToolParamSpec, _schema


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

TREE_QUERY_MAX_LENGTH = 64 * 1024
TREE_QUERY_MAX_CAPTURES = 100_000
TEXT_PARAM_MAX_LENGTH = 4 * 1024 * 1024
MAX_JSON_ARG_BYTES = 128 * 1024 * 1024
MAX_JSON_ARG_DEPTH = 64
MAX_REQUEST_BYTES = 128 * 1024 * 1024
MAX_INDEX_WATCH_CONFIG_BYTES = 4 * 1024 * 1024
MAX_INDEX_WATCH_TARGETS = 256
BYPASS_REASON_MAX_LENGTH = 4 * 1024
MAX_BATCH_CALLS = 32
MAX_POSITION_EDITS = 10_000
MAX_POSITION_EDIT_TEXT_BYTES = 64 * 1024 * 1024
MAX_SEMANTIC_EXPAND_NODES = 10_000
MAX_SEMANTIC_SKELETON_DEPTH = 64
MAX_WORKSPACE_EDIT_PREVIEW_FILES = 32
MAX_GRAPH_DEPTH = 64
MAX_GRAPH_NODES = 10_000
MAX_SYMBOL_LIMIT = 10_000
MAX_WORKSPACE_SCAN_FILES = 200_000
MAX_WORKSPACE_SCAN_FILE_BYTES = 64 * 1024 * 1024
MAX_WORKSPACE_SCAN_TIMEOUT_MS = 5 * 60 * 1000
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
        string_max_bytes=BYPASS_REASON_MAX_LENGTH,
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
            maximum=MAX_SEMANTIC_SKELETON_DEPTH,
        ),
        optional=True,
        default=2,
        int_max_value=MAX_SEMANTIC_SKELETON_DEPTH,
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
            "maxItems": MAX_POSITION_EDITS,
        }
    ),
    "files": ToolParamSpec(
        {
            "type": "array",
            "description": "Files with ordered position edits to preview without writing to disk.",
            "minItems": 1,
            "maxItems": MAX_WORKSPACE_EDIT_PREVIEW_FILES,
            "items": {
                "type": "object",
                "properties": {
                    "file_path": _schema("string", "Source file path."),
                    "source": _schema(
                        "string",
                        "Optional unsaved source text.",
                        allow_empty=True,
                        max_length=TEXT_PARAM_MAX_LENGTH,
                    ),
                    "edits": {
                        "type": "array",
                        "description": "Ordered LSP-style position edits.",
                        "items": POSITION_EDIT_SCHEMA,
                        "maxItems": MAX_POSITION_EDITS,
                    },
                },
                "required": ["file_path", "edits"],
                "additionalProperties": False,
            },
        }
    ),
    "expand_nodes": ToolParamSpec(
        {
            "type": "array",
            "description": "Semantic selectors to expand in the returned skeleton.",
            "items": _schema("string", "Semantic selector."),
            "maxItems": MAX_SEMANTIC_EXPAND_NODES,
        },
        optional=True,
    ),
    "file_path": ToolParamSpec(
        _schema(
            "string",
            "Source file path. Python (.py, .pyi), C (.c, .h), and C++ extensions are supported; C++ uses the Tree-sitter C++ grammar.",
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
    "timeout_ms": ToolParamSpec(
        _schema(
            "integer",
            "Optional cooperative timeout for workspace scanning, indexing, AST patching and previews, virtual-file operations, symbol queries, trace expansion, and raw Tree-sitter queries in milliseconds.",
            minimum=1,
            maximum=MAX_WORKSPACE_SCAN_TIMEOUT_MS,
        ),
        optional=True,
        int_max_value=MAX_WORKSPACE_SCAN_TIMEOUT_MS,
    ),
    "new_code": ToolParamSpec(
        _schema(
            "string",
            "Replacement source code for the selected AST node.",
            max_length=TEXT_PARAM_MAX_LENGTH,
        ),
        string_max_length=TEXT_PARAM_MAX_LENGTH,
        string_max_bytes=TEXT_PARAM_MAX_LENGTH,
    ),
    "new_text": ToolParamSpec(
        _schema(
            "string",
            "Replacement text for a byte-range edit.",
            allow_empty=True,
            max_length=TEXT_PARAM_MAX_LENGTH,
        ),
        string_max_length=TEXT_PARAM_MAX_LENGTH,
        string_max_bytes=TEXT_PARAM_MAX_LENGTH,
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
        ),
        string_max_length=TREE_QUERY_MAX_LENGTH,
        string_max_bytes=TREE_QUERY_MAX_LENGTH,
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
        string_max_bytes=TEXT_PARAM_MAX_LENGTH,
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
