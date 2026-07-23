from __future__ import annotations

from typing import Any


class GatewaySymbolListRoutes:
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
