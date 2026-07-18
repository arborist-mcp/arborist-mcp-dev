from __future__ import annotations

from typing import Any


class GatewayIndexRoutes:
    def _rebuild_symbol_index(self, params: dict[str, Any]) -> dict[str, Any]:
        return self._run_workspace_index_scan(params, "rebuild_symbol_index_json")

    def _refresh_symbol_index(self, params: dict[str, Any]) -> dict[str, Any]:
        return self._run_workspace_index_scan(params, "refresh_symbol_index_json")

    def _run_workspace_index_scan(
        self,
        params: dict[str, Any],
        core_method_name: str,
    ) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        db_path = self._require_string(params, "db_path")
        max_files = self._optional_positive_int(params, "max_files", default=20000)
        max_file_bytes = self._optional_positive_int_or_none(params, "max_file_bytes")
        timeout_ms = self._optional_positive_int_or_none(params, "timeout_ms")
        core_method = getattr(self._require_core(), core_method_name)
        if timeout_ms is not None:
            payload = core_method(
                workspace_root,
                db_path,
                max_files,
                max_file_bytes,
                timeout_ms,
            )
        elif max_file_bytes is None:
            payload = core_method(workspace_root, db_path, max_files)
        else:
            payload = core_method(workspace_root, db_path, max_files, max_file_bytes)
        return self._decode_core_object(payload)

    def _inspect_symbol_index(self, params: dict[str, Any]) -> dict[str, Any]:
        db_path = self._require_string(params, "db_path")
        timeout_ms = self._optional_positive_int_or_none(params, "timeout_ms")
        core = self._require_core()
        if timeout_ms is None:
            payload = core.inspect_symbol_index_json(db_path)
        else:
            payload = core.inspect_symbol_index_json(db_path, timeout_ms)
        return self._decode_core_object(payload)

    def _migrate_symbol_index(self, params: dict[str, Any]) -> dict[str, Any]:
        db_path = self._require_string(params, "db_path")
        payload = self._require_core().migrate_symbol_index_json(db_path)
        return self._decode_core_object(payload)

    def _register_symbol_index(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        db_path = self._require_string(params, "db_path")
        max_files = self._optional_positive_int(params, "max_files", default=20000)
        max_file_bytes = self._optional_positive_int_or_none(params, "max_file_bytes")
        timeout_ms = self._optional_positive_int_or_none(params, "timeout_ms")
        core = self._require_core()
        if timeout_ms is not None:
            payload = core.register_symbol_index_json(
                workspace_root,
                db_path,
                max_files,
                max_file_bytes,
                timeout_ms,
            )
        elif max_file_bytes is not None:
            payload = core.register_symbol_index_json(
                workspace_root,
                db_path,
                max_files,
                max_file_bytes,
            )
        elif max_files != 20000:
            payload = core.register_symbol_index_json(workspace_root, db_path, max_files)
        else:
            payload = core.register_symbol_index_json(workspace_root, db_path)
        return self._decode_core_object(payload)

    def _refresh_symbol_index_for_file(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        db_path = self._require_string(params, "db_path")
        file_path = self._require_string(params, "file_path")
        max_files = self._optional_positive_int(params, "max_files", default=20000)
        max_file_bytes = self._optional_positive_int_or_none(params, "max_file_bytes")
        timeout_ms = self._optional_positive_int_or_none(params, "timeout_ms")
        core = self._require_core()
        if timeout_ms is not None:
            payload = core.refresh_symbol_index_for_file_json(
                workspace_root,
                db_path,
                file_path,
                max_files,
                max_file_bytes,
                timeout_ms,
            )
        elif max_file_bytes is None:
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

    def _refresh_registered_symbol_indexes(self, params: dict[str, Any]) -> list[dict[str, Any]]:
        max_files = self._optional_positive_int(params, "max_files", default=20000)
        max_file_bytes = self._optional_positive_int_or_none(params, "max_file_bytes")
        timeout_ms = self._optional_positive_int_or_none(params, "timeout_ms")
        core = self._require_core()
        if timeout_ms is not None:
            payload = core.refresh_registered_symbol_indexes_json(
                max_files,
                max_file_bytes,
                timeout_ms,
            )
        elif max_file_bytes is None:
            payload = core.refresh_registered_symbol_indexes_json(max_files)
        else:
            payload = core.refresh_registered_symbol_indexes_json(max_files, max_file_bytes)
        return self._decode_core_object_array(payload)
