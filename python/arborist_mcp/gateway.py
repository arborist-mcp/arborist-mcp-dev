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
    ToolSpec("arborist/get_semantic_skeleton", "_get_semantic_skeleton", ("file_path", "depth_limit", "source", "expand_nodes"), "read"),
    ToolSpec("arborist/patch_ast_node", "_patch_ast_node", ("file_path", "semantic_path", "new_code", "source", "bypass_reason"), "write"),
    ToolSpec("arborist/patch_ast_node_at_position", "_patch_ast_node_at_position", ("file_path", "position", "new_code", "source", "bypass_reason"), "write"),
    ToolSpec("arborist/patch_virtual_ast_node", "_patch_virtual_ast_node", ("file_path", "semantic_path", "new_code", "bypass_reason"), "vfs"),
    ToolSpec("arborist/patch_virtual_ast_node_at_position", "_patch_virtual_ast_node_at_position", ("file_path", "position", "new_code", "bypass_reason"), "vfs"),
    ToolSpec("arborist/register_symbol_index", "_register_symbol_index", ("workspace_root", "db_path"), "index"),
    ToolSpec("arborist/refresh_symbol_index_for_file", "_refresh_symbol_index_for_file", ("workspace_root", "db_path", "file_path"), "index"),
    ToolSpec("arborist/unregister_symbol_index", "_unregister_symbol_index", ("workspace_root",), "index", "boolean"),
    ToolSpec("arborist/list_symbol_indexes", "_list_symbol_indexes", (), "index", "object_array"),
    ToolSpec("arborist/inspect_symbol_index", "_inspect_symbol_index", ("db_path",), "index"),
    ToolSpec("arborist/did_open", "_did_open", ("file_path", "source"), "vfs"),
    ToolSpec("arborist/did_change", "_did_change", ("file_path", "edits"), "vfs"),
    ToolSpec("arborist/did_close", "_did_close", ("file_path", "persist"), "vfs"),
    ToolSpec("arborist/list_virtual_files", "_list_virtual_files", ("dirty_only",), "vfs", "object_array"),
    ToolSpec("arborist/read_virtual_file", "_read_virtual_file", ("file_path",), "vfs"),
    ToolSpec("arborist/apply_buffer_edit", "_apply_buffer_edit", ("file_path", "start_byte", "old_end_byte", "new_text"), "vfs"),
    ToolSpec("arborist/commit_virtual_file", "_commit_virtual_file", ("file_path",), "vfs"),
    ToolSpec("arborist/discard_virtual_file", "_discard_virtual_file", ("file_path",), "vfs"),
    ToolSpec("arborist/rebuild_symbol_index", "_rebuild_symbol_index", ("workspace_root", "db_path"), "index"),
    ToolSpec("arborist/trace_symbol_graph", "_trace_symbol_graph", ("workspace_root", "symbol_path", "direction", "index_db_path", "file_path", "source"), "trace"),
    ToolSpec("arborist/trace_symbol_neighborhood", "_trace_symbol_neighborhood", ("workspace_root", "symbol_path", "direction", "max_depth", "max_nodes", "index_db_path", "file_path", "source"), "trace"),
    ToolSpec("arborist/trace_symbol_graph_at_position", "_trace_symbol_graph_at_position", ("workspace_root", "file_path", "position", "direction", "source", "index_db_path"), "trace"),
    ToolSpec("arborist/trace_symbol_neighborhood_at_position", "_trace_symbol_neighborhood_at_position", ("workspace_root", "file_path", "position", "direction", "max_depth", "max_nodes", "source", "index_db_path"), "trace"),
    ToolSpec("arborist/read_symbol", "_read_symbol", ("workspace_root", "symbol_path", "index_db_path", "file_path", "source"), "read"),
    ToolSpec("arborist/read_symbol_at_position", "_read_symbol_at_position", ("workspace_root", "file_path", "position", "source", "index_db_path"), "read"),
    ToolSpec("arborist/read_symbol_context", "_read_symbol_context", ("workspace_root", "symbol_path", "direction", "index_db_path", "file_path", "source"), "read"),
    ToolSpec("arborist/read_symbol_context_at_position", "_read_symbol_context_at_position", ("workspace_root", "file_path", "position", "direction", "source", "index_db_path"), "read"),
    ToolSpec("arborist/read_symbol_neighborhood_context", "_read_symbol_neighborhood_context", ("workspace_root", "symbol_path", "direction", "max_depth", "max_nodes", "index_db_path", "file_path", "source"), "read"),
    ToolSpec("arborist/read_symbol_neighborhood_context_at_position", "_read_symbol_neighborhood_context_at_position", ("workspace_root", "file_path", "position", "direction", "max_depth", "max_nodes", "source", "index_db_path"), "read"),
    ToolSpec("arborist/read_symbol_discovery_context", "_read_symbol_discovery_context", ("workspace_root", "symbol_path", "direction", "max_depth", "max_nodes", "index_db_path", "file_path", "source"), "read"),
    ToolSpec("arborist/read_symbol_discovery_context_at_position", "_read_symbol_discovery_context_at_position", ("workspace_root", "file_path", "position", "direction", "max_depth", "max_nodes", "source", "index_db_path"), "read"),
    ToolSpec("arborist/list_symbols", "_list_symbols", ("workspace_root", "limit", "index_db_path", "file_path_contains", "node_kind", "file_path", "source"), "read"),
    ToolSpec("arborist/list_symbols_context", "_list_symbols_context", ("workspace_root", "limit", "index_db_path", "file_path_contains", "node_kind", "file_path", "source"), "read"),
    ToolSpec("arborist/list_symbols_neighborhood_context", "_list_symbols_neighborhood_context", ("workspace_root", "limit", "direction", "max_depth", "max_nodes", "index_db_path", "file_path_contains", "node_kind", "file_path", "source"), "read"),
    ToolSpec("arborist/list_symbols_discovery_context", "_list_symbols_discovery_context", ("workspace_root", "limit", "direction", "max_depth", "max_nodes", "index_db_path", "file_path_contains", "node_kind", "file_path", "source"), "read"),
    ToolSpec("arborist/search_symbols", "_search_symbols", ("workspace_root", "query", "limit", "index_db_path", "file_path_contains", "node_kind", "file_path", "source"), "read"),
    ToolSpec("arborist/search_symbols_context", "_search_symbols_context", ("workspace_root", "query", "limit", "index_db_path", "file_path_contains", "node_kind", "file_path", "source"), "read"),
    ToolSpec("arborist/search_symbols_neighborhood_context", "_search_symbols_neighborhood_context", ("workspace_root", "query", "limit", "direction", "max_depth", "max_nodes", "index_db_path", "file_path_contains", "node_kind", "file_path", "source"), "read"),
    ToolSpec("arborist/search_symbols_discovery_context", "_search_symbols_discovery_context", ("workspace_root", "query", "limit", "direction", "max_depth", "max_nodes", "index_db_path", "file_path_contains", "node_kind", "file_path", "source"), "read"),
    ToolSpec("arborist/replay_patch_evidence_against_trace", "_replay_patch_evidence_against_trace", ("patch", "trace"), "trace"),
    ToolSpec("arborist/validate_patch_commit_with_trace", "_validate_patch_commit_with_trace", ("patch", "trace"), "trace"),
    ToolSpec("arborist/validate_patch_with_trace_context", "_validate_patch_with_trace_context", ("workspace_root", "file_path", "semantic_path", "new_code", "source", "bypass_reason", "direction", "index_db_path"), "trace"),
    ToolSpec("arborist/validate_patch_with_trace_context_at_position", "_validate_patch_with_trace_context_at_position", ("workspace_root", "file_path", "position", "new_code", "source", "bypass_reason", "direction", "index_db_path"), "trace"),
    ToolSpec("arborist/validate_patch_with_graph_context", "_validate_patch_with_graph_context", ("workspace_root", "file_path", "semantic_path", "new_code", "source", "bypass_reason", "direction", "max_depth", "max_nodes", "index_db_path"), "read"),
    ToolSpec("arborist/validate_patch_with_graph_context_at_position", "_validate_patch_with_graph_context_at_position", ("workspace_root", "file_path", "position", "new_code", "source", "bypass_reason", "direction", "max_depth", "max_nodes", "index_db_path"), "read"),
    ToolSpec("arborist/validate_patch_with_neighborhood_context", "_validate_patch_with_neighborhood_context", ("workspace_root", "file_path", "semantic_path", "new_code", "source", "bypass_reason", "direction", "max_depth", "max_nodes", "index_db_path"), "read"),
    ToolSpec("arborist/validate_patch_with_neighborhood_context_at_position", "_validate_patch_with_neighborhood_context_at_position", ("workspace_root", "file_path", "position", "new_code", "source", "bypass_reason", "direction", "max_depth", "max_nodes", "index_db_path"), "read"),
    ToolSpec("arborist/validate_patch_with_discovery_context", "_validate_patch_with_discovery_context", ("workspace_root", "file_path", "semantic_path", "new_code", "source", "bypass_reason", "direction", "max_depth", "max_nodes", "index_db_path"), "read"),
    ToolSpec("arborist/validate_patch_with_discovery_context_at_position", "_validate_patch_with_discovery_context_at_position", ("workspace_root", "file_path", "position", "new_code", "source", "bypass_reason", "direction", "max_depth", "max_nodes", "index_db_path"), "read"),
    ToolSpec("arborist/execute_tree_query", "_execute_tree_query", ("file_path", "query", "source"), "read", "object_array"),
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
        "max_depth",
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
    )
)
MUTATING_TOOLS = frozenset(
    tool_name
    for tool_name, category in TOOL_CATEGORIES.items()
    if category in {"write", "vfs", "index"}
) - NON_MUTATING_STATE_TOOLS


