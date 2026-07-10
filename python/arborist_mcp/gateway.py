from __future__ import annotations

import argparse
import importlib
import json
import math
import sys
from pathlib import Path
from typing import Any, NamedTuple

from . import __version__


class ToolSpec(NamedTuple):
    name: str
    handler: str
    params: tuple[str, ...]
    category: str
    result_schema: str = "object"


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
    ToolSpec("arborist/refresh_symbol_index_for_file", "_refresh_symbol_index_for_file", ("workspace_root", "db_path", "file_path", "max_files"), "index", "symbol_index_stats"),
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
    ToolSpec("arborist/rebuild_symbol_index", "_rebuild_symbol_index", ("workspace_root", "db_path", "max_files"), "index", "symbol_index_stats"),
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
TOOL_HANDLERS = {spec.name: spec.handler for spec in TOOL_SPECS}
TOOL_PARAM_NAMES = {spec.name: spec.params for spec in TOOL_SPECS}
TOOL_CATEGORIES = {spec.name: spec.category for spec in TOOL_SPECS}
TOOL_RESULT_SCHEMA_KEYS = {
    spec.name: spec.result_schema
    for spec in TOOL_SPECS
    if spec.result_schema != "object"
}


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
OPTIONAL_TOOL_PARAMS = frozenset(
    (
        "bypass_reason",
        "depth_limit",
        "direction",
        "dirty_only",
        "expand_nodes",
        "file_path_contains",
        "index_db_path",
        "limit",
        "max_captures",
        "max_depth",
        "max_files",
        "max_nodes",
        "node_kind",
        "persist",
        "source",
        "workspace_root",
    )
)
SOURCE_ANCHORED_OPTIONAL_FILE_PATH_TOOLS = frozenset(
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
TEXT_PARAM_MAX_LENGTH = 4 * 1024 * 1024
BYPASS_REASON_MAX_LENGTH = 4 * 1024
MAX_BATCH_CALLS = 32
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
    tool_name
    for tool_name, category in TOOL_CATEGORIES.items()
    if category in {"write", "vfs", "index"}
) - NON_MUTATING_STATE_TOOLS
BATCH_ALLOWED_TOOLS = frozenset(
    tool_name
    for tool_name, category in TOOL_CATEGORIES.items()
    if (
        (category in READ_ONLY_CATEGORIES or tool_name in NON_MUTATING_STATE_TOOLS)
        and tool_name != "arborist/batch"
    )
)


