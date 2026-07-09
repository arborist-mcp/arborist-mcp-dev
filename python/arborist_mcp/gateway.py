from __future__ import annotations

import argparse
import importlib
import json
import math
import sys
from pathlib import Path
from typing import Any

from . import __version__


TOOL_HANDLERS = {
    "arborist/get_semantic_skeleton": "_get_semantic_skeleton",
    "arborist/patch_ast_node": "_patch_ast_node",
    "arborist/patch_virtual_ast_node": "_patch_virtual_ast_node",
    "arborist/register_symbol_index": "_register_symbol_index",
    "arborist/refresh_symbol_index_for_file": "_refresh_symbol_index_for_file",
    "arborist/unregister_symbol_index": "_unregister_symbol_index",
    "arborist/list_symbol_indexes": "_list_symbol_indexes",
    "arborist/did_open": "_did_open",
    "arborist/did_change": "_did_change",
    "arborist/did_close": "_did_close",
    "arborist/list_virtual_files": "_list_virtual_files",
    "arborist/read_virtual_file": "_read_virtual_file",
    "arborist/apply_buffer_edit": "_apply_buffer_edit",
    "arborist/commit_virtual_file": "_commit_virtual_file",
    "arborist/discard_virtual_file": "_discard_virtual_file",
    "arborist/rebuild_symbol_index": "_rebuild_symbol_index",
    "arborist/trace_symbol_graph": "_trace_symbol_graph",
    "arborist/trace_symbol_neighborhood": "_trace_symbol_neighborhood",
    "arborist/read_symbol": "_read_symbol",
    "arborist/read_symbol_at_position": "_read_symbol_at_position",
    "arborist/read_symbol_context": "_read_symbol_context",
    "arborist/read_symbol_context_at_position": "_read_symbol_context_at_position",
    "arborist/read_symbol_neighborhood_context": "_read_symbol_neighborhood_context",
    "arborist/read_symbol_neighborhood_context_at_position": "_read_symbol_neighborhood_context_at_position",
    "arborist/read_symbol_discovery_context": "_read_symbol_discovery_context",
    "arborist/read_symbol_discovery_context_at_position": "_read_symbol_discovery_context_at_position",
    "arborist/list_symbols": "_list_symbols",
    "arborist/list_symbols_context": "_list_symbols_context",
    "arborist/list_symbols_neighborhood_context": "_list_symbols_neighborhood_context",
    "arborist/list_symbols_discovery_context": "_list_symbols_discovery_context",
    "arborist/search_symbols": "_search_symbols",
    "arborist/search_symbols_context": "_search_symbols_context",
    "arborist/search_symbols_neighborhood_context": "_search_symbols_neighborhood_context",
    "arborist/search_symbols_discovery_context": "_search_symbols_discovery_context",
    "arborist/replay_patch_evidence_against_trace": "_replay_patch_evidence_against_trace",
    "arborist/validate_patch_commit_with_trace": "_validate_patch_commit_with_trace",
    "arborist/validate_patch_with_trace_context": "_validate_patch_with_trace_context",
    "arborist/validate_patch_with_graph_context": "_validate_patch_with_graph_context",
    "arborist/validate_patch_with_neighborhood_context": "_validate_patch_with_neighborhood_context",
    "arborist/validate_patch_with_discovery_context": "_validate_patch_with_discovery_context",
    "arborist/execute_tree_query": "_execute_tree_query",
}
TOOL_NAMES = tuple(TOOL_HANDLERS)
TOOL_PARAM_NAMES = {
    "arborist/get_semantic_skeleton": (
        "file_path",
        "depth_limit",
        "source",
        "expand_nodes",
    ),
    "arborist/patch_ast_node": (
        "file_path",
        "semantic_path",
        "new_code",
        "source",
        "bypass_reason",
    ),
    "arborist/patch_virtual_ast_node": (
        "file_path",
        "semantic_path",
        "new_code",
        "bypass_reason",
    ),
    "arborist/register_symbol_index": ("workspace_root", "db_path"),
    "arborist/refresh_symbol_index_for_file": (
        "workspace_root",
        "db_path",
        "file_path",
    ),
    "arborist/unregister_symbol_index": ("workspace_root",),
    "arborist/list_symbol_indexes": (),
    "arborist/did_open": ("file_path", "source"),
    "arborist/did_change": ("file_path", "edits"),
    "arborist/did_close": ("file_path", "persist"),
    "arborist/list_virtual_files": ("dirty_only",),
    "arborist/read_virtual_file": ("file_path",),
    "arborist/apply_buffer_edit": (
        "file_path",
        "start_byte",
        "old_end_byte",
        "new_text",
    ),
    "arborist/commit_virtual_file": ("file_path",),
    "arborist/discard_virtual_file": ("file_path",),
    "arborist/rebuild_symbol_index": ("workspace_root", "db_path"),
    "arborist/trace_symbol_graph": (
        "workspace_root",
        "symbol_path",
        "direction",
        "index_db_path",
    ),
    "arborist/trace_symbol_neighborhood": (
        "workspace_root",
        "symbol_path",
        "direction",
        "max_depth",
        "max_nodes",
        "index_db_path",
    ),
    "arborist/read_symbol": (
        "workspace_root",
        "symbol_path",
        "index_db_path",
    ),
    "arborist/read_symbol_at_position": (
        "workspace_root",
        "file_path",
        "position",
        "index_db_path",
    ),
    "arborist/read_symbol_context": (
        "workspace_root",
        "symbol_path",
        "direction",
        "index_db_path",
    ),
    "arborist/read_symbol_context_at_position": (
        "workspace_root",
        "file_path",
        "position",
        "direction",
        "index_db_path",
    ),
    "arborist/read_symbol_neighborhood_context": (
        "workspace_root",
        "symbol_path",
        "direction",
        "max_depth",
        "max_nodes",
        "index_db_path",
    ),
    "arborist/read_symbol_neighborhood_context_at_position": (
        "workspace_root",
        "file_path",
        "position",
        "direction",
        "max_depth",
        "max_nodes",
        "index_db_path",
    ),
    "arborist/read_symbol_discovery_context": (
        "workspace_root",
        "symbol_path",
        "direction",
        "max_depth",
        "max_nodes",
        "index_db_path",
    ),
    "arborist/read_symbol_discovery_context_at_position": (
        "workspace_root",
        "file_path",
        "position",
        "direction",
        "max_depth",
        "max_nodes",
        "index_db_path",
    ),
    "arborist/list_symbols": (
        "workspace_root",
        "limit",
        "index_db_path",
        "file_path_contains",
        "node_kind",
    ),
    "arborist/list_symbols_context": (
        "workspace_root",
        "limit",
        "index_db_path",
        "file_path_contains",
        "node_kind",
    ),
    "arborist/list_symbols_neighborhood_context": (
        "workspace_root",
        "limit",
        "direction",
        "max_depth",
        "max_nodes",
        "index_db_path",
        "file_path_contains",
        "node_kind",
    ),
    "arborist/list_symbols_discovery_context": (
        "workspace_root",
        "limit",
        "direction",
        "max_depth",
        "max_nodes",
        "index_db_path",
        "file_path_contains",
        "node_kind",
    ),
    "arborist/search_symbols": (
        "workspace_root",
        "query",
        "limit",
        "index_db_path",
        "file_path_contains",
        "node_kind",
    ),
    "arborist/search_symbols_context": (
        "workspace_root",
        "query",
        "limit",
        "index_db_path",
        "file_path_contains",
        "node_kind",
    ),
    "arborist/search_symbols_neighborhood_context": (
        "workspace_root",
        "query",
        "limit",
        "direction",
        "max_depth",
        "max_nodes",
        "index_db_path",
        "file_path_contains",
        "node_kind",
    ),
    "arborist/search_symbols_discovery_context": (
        "workspace_root",
        "query",
        "limit",
        "direction",
        "max_depth",
        "max_nodes",
        "index_db_path",
        "file_path_contains",
        "node_kind",
    ),
    "arborist/replay_patch_evidence_against_trace": ("patch", "trace"),
    "arborist/validate_patch_commit_with_trace": ("patch", "trace"),
    "arborist/validate_patch_with_trace_context": (
        "workspace_root",
        "file_path",
        "semantic_path",
        "new_code",
        "source",
        "bypass_reason",
        "direction",
    ),
    "arborist/validate_patch_with_graph_context": (
        "workspace_root",
        "file_path",
        "semantic_path",
        "new_code",
        "source",
        "bypass_reason",
        "direction",
        "max_depth",
        "max_nodes",
    ),
    "arborist/validate_patch_with_neighborhood_context": (
        "workspace_root",
        "file_path",
        "semantic_path",
        "new_code",
        "source",
        "bypass_reason",
        "direction",
        "max_depth",
        "max_nodes",
    ),
    "arborist/validate_patch_with_discovery_context": (
        "workspace_root",
        "file_path",
        "semantic_path",
        "new_code",
        "source",
        "bypass_reason",
        "direction",
        "max_depth",
        "max_nodes",
    ),
    "arborist/execute_tree_query": ("file_path", "query", "source"),
}


