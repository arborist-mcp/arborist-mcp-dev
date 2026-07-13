from __future__ import annotations

import importlib
import json
import math
from pathlib import Path
from typing import Any

from . import __version__
from .gateway_cli import (
    build_parser as _build_parser,
    main as _gateway_main,
    run_stdio as _run_stdio,
)
from .batch_tools import batch_tools
from .jsonrpc import (
    error_response,
    _reject_duplicate_object_keys,
    _reject_nonstandard_json_constant,
    is_notification_request,
    is_valid_request_id,
    parse_request_json,
    print_response as _print_response,
    serialize_response as _serialize_response,
    write_response as _write_response,
)
from .mcp_lifecycle import initialized, initialize, server_info
from .mcp_tools import tools_call, tools_list
from .mcp_validation import reject_unexpected_params
from .resources import resources_list, resources_read
from .tool_manifest import (
    build_resource_catalog,
    build_tool_catalog,
    required_tool_params,
)
from .tool_specs import (
    BYPASS_REASON_MAX_LENGTH,
    MAX_BATCH_CALLS,
    MAX_GRAPH_DEPTH,
    MAX_GRAPH_NODES,
    MAX_SYMBOL_LIMIT,
    MAX_WORKSPACE_SCAN_FILE_BYTES,
    MAX_WORKSPACE_SCAN_FILES,
    MCP_PROTOCOL_VERSION,
    MUTATING_TOOLS,
    NON_MUTATING_STATE_TOOLS,
    OPTIONAL_TOOL_PARAMS,
    READ_ONLY_CATEGORIES,
    SOURCE_ANCHORED_OPTIONAL_FILE_PATH_TOOLS,
    STRING_PARAM_MAX_LENGTHS,
    TEXT_PARAM_MAX_LENGTH,
    TOOL_CATEGORIES,
    TOOL_CATALOG_RESOURCE_URI,
    TOOL_HANDLERS,
    TOOL_NAMES,
    TOOL_PARAM_DEFAULTS,
    TOOL_PARAM_NAMES,
    TOOL_PARAM_SCHEMAS,
    TOOL_PARAM_SPECS,
    TOOL_SPECS,
    TOOL_SPECS_BY_NAME,
    TREE_QUERY_MAX_LENGTH,
    TREE_QUERY_MAX_CAPTURES,
    WRITING_TOOLS,
    tool_param_spec,
    tool_spec,
)
from .tool_result_schemas import (
    JsonRpcError,
    OBJECT_RESULT_SCHEMA,
    PATCH_AST_NODE_RESULT_SCHEMA,
    SEMANTIC_SKELETON_RESULT_SCHEMA,
    SYMBOL_INDEX_HEALTH_RESULT_SCHEMA,
    SYMBOL_LIST_RESULT_SCHEMA,
    TOOL_RESULT_SCHEMAS,
)


def build_parser():
    return _build_parser(__version__)


def run_stdio() -> int:
    return _run_stdio(
        gateway_factory=ArboristGateway,
        parse_request=parse_request_json,
        is_notification=is_notification_request,
        serialize_response=_serialize_response,
        write_response=_write_response,
    )