def _schema(
    schema_type: str,
    description: str,
    *,
    default: Any = None,
    enum: tuple[str, ...] | None = None,
    minimum: int | None = None,
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
TOOL_PARAM_SCHEMAS = {
    "bypass_reason": _schema(
        "string",
        "Required explanation when intentionally bypassing trace-backed commit gates.",
        max_length=BYPASS_REASON_MAX_LENGTH,
    ),
    "calls": {
        "type": "array",
        "description": "Read-only Arborist tool calls to execute in order.",
        "items": BATCH_CALL_SCHEMA,
        "minItems": 1,
        "maxItems": MAX_BATCH_CALLS,
    },
    "db_path": _schema("string", "SQLite symbol-index database path."),
    "depth_limit": _schema(
        "integer",
        "Maximum semantic skeleton expansion depth.",
        default=2,
        minimum=0,
    ),
    "direction": _schema(
        "string",
        "Graph direction to inspect.",
        default="both",
        enum=("callers", "callees", "both"),
    ),
    "dirty_only": _schema(
        "boolean",
        "When true, list only virtual files with unsaved changes.",
        default=False,
    ),
    "edits": {
        "type": "array",
        "description": "Ordered LSP-style position edits to apply to an open virtual file.",
        "items": POSITION_EDIT_SCHEMA,
    },
    "expand_nodes": {
        "type": "array",
        "description": "Semantic selectors to expand in the returned skeleton.",
        "items": _schema("string", "Semantic selector."),
    },
    "file_path": _schema(
        "string",
        "Source file path. Python and C extensions are supported; .hpp and .hh use the C grammar, not full C++ parsing.",
    ),
    "file_path_contains": _schema(
        "string",
        "Optional substring filter applied to indexed file paths.",
    ),
    "index_db_path": _schema(
        "string",
        "Optional persisted symbol-index database path.",
    ),
    "limit": _schema("integer", "Maximum number of symbols to return.", minimum=0),
    "max_depth": _schema(
        "integer",
        "Maximum graph expansion depth.",
        default=2,
        minimum=0,
    ),
    "max_nodes": _schema(
        "integer",
        "Maximum graph node count. Must be greater than zero.",
        default=64,
        minimum=1,
    ),
    "max_captures": _schema(
        "integer",
        "Maximum Tree-sitter query captures to return. Must be greater than zero.",
        default=10000,
        minimum=1,
    ),
    "max_files": _schema(
        "integer",
        "Maximum source files to scan while indexing a workspace. Must be greater than zero.",
        default=20000,
        minimum=1,
    ),
    "new_code": _schema(
        "string",
        "Replacement source code for the selected AST node.",
        max_length=TEXT_PARAM_MAX_LENGTH,
    ),
    "new_text": _schema(
        "string",
        "Replacement text for a byte-range edit.",
        allow_empty=True,
        max_length=TEXT_PARAM_MAX_LENGTH,
    ),
    "node_kind": _schema("string", "Optional Tree-sitter node-kind filter."),
    "old_end_byte": _schema(
        "integer",
        "Exclusive end byte of the old range.",
        minimum=0,
    ),
    "patch": JSON_OBJECT_SCHEMA,
    "persist": _schema(
        "boolean",
        "When closing a virtual file, commit changes to disk before closing.",
        default=False,
    ),
    "position": POSITION_SCHEMA,
    "query": _schema(
        "string",
        "Tree-sitter query or symbol search text.",
        max_length=TREE_QUERY_MAX_LENGTH,
    ),
    "semantic_path": _schema("string", "Stable Arborist semantic selector."),
    "source": _schema(
        "string",
        "Optional unsaved source buffer to analyze instead of reading from disk.",
        allow_empty=True,
        max_length=TEXT_PARAM_MAX_LENGTH,
    ),
    "start_byte": _schema("integer", "Inclusive start byte for a buffer edit.", minimum=0),
    "symbol_path": _schema("string", "Stable symbol path or symbol_id selector."),
    "trace": JSON_OBJECT_SCHEMA,
    "workspace_root": _schema(
        "string",
        "Workspace root for index, trace, and symbol operations.",
        default=".",
    ),
}
TOOL_PARAM_DEFAULTS = {
    "depth_limit": 2,
    "direction": "both",
    "dirty_only": False,
    "limit": {
        "list": 100,
        "search": 20,
    },
    "max_depth": 2,
    "max_nodes": 64,
    "max_captures": 10000,
    "max_files": 20000,
    "persist": False,
    "workspace_root": ".",
}
STRING_PARAM_MAX_LENGTHS = {
    "bypass_reason": BYPASS_REASON_MAX_LENGTH,
    "new_code": TEXT_PARAM_MAX_LENGTH,
    "new_text": TEXT_PARAM_MAX_LENGTH,
    "source": TEXT_PARAM_MAX_LENGTH,
}
OBJECT_RESULT_SCHEMA = {
    "type": "object",
    "description": "JSON object result returned by Arborist for this tool.",
    "additionalProperties": True,
}
OBJECT_ARRAY_RESULT_SCHEMA = {
    "type": "array",
    "description": "JSON array of object results returned by Arborist for this tool.",
    "items": OBJECT_RESULT_SCHEMA,
}
BATCH_CALL_RESULT_SCHEMA = {
    "type": "object",
    "description": "Result returned by one inner batch call.",
    "properties": {
        "name": _schema("string", "Arborist tool name that was called."),
        "result": {
            "description": "Result returned by the inner tool. Filled from the batch-allowed tool schemas below.",
        },
    },
    "required": ["name", "result"],
    "additionalProperties": False,
}
BATCH_RESULT_SCHEMA = {
    "type": "array",
    "description": "Ordered results for the requested read-only batch calls.",
    "items": BATCH_CALL_RESULT_SCHEMA,
}
BOOLEAN_RESULT_SCHEMA = {
    "type": "boolean",
    "description": "Boolean success result returned by Arborist for this tool.",
}
NULL_RESULT_SCHEMA = {"type": "null"}
NULLABLE_STRING_RESULT_SCHEMA = {"anyOf": [_schema("string", "String value."), NULL_RESULT_SCHEMA]}
NULLABLE_INTEGER_RESULT_SCHEMA = {
    "anyOf": [_schema("integer", "Integer value.", minimum=0), NULL_RESULT_SCHEMA]
}
POSITION_RESULT_SCHEMA = {
    "type": "object",
    "description": "Zero-based source position.",
    "properties": {
        "row": _schema("integer", "Zero-based row.", minimum=0),
        "column": _schema("integer", "Zero-based UTF-8 byte column.", minimum=0),
    },
    "required": ["row", "column"],
    "additionalProperties": False,
}
BYTE_RANGE_RESULT_SCHEMA = {
    "type": "array",
    "description": "Inclusive start and exclusive end byte offsets.",
    "items": _schema("integer", "Byte offset.", minimum=0),
    "minItems": 2,
    "maxItems": 2,
}
STRING_ARRAY_RESULT_SCHEMA = {
    "type": "array",
    "description": "String values.",
    "items": _schema("string", "String value."),
}
SEMANTIC_SKELETON_SYMBOL_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol metadata available from a semantic skeleton.",
    "properties": {
        "symbol_id": _schema("string", "Stable symbol identifier."),
        "semantic_path": _schema("string", "Stable Arborist semantic selector."),
        "scope_path": NULLABLE_STRING_RESULT_SCHEMA,
        "node_kind": _schema("string", "Tree-sitter node kind."),
        "byte_range": BYTE_RANGE_RESULT_SCHEMA,
        "signature": NULLABLE_STRING_RESULT_SCHEMA,
        "parameters": STRING_ARRAY_RESULT_SCHEMA,
        "return_type": NULLABLE_STRING_RESULT_SCHEMA,
        "docstring": NULLABLE_STRING_RESULT_SCHEMA,
    },
    "required": [
        "symbol_id",
        "semantic_path",
        "scope_path",
        "node_kind",
        "byte_range",
        "signature",
        "parameters",
        "return_type",
        "docstring",
    ],
    "additionalProperties": False,
}
SEMANTIC_SKELETON_RESULT_SCHEMA = {
    "type": "object",
    "description": "Semantic skeleton and available semantic selectors for a source file.",
    "properties": {
        "file": _schema("string", "Normalized source file path."),
        "skeleton": _schema("string", "Semantic skeleton text.", allow_empty=True),
        "available_paths": {
            "type": "array",
            "description": "Semantic selectors available for expansion or patching.",
            "items": _schema("string", "Semantic selector."),
        },
        "available_symbols": {
            "type": "array",
            "description": "Symbol metadata aligned with available_paths.",
            "items": SEMANTIC_SKELETON_SYMBOL_RESULT_SCHEMA,
        },
    },
    "required": ["file", "skeleton", "available_paths", "available_symbols"],
    "additionalProperties": False,
}
SYMBOL_SUMMARY_RESULT_SCHEMA = {
    "type": "object",
    "description": "Compact symbol metadata.",
    "properties": {
        "symbol_id": _schema("string", "Stable symbol identifier."),
        "semantic_path": _schema("string", "Stable Arborist semantic selector."),
        "scope_path": NULLABLE_STRING_RESULT_SCHEMA,
        "file_path": _schema("string", "Normalized source file path."),
        "node_kind": _schema("string", "Tree-sitter node kind."),
        "origin_type": _schema("string", "Symbol origin classification."),
        "evidence_key": _schema("string", "Trace evidence identity key."),
        "byte_range": BYTE_RANGE_RESULT_SCHEMA,
        "signature": NULLABLE_STRING_RESULT_SCHEMA,
        "parameters": STRING_ARRAY_RESULT_SCHEMA,
        "return_type": NULLABLE_STRING_RESULT_SCHEMA,
        "docstring": NULLABLE_STRING_RESULT_SCHEMA,
    },
    "required": [
        "symbol_id",
        "semantic_path",
        "scope_path",
        "file_path",
        "node_kind",
        "origin_type",
        "evidence_key",
        "byte_range",
        "signature",
        "parameters",
        "return_type",
        "docstring",
    ],
    "additionalProperties": False,
}
SYMBOL_META_RESULT_SCHEMA = {
    "type": "object",
    "description": "Resolved symbol metadata including graph relationships.",
    "properties": {
        **SYMBOL_SUMMARY_RESULT_SCHEMA["properties"],
        "dependencies": STRING_ARRAY_RESULT_SCHEMA,
        "references": STRING_ARRAY_RESULT_SCHEMA,
    },
    "required": [
        *SYMBOL_SUMMARY_RESULT_SCHEMA["required"],
        "dependencies",
        "references",
    ],
    "additionalProperties": False,
}
TRACE_EVIDENCE_KEYS_RESULT_SCHEMA = {
    "type": "object",
    "description": "Trace evidence keys for the root symbol and adjacent symbols.",
    "properties": {
        "symbol": _schema("string", "Root symbol evidence key."),
        "callers": STRING_ARRAY_RESULT_SCHEMA,
        "callees": STRING_ARRAY_RESULT_SCHEMA,
    },
    "required": ["symbol", "callers", "callees"],
    "additionalProperties": False,
}
TRACE_SYMBOL_GRAPH_RESULT_SCHEMA = {
    "type": "object",
    "description": "One-hop caller/callee symbol graph.",
    "properties": {
        "symbol": SYMBOL_META_RESULT_SCHEMA,
        "callers": {
            "type": "array",
            "description": "Direct caller symbols.",
            "items": SYMBOL_SUMMARY_RESULT_SCHEMA,
        },
        "callees": {
            "type": "array",
            "description": "Direct callee symbols.",
            "items": SYMBOL_SUMMARY_RESULT_SCHEMA,
        },
        "evidence_keys": TRACE_EVIDENCE_KEYS_RESULT_SCHEMA,
        "indexed_files": _schema("integer", "Number of indexed files.", minimum=0),
    },
    "required": ["symbol", "callers", "callees", "evidence_keys", "indexed_files"],
    "additionalProperties": False,
}
TRACE_NEIGHBORHOOD_NODE_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol neighborhood node with traversal depth.",
    "properties": {
        "symbol": SYMBOL_SUMMARY_RESULT_SCHEMA,
        "depth": _schema("integer", "Traversal depth from root symbol.", minimum=0),
    },
    "required": ["symbol", "depth"],
    "additionalProperties": False,
}
TRACE_NEIGHBORHOOD_EDGE_RESULT_SCHEMA = {
    "type": "object",
    "description": "Directed edge between two symbol identifiers.",
    "properties": {
        "from_symbol_id": _schema("string", "Source symbol identifier."),
        "to_symbol_id": _schema("string", "Target symbol identifier."),
    },
    "required": ["from_symbol_id", "to_symbol_id"],
    "additionalProperties": False,
}
TRACE_SYMBOL_NEIGHBORHOOD_RESULT_SCHEMA = {
    "type": "object",
    "description": "Bounded caller/callee symbol neighborhood.",
    "properties": {
        "symbol": SYMBOL_META_RESULT_SCHEMA,
        "direction": _schema(
            "string",
            "Graph direction used for traversal.",
            enum=("callers", "callees", "both"),
        ),
        "max_depth": _schema("integer", "Configured traversal depth.", minimum=0),
        "max_nodes": _schema("integer", "Configured node limit.", minimum=0),
        "truncated": _schema("boolean", "Whether traversal stopped at the node limit."),
        "indexed_files": _schema("integer", "Number of indexed files.", minimum=0),
        "nodes": {
            "type": "array",
            "description": "Neighborhood nodes.",
            "items": TRACE_NEIGHBORHOOD_NODE_RESULT_SCHEMA,
        },
        "edges": {
            "type": "array",
            "description": "Neighborhood directed edges.",
            "items": TRACE_NEIGHBORHOOD_EDGE_RESULT_SCHEMA,
        },
    },
    "required": [
        "symbol",
        "direction",
        "max_depth",
        "max_nodes",
        "truncated",
        "indexed_files",
        "nodes",
        "edges",
    ],
    "additionalProperties": False,
}
SYMBOL_READ_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol source slice and location.",
    "properties": {
        "indexed_files": _schema("integer", "Number of indexed files.", minimum=0),
        "symbol": SYMBOL_SUMMARY_RESULT_SCHEMA,
        "source": _schema("string", "Selected symbol source text.", allow_empty=True),
        "start_point": POSITION_RESULT_SCHEMA,
        "end_point": POSITION_RESULT_SCHEMA,
    },
    "required": ["indexed_files", "symbol", "source", "start_point", "end_point"],
    "additionalProperties": False,
}
SYMBOL_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol read result plus one-hop trace context.",
    "properties": {
        "read": SYMBOL_READ_RESULT_SCHEMA,
        "trace": TRACE_SYMBOL_GRAPH_RESULT_SCHEMA,
    },
    "required": ["read", "trace"],
    "additionalProperties": False,
}
SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol neighborhood plus source reads for included nodes.",
    "properties": {
        "neighborhood": TRACE_SYMBOL_NEIGHBORHOOD_RESULT_SCHEMA,
        "reads": {
            "type": "array",
            "description": "Source reads for neighborhood symbols.",
            "items": SYMBOL_READ_RESULT_SCHEMA,
        },
    },
    "required": ["neighborhood", "reads"],
    "additionalProperties": False,
}
SYMBOL_DISCOVERY_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Read, trace, and neighborhood context for symbol discovery.",
    "properties": {
        "read": SYMBOL_READ_RESULT_SCHEMA,
        "trace": TRACE_SYMBOL_GRAPH_RESULT_SCHEMA,
        "neighborhood_context": SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
    },
    "required": ["read", "trace", "neighborhood_context"],
    "additionalProperties": False,
}
SYMBOL_LIST_RESULT_SCHEMA = {
    "type": "object",
    "description": "Bounded symbol list.",
    "properties": {
        "indexed_files": _schema("integer", "Number of indexed files.", minimum=0),
        "total_symbols": _schema("integer", "Total matching symbols before truncation.", minimum=0),
        "truncated": _schema("boolean", "Whether results were truncated by limit."),
        "symbols": {
            "type": "array",
            "description": "Symbol summaries.",
            "items": SYMBOL_SUMMARY_RESULT_SCHEMA,
        },
    },
    "required": ["indexed_files", "total_symbols", "truncated", "symbols"],
    "additionalProperties": False,
}
SYMBOL_LIST_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol list plus source reads.",
    "properties": {
        "list": SYMBOL_LIST_RESULT_SCHEMA,
        "reads": {
            "type": "array",
            "description": "Source reads for listed symbols.",
            "items": SYMBOL_READ_RESULT_SCHEMA,
        },
    },
    "required": ["list", "reads"],
    "additionalProperties": False,
}
SYMBOL_LIST_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol list plus neighborhood contexts.",
    "properties": {
        "list": SYMBOL_LIST_RESULT_SCHEMA,
        "contexts": {
            "type": "array",
            "description": "Neighborhood contexts for listed symbols.",
            "items": SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
        },
    },
    "required": ["list", "contexts"],
    "additionalProperties": False,
}
SYMBOL_LIST_DISCOVERY_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol list plus reads and neighborhood contexts.",
    "properties": {
        "list": SYMBOL_LIST_RESULT_SCHEMA,
        "reads": {
            "type": "array",
            "description": "Source reads for listed symbols.",
            "items": SYMBOL_READ_RESULT_SCHEMA,
        },
        "contexts": {
            "type": "array",
            "description": "Neighborhood contexts for listed symbols.",
            "items": SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
        },
    },
    "required": ["list", "reads", "contexts"],
    "additionalProperties": False,
}
SYMBOL_SEARCH_MATCH_DETAIL_RESULT_SCHEMA = {
    "type": "object",
    "description": "Matched fields and score for a search hit.",
    "properties": {
        "symbol_id": _schema("string", "Matched symbol identifier."),
        "score": _schema("integer", "Lower scores are better matches.", minimum=0),
        "matched_fields": STRING_ARRAY_RESULT_SCHEMA,
    },
    "required": ["symbol_id", "score", "matched_fields"],
    "additionalProperties": False,
}
SYMBOL_SEARCH_RESULT_SCHEMA = {
    "type": "object",
    "description": "Bounded symbol search result.",
    "properties": {
        "query": _schema("string", "Search query."),
        "indexed_files": _schema("integer", "Number of indexed files.", minimum=0),
        "total_matches": _schema("integer", "Total matches before truncation.", minimum=0),
        "truncated": _schema("boolean", "Whether results were truncated by limit."),
        "matches": {
            "type": "array",
            "description": "Matched symbol summaries.",
            "items": SYMBOL_SUMMARY_RESULT_SCHEMA,
        },
        "match_details": {
            "type": "array",
            "description": "Search scoring and matched fields.",
            "items": SYMBOL_SEARCH_MATCH_DETAIL_RESULT_SCHEMA,
        },
    },
    "required": [
        "query",
        "indexed_files",
        "total_matches",
        "truncated",
        "matches",
        "match_details",
    ],
    "additionalProperties": False,
}
SYMBOL_SEARCH_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol search plus source reads.",
    "properties": {
        "search": SYMBOL_SEARCH_RESULT_SCHEMA,
        "reads": {
            "type": "array",
            "description": "Source reads for search matches.",
            "items": SYMBOL_READ_RESULT_SCHEMA,
        },
    },
    "required": ["search", "reads"],
    "additionalProperties": False,
}
SYMBOL_SEARCH_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol search plus neighborhood contexts.",
    "properties": {
        "search": SYMBOL_SEARCH_RESULT_SCHEMA,
        "contexts": {
            "type": "array",
            "description": "Neighborhood contexts for search matches.",
            "items": SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
        },
    },
    "required": ["search", "contexts"],
    "additionalProperties": False,
}
SYMBOL_SEARCH_DISCOVERY_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Symbol search plus reads and neighborhood contexts.",
    "properties": {
        "search": SYMBOL_SEARCH_RESULT_SCHEMA,
        "reads": {
            "type": "array",
            "description": "Source reads for search matches.",
            "items": SYMBOL_READ_RESULT_SCHEMA,
        },
        "contexts": {
            "type": "array",
            "description": "Neighborhood contexts for search matches.",
            "items": SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
        },
    },
    "required": ["search", "reads", "contexts"],
    "additionalProperties": False,
}
VALIDATION_ISSUE_RESULT_SCHEMA = {
    "type": "object",
    "description": "Tree-sitter validation issue with byte and point ranges.",
    "properties": {
        "kind": _schema("string", "Validation issue kind."),
        "message": _schema("string", "Validation issue message."),
        "start_byte": _schema("integer", "Inclusive start byte.", minimum=0),
        "end_byte": _schema("integer", "Exclusive end byte.", minimum=0),
        "start_point": POSITION_RESULT_SCHEMA,
        "end_point": POSITION_RESULT_SCHEMA,
    },
    "required": ["kind", "message", "start_byte", "end_byte", "start_point", "end_point"],
    "additionalProperties": False,
}
VALIDATION_BINDING_RESULT_SCHEMA = {
    "type": "object",
    "description": "Resolved validation binding.",
    "properties": {
        "name": _schema("string", "Identifier name."),
        "symbol": SYMBOL_SUMMARY_RESULT_SCHEMA,
    },
    "required": ["name", "symbol"],
    "additionalProperties": False,
}
DISAMBIGUATION_CONTEXT_RESULT_SCHEMA = {
    "type": "object",
    "description": "C include-family disambiguation context.",
    "properties": {
        "active_include_family": NULLABLE_STRING_RESULT_SCHEMA,
        "preferred_family": NULLABLE_STRING_RESULT_SCHEMA,
        "visible_include_families": STRING_ARRAY_RESULT_SCHEMA,
        "candidate_include_families": STRING_ARRAY_RESULT_SCHEMA,
        "candidate_symbol_ids": STRING_ARRAY_RESULT_SCHEMA,
    },
    "required": [
        "active_include_family",
        "preferred_family",
        "visible_include_families",
        "candidate_include_families",
        "candidate_symbol_ids",
    ],
    "additionalProperties": False,
}
VALIDATION_AMBIGUITY_RESULT_SCHEMA = {
    "type": "object",
    "description": "Ambiguous identifier validation result.",
    "properties": {
        "name": _schema("string", "Identifier name."),
        "candidates": {
            "type": "array",
            "description": "Candidate symbols for the identifier.",
            "items": SYMBOL_SUMMARY_RESULT_SCHEMA,
        },
        "reason": _schema("string", "Why the identifier is ambiguous."),
        "disambiguation_context": DISAMBIGUATION_CONTEXT_RESULT_SCHEMA,
    },
    "required": ["name", "candidates", "reason", "disambiguation_context"],
    "additionalProperties": False,
}
VALIDATION_BINDING_DECISION_RESULT_SCHEMA = {
    "type": "object",
    "description": "Patch validation binding decision.",
    "properties": {
        "name": _schema("string", "Identifier name."),
        "status": _schema("string", "Decision status."),
        "reason": _schema("string", "Decision reason."),
        "selected_symbol_id": NULLABLE_STRING_RESULT_SCHEMA,
        "candidates": {
            "type": "array",
            "description": "Candidate symbols considered by the decision.",
            "items": SYMBOL_SUMMARY_RESULT_SCHEMA,
        },
    },
    "required": ["name", "status", "reason", "selected_symbol_id", "candidates"],
    "additionalProperties": False,
}
PATCH_EVIDENCE_INVARIANT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Trace evidence invariant checked by the commit gate.",
    "properties": {
        "name": _schema("string", "Invariant name."),
        "status": _schema("string", "Invariant status."),
        "reason": _schema("string", "Invariant reason."),
        "selected_evidence_key": NULLABLE_STRING_RESULT_SCHEMA,
        "candidate_evidence_keys": STRING_ARRAY_RESULT_SCHEMA,
    },
    "required": [
        "name",
        "status",
        "reason",
        "selected_evidence_key",
        "candidate_evidence_keys",
    ],
    "additionalProperties": False,
}
PATCH_COMMIT_GATE_RESULT_SCHEMA = {
    "type": "object",
    "description": "Commit gate decision for a patch result.",
    "properties": {
        "status": _schema("string", "Commit gate status."),
        "allowed": _schema("boolean", "Whether the patch may be committed."),
        "reason": _schema("string", "Commit gate reason."),
        "bypass_reason": NULLABLE_STRING_RESULT_SCHEMA,
        "blocking_decisions": {
            "type": "array",
            "description": "Binding decisions that block a normal commit.",
            "items": VALIDATION_BINDING_DECISION_RESULT_SCHEMA,
        },
        "evidence_invariants": {
            "type": "array",
            "description": "Trace evidence invariants evaluated by the gate.",
            "items": PATCH_EVIDENCE_INVARIANT_RESULT_SCHEMA,
        },
        "syntax_error_count": _schema("integer", "Number of syntax errors.", minimum=0),
    },
    "required": [
        "status",
        "allowed",
        "reason",
        "bypass_reason",
        "blocking_decisions",
        "evidence_invariants",
        "syntax_error_count",
    ],
    "additionalProperties": False,
}
PATCH_VALIDATION_RESULT_SCHEMA = {
    "type": "object",
    "description": "Patch validation audit report.",
    "properties": {
        "syntax_errors": {
            "type": "array",
            "description": "Syntax errors detected after patching.",
            "items": VALIDATION_ISSUE_RESULT_SCHEMA,
        },
        "unresolved_identifiers": STRING_ARRAY_RESULT_SCHEMA,
        "resolved_identifiers": {
            "type": "array",
            "description": "Identifiers resolved during validation.",
            "items": VALIDATION_BINDING_RESULT_SCHEMA,
        },
        "ambiguous_identifiers": {
            "type": "array",
            "description": "Identifiers that matched multiple candidate symbols.",
            "items": VALIDATION_AMBIGUITY_RESULT_SCHEMA,
        },
        "binding_decisions": {
            "type": "array",
            "description": "Binding decisions made by validation.",
            "items": VALIDATION_BINDING_DECISION_RESULT_SCHEMA,
        },
        "commit_gate": PATCH_COMMIT_GATE_RESULT_SCHEMA,
    },
    "required": [
        "syntax_errors",
        "unresolved_identifiers",
        "resolved_identifiers",
        "ambiguous_identifiers",
        "binding_decisions",
        "commit_gate",
    ],
    "additionalProperties": False,
}
PATCH_AST_NODE_RESULT_SCHEMA = {
    "type": "object",
    "description": "Semantic patch result.",
    "properties": {
        "file": _schema("string", "Normalized patched file path."),
        "target_path": _schema("string", "Requested semantic target path."),
        "resolved_path": _schema("string", "Resolved semantic target path."),
        "resolved_symbol_id": _schema("string", "Resolved target symbol identifier."),
        "applied": _schema("boolean", "Whether the patch was applied."),
        "bypass_applied": _schema("boolean", "Whether a bypass reason was used."),
        "updated_source": _schema("string", "Updated source text.", allow_empty=True),
        "validation": PATCH_VALIDATION_RESULT_SCHEMA,
    },
    "required": [
        "file",
        "target_path",
        "resolved_path",
        "resolved_symbol_id",
        "applied",
        "bypass_applied",
        "updated_source",
        "validation",
    ],
    "additionalProperties": False,
}
PATCH_PREVIEW_RESULT_SCHEMA = {
    "type": "object",
    "description": "Dry-run semantic patch preview.",
    "properties": {
        "patch": PATCH_AST_NODE_RESULT_SCHEMA,
        "unified_diff": _schema("string", "Unified diff for the preview.", allow_empty=True),
        "changed": _schema("boolean", "Whether the preview changes source text."),
    },
    "required": ["patch", "unified_diff", "changed"],
    "additionalProperties": False,
}
VIRTUAL_EDIT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Virtual file edit result.",
    "properties": {
        "file": _schema("string", "Normalized virtual file path."),
        "source": _schema("string", "Current virtual buffer source.", allow_empty=True),
        "dirty": _schema("boolean", "Whether the virtual buffer differs from disk."),
        "version": _schema("integer", "Virtual buffer version.", minimum=0),
        "incremental_parse": _schema("boolean", "Whether Tree-sitter reused incremental parsing."),
        "validation": PATCH_VALIDATION_RESULT_SCHEMA,
    },
    "required": ["file", "source", "dirty", "version", "incremental_parse", "validation"],
    "additionalProperties": False,
}
TRACE_PATCH_EVIDENCE_REPLAY_ITEM_RESULT_SCHEMA = {
    "type": "object",
    "description": "Single trace evidence replay check.",
    "properties": {
        "name": _schema("string", "Replay item name."),
        "status": _schema("string", "Replay item status."),
        "selected_evidence_key": NULLABLE_STRING_RESULT_SCHEMA,
        "matched_in_trace": _schema("boolean", "Whether selected evidence was found in trace."),
        "trace_match_scope": _schema("string", "Where the evidence matched."),
        "candidate_evidence_keys": STRING_ARRAY_RESULT_SCHEMA,
    },
    "required": [
        "name",
        "status",
        "selected_evidence_key",
        "matched_in_trace",
        "trace_match_scope",
        "candidate_evidence_keys",
    ],
    "additionalProperties": False,
}
TRACE_PATCH_EVIDENCE_REPLAY_RESULT_SCHEMA = {
    "type": "object",
    "description": "Trace evidence replay result.",
    "properties": {
        "consistent": _schema("boolean", "Whether replay is consistent with the trace."),
        "matched_items": _schema("integer", "Number of matched replay items.", minimum=0),
        "blocked_items": _schema("integer", "Number of blocked replay items.", minimum=0),
        "items": {
            "type": "array",
            "description": "Replay item details.",
            "items": TRACE_PATCH_EVIDENCE_REPLAY_ITEM_RESULT_SCHEMA,
        },
    },
    "required": ["consistent", "matched_items", "blocked_items", "items"],
    "additionalProperties": False,
}
PATCH_TRACE_VALIDATION_RESULT_SCHEMA = {
    "type": "object",
    "description": "Patch commit decision against trace evidence.",
    "properties": {
        "allowed": _schema("boolean", "Whether trace validation allows commit."),
        "status": _schema("string", "Trace validation status."),
        "reason": _schema("string", "Trace validation reason."),
        "patch_gate_status": _schema("string", "Underlying patch commit gate status."),
        "replay_status": _schema("string", "Trace replay status."),
        "replay": TRACE_PATCH_EVIDENCE_REPLAY_RESULT_SCHEMA,
    },
    "required": [
        "allowed",
        "status",
        "reason",
        "patch_gate_status",
        "replay_status",
        "replay",
    ],
    "additionalProperties": False,
}
NULLABLE_TRACE_SYMBOL_GRAPH_RESULT_SCHEMA = {
    "anyOf": [TRACE_SYMBOL_GRAPH_RESULT_SCHEMA, NULL_RESULT_SCHEMA]
}
NULLABLE_TRACE_SYMBOL_NEIGHBORHOOD_RESULT_SCHEMA = {
    "anyOf": [TRACE_SYMBOL_NEIGHBORHOOD_RESULT_SCHEMA, NULL_RESULT_SCHEMA]
}
NULLABLE_PATCH_TRACE_VALIDATION_RESULT_SCHEMA = {
    "anyOf": [PATCH_TRACE_VALIDATION_RESULT_SCHEMA, NULL_RESULT_SCHEMA]
}
NULLABLE_SYMBOL_READ_RESULT_SCHEMA = {"anyOf": [SYMBOL_READ_RESULT_SCHEMA, NULL_RESULT_SCHEMA]}
NULLABLE_SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA = {
    "anyOf": [SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA, NULL_RESULT_SCHEMA]
}
TRACE_BACKED_PATCH_RESULT_SCHEMA = {
    "type": "object",
    "description": "Patch result with optional one-hop trace validation context.",
    "properties": {
        "patch": PATCH_AST_NODE_RESULT_SCHEMA,
        "trace_target": _schema("string", "Trace target symbol selector."),
        "trace": NULLABLE_TRACE_SYMBOL_GRAPH_RESULT_SCHEMA,
        "trace_validation": NULLABLE_PATCH_TRACE_VALIDATION_RESULT_SCHEMA,
        "trace_error": NULLABLE_STRING_RESULT_SCHEMA,
    },
    "required": ["patch", "trace_target", "trace", "trace_validation", "trace_error"],
    "additionalProperties": False,
}
GRAPH_BACKED_PATCH_RESULT_SCHEMA = {
    "type": "object",
    "description": "Patch result with optional trace graph and neighborhood context.",
    "properties": {
        "patch": PATCH_AST_NODE_RESULT_SCHEMA,
        "trace_target": _schema("string", "Trace target symbol selector."),
        "trace": NULLABLE_TRACE_SYMBOL_GRAPH_RESULT_SCHEMA,
        "neighborhood": NULLABLE_TRACE_SYMBOL_NEIGHBORHOOD_RESULT_SCHEMA,
        "trace_validation": NULLABLE_PATCH_TRACE_VALIDATION_RESULT_SCHEMA,
        "trace_error": NULLABLE_STRING_RESULT_SCHEMA,
    },
    "required": [
        "patch",
        "trace_target",
        "trace",
        "neighborhood",
        "trace_validation",
        "trace_error",
    ],
    "additionalProperties": False,
}
NEIGHBORHOOD_CONTEXT_PATCH_RESULT_SCHEMA = {
    "type": "object",
    "description": "Patch result with optional symbol neighborhood context.",
    "properties": {
        "patch": PATCH_AST_NODE_RESULT_SCHEMA,
        "trace_target": _schema("string", "Trace target symbol selector."),
        "trace": NULLABLE_TRACE_SYMBOL_GRAPH_RESULT_SCHEMA,
        "neighborhood_context": NULLABLE_SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
        "trace_validation": NULLABLE_PATCH_TRACE_VALIDATION_RESULT_SCHEMA,
        "trace_error": NULLABLE_STRING_RESULT_SCHEMA,
    },
    "required": [
        "patch",
        "trace_target",
        "trace",
        "neighborhood_context",
        "trace_validation",
        "trace_error",
    ],
    "additionalProperties": False,
}
DISCOVERY_CONTEXT_PATCH_RESULT_SCHEMA = {
    "type": "object",
    "description": "Patch result with optional read and neighborhood discovery context.",
    "properties": {
        "patch": PATCH_AST_NODE_RESULT_SCHEMA,
        "trace_target": _schema("string", "Trace target symbol selector."),
        "trace": NULLABLE_TRACE_SYMBOL_GRAPH_RESULT_SCHEMA,
        "read": NULLABLE_SYMBOL_READ_RESULT_SCHEMA,
        "neighborhood_context": NULLABLE_SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
        "trace_validation": NULLABLE_PATCH_TRACE_VALIDATION_RESULT_SCHEMA,
        "trace_error": NULLABLE_STRING_RESULT_SCHEMA,
    },
    "required": [
        "patch",
        "trace_target",
        "trace",
        "read",
        "neighborhood_context",
        "trace_validation",
        "trace_error",
    ],
    "additionalProperties": False,
}
QUERY_CAPTURE_RESULT_SCHEMA = {
    "type": "object",
    "description": "Tree-sitter query capture with optional Arborist owner metadata.",
    "properties": {
        "capture_name": _schema("string", "Tree-sitter capture name without the @ prefix."),
        "node_kind": _schema("string", "Captured Tree-sitter node kind."),
        "text": _schema("string", "Captured source text.", allow_empty=True),
        "owner_symbol_id": NULLABLE_STRING_RESULT_SCHEMA,
        "owner_semantic_path": NULLABLE_STRING_RESULT_SCHEMA,
        "owner_scope_path": NULLABLE_STRING_RESULT_SCHEMA,
        "start_byte": _schema("integer", "Inclusive start byte of the captured node.", minimum=0),
        "end_byte": _schema("integer", "Exclusive end byte of the captured node.", minimum=0),
        "start_point": POSITION_RESULT_SCHEMA,
        "end_point": POSITION_RESULT_SCHEMA,
    },
    "required": [
        "capture_name",
        "node_kind",
        "text",
        "owner_symbol_id",
        "owner_semantic_path",
        "owner_scope_path",
        "start_byte",
        "end_byte",
        "start_point",
        "end_point",
    ],
    "additionalProperties": False,
}
QUERY_CAPTURE_ARRAY_RESULT_SCHEMA = {
    "type": "array",
    "description": "Tree-sitter query captures.",
    "items": QUERY_CAPTURE_RESULT_SCHEMA,
}
VIRTUAL_FILE_SNAPSHOT_RESULT_SCHEMA = {
    "type": "object",
    "description": "Session-scoped virtual file snapshot.",
    "properties": {
        "file": _schema("string", "Normalized virtual file path."),
        "source": _schema("string", "Current virtual buffer source.", allow_empty=True),
        "disk_source": _schema("string", "Current on-disk source baseline.", allow_empty=True),
        "dirty": _schema("boolean", "Whether the virtual buffer differs from disk."),
        "version": _schema("integer", "Virtual buffer version.", minimum=0),
        "syntax_error_count": _schema(
            "integer", "Current Tree-sitter syntax error count.", minimum=0
        ),
    },
    "required": [
        "file",
        "source",
        "disk_source",
        "dirty",
        "version",
        "syntax_error_count",
    ],
    "additionalProperties": False,
}
VIRTUAL_FILE_STATUS_RESULT_SCHEMA = {
    "type": "object",
    "description": "Virtual file list entry.",
    "properties": {
        "file": _schema("string", "Normalized virtual file path."),
        "dirty": _schema("boolean", "Whether the virtual buffer differs from disk."),
        "version": _schema("integer", "Virtual buffer version.", minimum=0),
        "syntax_error_count": _schema(
            "integer", "Current Tree-sitter syntax error count.", minimum=0
        ),
    },
    "required": ["file", "dirty", "version", "syntax_error_count"],
    "additionalProperties": False,
}
VIRTUAL_FILE_STATUS_ARRAY_RESULT_SCHEMA = {
    "type": "array",
    "description": "Virtual file status entries.",
    "items": VIRTUAL_FILE_STATUS_RESULT_SCHEMA,
}
SYMBOL_INDEX_STATS_RESULT_SCHEMA = {
    "type": "object",
    "description": "Persisted symbol-index rebuild or refresh statistics.",
    "properties": {
        "db_path": _schema("string", "Normalized SQLite symbol-index database path."),
        "indexed_files": _schema("integer", "Number of indexed files.", minimum=0),
        "indexed_symbols": _schema("integer", "Number of indexed symbols.", minimum=0),
        "rebuilt_files": _schema("integer", "Number of files rebuilt during this operation.", minimum=0),
        "reused_files": _schema("integer", "Number of indexed files reused from prior state.", minimum=0),
    },
    "required": ["db_path", "indexed_files", "indexed_symbols", "rebuilt_files", "reused_files"],
    "additionalProperties": False,
}
REGISTERED_SYMBOL_INDEX_RESULT_SCHEMA = {
    "type": "object",
    "description": "Registered workspace-to-symbol-index mapping.",
    "properties": {
        "workspace_root": _schema("string", "Normalized workspace root path."),
        "db_path": _schema("string", "Normalized SQLite symbol-index database path."),
    },
    "required": ["workspace_root", "db_path"],
    "additionalProperties": False,
}
REGISTERED_SYMBOL_INDEX_ARRAY_RESULT_SCHEMA = {
    "type": "array",
    "description": "Registered workspace-to-symbol-index mappings.",
    "items": REGISTERED_SYMBOL_INDEX_RESULT_SCHEMA,
}
SYMBOL_INDEX_HEALTH_RESULT_SCHEMA = {
    "type": "object",
    "description": "Read-only diagnostic summary for a persisted symbol index.",
    "properties": {
        "response_schema_version": _schema(
            "string", "Version of the inspect_symbol_index response schema."
        ),
        "db_path": _schema("string", "Normalized SQLite symbol-index database path."),
        "exists": _schema("boolean", "Whether the database file exists."),
        "ok": _schema("boolean", "Whether the index passed all inspected health checks."),
        "schema_version": NULLABLE_STRING_RESULT_SCHEMA,
        "expected_schema_version": _schema("string", "Schema version supported by this Arborist build."),
        "workspace_root": NULLABLE_STRING_RESULT_SCHEMA,
        "indexed_files": NULLABLE_INTEGER_RESULT_SCHEMA,
        "indexed_symbols": NULLABLE_INTEGER_RESULT_SCHEMA,
        "file_state_entries": NULLABLE_INTEGER_RESULT_SCHEMA,
        "fresh_file_count": NULLABLE_INTEGER_RESULT_SCHEMA,
        "stale_files": {
            "type": "array",
            "description": "Indexed files whose current content no longer matches persisted fingerprints.",
            "items": _schema("string", "Stale indexed file path."),
        },
        "missing_files": {
            "type": "array",
            "description": "Indexed files that no longer exist on disk.",
            "items": _schema("string", "Missing indexed file path."),
        },
        "unreadable_files": {
            "type": "array",
            "description": "Indexed files that exist but could not be read during freshness inspection.",
            "items": _schema("string", "Unreadable indexed file path."),
        },
        "issues": {
            "type": "array",
            "description": "Human-readable health issues. Empty when ok is true.",
            "items": _schema("string", "Health issue."),
        },
    },
    "required": [
        "response_schema_version",
        "db_path",
        "exists",
        "ok",
        "schema_version",
        "expected_schema_version",
        "workspace_root",
        "indexed_files",
        "indexed_symbols",
        "file_state_entries",
        "fresh_file_count",
        "stale_files",
        "missing_files",
        "unreadable_files",
        "issues",
    ],
    "additionalProperties": False,
}
TOOL_RESULT_SCHEMAS = {
    tool_name: {
        "batch": BATCH_RESULT_SCHEMA,
        "object_array": OBJECT_ARRAY_RESULT_SCHEMA,
        "boolean": BOOLEAN_RESULT_SCHEMA,
        "semantic_skeleton": SEMANTIC_SKELETON_RESULT_SCHEMA,
        "query_capture_array": QUERY_CAPTURE_ARRAY_RESULT_SCHEMA,
        "virtual_file_snapshot": VIRTUAL_FILE_SNAPSHOT_RESULT_SCHEMA,
        "virtual_file_status_array": VIRTUAL_FILE_STATUS_ARRAY_RESULT_SCHEMA,
        "virtual_edit": VIRTUAL_EDIT_RESULT_SCHEMA,
        "symbol_index_stats": SYMBOL_INDEX_STATS_RESULT_SCHEMA,
        "registered_symbol_index": REGISTERED_SYMBOL_INDEX_RESULT_SCHEMA,
        "registered_symbol_index_array": REGISTERED_SYMBOL_INDEX_ARRAY_RESULT_SCHEMA,
        "symbol_index_health": SYMBOL_INDEX_HEALTH_RESULT_SCHEMA,
        "trace_symbol_graph": TRACE_SYMBOL_GRAPH_RESULT_SCHEMA,
        "trace_symbol_neighborhood": TRACE_SYMBOL_NEIGHBORHOOD_RESULT_SCHEMA,
        "symbol_read": SYMBOL_READ_RESULT_SCHEMA,
        "symbol_context": SYMBOL_CONTEXT_RESULT_SCHEMA,
        "symbol_neighborhood_context": SYMBOL_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
        "symbol_discovery_context": SYMBOL_DISCOVERY_CONTEXT_RESULT_SCHEMA,
        "symbol_list": SYMBOL_LIST_RESULT_SCHEMA,
        "symbol_list_context": SYMBOL_LIST_CONTEXT_RESULT_SCHEMA,
        "symbol_list_neighborhood_context": SYMBOL_LIST_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
        "symbol_list_discovery_context": SYMBOL_LIST_DISCOVERY_CONTEXT_RESULT_SCHEMA,
        "symbol_search": SYMBOL_SEARCH_RESULT_SCHEMA,
        "symbol_search_context": SYMBOL_SEARCH_CONTEXT_RESULT_SCHEMA,
        "symbol_search_neighborhood_context": SYMBOL_SEARCH_NEIGHBORHOOD_CONTEXT_RESULT_SCHEMA,
        "symbol_search_discovery_context": SYMBOL_SEARCH_DISCOVERY_CONTEXT_RESULT_SCHEMA,
        "patch_ast_node": PATCH_AST_NODE_RESULT_SCHEMA,
        "patch_preview": PATCH_PREVIEW_RESULT_SCHEMA,
        "trace_patch_evidence_replay": TRACE_PATCH_EVIDENCE_REPLAY_RESULT_SCHEMA,
        "patch_trace_validation": PATCH_TRACE_VALIDATION_RESULT_SCHEMA,
        "trace_backed_patch": TRACE_BACKED_PATCH_RESULT_SCHEMA,
        "graph_backed_patch": GRAPH_BACKED_PATCH_RESULT_SCHEMA,
        "neighborhood_context_patch": NEIGHBORHOOD_CONTEXT_PATCH_RESULT_SCHEMA,
        "discovery_context_patch": DISCOVERY_CONTEXT_PATCH_RESULT_SCHEMA,
    }[schema_key]
    for tool_name, schema_key in TOOL_RESULT_SCHEMA_KEYS.items()
}
BATCH_CALL_RESULT_SCHEMA["properties"]["result"] = {
    "description": "Result returned by the inner read-only tool.",
    "anyOf": [
        TOOL_RESULT_SCHEMAS[tool_name]
        for tool_name in TOOL_NAMES
        if tool_name in BATCH_ALLOWED_TOOLS and tool_name in TOOL_RESULT_SCHEMAS
    ],
}
TOOL_RESULT_SCHEMAS["arborist/batch"] = BATCH_RESULT_SCHEMA


