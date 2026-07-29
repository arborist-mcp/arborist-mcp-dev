from __future__ import annotations

from typing import Any, NamedTuple

from .tool_definitions import (
    TOOL_CATEGORIES,
    TOOL_HANDLERS,
    TOOL_NAMES,
    TOOL_PARAM_NAMES,
    TOOL_SPECS,
    TOOL_SPECS_BY_NAME,
)
from .tool_spec_models import ToolParamSpec, ToolSpec, _schema
from .tool_param_specs import (
    _SOURCE_ANCHORED_FILE_PATH_TOOLS,
    TREE_QUERY_MAX_LENGTH,
    TREE_QUERY_MAX_CAPTURES,
    TEXT_PARAM_MAX_LENGTH,
    MAX_JSON_ARG_BYTES,
    MAX_JSON_ARG_DEPTH,
    MAX_REQUEST_BYTES,
    MAX_INDEX_WATCH_CONFIG_BYTES,
    MAX_INDEX_WATCH_TARGETS,
    BYPASS_REASON_MAX_LENGTH,
    MAX_BATCH_CALLS,
    MAX_POSITION_EDITS,
    MAX_POSITION_EDIT_TEXT_BYTES,
    MAX_SEMANTIC_EXPAND_NODES,
    MAX_SEMANTIC_SKELETON_DEPTH,
    MAX_WORKSPACE_EDIT_PREVIEW_FILES,
    MAX_GRAPH_DEPTH,
    MAX_GRAPH_NODES,
    MAX_SYMBOL_LIMIT,
    MAX_WORKSPACE_SCAN_FILES,
    MAX_WORKSPACE_SCAN_FILE_BYTES,
    MAX_WORKSPACE_SCAN_TIMEOUT_MS,
    POSITION_SCHEMA,
    POSITION_EDIT_SCHEMA,
    JSON_OBJECT_SCHEMA,
    BATCH_CALL_SCHEMA,
    TOOL_PARAM_SPECS,
    TOOL_PARAM_SCHEMAS,
    OPTIONAL_TOOL_PARAMS,
    SOURCE_ANCHORED_OPTIONAL_FILE_PATH_TOOLS,
    TOOL_PARAM_DEFAULTS,
    STRING_PARAM_MAX_LENGTHS,
)


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
READ_ONLY_CATEGORIES = frozenset(("read", "trace"))
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


def tool_param_spec(param_name: str) -> ToolParamSpec:
    return TOOL_PARAM_SPECS[param_name]
