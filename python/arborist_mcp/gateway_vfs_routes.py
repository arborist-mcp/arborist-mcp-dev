from __future__ import annotations

from typing import Any

from .tool_result_schemas import JsonRpcError


class GatewayVfsRoutes:
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