def _schema(
    schema_type: str,
    description: str,
    *,
    default: Any = None,
    enum: tuple[str, ...] | None = None,
    minimum: int | None = None,
    min_items: int | None = None,
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
TOOL_PARAM_SCHEMAS = {
    "bypass_reason": _schema(
        "string",
        "Required explanation when intentionally bypassing trace-backed commit gates.",
    ),
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
    "file_path": _schema("string", "Source file path. Python and C extensions are supported."),
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
    "new_code": _schema("string", "Replacement source code for the selected AST node."),
    "new_text": _schema(
        "string",
        "Replacement text for a byte-range edit.",
        allow_empty=True,
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
    "query": _schema("string", "Tree-sitter query or symbol search text."),
    "semantic_path": _schema("string", "Stable Arborist semantic selector."),
    "source": _schema(
        "string",
        "Optional unsaved source buffer to analyze instead of reading from disk.",
        allow_empty=True,
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
    "persist": False,
    "workspace_root": ".",
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
BOOLEAN_RESULT_SCHEMA = {
    "type": "boolean",
    "description": "Boolean success result returned by Arborist for this tool.",
}
TOOL_RESULT_SCHEMAS = {
    tool_name: {
        "object_array": OBJECT_ARRAY_RESULT_SCHEMA,
        "boolean": BOOLEAN_RESULT_SCHEMA,
    }[schema_key]
    for tool_name, schema_key in TOOL_RESULT_SCHEMA_KEYS.items()
}


class JsonRpcError(ValueError):
    def __init__(self, code: int, message: str) -> None:
        super().__init__(message)
        self.code = code


def is_mcp_initialize(params: dict[str, Any]) -> bool:
    return bool(MCP_INITIALIZE_MARKERS & set(params))


def build_tool_catalog() -> list[dict[str, Any]]:
    return [build_tool_descriptor(tool_name) for tool_name in TOOL_NAMES]


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
                "capabilities": {"tools": list(TOOL_NAMES)},
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
        query = self._require_string(params, "query")
        source = self._optional_string(params, "source", allow_empty=True)
        payload = self._require_core().execute_tree_query_json(file_path, query, source)
        return self._decode_core_object_array(payload)

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
        max_nodes = self._optional_int(params, "max_nodes", default=64)
        if max_nodes == 0:
            raise JsonRpcError(-32602, "invalid positive int param: max_nodes")
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
        max_nodes = self._optional_int(params, "max_nodes", default=64)
        if max_nodes == 0:
            raise JsonRpcError(-32602, "invalid positive int param: max_nodes")
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
        max_nodes = self._optional_int(params, "max_nodes", default=64)
        if max_nodes == 0:
            raise JsonRpcError(-32602, "invalid positive int param: max_nodes")
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
        max_nodes = self._optional_int(params, "max_nodes", default=64)
        if max_nodes == 0:
            raise JsonRpcError(-32602, "invalid positive int param: max_nodes")
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
        max_nodes = self._optional_int(params, "max_nodes", default=64)
        if max_nodes == 0:
            raise JsonRpcError(-32602, "invalid positive int param: max_nodes")
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
        max_nodes = self._optional_int(params, "max_nodes", default=64)
        if max_nodes == 0:
            raise JsonRpcError(-32602, "invalid positive int param: max_nodes")
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
        query = self._require_string(params, "query")
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
        query = self._require_string(params, "query")
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
        query = self._require_string(params, "query")
        limit = self._optional_int(params, "limit", default=20)
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        max_depth = self._optional_int(params, "max_depth", default=2)
        max_nodes = self._optional_int(params, "max_nodes", default=64)
        if max_nodes == 0:
            raise JsonRpcError(-32602, "invalid positive int param: max_nodes")
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
        query = self._require_string(params, "query")
        limit = self._optional_int(params, "limit", default=20)
        direction = self._optional_choice(
            params,
            "direction",
            default="both",
            allowed=("callers", "callees", "both"),
        )
        max_depth = self._optional_int(params, "max_depth", default=2)
        max_nodes = self._optional_int(params, "max_nodes", default=64)
        if max_nodes == 0:
            raise JsonRpcError(-32602, "invalid positive int param: max_nodes")
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
        max_nodes = self._optional_int(params, "max_nodes", default=64)
        if max_nodes == 0:
            raise JsonRpcError(-32602, "invalid positive int param: max_nodes")
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
        max_nodes = self._optional_int(params, "max_nodes", default=64)
        if max_nodes == 0:
            raise JsonRpcError(-32602, "invalid positive int param: max_nodes")
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
        max_nodes = self._optional_int(params, "max_nodes", default=64)
        if max_nodes == 0:
            raise JsonRpcError(-32602, "invalid positive int param: max_nodes")
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
        max_nodes = self._optional_int(params, "max_nodes", default=64)
        if max_nodes == 0:
            raise JsonRpcError(-32602, "invalid positive int param: max_nodes")
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
        max_nodes = self._optional_int(params, "max_nodes", default=64)
        if max_nodes == 0:
            raise JsonRpcError(-32602, "invalid positive int param: max_nodes")
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
        max_nodes = self._optional_int(params, "max_nodes", default=64)
        if max_nodes == 0:
            raise JsonRpcError(-32602, "invalid positive int param: max_nodes")
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
        max_nodes = self._optional_int(params, "max_nodes", default=64)
        if max_nodes == 0:
            raise JsonRpcError(-32602, "invalid positive int param: max_nodes")
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
        max_nodes = self._optional_int(params, "max_nodes", default=64)
        if max_nodes == 0:
            raise JsonRpcError(-32602, "invalid positive int param: max_nodes")
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
        payload = self._require_core().rebuild_symbol_index_json(workspace_root, db_path)
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
        payload = self._require_core().refresh_symbol_index_for_file_json(
            workspace_root,
            db_path,
            file_path,
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
        params: dict[str, Any], key: str, allow_empty: bool = False
    ) -> str:
        value = params.get(key)
        if not isinstance(value, str) or (not allow_empty and not value.strip()):
            raise JsonRpcError(-32602, f"missing required string param: {key}")
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
        return value

    @staticmethod
    def _optional_int(params: dict[str, Any], key: str, default: int) -> int:
        value = params.get(key, default)
        if not isinstance(value, int) or isinstance(value, bool):
            raise JsonRpcError(-32602, f"invalid int param: {key}")
        if value < 0:
            raise JsonRpcError(-32602, f"invalid non-negative int param: {key}")
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