class JsonRpcError(ValueError):
    def __init__(self, code: int, message: str) -> None:
        super().__init__(message)
        self.code = code


def is_mcp_initialize(params: dict[str, Any]) -> bool:
    return bool(MCP_INITIALIZE_MARKERS & set(params))


def build_tool_catalog() -> list[dict[str, Any]]:
    return [build_tool_descriptor(tool_name) for tool_name in TOOL_NAMES]


def build_resource_catalog() -> list[dict[str, Any]]:
    return [
        {
            "uri": TOOL_CATALOG_RESOURCE_URI,
            "name": "Arborist tool catalog",
            "description": "Generated MCP tools/list snapshot for this Arborist gateway.",
            "mimeType": TOOL_CATALOG_RESOURCE_MIME_TYPE,
        }
    ]


def build_tool_descriptor(tool_name: str) -> dict[str, Any]:
    category = TOOL_CATEGORIES[tool_name]
    tool: dict[str, Any] = {
        "name": tool_name,
        "title": _tool_title(tool_name),
        "description": _tool_description(tool_name, category),
        "inputSchema": build_tool_input_schema(tool_name),
        "outputSchema": build_tool_output_schema_for_tool(tool_name),
        "annotations": {
            "readOnlyHint": category in READ_ONLY_CATEGORIES
            or tool_name in NON_MUTATING_STATE_TOOLS,
            "destructiveHint": tool_name in WRITING_TOOLS,
        },
        "metadata": {
            "category": category,
            "legacyMethod": tool_name,
            "mutatesState": tool_name in MUTATING_TOOLS,
        },
    }
    return tool