def main(argv: list[str] | None = None) -> int:
    return _gateway_main(
        argv=argv,
        version=__version__,
        gateway_factory=ArboristGateway,
        build_tool_catalog=build_tool_catalog,
        parse_request=parse_request_json,
        is_notification=is_notification_request,
        serialize_response=_serialize_response,
        print_response=_print_response,
        run_stdio=run_stdio,
    )


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
            return error_response(None, -32600, "invalid request: expected object")

        request_id = request.get("id")
        response_id = request_id if is_valid_request_id(request_id) else None
        jsonrpc_version = request.get("jsonrpc")
        if jsonrpc_version != "2.0":
            return error_response(
                response_id,
                -32600,
                "invalid request: expected jsonrpc='2.0'",
            )

        method = request.get("method")
        params = request.get("params", {})

        if "id" in request and not is_valid_request_id(request_id):
            return error_response(None, -32600, "invalid request: invalid id")

        if not isinstance(method, str) or not method:
            return error_response(response_id, -32600, "invalid request: missing method")

        if not isinstance(params, dict):
            return error_response(response_id, -32602, "invalid params: expected object")

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
            elif method in TOOL_SPECS_BY_NAME:
                spec = tool_spec(method)
                self._reject_unexpected_params(params, spec.params)
                handler = getattr(self, spec.handler)
                result = handler(params)
            else:
                return error_response(response_id, -32601, f"method not found: {method}")

            return {"jsonrpc": "2.0", "id": request_id, "result": result}
        except JsonRpcError as exc:
            return error_response(response_id, exc.code, str(exc))
        except ValueError as exc:
            return error_response(response_id, -32602, str(exc))
        except Exception as exc:  # noqa: BLE001
            return error_response(response_id, -32000, str(exc))

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

    @staticmethod
    def _ensure_write_path_inside_server_workspace(file_path: str) -> None:
        workspace = Path.cwd().resolve()
        candidate = Path(file_path).resolve(strict=False)
        try:
            candidate.relative_to(workspace)
        except ValueError as exc:
            raise JsonRpcError(
                -32602,
                f"invalid params: file_path is outside server workspace: {file_path}",
            ) from exc

    def _initialize(self, params: dict[str, Any]) -> dict[str, Any]:
        return initialize(
            params,
            server_info=server_info(__version__),
            supported_languages=lambda: self._require_core().supported_languages(),
        )

    def _initialized(self, params: dict[str, Any]) -> dict[str, Any]:
        return initialized(params)

    def _tools_list(self, params: dict[str, Any]) -> dict[str, Any]:
        return tools_list(params)

    def _resources_list(self, params: dict[str, Any]) -> dict[str, Any]:
        return resources_list(params)

    def _resources_read(self, params: dict[str, Any]) -> dict[str, Any]:
        return resources_read(params)

    def _tools_call(self, params: dict[str, Any]) -> dict[str, Any]:
        return tools_call(params, self._execute_tool)

    def _execute_tool(self, tool_name: str, params: dict[str, Any]) -> Any:
        spec = tool_spec(tool_name)
        handler = getattr(self, spec.handler)
        return handler(params)

    def _batch(self, params: dict[str, Any]) -> list[dict[str, Any]]:
        return batch_tools(params, self._execute_tool)

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
        if source is None:
            self._ensure_write_path_inside_server_workspace(file_path)
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
        if source is None:
            self._ensure_write_path_inside_server_workspace(file_path)
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
        max_file_bytes = self._optional_positive_int_or_none(params, "max_file_bytes")
        core = self._require_core()
        if max_file_bytes is None:
            payload = core.rebuild_symbol_index_json(workspace_root, db_path, max_files)
        else:
            payload = core.rebuild_symbol_index_json(
                workspace_root, db_path, max_files, max_file_bytes
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
        max_file_bytes = self._optional_positive_int_or_none(params, "max_file_bytes")
        core = self._require_core()
        if max_file_bytes is None:
            payload = core.refresh_symbol_index_for_file_json(
                workspace_root,
                db_path,
                file_path,
                max_files,
            )
        else:
            payload = core.refresh_symbol_index_for_file_json(
                workspace_root,
                db_path,
                file_path,
                max_files,
                max_file_bytes,
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
        if persist:
            self._ensure_write_path_inside_server_workspace(file_path)
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
        self._ensure_write_path_inside_server_workspace(file_path)
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
        param_spec = TOOL_PARAM_SPECS.get(key)
        effective_max_length = max_length or (
            param_spec.string_max_length if param_spec is not None else None
        )
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
        param_spec = TOOL_PARAM_SPECS.get(key)
        effective_max_length = max_length or (
            param_spec.string_max_length if param_spec is not None else None
        )
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
        param_spec = TOOL_PARAM_SPECS.get(key)
        max_value = param_spec.int_max_value if param_spec is not None else None
        if max_value is not None and value > max_value:
            raise JsonRpcError(
                -32602,
                f"invalid int param: {key} exceeds maximum {max_value}",
            )
        return value

    @staticmethod
    def _optional_positive_int(params: dict[str, Any], key: str, default: int) -> int:
        value = ArboristGateway._optional_int(params, key, default)
        if value == 0:
            raise JsonRpcError(-32602, f"invalid positive int param: {key}")
        return value

    @staticmethod
    def _optional_positive_int_or_none(params: dict[str, Any], key: str) -> int | None:
        if key not in params:
            return None
        value = ArboristGateway._optional_int(params, key, 1)
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
        reject_unexpected_params(params, allowed_keys)

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


if __name__ == "__main__":
    raise SystemExit(main())
