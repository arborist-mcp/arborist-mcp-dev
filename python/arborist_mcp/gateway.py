from __future__ import annotations

import argparse
import importlib
import json
import sys
from pathlib import Path
from typing import Any


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
                result = {
                    "serverInfo": {
                        "name": "arborist-mcp",
                        "version": "0.1.0",
                    },
                    "capabilities": {
                        "tools": [
                            "arborist/get_semantic_skeleton",
                            "arborist/patch_ast_node",
                            "arborist/patch_virtual_ast_node",
                            "arborist/register_symbol_index",
                            "arborist/refresh_symbol_index_for_file",
                            "arborist/unregister_symbol_index",
                            "arborist/list_symbol_indexes",
                            "arborist/did_open",
                            "arborist/did_change",
                            "arborist/did_close",
                            "arborist/list_virtual_files",
                            "arborist/read_virtual_file",
                            "arborist/apply_buffer_edit",
                            "arborist/commit_virtual_file",
                            "arborist/discard_virtual_file",
                            "arborist/rebuild_symbol_index",
                            "arborist/trace_symbol_graph",
                            "arborist/replay_patch_evidence_against_trace",
                            "arborist/validate_patch_commit_with_trace",
                            "arborist/validate_patch_with_trace_context",
                            "arborist/execute_tree_query",
                        ]
                    },
                    "supportedLanguages": self._require_core().supported_languages(),
                }
            elif method == "arborist/get_semantic_skeleton":
                result = self._get_semantic_skeleton(params)
            elif method == "arborist/patch_ast_node":
                result = self._patch_ast_node(params)
            elif method == "arborist/patch_virtual_ast_node":
                result = self._patch_virtual_ast_node(params)
            elif method == "arborist/register_symbol_index":
                result = self._register_symbol_index(params)
            elif method == "arborist/refresh_symbol_index_for_file":
                result = self._refresh_symbol_index_for_file(params)
            elif method == "arborist/unregister_symbol_index":
                result = self._unregister_symbol_index(params)
            elif method == "arborist/list_symbol_indexes":
                result = self._list_symbol_indexes()
            elif method == "arborist/did_open":
                result = self._did_open(params)
            elif method == "arborist/did_change":
                result = self._did_change(params)
            elif method == "arborist/did_close":
                result = self._did_close(params)
            elif method == "arborist/list_virtual_files":
                result = self._list_virtual_files(params)
            elif method == "arborist/read_virtual_file":
                result = self._read_virtual_file(params)
            elif method == "arborist/apply_buffer_edit":
                result = self._apply_buffer_edit(params)
            elif method == "arborist/commit_virtual_file":
                result = self._commit_virtual_file(params)
            elif method == "arborist/discard_virtual_file":
                result = self._discard_virtual_file(params)
            elif method == "arborist/rebuild_symbol_index":
                result = self._rebuild_symbol_index(params)
            elif method == "arborist/trace_symbol_graph":
                result = self._trace_symbol_graph(params)
            elif method == "arborist/replay_patch_evidence_against_trace":
                result = self._replay_patch_evidence_against_trace(params)
            elif method == "arborist/validate_patch_commit_with_trace":
                result = self._validate_patch_commit_with_trace(params)
            elif method == "arborist/validate_patch_with_trace_context":
                result = self._validate_patch_with_trace_context(params)
            elif method == "arborist/execute_tree_query":
                result = self._execute_tree_query(params)
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
        return self._decode_core_payload(payload)

    def _execute_tree_query(self, params: dict[str, Any]) -> list[dict[str, Any]]:
        file_path = self._require_string(params, "file_path")
        query = self._require_string(params, "query")
        source = self._optional_string(params, "source", allow_empty=True)
        payload = self._require_core().execute_tree_query_json(file_path, query, source)
        return self._decode_core_payload(payload)

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
        return self._decode_core_payload(payload)

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
        return self._decode_core_payload(payload)

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
        return self._decode_core_payload(payload)

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
        return self._decode_core_payload(payload)

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
        return self._decode_core_payload(payload)

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
        return self._decode_core_payload(payload)

    def _rebuild_symbol_index(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        db_path = self._require_string(params, "db_path")
        payload = self._require_core().rebuild_symbol_index_json(workspace_root, db_path)
        return self._decode_core_payload(payload)

    def _register_symbol_index(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        db_path = self._require_string(params, "db_path")
        payload = self._require_core().register_symbol_index_json(workspace_root, db_path)
        return self._decode_core_payload(payload)

    def _refresh_symbol_index_for_file(self, params: dict[str, Any]) -> dict[str, Any]:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        db_path = self._require_string(params, "db_path")
        file_path = self._require_string(params, "file_path")
        payload = self._require_core().refresh_symbol_index_for_file_json(
            workspace_root,
            db_path,
            file_path,
        )
        return self._decode_core_payload(payload)

    def _unregister_symbol_index(self, params: dict[str, Any]) -> bool:
        workspace_root = self._optional_string(params, "workspace_root", default=".")
        return self._require_core().unregister_symbol_index_json(workspace_root)

    def _list_symbol_indexes(self) -> list[dict[str, Any]]:
        payload = self._require_core().list_symbol_indexes_json()
        return self._decode_core_payload(payload)

    def _did_open(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        source = self._optional_string(params, "source", allow_empty=True)
        payload = self._require_core().open_virtual_file_json(file_path, source)
        return self._decode_core_payload(payload)

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
        return self._decode_core_payload(payload)

    def _did_close(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        persist = self._optional_bool(params, "persist", default=False)
        payload = self._require_core().close_virtual_file_json(file_path, persist)
        return self._decode_core_payload(payload)

    def _list_virtual_files(self, params: dict[str, Any]) -> list[dict[str, Any]]:
        dirty_only = self._optional_bool(params, "dirty_only", default=False)
        payload = self._require_core().list_virtual_files_json(dirty_only)
        return self._decode_core_payload(payload)

    def _read_virtual_file(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        payload = self._require_core().read_virtual_file_json(file_path)
        return self._decode_core_payload(payload)

    def _apply_buffer_edit(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        start_byte = self._require_nonnegative_int(params, "start_byte")
        old_end_byte = self._require_nonnegative_int(params, "old_end_byte")
        new_text = self._require_string(params, "new_text", allow_empty=True)
        payload = self._require_core().apply_buffer_edit_json(
            file_path,
            start_byte,
            old_end_byte,
            new_text,
        )
        return self._decode_core_payload(payload)

    def _commit_virtual_file(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        payload = self._require_core().commit_virtual_file_json(file_path)
        return self._decode_core_payload(payload)

    def _discard_virtual_file(self, params: dict[str, Any]) -> dict[str, Any]:
        file_path = self._require_string(params, "file_path")
        payload = self._require_core().discard_virtual_file_json(file_path)
        return self._decode_core_payload(payload)

    @staticmethod
    def _decode_core_payload(payload: str) -> Any:
        try:
            return json.loads(
                payload,
                parse_constant=_reject_nonstandard_json_constant,
            )
        except (json.JSONDecodeError, ValueError) as exc:
            raise JsonRpcError(-32000, f"invalid JSON from arborist core: {exc}") from exc

    @staticmethod
    def _require_string(
        params: dict[str, Any], key: str, allow_empty: bool = False
    ) -> str:
        value = params.get(key)
        if not isinstance(value, str) or (not allow_empty and not value):
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
        value = params.get(key, default)
        if value is None:
            return None
        if not isinstance(value, str) or (not allow_empty and not value):
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
        if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
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
            ArboristGateway._validate_position(edit.get("start"), f"edits[{index}].start")
            ArboristGateway._validate_position(edit.get("end"), f"edits[{index}].end")
            if not isinstance(edit.get("new_text"), str):
                raise JsonRpcError(-32602, f"invalid string param: edits[{index}].new_text")

    @staticmethod
    def _validate_position(value: Any, key: str) -> None:
        if not isinstance(value, dict):
            raise JsonRpcError(-32602, f"invalid position param: {key}")
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
    def _encode_json_param(value: Any, key: str) -> str:
        try:
            return json.dumps(value, ensure_ascii=False, allow_nan=False)
        except (TypeError, ValueError) as exc:
            raise JsonRpcError(-32602, f"invalid JSON-compatible param: {key}") from exc


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


def parse_request_json(raw_request: str) -> tuple[Any | None, dict[str, Any] | None]:
    try:
        return json.loads(
            raw_request,
            parse_constant=_reject_nonstandard_json_constant,
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
