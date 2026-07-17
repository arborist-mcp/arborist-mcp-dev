from __future__ import annotations

from tests.gateway_protocol.helpers import GatewayProtocolTestCase, make_recording_json_core

SUITE_NAME = "gateway-management-routes"
REQUIRES_EXTENSION = False
COVERED_TOOLS = (
    "arborist/patch_virtual_ast_node",
    "arborist/register_symbol_index",
    "arborist/refresh_symbol_index_for_file",
    "arborist/unregister_symbol_index",
    "arborist/list_symbol_indexes",
    "arborist/refresh_registered_symbol_indexes",
    "arborist/inspect_symbol_index",
    "arborist/migrate_symbol_index",
    "arborist/export_patch_diagnostics_sarif",
    "arborist/preview_workspace_position_edits",
    "arborist/did_open",
    "arborist/did_close",
    "arborist/list_virtual_files",
    "arborist/read_virtual_file",
    "arborist/commit_virtual_file",
    "arborist/discard_virtual_file",
    "arborist/rebuild_symbol_index",
    "arborist/refresh_symbol_index",
)


class GatewayManagementRouteTests(GatewayProtocolTestCase):
    def assert_routed_json(
        self,
        *,
        core_method: str,
        rpc_method: str,
        params: dict[str, object],
        payload: object,
        request_id: int,
        expected_call: tuple[object, ...],
        check_result,
    ) -> None:
        core = make_recording_json_core(**{core_method: payload})
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(core),
                rpc_method,
                params,
                request_id=request_id,
            ),
            request_id=request_id,
        )
        check_result(result)
        self.assertEqual(core.calls_for(core_method), [expected_call])

    def test_symbol_index_management_routes_params_to_core(self) -> None:
        cases = [
            {
                "core_method": "register_symbol_index_json",
                "rpc_method": "arborist/register_symbol_index",
                "request_id": 104,
                "params": {
                    "workspace_root": ".",
                    "db_path": "symbols.db",
                },
                "payload": {},
                "expected_call": (".", "symbols.db"),
                "check": lambda result: self.assertEqual(result, {}),
            },
            {
                "core_method": "refresh_symbol_index_for_file_json",
                "rpc_method": "arborist/refresh_symbol_index_for_file",
                "request_id": 105,
                "params": {
                    "workspace_root": ".",
                    "db_path": "symbols.db",
                    "file_path": "graph_b.py",
                },
                "payload": {},
                "expected_call": (".", "symbols.db", "graph_b.py", 20000),
                "check": lambda result: self.assertEqual(result, {}),
            },
            {
                "core_method": "refresh_symbol_index_for_file_json",
                "rpc_method": "arborist/refresh_symbol_index_for_file",
                "request_id": 116,
                "params": {
                    "workspace_root": ".",
                    "db_path": "symbols.db",
                    "file_path": "graph_b.py",
                    "max_files": 17,
                },
                "payload": {},
                "expected_call": (".", "symbols.db", "graph_b.py", 17),
                "check": lambda result: self.assertEqual(result, {}),
            },
            {
                "core_method": "refresh_symbol_index_for_file_json",
                "rpc_method": "arborist/refresh_symbol_index_for_file",
                "request_id": 118,
                "params": {
                    "workspace_root": ".",
                    "db_path": "symbols.db",
                    "file_path": "graph_b.py",
                    "max_file_bytes": 4096,
                },
                "payload": {},
                "expected_call": (".", "symbols.db", "graph_b.py", 20000, 4096),
                "check": lambda result: self.assertEqual(result, {}),
            },
            {
                "core_method": "unregister_symbol_index_json",
                "rpc_method": "arborist/unregister_symbol_index",
                "request_id": 106,
                "params": {
                    "workspace_root": ".",
                },
                "payload": True,
                "expected_call": (".",),
                "check": lambda result: self.assertTrue(result),
            },
            {
                "core_method": "list_symbol_indexes_json",
                "rpc_method": "arborist/list_symbol_indexes",
                "request_id": 107,
                "params": {},
                "payload": [],
                "expected_call": (),
                "check": lambda result: self.assertEqual(result, []),
            },
            {
                "core_method": "refresh_registered_symbol_indexes_json",
                "rpc_method": "arborist/refresh_registered_symbol_indexes",
                "request_id": 126,
                "params": {},
                "payload": [],
                "expected_call": (20000,),
                "check": lambda result: self.assertEqual(result, []),
            },
            {
                "core_method": "inspect_symbol_index_json",
                "rpc_method": "arborist/inspect_symbol_index",
                "request_id": 108,
                "params": {
                    "db_path": "symbols.db",
                },
                "payload": {
                    "response_schema_version": "4",
                    "db_path": "symbols.db",
                    "exists": True,
                    "ok": True,
                    "schema_version": "2",
                    "expected_schema_version": "2",
                    "migration": {
                        "required": False,
                        "action": "none",
                        "reason": "index schema and persisted file fingerprints are current",
                    },
                    "workspace_root": ".",
                    "indexed_files": 1,
                    "indexed_symbols": 1,
                    "file_state_entries": 1,
                    "fresh_file_count": 1,
                    "stale_files": [],
                    "missing_files": [],
                    "unreadable_files": [],
                    "unindexed_files": [],
                    "issues": [],
                },
                "expected_call": ("symbols.db",),
                "check": lambda result: self.assertTrue(result["ok"]),
            },
            {
                "core_method": "migrate_symbol_index_json",
                "rpc_method": "arborist/migrate_symbol_index",
                "request_id": 123,
                "params": {
                    "db_path": "symbols.db",
                },
                "payload": {
                    "response_schema_version": "4",
                    "db_path": "symbols.db",
                    "exists": True,
                    "ok": True,
                    "schema_version": "2",
                    "expected_schema_version": "2",
                    "migration": {
                        "required": False,
                        "action": "none",
                        "reason": "index schema and persisted file fingerprints are current",
                    },
                    "workspace_root": ".",
                    "indexed_files": 1,
                    "indexed_symbols": 1,
                    "file_state_entries": 1,
                    "fresh_file_count": 1,
                    "stale_files": [],
                    "missing_files": [],
                    "unreadable_files": [],
                    "unindexed_files": [],
                    "issues": [],
                },
                "expected_call": ("symbols.db",),
                "check": lambda result: self.assertTrue(result["ok"]),
            },
            {
                "core_method": "export_patch_diagnostics_sarif_json",
                "rpc_method": "arborist/export_patch_diagnostics_sarif",
                "request_id": 124,
                "params": {"patch": {}},
                "payload": {"version": "2.1.0", "runs": []},
                "expected_call": ("{}",),
                "check": lambda result: self.assertEqual(result["version"], "2.1.0"),
            },
            {
                "core_method": "preview_workspace_position_edits_json",
                "rpc_method": "arborist/preview_workspace_position_edits",
                "request_id": 125,
                "params": {"files": [{"file_path": "sample.py", "edits": []}]},
                "payload": {"changed": False, "files": [{"file": "sample.py"}]},
                "expected_call": ('[{"file_path": "sample.py", "edits": []}]',),
                "check": lambda result: self.assertFalse(result["changed"]),
            },
            {
                "core_method": "rebuild_symbol_index_json",
                "rpc_method": "arborist/rebuild_symbol_index",
                "request_id": 109,
                "params": {
                    "workspace_root": ".",
                    "db_path": "symbols.db",
                },
                "payload": {},
                "expected_call": (".", "symbols.db", 20000),
                "check": lambda result: self.assertEqual(result, {}),
            },
            {
                "core_method": "rebuild_symbol_index_json",
                "rpc_method": "arborist/rebuild_symbol_index",
                "request_id": 117,
                "params": {
                    "workspace_root": ".",
                    "db_path": "symbols.db",
                    "max_files": 17,
                },
                "payload": {},
                "expected_call": (".", "symbols.db", 17),
                "check": lambda result: self.assertEqual(result, {}),
            },
            {
                "core_method": "rebuild_symbol_index_json",
                "rpc_method": "arborist/rebuild_symbol_index",
                "request_id": 119,
                "params": {
                    "workspace_root": ".",
                    "db_path": "symbols.db",
                    "max_file_bytes": 4096,
                },
                "payload": {},
                "expected_call": (".", "symbols.db", 20000, 4096),
                "check": lambda result: self.assertEqual(result, {}),
            },
            {
                "core_method": "refresh_symbol_index_json",
                "rpc_method": "arborist/refresh_symbol_index",
                "request_id": 120,
                "params": {
                    "workspace_root": ".",
                    "db_path": "symbols.db",
                },
                "payload": {},
                "expected_call": (".", "symbols.db", 20000),
                "check": lambda result: self.assertEqual(result, {}),
            },
            {
                "core_method": "refresh_symbol_index_json",
                "rpc_method": "arborist/refresh_symbol_index",
                "request_id": 121,
                "params": {
                    "workspace_root": ".",
                    "db_path": "symbols.db",
                    "max_files": 17,
                },
                "payload": {},
                "expected_call": (".", "symbols.db", 17),
                "check": lambda result: self.assertEqual(result, {}),
            },
            {
                "core_method": "refresh_symbol_index_json",
                "rpc_method": "arborist/refresh_symbol_index",
                "request_id": 122,
                "params": {
                    "workspace_root": ".",
                    "db_path": "symbols.db",
                    "max_file_bytes": 4096,
                },
                "payload": {},
                "expected_call": (".", "symbols.db", 20000, 4096),
                "check": lambda result: self.assertEqual(result, {}),
            },
        ]

        for case in cases:
            with self.subTest(method=case["rpc_method"]):
                self.assert_routed_json(
                    core_method=case["core_method"],
                    rpc_method=case["rpc_method"],
                    params=case["params"],
                    payload=case["payload"],
                    request_id=case["request_id"],
                    expected_call=case["expected_call"],
                    check_result=case["check"],
                )

    def test_virtual_file_management_routes_params_to_core(self) -> None:
        cases = [
            {
                "core_method": "open_virtual_file_json",
                "rpc_method": "arborist/did_open",
                "request_id": 109,
                "params": {
                    "file_path": "sample.py",
                    "source": "def helper() -> int:\n    return 1\n",
                },
                "payload": {},
                "expected_call": ("sample.py", "def helper() -> int:\n    return 1\n"),
                "check": lambda result: self.assertEqual(result, {}),
            },
            {
                "core_method": "close_virtual_file_json",
                "rpc_method": "arborist/did_close",
                "request_id": 110,
                "params": {
                    "file_path": "sample.py",
                    "persist": True,
                },
                "payload": {},
                "expected_call": ("sample.py", True),
                "check": lambda result: self.assertEqual(result, {}),
            },
            {
                "core_method": "list_virtual_files_json",
                "rpc_method": "arborist/list_virtual_files",
                "request_id": 111,
                "params": {
                    "dirty_only": True,
                },
                "payload": [],
                "expected_call": (True,),
                "check": lambda result: self.assertEqual(result, []),
            },
            {
                "core_method": "read_virtual_file_json",
                "rpc_method": "arborist/read_virtual_file",
                "request_id": 112,
                "params": {
                    "file_path": "sample.py",
                },
                "payload": {
                    "file_path": "sample.py",
                    "current_source": "def helper() -> int:\n    return 1\n",
                    "disk_source": None,
                    "dirty": True,
                },
                "expected_call": ("sample.py",),
                "check": lambda result: (
                    self.assertEqual(result["file_path"], "sample.py"),
                    self.assertTrue(result["dirty"]),
                ),
            },
            {
                "core_method": "patch_virtual_ast_node_json",
                "rpc_method": "arborist/patch_virtual_ast_node",
                "request_id": 113,
                "params": {
                    "file_path": "sample.py",
                    "semantic_path": "helper",
                    "new_code": "def helper() -> int:\n    return 2\n",
                    "bypass_reason": "known-safe",
                },
                "payload": {},
                "expected_call": (
                    "sample.py",
                    "helper",
                    "def helper() -> int:\n    return 2\n",
                    "known-safe",
                ),
                "check": lambda result: self.assertEqual(result, {}),
            },
            {
                "core_method": "commit_virtual_file_json",
                "rpc_method": "arborist/commit_virtual_file",
                "request_id": 114,
                "params": {
                    "file_path": "sample.py",
                },
                "payload": {},
                "expected_call": ("sample.py",),
                "check": lambda result: self.assertEqual(result, {}),
            },
            {
                "core_method": "discard_virtual_file_json",
                "rpc_method": "arborist/discard_virtual_file",
                "request_id": 115,
                "params": {
                    "file_path": "sample.py",
                },
                "payload": {},
                "expected_call": ("sample.py",),
                "check": lambda result: self.assertEqual(result, {}),
            },
        ]

        for case in cases:
            with self.subTest(method=case["rpc_method"]):
                self.assert_routed_json(
                    core_method=case["core_method"],
                    rpc_method=case["rpc_method"],
                    params=case["params"],
                    payload=case["payload"],
                    request_id=case["request_id"],
                    expected_call=case["expected_call"],
                    check_result=case["check"],
                )