def build_tool_output_schema() -> dict[str, Any]:
    return {
        "type": "object",
        "properties": {
            "result": OBJECT_RESULT_SCHEMA,
        },
        "required": ["result"],
        "additionalProperties": False,
    }


def build_tool_output_schema_for_tool(tool_name: str) -> dict[str, Any]:
    result_schema = TOOL_RESULT_SCHEMAS.get(tool_name, OBJECT_RESULT_SCHEMA)
    return {
        "type": "object",
        "properties": {
            "result": result_schema,
        },
        "required": ["result"],
        "additionalProperties": False,
    }


def build_tool_input_schema(tool_name: str) -> dict[str, Any]:
    properties: dict[str, Any] = {}
    for param_name in TOOL_PARAM_NAMES[tool_name]:
        param_schema = dict(TOOL_PARAM_SCHEMAS[param_name])
        default = tool_param_default(tool_name, param_name)
        if default is not None:
            param_schema["default"] = default
        properties[param_name] = param_schema

    return {
        "type": "object",
        "properties": properties,
        "required": list(required_tool_params(tool_name)),
        "additionalProperties": False,
    }


def required_tool_params(tool_name: str) -> tuple[str, ...]:
    return tuple(
        param_name
        for param_name in TOOL_PARAM_NAMES[tool_name]
        if param_name not in OPTIONAL_TOOL_PARAMS
        and not (
            param_name == "file_path"
            and tool_name in SOURCE_ANCHORED_OPTIONAL_FILE_PATH_TOOLS
        )
    )


def tool_param_default(tool_name: str, param_name: str) -> Any:
    default = TOOL_PARAM_DEFAULTS.get(param_name)
    if isinstance(default, dict):
        if tool_name.startswith("arborist/list_symbols"):
            return default["list"]
        if tool_name.startswith("arborist/search_symbols"):
            return default["search"]
        return None
    return default


def _tool_title(tool_name: str) -> str:
    return tool_name.removeprefix("arborist/").replace("_", " ").title()