class JsonRpcError(ValueError):
    def __init__(self, code: int, message: str) -> None:
        super().__init__(message)
        self.code = code


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
                self._reject_unexpected_params(params, ())
                result = {
                    "serverInfo": {
                        "name": "arborist-mcp",
                        "version": __version__,
                    },
                    "capabilities": {"tools": list(TOOL_NAMES)},
                    "supportedLanguages": self._require_core().supported_languages(),
                }
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
        payload = self._require_core().trace_symbol_graph_json(
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
        payload = self._require_core().trace_symbol_neighborhood_json(
            workspace_root,
            symbol_path,
            direction,
            max_depth,
            max_nodes,
            index_db_path,
        )
        return self._decode_core_object(payload)

    def _read_symbol(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        symbol_path = self._require_string(params, "symbol_path")
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().read_symbol_json(
            workspace_root,
            symbol_path,
            index_db_path,
        )
        return self._decode_core_object(payload)

    def _read_symbol_at_position(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        file_path = self._require_string(params, "file_path")
        row, column = self._require_position(params, "position")
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().read_symbol_at_position_json(
            workspace_root,
            file_path,
            row,
            column,
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
        payload = self._require_core().read_symbol_context_json(
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
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().read_symbol_context_at_position_json(
            workspace_root,
            file_path,
            row,
            column,
            direction,
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
        payload = self._require_core().read_symbol_neighborhood_context_json(
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
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().read_symbol_neighborhood_context_at_position_json(
            workspace_root,
            file_path,
            row,
            column,
            direction,
            max_depth,
            max_nodes,
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
        payload = self._require_core().read_symbol_discovery_context_json(
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
        index_db_path = self._optional_string(params, "index_db_path")
        payload = self._require_core().read_symbol_discovery_context_at_position_json(
            workspace_root,
            file_path,
            row,
            column,
            direction,
            max_depth,
            max_nodes,
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
        payload = self._require_core().search_symbols_json(
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
        payload = self._require_core().search_symbols_context_json(
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
        payload = self._require_core().search_symbols_neighborhood_context_json(
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
        payload = self._require_core().search_symbols_discovery_context_json(
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
        payload = self._require_core().list_symbols_json(
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
        payload = self._require_core().list_symbols_context_json(
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
        payload = self._require_core().list_symbols_neighborhood_context_json(
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
        payload = self._require_core().list_symbols_discovery_context_json(
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
        payload = self._require_core().validate_patch_with_trace_context_json(
            workspace_root,
            file_path,
            semantic_path,
            new_code,
            source,
            bypass_reason,
            direction,
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
        )
        return self._decode_core_object(payload)

    def _rebuild_symbol_index(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        db_path = self._require_string(params, "db_path")
        payload = self._require_core().rebuild_symbol_index_json(workspace_root, db_path)
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
        description="Thin stdio JSON-RPC gateway for the Arborist Rust core."
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
