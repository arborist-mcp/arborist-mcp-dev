from __future__ import annotations

import importlib
import json
from pathlib import Path
from typing import Any

from . import __version__
from .gateway_cli import (
    build_parser as _build_parser,
    main as _gateway_main,
    run_stdio as _run_stdio,
)
from .batch_tools import batch_tools
from .gateway_index_routes import GatewayIndexRoutes
from .gateway_patch_routes import GatewayPatchRoutes
from .gateway_symbol_routes import GatewaySymbolRoutes
from .gateway_params import GatewayParameterValidation
from .gateway_trace_routes import GatewayTraceRoutes
from .gateway_vfs_routes import GatewayVfsRoutes
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
    MAX_WORKSPACE_SCAN_TIMEOUT_MS,
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


class ArboristGateway(
    GatewayIndexRoutes,
    GatewayPatchRoutes,
    GatewaySymbolRoutes,
    GatewayTraceRoutes,
    GatewayVfsRoutes,
    GatewayParameterValidation,
):
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
        timeout_ms = self._optional_positive_int_or_none(params, "timeout_ms")
        payload = self._require_core().execute_tree_query_json(
            file_path, query, source, max_captures, timeout_ms
        )
        return self._decode_core_object_array(payload)

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

if __name__ == "__main__":
    raise SystemExit(main())