def _tool_description(tool_name: str, category: str) -> str:
    method_name = tool_name.removeprefix("arborist/")
    category_descriptions = {
        "read": "Read semantic source information without writing project files.",
        "write": "Patch persisted source files through Arborist semantic targeting.",
        "vfs": "Manage or inspect Arborist's session-scoped virtual-file state.",
        "index": "Build, refresh, register, or inspect persisted symbol indexes.",
        "trace": "Read trace, graph, or trace-backed validation context.",
    }
    return f"{category_descriptions[category]} Legacy JSON-RPC method: arborist/{method_name}."


def _load_core_class() -> type[Any]:
    module = importlib.import_module("._arborist_core", __package__)
    return module.ArboristCore


class ArboristGateway:
    def __init__(self) -> None:
        self._core: Any | None = None

    def _require_core(self) -> Any:
        core = getattr(self, "_core", None)
        if core is None:
            try:
                core_class = _load_core_class()
                core = core_class()
                self._core = core
            except Exception as exc:  # noqa: BLE001
                raise JsonRpcError(-32000, f"failed to load arborist core: {exc}") from exc
        return core

    def handle_request(self, request: Any) -> dict[str, Any]:
        if not isinstance(request, dict):
            return self._error_response(None, -32600, "invalid request: expected object")

        request_id = request.get("id")
        response_id = request_id if is_valid_request_id(request_id) else None
        jsonrpc_version = request.get("jsonrpc")
        if jsonrpc_version != "2.0":
            return self._error_response(
                response_id,
                -32600,
                "invalid request: expected jsonrpc='2.0'",
            )

        method = request.get("method")
        params = request.get("params", {})

        if "id" in request and not is_valid_request_id(request_id):
            return self._error_response(None, -32600, "invalid request: invalid id")

        if not isinstance(method, str) or not method:
            return self._error_response(response_id, -32600, "invalid request: missing method")

        if not isinstance(params, dict):
            return self._error_response(response_id, -32602, "invalid params: expected object")

        try:
            if method == "initialize":
                result = self._initialize(params)
            elif method == "notifications/initialized":
                result = self._initialized(params)
            elif method == "tools/list":
                result = self._tools_list(params)
            elif method == "tools/call":
                result = self._tools_call(params)
            elif method == "resources/list":
                result = self._resources_list(params)
            elif method == "resources/read":
                result = self._resources_read(params)
            elif method in TOOL_HANDLERS:
                self._reject_unexpected_params(params, TOOL_PARAM_NAMES[method])
                handler = getattr(self, TOOL_HANDLERS[method])
                result = handler(params)
            else:
                return self._error_response(response_id, -32601, f"method not found: {method}")

            return {"jsonrpc": "2.0", "id": request_id, "result": result}
        except JsonRpcError as exc:
            return self._error_response(response_id, exc.code, str(exc))
        except ValueError as exc:
            return self._error_response(response_id, -32602, str(exc))
        except Exception as exc:  # noqa: BLE001
            return self._error_response(response_id, -32000, str(exc))

    @staticmethod
    def _error_response(
        request_id: Any,
        code: int,
        message: str,
    ) -> dict[str, Any]:
        return {
            "jsonrpc": "2.0",
            "id": request_id,
            "error": {
                "code": code,
                "message": message,
            },
        }

    @staticmethod
    def _require_file_path_for_source(
        source: str | None,
        file_path: str | None,
    ) -> None:
        if source is not None and file_path is None:
            raise JsonRpcError(
                -32602,
                "invalid params: file_path is required when source is provided",
            )

    def _initialize(self, params: dict[str, Any]) -> dict[str, Any]:
        if not is_mcp_initialize(params):
            self._reject_unexpected_params(params, ())
            return {
                "serverInfo": self._server_info(),
                "capabilities": {
                    "tools": list(TOOL_NAMES),
                    "resources": build_resource_catalog(),
                },
                "supportedLanguages": self._require_core().supported_languages(),
            }

        self._reject_unexpected_params(params, MCP_INITIALIZE_PARAM_NAMES)
        self._optional_string(
            params,
            "protocolVersion",
            default=MCP_PROTOCOL_VERSION,
        )
        capabilities = params.get("capabilities", {})
        if not isinstance(capabilities, dict):
            raise JsonRpcError(-32602, "invalid params: capabilities must be an object")
        client_info = params.get("clientInfo", {})
        if not isinstance(client_info, dict):
            raise JsonRpcError(-32602, "invalid params: clientInfo must be an object")

        return {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {
                "tools": {
                    "listChanged": False,
                },
                "resources": {
                    "subscribe": False,
                    "listChanged": False,
                },
            },
            "serverInfo": self._server_info(),
            "instructions": (
                "Use tools/list to discover Arborist tools and tools/call with "
                "arguments matching each tool inputSchema."
            ),
            "supportedLanguages": self._require_core().supported_languages(),
        }

    def _initialized(self, params: dict[str, Any]) -> dict[str, Any]:
        self._reject_unexpected_params(params, MCP_INITIALIZED_PARAM_NAMES)
        return {}

    def _tools_list(self, params: dict[str, Any]) -> dict[str, Any]:
        self._reject_unexpected_params(params, MCP_TOOL_LIST_PARAM_NAMES)
        cursor = params.get("cursor")
        if cursor is not None and not isinstance(cursor, str):
            raise JsonRpcError(-32602, "invalid params: cursor must be a string")
        return {"tools": build_tool_catalog()}

    def _resources_list(self, params: dict[str, Any]) -> dict[str, Any]:
        self._reject_unexpected_params(params, MCP_RESOURCE_LIST_PARAM_NAMES)
        cursor = params.get("cursor")
        if cursor is not None and not isinstance(cursor, str):
            raise JsonRpcError(-32602, "invalid params: cursor must be a string")
        return {"resources": build_resource_catalog()}

    def _resources_read(self, params: dict[str, Any]) -> dict[str, Any]:
        self._reject_unexpected_params(params, MCP_RESOURCE_READ_PARAM_NAMES)
        uri = params.get("uri")
        if not isinstance(uri, str) or not uri.strip():
            raise JsonRpcError(-32602, "missing required string param: uri")
        if uri != TOOL_CATALOG_RESOURCE_URI:
            raise JsonRpcError(-32602, f"unknown resource: {uri}")
        return {
            "contents": [
                {
                    "uri": TOOL_CATALOG_RESOURCE_URI,
                    "mimeType": TOOL_CATALOG_RESOURCE_MIME_TYPE,
                    "text": json.dumps(build_tool_catalog(), ensure_ascii=False, indent=2),
                }
            ]
        }

    def _tools_call(self, params: dict[str, Any]) -> dict[str, Any]:
        self._reject_unexpected_params(params, MCP_TOOL_CALL_PARAM_NAMES)
        tool_name = params.get("name")
        if not isinstance(tool_name, str) or not tool_name.strip():
            raise JsonRpcError(-32602, "missing required string param: name")
        if tool_name not in TOOL_HANDLERS:
            raise JsonRpcError(-32602, f"unknown tool: {tool_name}")
        arguments = params.get("arguments", {})
        if not isinstance(arguments, dict):
            raise JsonRpcError(-32602, "invalid params: arguments must be an object")

        try:
            self._reject_unexpected_params(arguments, TOOL_PARAM_NAMES[tool_name])
            handler = getattr(self, TOOL_HANDLERS[tool_name])
            tool_result = handler(arguments)
        except JsonRpcError as exc:
            return self._mcp_tool_error(str(exc))
        except ValueError as exc:
            return self._mcp_tool_error(str(exc))
        except Exception as exc:  # noqa: BLE001
            return self._mcp_tool_error(str(exc))

        return self._mcp_tool_result(tool_result)

    def _batch(self, params: dict[str, Any]) -> list[dict[str, Any]]:
        calls = params.get("calls")
        if not isinstance(calls, list):
            raise JsonRpcError(-32602, "missing required array param: calls")
        if not calls:
            raise JsonRpcError(-32602, "invalid params: calls must not be empty")
        if len(calls) > MAX_BATCH_CALLS:
            raise JsonRpcError(
                -32602,
                f"invalid params: calls must contain at most {MAX_BATCH_CALLS} entries",
            )

        results: list[dict[str, Any]] = []
        for index, call in enumerate(calls):
            if not isinstance(call, dict):
                raise JsonRpcError(
                    -32602,
                    f"invalid params: calls[{index}] must be an object",
                )
            self._reject_unexpected_params(call, ("name", "arguments"))
            tool_name = call.get("name")
            if not isinstance(tool_name, str) or not tool_name.strip():
                raise JsonRpcError(
                    -32602,
                    f"missing required string param: calls[{index}].name",
                )
            if tool_name not in TOOL_HANDLERS:
                raise JsonRpcError(-32602, f"unknown batch tool: {tool_name}")
            if tool_name == "arborist/batch":
                raise JsonRpcError(-32602, "batch calls may not include arborist/batch")
            if tool_name not in BATCH_ALLOWED_TOOLS:
                raise JsonRpcError(
                    -32602,
                    f"batch only supports read-only tools: {tool_name}",
                )

            arguments = call.get("arguments", {})
            if not isinstance(arguments, dict):
                raise JsonRpcError(
                    -32602,
                    f"invalid params: calls[{index}].arguments must be an object",
                )
            self._reject_unexpected_params(arguments, TOOL_PARAM_NAMES[tool_name])
            handler = getattr(self, TOOL_HANDLERS[tool_name])
            results.append({"name": tool_name, "result": handler(arguments)})

        return results

    @staticmethod
    def _server_info() -> dict[str, Any]:
        return {
            "name": "arborist-mcp",
            "version": __version__,
        }

    @staticmethod
    def _mcp_tool_result(tool_result: Any) -> dict[str, Any]:
        return {
            "content": [
                {
                    "type": "text",
                    "text": json.dumps(tool_result, ensure_ascii=False, allow_nan=False),
                }
            ],
            "structuredContent": {"result": tool_result},
            "isError": False,
        }

    @staticmethod
    def _mcp_tool_error(message: str) -> dict[str, Any]:
        return {
            "content": [
                {
                    "type": "text",
                    "text": message,
                }
            ],
            "isError": True,
        }

    def _get_semantic_skeleton(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        depth_limit = self._optional_int(params, "depth_limit", default=2)
        source = self._optional_string(params, "source", allow_empty=True)
        expand_nodes = self._optional_string_list(params, "expand_nodes")
        payload = self._require_core().get_semantic_skeleton_json(
            file_path,
            source,
            depth_limit,
            expand_nodes,
        )
        return self._decode_core_object(payload)

    def _execute_tree_query(self, params: dict[str, Any]) -> list[dict[str, Any]]:
        file_path = self._require_string(params, "file_path")
        query = self._require_string(params, "query", max_length=TREE_QUERY_MAX_LENGTH)
        source = self._optional_string(params, "source", allow_empty=True)
        max_captures = self._optional_positive_int(params, "max_captures", default=10000)
        payload = self._require_core().execute_tree_query_json(
            file_path, query, source, max_captures
        )
        return self._decode_core_object_array(payload)

    def _preview_patch_ast_node(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        semantic_path = self._require_string(params, "semantic_path")
        new_code = self._require_string(params, "new_code")
        source = self._optional_string(params, "source", allow_empty=True)
        bypass_reason = self._optional_string(params, "bypass_reason")
        payload = self._require_core().preview_patch_ast_node_json(
            file_path,
            semantic_path,
            new_code,
            source,
            bypass_reason,
        )
        return self._decode_core_object(payload)

    def _preview_patch_ast_node_at_position(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        row, column = self._require_position(params, "position")
        new_code = self._require_string(params, "new_code")
        source = self._optional_string(params, "source", allow_empty=True)
        bypass_reason = self._optional_string(params, "bypass_reason")
        payload = self._require_core().preview_patch_ast_node_at_position_json(
            file_path,
            row,
            column,
            new_code,
            source,
            bypass_reason,
        )
        return self._decode_core_object(payload)

    def _patch_ast_node(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        semantic_path = self._require_string(params, "semantic_path")
        new_code = self._require_string(params, "new_code")
        source = self._optional_string(params, "source", allow_empty=True)
        bypass_reason = self._optional_string(params, "bypass_reason")
        payload = self._require_core().patch_ast_node_json(
            file_path,
            semantic_path,
            new_code,
            source,
            bypass_reason,
        )
        return self._decode_core_object(payload)

    def _patch_ast_node_at_position(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        row, column = self._require_position(params, "position")
        new_code = self._require_string(params, "new_code")
        source = self._optional_string(params, "source", allow_empty=True)
        bypass_reason = self._optional_string(params, "bypass_reason")
        payload = self._require_core().patch_ast_node_at_position_json(
            file_path,
            row,
            column,
            new_code,
            source,
            bypass_reason,
        )
        return self._decode_core_object(payload)

    def _patch_virtual_ast_node(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        semantic_path = self._require_string(params, "semantic_path")
        new_code = self._require_string(params, "new_code")
        bypass_reason = self._optional_string(params, "bypass_reason")
        payload = self._require_core().patch_virtual_ast_node_json(
            file_path,
            semantic_path,
            new_code,
            bypass_reason,
        )
        return self._decode_core_object(payload)

    def _patch_virtual_ast_node_at_position(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        row, column = self._require_position(params, "position")
        new_code = self._require_string(params, "new_code")
        bypass_reason = self._optional_string(params, "bypass_reason")
        payload = self._require_core().patch_virtual_ast_node_at_position_json(
            file_path,
            row,
            column,
            new_code,
            bypass_reason,
        )
        return self._decode_core_object(payload)

    def _trace_symbol_graph(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        symbol_path = self._require_string(params, "symbol_path")
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        index_db_path = self._optional_string(params, "index_db_path")
        file_path = self._optional_string(params, "file_path")
        source = self._optional_string(params, "source", allow_empty=True)
        self._require_file_path_for_source(source, file_path)
        core = self._require_core()
        if source is not None:
            payload = core.trace_symbol_graph_json(
                workspace_root,
                symbol_path,
                direction,
                index_db_path,
                file_path,
                source,
            )
        else:
            payload = core.trace_symbol_graph_json(
                workspace_root,
                symbol_path,
                direction,
                index_db_path,
            )
        return self._decode_core_object(payload)

    def _trace_symbol_neighborhood(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        symbol_path = self._require_string(params, "symbol_path")
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        max_depth = self._optional_int(params, "max_depth", default=2)
        max_nodes = self._optional_positive_int(params, "max_nodes", default=64)
        index_db_path = self._optional_string(params, "index_db_path")
        file_path = self._optional_string(params, "file_path")
        source = self._optional_string(params, "source", allow_empty=True)
        self._require_file_path_for_source(source, file_path)
        core = self._require_core()
        if source is not None:
            payload = core.trace_symbol_neighborhood_json(
                workspace_root,
                symbol_path,
                direction,
                max_depth,
                max_nodes,
                index_db_path,
                file_path,
                source,
            )
        else:
            payload = core.trace_symbol_neighborhood_json(
                workspace_root,
                symbol_path,
                direction,
                max_depth,
                max_nodes,
                index_db_path,
            )
        return self._decode_core_object(payload)

    def _trace_symbol_graph_at_position(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        file_path = self._require_string(params, "file_path")
        row, column = self._require_position(params, "position")
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        source = self._optional_string(params, "source", allow_empty=True)
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().trace_symbol_graph_at_position_json(
            workspace_root,
            file_path,
            row,
            column,
            direction,
            source,
            index_db_path,
        )
        return self._decode_core_object(payload)

    def _trace_symbol_neighborhood_at_position(
        self, params: dict[str, Any]
    ) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        file_path = self._require_string(params, "file_path")
        row, column = self._require_position(params, "position")
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        max_depth = self._optional_int(params, "max_depth", default=2)
        max_nodes = self._optional_positive_int(params, "max_nodes", default=64)
        source = self._optional_string(params, "source", allow_empty=True)
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().trace_symbol_neighborhood_at_position_json(
            workspace_root,
            file_path,
            row,
            column,
            direction,
            max_depth,
            max_nodes,
            source,
            index_db_path,
        )
        return self._decode_core_object(payload)

    def _read_symbol(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        symbol_path = self._require_string(params, "symbol_path")
        index_db_path = self._optional_string(params, "index_db_path")
        file_path = self._optional_string(params, "file_path")
        source = self._optional_string(params, "source", allow_empty=True)
        self._require_file_path_for_source(source, file_path)
        core = self._require_core()
        if source is not None:
            payload = core.read_symbol_json(
                workspace_root,
                symbol_path,
                index_db_path,
                file_path,
                source,
            )
        else:
            payload = core.read_symbol_json(
                workspace_root,
                symbol_path,
                index_db_path,
            )
        return self._decode_core_object(payload)

    def _read_symbol_at_position(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        file_path = self._require_string(params, "file_path")
        row, column = self._require_position(params, "position")
        source = self._optional_string(params, "source", allow_empty=True)
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().read_symbol_at_position_json(
            workspace_root,
            file_path,
            row,
            column,
            source,
            index_db_path,
        )
        return self._decode_core_object(payload)

    def _read_symbol_context(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        symbol_path = self._require_string(params, "symbol_path")
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        index_db_path = self._optional_string(params, "index_db_path")
        file_path = self._optional_string(params, "file_path")
        source = self._optional_string(params, "source", allow_empty=True)
        self._require_file_path_for_source(source, file_path)
        core = self._require_core()
        if source is not None:
            payload = core.read_symbol_context_json(
                workspace_root,
                symbol_path,
                direction,
                index_db_path,
                file_path,
                source,
            )
        else:
            payload = core.read_symbol_context_json(
                workspace_root,
                symbol_path,
                direction,
                index_db_path,
            )
        return self._decode_core_object(payload)

    def _read_symbol_context_at_position(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        file_path = self._require_string(params, "file_path")
        row, column = self._require_position(params, "position")
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        source = self._optional_string(params, "source", allow_empty=True)
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().read_symbol_context_at_position_json(
            workspace_root,
            file_path,
            row,
            column,
            direction,
            source,
            index_db_path,
        )
        return self._decode_core_object(payload)

    def _read_symbol_neighborhood_context(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        symbol_path = self._require_string(params, "symbol_path")
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        max_depth = self._optional_int(params, "max_depth", default=2)
        max_nodes = self._optional_positive_int(params, "max_nodes", default=64)
        index_db_path = self._optional_string(params, "index_db_path")
        file_path = self._optional_string(params, "file_path")
        source = self._optional_string(params, "source", allow_empty=True)
        self._require_file_path_for_source(source, file_path)
        core = self._require_core()
        if source is not None:
            payload = core.read_symbol_neighborhood_context_json(
                workspace_root,
                symbol_path,
                direction,
                max_depth,
                max_nodes,
                index_db_path,
                file_path,
                source,
            )
        else:
            payload = core.read_symbol_neighborhood_context_json(
                workspace_root,
                symbol_path,
                direction,
                max_depth,
                max_nodes,
                index_db_path,
            )
        return self._decode_core_object(payload)

    def _read_symbol_neighborhood_context_at_position(
        self, params: dict[str, Any]
    ) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        file_path = self._require_string(params, "file_path")
        row, column = self._require_position(params, "position")
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        max_depth = self._optional_int(params, "max_depth", default=2)
        max_nodes = self._optional_positive_int(params, "max_nodes", default=64)
        source = self._optional_string(params, "source", allow_empty=True)
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().read_symbol_neighborhood_context_at_position_json(
            workspace_root,
            file_path,
            row,
            column,
            direction,
            max_depth,
            max_nodes,
            source,
            index_db_path,
        )
        return self._decode_core_object(payload)

    def _read_symbol_discovery_context(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        symbol_path = self._require_string(params, "symbol_path")
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        max_depth = self._optional_int(params, "max_depth", default=2)
        max_nodes = self._optional_positive_int(params, "max_nodes", default=64)
        index_db_path = self._optional_string(params, "index_db_path")
        file_path = self._optional_string(params, "file_path")
        source = self._optional_string(params, "source", allow_empty=True)
        self._require_file_path_for_source(source, file_path)
        core = self._require_core()
        if source is not None:
            payload = core.read_symbol_discovery_context_json(
                workspace_root,
                symbol_path,
                direction,
                max_depth,
                max_nodes,
                index_db_path,
                file_path,
                source,
            )
        else:
            payload = core.read_symbol_discovery_context_json(
                workspace_root,
                symbol_path,
                direction,
                max_depth,
                max_nodes,
                index_db_path,
            )
        return self._decode_core_object(payload)

    def _read_symbol_discovery_context_at_position(
        self, params: dict[str, Any]
    ) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        file_path = self._require_string(params, "file_path")
        row, column = self._require_position(params, "position")
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        max_depth = self._optional_int(params, "max_depth", default=2)
        max_nodes = self._optional_positive_int(params, "max_nodes", default=64)
        source = self._optional_string(params, "source", allow_empty=True)
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().read_symbol_discovery_context_at_position_json(
            workspace_root,
            file_path,
            row,
            column,
            direction,
            max_depth,
            max_nodes,
            source,
            index_db_path,
        )
        return self._decode_core_object(payload)

    def _search_symbols(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        query = self._require_string(params, "query", max_length=TREE_QUERY_MAX_LENGTH)
        limit = self._optional_int(params, "limit", default=20)
        index_db_path = self._optional_string(params, "index_db_path")
        file_path_contains = self._optional_string(params, "file_path_contains")
        node_kind = self._optional_string(params, "node_kind")
        file_path = self._optional_string(params, "file_path")
        source = self._optional_string(params, "source", allow_empty=True)
        self._require_file_path_for_source(source, file_path)
        core = self._require_core()
        if source is not None:
            payload = core.search_symbols_json(
                workspace_root,
                query,
                limit,
                index_db_path,
                file_path_contains,
                node_kind,
                file_path,
                source,
            )
        else:
            payload = core.search_symbols_json(
                workspace_root,
                query,
                limit,
                index_db_path,
                file_path_contains,
                node_kind,
            )
        return self._decode_core_object(payload)

    def _search_symbols_context(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        query = self._require_string(params, "query", max_length=TREE_QUERY_MAX_LENGTH)
        limit = self._optional_int(params, "limit", default=20)
        index_db_path = self._optional_string(params, "index_db_path")
        file_path_contains = self._optional_string(params, "file_path_contains")
        node_kind = self._optional_string(params, "node_kind")
        file_path = self._optional_string(params, "file_path")
        source = self._optional_string(params, "source", allow_empty=True)
        self._require_file_path_for_source(source, file_path)
        core = self._require_core()
        if source is not None:
            payload = core.search_symbols_context_json(
                workspace_root,
                query,
                limit,
                index_db_path,
                file_path_contains,
                node_kind,
                file_path,
                source,
            )
        else:
            payload = core.search_symbols_context_json(
                workspace_root,
                query,
                limit,
                index_db_path,
                file_path_contains,
                node_kind,
            )
        return self._decode_core_object(payload)

    def _search_symbols_neighborhood_context(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        query = self._require_string(params, "query", max_length=TREE_QUERY_MAX_LENGTH)
        limit = self._optional_int(params, "limit", default=20)
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        max_depth = self._optional_int(params, "max_depth", default=2)
        max_nodes = self._optional_positive_int(params, "max_nodes", default=64)
        index_db_path = self._optional_string(params, "index_db_path")
        file_path_contains = self._optional_string(params, "file_path_contains")
        node_kind = self._optional_string(params, "node_kind")
        file_path = self._optional_string(params, "file_path")
        source = self._optional_string(params, "source", allow_empty=True)
        self._require_file_path_for_source(source, file_path)
        core = self._require_core()
        if source is not None:
            payload = core.search_symbols_neighborhood_context_json(
                workspace_root,
                query,
                limit,
                direction,
                max_depth,
                max_nodes,
                index_db_path,
                file_path_contains,
                node_kind,
                file_path,
                source,
            )
        else:
            payload = core.search_symbols_neighborhood_context_json(
                workspace_root,
                query,
                limit,
                direction,
                max_depth,
                max_nodes,
                index_db_path,
                file_path_contains,
                node_kind,
            )
        return self._decode_core_object(payload)

    def _search_symbols_discovery_context(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        query = self._require_string(params, "query", max_length=TREE_QUERY_MAX_LENGTH)
        limit = self._optional_int(params, "limit", default=20)
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        max_depth = self._optional_int(params, "max_depth", default=2)
        max_nodes = self._optional_positive_int(params, "max_nodes", default=64)
        index_db_path = self._optional_string(params, "index_db_path")
        file_path_contains = self._optional_string(params, "file_path_contains")
        node_kind = self._optional_string(params, "node_kind")
        file_path = self._optional_string(params, "file_path")
        source = self._optional_string(params, "source", allow_empty=True)
        self._require_file_path_for_source(source, file_path)
        core = self._require_core()
        if source is not None:
            payload = core.search_symbols_discovery_context_json(
                workspace_root,
                query,
                limit,
                direction,
                max_depth,
                max_nodes,
                index_db_path,
                file_path_contains,
                node_kind,
                file_path,
                source,
            )
        else:
            payload = core.search_symbols_discovery_context_json(
                workspace_root,
                query,
                limit,
                direction,
                max_depth,
                max_nodes,
                index_db_path,
                file_path_contains,
                node_kind,
            )
        return self._decode_core_object(payload)

    def _list_symbols(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        limit = self._optional_int(params, "limit", default=100)
        index_db_path = self._optional_string(params, "index_db_path")
        file_path_contains = self._optional_string(params, "file_path_contains")
        node_kind = self._optional_string(params, "node_kind")
        file_path = self._optional_string(params, "file_path")
        source = self._optional_string(params, "source", allow_empty=True)
        self._require_file_path_for_source(source, file_path)
        core = self._require_core()
        if source is not None:
            payload = core.list_symbols_json(
                workspace_root,
                limit,
                index_db_path,
                file_path_contains,
                node_kind,
                file_path,
                source,
            )
        else:
            payload = core.list_symbols_json(
                workspace_root,
                limit,
                index_db_path,
                file_path_contains,
                node_kind,
            )
        return self._decode_core_object(payload)

    def _list_symbols_context(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        limit = self._optional_int(params, "limit", default=100)
        index_db_path = self._optional_string(params, "index_db_path")
        file_path_contains = self._optional_string(params, "file_path_contains")
        node_kind = self._optional_string(params, "node_kind")
        file_path = self._optional_string(params, "file_path")
        source = self._optional_string(params, "source", allow_empty=True)
        self._require_file_path_for_source(source, file_path)
        core = self._require_core()
        if source is not None:
            payload = core.list_symbols_context_json(
                workspace_root,
                limit,
                index_db_path,
                file_path_contains,
                node_kind,
                file_path,
                source,
            )
        else:
            payload = core.list_symbols_context_json(
                workspace_root,
                limit,
                index_db_path,
                file_path_contains,
                node_kind,
            )
        return self._decode_core_object(payload)

    def _list_symbols_neighborhood_context(
        self, params: dict[str, Any]
    ) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        limit = self._optional_int(params, "limit", default=100)
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        max_depth = self._optional_int(params, "max_depth", default=2)
        max_nodes = self._optional_positive_int(params, "max_nodes", default=64)
        index_db_path = self._optional_string(params, "index_db_path")
        file_path_contains = self._optional_string(params, "file_path_contains")
        node_kind = self._optional_string(params, "node_kind")
        file_path = self._optional_string(params, "file_path")
        source = self._optional_string(params, "source", allow_empty=True)
        self._require_file_path_for_source(source, file_path)
        core = self._require_core()
        if source is not None:
            payload = core.list_symbols_neighborhood_context_json(
                workspace_root,
                limit,
                direction,
                max_depth,
                max_nodes,
                index_db_path,
                file_path_contains,
                node_kind,
                file_path,
                source,
            )
        else:
            payload = core.list_symbols_neighborhood_context_json(
                workspace_root,
                limit,
                direction,
                max_depth,
                max_nodes,
                index_db_path,
                file_path_contains,
                node_kind,
            )
        return self._decode_core_object(payload)

    def _list_symbols_discovery_context(
        self, params: dict[str, Any]
    ) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        limit = self._optional_int(params, "limit", default=100)
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        max_depth = self._optional_int(params, "max_depth", default=2)
        max_nodes = self._optional_positive_int(params, "max_nodes", default=64)
        index_db_path = self._optional_string(params, "index_db_path")
        file_path_contains = self._optional_string(params, "file_path_contains")
        node_kind = self._optional_string(params, "node_kind")
        file_path = self._optional_string(params, "file_path")
        source = self._optional_string(params, "source", allow_empty=True)
        self._require_file_path_for_source(source, file_path)
        core = self._require_core()
        if source is not None:
            payload = core.list_symbols_discovery_context_json(
                workspace_root,
                limit,
                direction,
                max_depth,
                max_nodes,
                index_db_path,
                file_path_contains,
                node_kind,
                file_path,
                source,
            )
        else:
            payload = core.list_symbols_discovery_context_json(
                workspace_root,
                limit,
                direction,
                max_depth,
                max_nodes,
                index_db_path,
                file_path_contains,
                node_kind,
            )
        return self._decode_core_object(payload)

    def _replay_patch_evidence_against_trace(
        self, params: dict[str, Any]
    ) -> dict[str, Any]:
        patch = params.get("patch")
        trace = params.get("trace")
        if not isinstance(patch, dict):
            raise JsonRpcError(-32602, "missing required object param: patch")
        if not isinstance(trace, dict):
            raise JsonRpcError(-32602, "missing required object param: trace")
        patch_json = self._encode_json_param(patch, "patch")
        trace_json = self._encode_json_param(trace, "trace")
        payload = self._require_core().replay_patch_evidence_against_trace_json(
            patch_json,
            trace_json,
        )
        return self._decode_core_object(payload)

    def _validate_patch_commit_with_trace(
        self, params: dict[str, Any]
    ) -> dict[str, Any]:
        patch = params.get("patch")
        trace = params.get("trace")
        if not isinstance(patch, dict):
            raise JsonRpcError(-32602, "missing required object param: patch")
        if not isinstance(trace, dict):
            raise JsonRpcError(-32602, "missing required object param: trace")
        patch_json = self._encode_json_param(patch, "patch")
        trace_json = self._encode_json_param(trace, "trace")
        payload = self._require_core().validate_patch_commit_with_trace_json(
            patch_json,
            trace_json,
        )
        return self._decode_core_object(payload)

    def _validate_patch_with_trace_context(
        self, params: dict[str, Any]
    ) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        file_path = self._require_string(params, "file_path")
        semantic_path = self._require_string(params, "semantic_path")
        new_code = self._require_string(params, "new_code")
        source = self._optional_string(params, "source", allow_empty=True)
        bypass_reason = self._optional_string(params, "bypass_reason")
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().validate_patch_with_trace_context_json(
            workspace_root,
            file_path,
            semantic_path,
            new_code,
            source,
            bypass_reason,
            direction,
            index_db_path,
        )
        return self._decode_core_object(payload)

    def _validate_patch_with_trace_context_at_position(
        self, params: dict[str, Any]
    ) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        file_path = self._require_string(params, "file_path")
        row, column = self._require_position(params, "position")
        new_code = self._require_string(params, "new_code")
        source = self._optional_string(params, "source", allow_empty=True)
        bypass_reason = self._optional_string(params, "bypass_reason")
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().validate_patch_with_trace_context_at_position_json(
            workspace_root,
            file_path,
            row,
            column,
            new_code,
            source,
            bypass_reason,
            direction,
            index_db_path,
        )
        return self._decode_core_object(payload)

    def _validate_patch_with_graph_context(
        self, params: dict[str, Any]
    ) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        file_path = self._require_string(params, "file_path")
        semantic_path = self._require_string(params, "semantic_path")
        new_code = self._require_string(params, "new_code")
        source = self._optional_string(params, "source", allow_empty=True)
        bypass_reason = self._optional_string(params, "bypass_reason")
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        max_depth = self._optional_int(params, "max_depth", default=2)
        max_nodes = self._optional_positive_int(params, "max_nodes", default=64)
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().validate_patch_with_graph_context_json(
            workspace_root,
            file_path,
            semantic_path,
            new_code,
            source,
            bypass_reason,
            direction,
            max_depth,
            max_nodes,
            index_db_path,
        )
        return self._decode_core_object(payload)

    def _validate_patch_with_graph_context_at_position(
        self, params: dict[str, Any]
    ) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        file_path = self._require_string(params, "file_path")
        row, column = self._require_position(params, "position")
        new_code = self._require_string(params, "new_code")
        source = self._optional_string(params, "source", allow_empty=True)
        bypass_reason = self._optional_string(params, "bypass_reason")
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        max_depth = self._optional_int(params, "max_depth", default=2)
        max_nodes = self._optional_positive_int(params, "max_nodes", default=64)
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().validate_patch_with_graph_context_at_position_json(
            workspace_root,
            file_path,
            row,
            column,
            new_code,
            source,
            bypass_reason,
            direction,
            max_depth,
            max_nodes,
            index_db_path,
        )
        return self._decode_core_object(payload)

    def _validate_patch_with_neighborhood_context(
        self, params: dict[str, Any]
    ) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        file_path = self._require_string(params, "file_path")
        semantic_path = self._require_string(params, "semantic_path")
        new_code = self._require_string(params, "new_code")
        source = self._optional_string(params, "source", allow_empty=True)
        bypass_reason = self._optional_string(params, "bypass_reason")
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        max_depth = self._optional_int(params, "max_depth", default=2)
        max_nodes = self._optional_positive_int(params, "max_nodes", default=64)
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().validate_patch_with_neighborhood_context_json(
            workspace_root,
            file_path,
            semantic_path,
            new_code,
            source,
            bypass_reason,
            direction,
            max_depth,
            max_nodes,
            index_db_path,
        )
        return self._decode_core_object(payload)

    def _validate_patch_with_neighborhood_context_at_position(
        self, params: dict[str, Any]
    ) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        file_path = self._require_string(params, "file_path")
        row, column = self._require_position(params, "position")
        new_code = self._require_string(params, "new_code")
        source = self._optional_string(params, "source", allow_empty=True)
        bypass_reason = self._optional_string(params, "bypass_reason")
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        max_depth = self._optional_int(params, "max_depth", default=2)
        max_nodes = self._optional_positive_int(params, "max_nodes", default=64)
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().validate_patch_with_neighborhood_context_at_position_json(
            workspace_root,
            file_path,
            row,
            column,
            new_code,
            source,
            bypass_reason,
            direction,
            max_depth,
            max_nodes,
            index_db_path,
        )
        return self._decode_core_object(payload)

    def _validate_patch_with_discovery_context(
        self, params: dict[str, Any]
    ) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        file_path = self._require_string(params, "file_path")
        semantic_path = self._require_string(params, "semantic_path")
        new_code = self._require_string(params, "new_code")
        source = self._optional_string(params, "source", allow_empty=True)
        bypass_reason = self._optional_string(params, "bypass_reason")
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        max_depth = self._optional_int(params, "max_depth", default=2)
        max_nodes = self._optional_positive_int(params, "max_nodes", default=64)
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().validate_patch_with_discovery_context_json(
            workspace_root,
            file_path,
            semantic_path,
            new_code,
            source,
            bypass_reason,
            direction,
            max_depth,
            max_nodes,
            index_db_path,
        )
        return self._decode_core_object(payload)

    def _validate_patch_with_discovery_context_at_position(
        self, params: dict[str, Any]
    ) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        file_path = self._require_string(params, "file_path")
        row, column = self._require_position(params, "position")
        new_code = self._require_string(params, "new_code")
        source = self._optional_string(params, "source", allow_empty=True)
        bypass_reason = self._optional_string(params, "bypass_reason")
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        max_depth = self._optional_int(params, "max_depth", default=2)
        max_nodes = self._optional_positive_int(params, "max_nodes", default=64)
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().validate_patch_with_discovery_context_at_position_json(
            workspace_root,
            file_path,
            row,
            column,
            new_code,
            source,
            bypass_reason,
            direction,
            max_depth,
            max_nodes,
            index_db_path,
        )
        return self._decode_core_object(payload)

    def _rebuild_symbol_index(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        db_path = self._require_string(params, "db_path")
        max_files = self._optional_positive_int(params, "max_files", default=20000)
        payload = self._require_core().rebuild_symbol_index_json(
            workspace_root, db_path, max_files
        )
        return self._decode_core_object(payload)

    def _inspect_symbol_index(self, params: dict[str, Any]) -> dict[str, Any]:
        db_path = self._require_string(params, "db_path")
        payload = self._require_core().inspect_symbol_index_json(db_path)
        return self._decode_core_object(payload)

    def _register_symbol_index(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        db_path = self._require_string(params, "db_path")
        payload = self._require_core().register_symbol_index_json(workspace_root, db_path)
        return self._decode_core_object(payload)

    def _refresh_symbol_index_for_file(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        db_path = self._require_string(params, "db_path")
        file_path = self._require_string(params, "file_path")
        max_files = self._optional_positive_int(params, "max_files", default=20000)
        payload = self._require_core().refresh_symbol_index_for_file_json(
            workspace_root,
            db_path,
            file_path,
            max_files,
        )
        return self._decode_core_object(payload)

    def _unregister_symbol_index(self, params: dict[str, Any]) -> bool:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        return self._require_core().unregister_symbol_index_json(workspace_root)

    def _list_symbol_indexes(self, params: dict[str, Any]) -> list[dict[str, Any]]:
        del params
        payload = self._require_core().list_symbol_indexes_json()
        return self._decode_core_object_array(payload)

    def _did_open(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        source = self._optional_string(params, "source", allow_empty=True)
        payload = self._require_core().open_virtual_file_json(file_path, source)
        return self._decode_core_object(payload)

    def _did_change(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        edits = params.get("edits")
        if not isinstance(edits, list):
            raise JsonRpcError(-32602, "missing required list param: edits")
        self._validate_position_edits(edits)
        edits_json = self._encode_json_param(edits, "edits")
        payload = self._require_core().apply_position_edits_json(
            file_path,
            edits_json,
        )
        return self._decode_core_object(payload)

    def _did_close(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        persist = self._optional_bool(params, "persist", default=False)
        payload = self._require_core().close_virtual_file_json(file_path, persist)
        return self._decode_core_object(payload)

    def _list_virtual_files(self, params: dict[str, Any]) -> list[dict[str, Any]]:
        dirty_only = self._optional_bool(params, "dirty_only", default=False)
        payload = self._require_core().list_virtual_files_json(dirty_only)
        return self._decode_core_object_array(payload)

    def _read_virtual_file(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        payload = self._require_core().read_virtual_file_json(file_path)
        return self._decode_core_object(payload)

    def _apply_buffer_edit(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        start_byte = self._require_nonnegative_int(params, "start_byte")
        old_end_byte = self._require_nonnegative_int(params, "old_end_byte")
        if start_byte > old_end_byte:
            raise JsonRpcError(
                -32602,
                "invalid buffer edit range: start_byte is after old_end_byte",
            )
        new_text = self._require_string(params, "new_text", allow_empty=True)
        payload = self._require_core().apply_buffer_edit_json(
            file_path,
            start_byte,
            old_end_byte,
            new_text,
        )
        return self._decode_core_object(payload)

    def _commit_virtual_file(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        payload = self._require_core().commit_virtual_file_json(file_path)
        return self._decode_core_object(payload)

    def _discard_virtual_file(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        payload = self._require_core().discard_virtual_file_json(file_path)
        return self._decode_core_object(payload)

    @staticmethod
    def _decode_core_payload(payload: str) -> Any:
        try:
            return json.loads(
                payload,
                parse_constant=_reject_nonstandard_json_constant,
                object_pairs_hook=_reject_duplicate_object_keys,
            )
        except (json.JSONDecodeError, ValueError) as exc:
            raise JsonRpcError(-32000, f"invalid JSON from arborist core: {exc}") from exc

    @staticmethod
    def _decode_core_object(payload: str) -> dict[str, Any]:
        value = ArboristGateway._decode_core_payload(payload)
        if not isinstance(value, dict):
            raise JsonRpcError(
                -32000,
                "invalid JSON from arborist core: expected object payload",
            )
        return value

    @staticmethod
    def _decode_core_object_array(payload: str) -> list[dict[str, Any]]:
        value = ArboristGateway._decode_core_payload(payload)
        if not isinstance(value, list):
            raise JsonRpcError(
                -32000,
                "invalid JSON from arborist core: expected array payload",
            )
        for index, item in enumerate(value):
            if not isinstance(item, dict):
                raise JsonRpcError(
                    -32000,
                    f"invalid JSON from arborist core: expected object item at index {index}",
                )
        return value

    @staticmethod
    def _require_string(
        params: dict[str, Any],
        key: str,
        allow_empty: bool = False,
        max_length: int | None = None,
    ) -> str:
        value = params.get(key)
        if not isinstance(value, str) or (not allow_empty and not value.strip()):
            raise JsonRpcError(-32602, f"missing required string param: {key}")
        effective_max_length = max_length or STRING_PARAM_MAX_LENGTHS.get(key)
        ArboristGateway._validate_string_length(value, key, effective_max_length)
        return value

    @staticmethod
    def _require_int(params: dict[str, Any], key: str) -> int:
        value = params.get(key)
        if not isinstance(value, int) or isinstance(value, bool):
            raise JsonRpcError(-32602, f"missing required int param: {key}")
        return value

    @staticmethod
    def _require_nonnegative_int(params: dict[str, Any], key: str) -> int:
        value = ArboristGateway._require_int(params, key)
        if value < 0:
            raise JsonRpcError(-32602, f"invalid non-negative int param: {key}")
        return value

    @staticmethod
    def _optional_string(
        params: dict[str, Any],
        key: str,
        default: str | None = None,
        allow_empty: bool = False,
        max_length: int | None = None,
    ) -> str | None:
        if key in params:
            value = params[key]
        else:
            value = default
        if value is None:
            if key in params and default is not None:
                raise JsonRpcError(-32602, f"invalid string param: {key}")
            return None
        if not isinstance(value, str) or (not allow_empty and not value.strip()):
            raise JsonRpcError(-32602, f"invalid string param: {key}")
        effective_max_length = max_length or STRING_PARAM_MAX_LENGTHS.get(key)
        ArboristGateway._validate_string_length(value, key, effective_max_length)
        return value

    @staticmethod
    def _validate_string_length(
        value: str,
        key: str,
        max_length: int | None,
    ) -> None:
        if max_length is not None and len(value) > max_length:
            raise JsonRpcError(
                -32602,
                f"invalid string param: {key} exceeds max length {max_length}",
            )

    @staticmethod
    def _optional_int(params: dict[str, Any], key: str, default: int) -> int:
        value = params.get(key, default)
        if not isinstance(value, int) or isinstance(value, bool):
            raise JsonRpcError(-32602, f"invalid int param: {key}")
        if value < 0:
            raise JsonRpcError(-32602, f"invalid non-negative int param: {key}")
        return value

    @staticmethod
    def _optional_positive_int(params: dict[str, Any], key: str, default: int) -> int:
        value = ArboristGateway._optional_int(params, key, default)
        if value == 0:
            raise JsonRpcError(-32602, f"invalid positive int param: {key}")
        return value

    @staticmethod
    def _optional_bool(params: dict[str, Any], key: str, default: bool) -> bool:
        value = params.get(key, default)
        if not isinstance(value, bool):
            raise JsonRpcError(-32602, f"invalid bool param: {key}")
        return value

    @staticmethod
    def _optional_string_list(params: dict[str, Any], key: str) -> list[str] | None:
        value = params.get(key)
        if value is None:
            return None
        if not isinstance(value, list) or not all(
            isinstance(item, str) and item.strip() for item in value
        ):
            raise JsonRpcError(-32602, f"invalid string list param: {key}")
        return value

    @staticmethod
    def _optional_choice(
        params: dict[str, Any],
        key: str,
        *,
        default: str,
        allowed: tuple[str, ...],
    ) -> str:
        value = ArboristGateway._optional_string(params, key, default=default)
        if value not in allowed:
            choices = "|".join(allowed)
            raise JsonRpcError(-32602, f"invalid {key} param: expected {choices}")
        return value

    @staticmethod
    def _validate_position_edits(edits: list[Any]) -> None:
        for index, edit in enumerate(edits):
            if not isinstance(edit, dict):
                raise JsonRpcError(-32602, f"invalid position edit at index {index}")
            extra_keys = set(edit) - {"start", "end", "new_text"}
            if extra_keys:
                key = sorted(extra_keys)[0]
                raise JsonRpcError(
                    -32602,
                    f"invalid position edit field: edits[{index}].{key}",
                )
            ArboristGateway._validate_position(edit.get("start"), f"edits[{index}].start")
            ArboristGateway._validate_position(edit.get("end"), f"edits[{index}].end")
            if ArboristGateway._position_tuple(edit["start"]) > ArboristGateway._position_tuple(
                edit["end"]
            ):
                raise JsonRpcError(
                    -32602,
                    f"invalid position edit range: edits[{index}].start is after edits[{index}].end",
                )
            if not isinstance(edit.get("new_text"), str):
                raise JsonRpcError(-32602, f"invalid string param: edits[{index}].new_text")
            ArboristGateway._validate_string_length(
                edit["new_text"],
                f"edits[{index}].new_text",
                TEXT_PARAM_MAX_LENGTH,
            )

    @staticmethod
    def _position_tuple(value: dict[str, Any]) -> tuple[int, int]:
        return (value["row"], value["column"])

    @staticmethod
    def _require_position(params: dict[str, Any], key: str) -> tuple[int, int]:
        value = params.get(key)
        ArboristGateway._validate_position(value, key)
        assert isinstance(value, dict)
        return (value["row"], value["column"])

    @staticmethod
    def _validate_position(value: Any, key: str) -> None:
        if not isinstance(value, dict):
            raise JsonRpcError(-32602, f"invalid position param: {key}")
        extra_keys = set(value) - {"row", "column"}
        if extra_keys:
            field = sorted(extra_keys)[0]
            raise JsonRpcError(-32602, f"invalid position field: {key}.{field}")
        for coordinate in ("row", "column"):
            coordinate_value = value.get(coordinate)
            if (
                not isinstance(coordinate_value, int)
                or isinstance(coordinate_value, bool)
                or coordinate_value < 0
            ):
                raise JsonRpcError(
                    -32602,
                    f"invalid non-negative int param: {key}.{coordinate}",
                )

    @staticmethod
    def _reject_unexpected_params(
        params: dict[str, Any], allowed_keys: tuple[str, ...]
    ) -> None:
        unexpected_keys = set(params) - set(allowed_keys)
        if unexpected_keys:
            key = sorted(unexpected_keys)[0]
            raise JsonRpcError(-32602, f"unexpected param: {key}")

    @staticmethod
    def _encode_json_param(value: Any, key: str) -> str:
        ArboristGateway._validate_json_param(value, key)
        try:
            return json.dumps(value, ensure_ascii=False, allow_nan=False)
        except (TypeError, ValueError) as exc:
            raise JsonRpcError(-32602, f"invalid JSON-compatible param: {key}") from exc

    @staticmethod
    def _validate_json_param(value: Any, path: str) -> None:
        if value is None or isinstance(value, (bool, str)):
            return
        if isinstance(value, int) and not isinstance(value, bool):
            return
        if isinstance(value, float):
            if math.isfinite(value):
                return
            raise JsonRpcError(-32602, f"invalid finite number param: {path}")
        if isinstance(value, list):
            for index, item in enumerate(value):
                ArboristGateway._validate_json_param(item, f"{path}[{index}]")
            return
        if isinstance(value, dict):
            for item_key, item_value in value.items():
                if not isinstance(item_key, str):
                    raise JsonRpcError(-32602, f"invalid string object key param: {path}")
                ArboristGateway._validate_json_param(
                    item_value,
                    f"{path}.{item_key}",
                )
            return
        raise JsonRpcError(-32602, f"invalid JSON-compatible param: {path}")


def is_notification_request(request: Any) -> bool:
    return (
        isinstance(request, dict)
        and request.get("jsonrpc") == "2.0"
        and "id" not in request
        and isinstance(request.get("method"), str)
        and bool(request.get("method"))
    )


def is_valid_request_id(request_id: Any) -> bool:
    if request_id is None or isinstance(request_id, str):
        return True

    if isinstance(request_id, bool):
        return False

    if isinstance(request_id, int):
        return True

    return False


def _reject_nonstandard_json_constant(name: str) -> Any:
    raise ValueError(f"non-standard JSON constant: {name}")


def _reject_duplicate_object_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    obj: dict[str, Any] = {}
    for key, value in pairs:
        if key in obj:
            raise ValueError(f"duplicate JSON object key: {key}")
        obj[key] = value
    return obj


def parse_request_json(raw_request: str) -> tuple[Any | None, dict[str, Any] | None]:
    try:
        return json.loads(
            raw_request,
            parse_constant=_reject_nonstandard_json_constant,
            object_pairs_hook=_reject_duplicate_object_keys,
        ), None
    except (json.JSONDecodeError, ValueError) as exc:
        return None, ArboristGateway._error_response(
            None,
            -32700,
            f"invalid JSON: {exc}",
        )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="MCP-compatible stdio JSON-RPC gateway for the Arborist Rust core."
    )
    parser.add_argument(
        "--version",
        action="version",
        version=f"%(prog)s {__version__}",
    )
    parser.add_argument(
        "--once",
        type=Path,
        help="Read one request from a JSON file and print the response.",
    )
    parser.add_argument(
        "--dump-tool-catalog",
        action="store_true",
        help="Print the generated MCP tool catalog as JSON and exit.",
    )
    return parser


def run_stdio() -> int:
    gateway: ArboristGateway | None = None

    for raw_line in sys.stdin:
        line = raw_line.strip()
        if not line:
            continue

        request, response = parse_request_json(line)
        if response is None:
            if gateway is None:
                gateway = ArboristGateway()
            response = gateway.handle_request(request)

        if response is not None and not is_notification_request(request):
            if not _write_response(_serialize_response(response) + "\n"):
                return 0

    return 0


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.dump_tool_catalog:
        if not _print_response(
            json.dumps(build_tool_catalog(), ensure_ascii=False, allow_nan=False, indent=2)
        ):
            return 0
        return 0

    if args.once:
        try:
            raw_request = args.once.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as exc:
            print(
                f"error: failed to read request file {args.once}: {exc}",
                file=sys.stderr,
            )
            return 1
        request, response = parse_request_json(raw_request)
        if response is None:
            gateway = ArboristGateway()
            response = gateway.handle_request(request)
        if response is not None and not is_notification_request(request):
            if not _print_response(_serialize_response(response, indent=2)):
                return 0
        return 0

    return run_stdio()


def _write_response(payload: str) -> bool:
    try:
        sys.stdout.write(payload)
        sys.stdout.flush()
    except BrokenPipeError:
        return False
    return True


def _serialize_response(response: dict[str, Any], indent: int | None = None) -> str:
    try:
        return json.dumps(response, ensure_ascii=False, allow_nan=False, indent=indent)
    except (TypeError, ValueError) as exc:
        response_id = response.get("id")
        fallback = {
            "jsonrpc": "2.0",
            "id": response_id if is_valid_request_id(response_id) else None,
            "error": {
                "code": -32000,
                "message": f"failed to serialize response: {exc}",
            },
        }
        return json.dumps(fallback, ensure_ascii=False, allow_nan=False, indent=indent)


def _print_response(payload: str) -> bool:
    try:
        print(payload)
    except BrokenPipeError:
        return False
    return True


if __name__ == "__main__":
    raise SystemExit(main())
