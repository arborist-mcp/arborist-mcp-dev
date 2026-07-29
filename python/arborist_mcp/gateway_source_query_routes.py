from __future__ import annotations

from typing import Any

from .tool_specs import MAX_SEMANTIC_EXPAND_NODES, TREE_QUERY_MAX_LENGTH


class GatewaySourceQueryRoutes:
    """Semantic skeleton and raw Tree-sitter query handlers for the gateway."""

    def _get_semantic_skeleton(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        depth_limit = self._optional_int(params, "depth_limit", default=2)
        source = self._optional_string(params, "source", allow_empty=True)
        expand_nodes = self._optional_string_list(
            params,
            "expand_nodes",
            max_items=MAX_SEMANTIC_EXPAND_NODES,
        )
        timeout_ms = self._optional_positive_int_or_none(params, "timeout_ms")
        payload = self._call_with_optional_timeout(
            self._require_core().get_semantic_skeleton_json,
            (file_path, source, depth_limit, expand_nodes),
            timeout_ms,
        )
        return self._decode_core_object(payload)

    def _execute_tree_query(self, params: dict[str, Any]) -> list[dict[str, Any]]:
        file_path = self._require_string(params, "file_path")
        query = self._require_string(params, "query", max_length=TREE_QUERY_MAX_LENGTH)
        source = self._optional_string(params, "source", allow_empty=True)
        max_captures = self._optional_positive_int(
            params,
            "max_captures",
            default=10000,
        )
        timeout_ms = self._optional_positive_int_or_none(params, "timeout_ms")
        payload = self._require_core().execute_tree_query_json(
            file_path,
            query,
            source,
            max_captures,
            timeout_ms,
        )
        return self._decode_core_object_array(payload)
