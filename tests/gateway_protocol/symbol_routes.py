from __future__ import annotations

from tests.gateway_protocol.helpers import (
    GatewayProtocolTestCase,
    make_recording_json_core,
)
from tests.gateway_protocol.semantic_fixtures import GatewaySemanticFixtureMixin

SUITE_NAME = "gateway-symbol-routes"
REQUIRES_EXTENSION = False
COVERED_TOOLS = (
    "arborist/list_symbols",
    "arborist/list_symbols_context",
    "arborist/list_symbols_discovery_context",
    "arborist/list_symbols_neighborhood_context",
    "arborist/patch_ast_node_at_position",
    "arborist/patch_virtual_ast_node_at_position",
    "arborist/read_symbol",
    "arborist/read_symbol_at_position",
    "arborist/read_symbol_context",
    "arborist/read_symbol_context_at_position",
    "arborist/read_symbol_discovery_context",
    "arborist/read_symbol_discovery_context_at_position",
    "arborist/read_symbol_neighborhood_context",
    "arborist/read_symbol_neighborhood_context_at_position",
    "arborist/search_symbols",
    "arborist/search_symbols_context",
    "arborist/search_symbols_discovery_context",
    "arborist/search_symbols_neighborhood_context",
    "arborist/trace_symbol_graph",
    "arborist/trace_symbol_graph_at_position",
    "arborist/trace_symbol_neighborhood",
    "arborist/trace_symbol_neighborhood_at_position",
    "arborist/validate_patch_with_discovery_context",
    "arborist/validate_patch_with_discovery_context_at_position",
    "arborist/validate_patch_with_graph_context",
    "arborist/validate_patch_with_graph_context_at_position",
    "arborist/validate_patch_with_neighborhood_context",
    "arborist/validate_patch_with_neighborhood_context_at_position",
    "arborist/validate_patch_with_trace_context",
    "arborist/validate_patch_with_trace_context_at_position",
)


class GatewaySymbolRouteTests(GatewaySemanticFixtureMixin, GatewayProtocolTestCase):
    def helper_symbol(
        self,
        *,
        file_path: str = "sample.py",
        origin_type: str = "workspace_symbol",
        include_trace_fields: bool = False,
        dependencies: list[str] | None = None,
        references: list[str] | None = None,
    ) -> dict[str, object]:
        return self.make_symbol(
            "helper",
            file_path=file_path,
            origin_type=origin_type,
            byte_range=(0, 10),
            include_trace_fields=include_trace_fields,
            dependencies=dependencies,
            references=references,
        )

    def orchestrate_symbol(
        self,
        *,
        file_path: str = "caller.py",
        origin_type: str = "workspace_symbol",
        include_trace_fields: bool = False,
        dependencies: list[str] | None = None,
        references: list[str] | None = None,
    ) -> dict[str, object]:
        return self.make_symbol(
            "orchestrate",
            file_path=file_path,
            origin_type=origin_type,
            byte_range=(0, 20),
            include_trace_fields=include_trace_fields,
            dependencies=dependencies,
            references=references,
        )

    def entrypoint_symbol(self) -> dict[str, object]:
        return self.make_symbol(
            "entrypoint",
            file_path="entry.py",
            origin_type="trace_caller",
            byte_range=(0, 20),
        )

    def helper_source(self) -> str:
        return "def helper() -> int:\n    return 1\n"

    def orchestrate_source(self) -> str:
        return "def orchestrate() -> int:\n    return helper()\n"

    def orchestrate_updated_source(self) -> str:
        return "def orchestrate(value: int) -> int:\n    return helper(value)\n"

    def make_search_result(self) -> dict[str, object]:
        return {
            "query": "helper",
            "indexed_files": 2,
            "total_matches": 1,
            "truncated": False,
            "matches": [self.helper_symbol()],
            "match_details": [
                {
                    "symbol_id": "helper",
                    "score": 1000,
                    "matched_fields": ["base_name", "semantic_path"],
                }
            ],
        }

    def make_list_result(self) -> dict[str, object]:
        return {
            "indexed_files": 2,
            "total_symbols": 1,
            "truncated": False,
            "symbols": [self.helper_symbol()],
        }

    def helper_read(self, *, file_path: str = "sample.py") -> dict[str, object]:
        return self.make_read(
            self.helper_symbol(file_path=file_path),
            source=self.helper_source(),
        )

    def orchestrate_read(
        self,
        *,
        file_path: str = "caller.py",
        source: str | None = None,
    ) -> dict[str, object]:
        return self.make_read(
            self.orchestrate_symbol(file_path=file_path),
            source=source or self.orchestrate_source(),
            indexed_files=3,
            end_point=(1, 18 if source is None else 24),
        )

    def helper_trace_context(self, *, file_path: str = "sample.py") -> dict[str, object]:
        return self.make_trace(
            self.helper_symbol(
                file_path=file_path,
                origin_type="trace_root",
                include_trace_fields=True,
                references=["orchestrate"],
            ),
            callers=[
                self.orchestrate_symbol(
                    file_path="caller.py" if file_path == "sample.py" else "graph_a.py",
                    origin_type="trace_caller",
                )
            ],
            indexed_files=2,
        )

    def helper_neighborhood_context(
        self,
        *,
        file_path: str = "sample.py",
    ) -> dict[str, object]:
        caller_file = "caller.py" if file_path == "sample.py" else "graph_a.py"
        helper_workspace = self.helper_symbol(file_path=file_path)
        helper_trace = self.helper_symbol(
            file_path=file_path,
            origin_type="trace_root",
            include_trace_fields=True,
            references=["orchestrate"],
        )
        orchestrate_caller = self.orchestrate_symbol(
            file_path=caller_file,
            origin_type="trace_caller",
        )
        return {
            "neighborhood": self.make_neighborhood(
                helper_trace,
                direction="callers",
                nodes=[(helper_workspace, 0), (orchestrate_caller, 1)],
                edges=[{"from_symbol_id": "orchestrate", "to_symbol_id": "helper"}],
                indexed_files=2,
            ),
            "reads": [
                self.helper_read(file_path=file_path),
                self.make_read(
                    orchestrate_caller,
                    source=self.orchestrate_source(),
                    end_point=(1, 18),
                ),
            ],
        }

    def orchestrate_trace_context(self) -> dict[str, object]:
        return self.make_trace(
            self.orchestrate_symbol(
                origin_type="trace_root",
                include_trace_fields=True,
                dependencies=["helper"],
                references=["entrypoint"],
            ),
            callers=[self.entrypoint_symbol()],
            callees=[self.helper_symbol(file_path="helper.py", origin_type="trace_callee")],
            indexed_files=3,
        )

    def orchestrate_neighborhood_context(self) -> dict[str, object]:
        orchestrate_workspace = self.orchestrate_symbol(file_path="caller.py")
        helper_callee = self.helper_symbol(
            file_path="helper.py",
            origin_type="trace_callee",
        )
        return {
            "neighborhood": self.make_neighborhood(
                self.orchestrate_symbol(
                    origin_type="trace_root",
                    include_trace_fields=True,
                    dependencies=["helper"],
                    references=["entrypoint"],
                ),
                direction="both",
                nodes=[(orchestrate_workspace, 0), (helper_callee, 1)],
                edges=[{"from_symbol_id": "orchestrate", "to_symbol_id": "helper"}],
                indexed_files=3,
            ),
            "reads": [
                self.make_read(
                    orchestrate_workspace,
                    source=self.orchestrate_updated_source(),
                    indexed_files=3,
                    end_point=(1, 24),
                ),
                self.make_read(
                    helper_callee,
                    source=self.helper_source(),
                    indexed_files=3,
                ),
            ],
        }

    def make_graph_context_payload(self) -> dict[str, object]:
        payload = {
            "patch": self.make_patch_result(),
            "trace_target": "orchestrate",
            "trace": self.orchestrate_trace_context(),
            "neighborhood": self.orchestrate_neighborhood_context()["neighborhood"],
            "trace_validation": self.make_trace_validation(),
            "trace_error": None,
        }
        return payload

    def make_neighborhood_context_payload(self) -> dict[str, object]:
        payload = self.make_graph_context_payload()
        payload["neighborhood_context"] = self.orchestrate_neighborhood_context()
        payload.pop("neighborhood")
        return payload

    def make_discovery_context_payload(self) -> dict[str, object]:
        payload = self.make_neighborhood_context_payload()
        payload["read"] = self.make_read(
            self.orchestrate_symbol(file_path="caller.py"),
            source=self.orchestrate_updated_source(),
            indexed_files=3,
            end_point=(1, 24),
        )
        return payload

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

    def test_trace_context_returns_trace_error_when_patch_gate_rejects(self) -> None:
        with self.temp_workspace(
            {
                "caller.py": "def orchestrate(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            caller = workspace.joinpath("caller.py")
            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/validate_patch_with_trace_context",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller),
                        "semantic_path": "orchestrate",
                        "new_code": (
                            "def orchestrate(value: int) -> int:\n"
                            "    return missing_helper(value)\n"
                        ),
                        "direction": "both",
                    },
                    request_id=41,
                ),
                request_id=41,
            )

        assert isinstance(result, dict)
        self.assertFalse(result["patch"]["applied"])
        self.assertEqual(result["trace_target"], result["patch"]["resolved_symbol_id"])
        self.assertIsNone(result["trace"])
        self.assertIsNone(result["trace_validation"])
        self.assertEqual(
            result["trace_error"],
            "trace skipped because patch validation rejected the patch",
        )

    def test_search_routes_params_to_core(self) -> None:
        helper_read = self.helper_read()
        helper_context = self.helper_neighborhood_context()
        source = "def helper(value: int) -> int:\n    return value + 2\n"
        cases = [
            {
                "core_method": "search_symbols_json",
                "rpc_method": "arborist/search_symbols",
                "request_id": 57,
                "params": {
                    "workspace_root": ".",
                    "query": "helper",
                    "limit": 5,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
                "payload": self.make_search_result(),
                "expected_call": (
                    ".",
                    "helper",
                    5,
                    "symbols.db",
                    "graph",
                    "function_definition",
                ),
                "check": lambda result: (
                    self.assertEqual(result["query"], "helper"),
                    self.assertEqual(result["total_matches"], 1),
                    self.assertFalse(result["truncated"]),
                    self.assertEqual(result["matches"][0]["semantic_path"], "helper"),
                    self.assertEqual(result["match_details"][0]["score"], 1000),
                ),
            },
            {
                "core_method": "search_symbols_json",
                "rpc_method": "arborist/search_symbols",
                "request_id": 174,
                "params": {
                    "workspace_root": ".",
                    "query": "helper",
                    "limit": 5,
                    "file_path": "graph_b.py",
                    "source": source,
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
                "payload": self.make_search_result(),
                "expected_call": (
                    ".",
                    "helper",
                    5,
                    None,
                    "graph",
                    "function_definition",
                    "graph_b.py",
                    source,
                ),
                "check": lambda result: (
                    self.assertEqual(result["query"], "helper"),
                    self.assertEqual(result["matches"][0]["semantic_path"], "helper"),
                ),
            },
            {
                "core_method": "search_symbols_context_json",
                "rpc_method": "arborist/search_symbols_context",
                "request_id": 77,
                "params": {
                    "workspace_root": ".",
                    "query": "helper",
                    "limit": 5,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
                "payload": {
                    "search": self.make_search_result(),
                    "reads": [helper_read],
                },
                "expected_call": (
                    ".",
                    "helper",
                    5,
                    "symbols.db",
                    "graph",
                    "function_definition",
                ),
                "check": lambda result: (
                    self.assertEqual(result["search"]["query"], "helper"),
                    self.assertEqual(result["search"]["total_matches"], 1),
                    self.assertEqual(
                        result["reads"][0]["symbol"]["semantic_path"], "helper"
                    ),
                    self.assertIn("def helper()", result["reads"][0]["source"]),
                ),
            },
            {
                "core_method": "search_symbols_neighborhood_context_json",
                "rpc_method": "arborist/search_symbols_neighborhood_context",
                "request_id": 78,
                "params": {
                    "workspace_root": ".",
                    "query": "helper",
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
                "payload": {
                    "search": self.make_search_result(),
                    "contexts": [helper_context],
                },
                "expected_call": (
                    ".",
                    "helper",
                    5,
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    "graph",
                    "function_definition",
                ),
                "check": lambda result: (
                    self.assertEqual(result["search"]["query"], "helper"),
                    self.assertEqual(
                        result["contexts"][0]["neighborhood"]["symbol"]["semantic_path"],
                        "helper",
                    ),
                    self.assertIn(
                        "def helper()",
                        result["contexts"][0]["reads"][0]["source"],
                    ),
                ),
            },
            {
                "core_method": "search_symbols_discovery_context_json",
                "rpc_method": "arborist/search_symbols_discovery_context",
                "request_id": 86,
                "params": {
                    "workspace_root": ".",
                    "query": "helper",
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
                "payload": {
                    "search": self.make_search_result(),
                    "reads": [helper_read],
                    "contexts": [helper_context],
                },
                "expected_call": (
                    ".",
                    "helper",
                    5,
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    "graph",
                    "function_definition",
                ),
                "check": lambda result: (
                    self.assertEqual(result["search"]["query"], "helper"),
                    self.assertEqual(
                        result["reads"][0]["symbol"]["semantic_path"], "helper"
                    ),
                    self.assertEqual(
                        result["contexts"][0]["neighborhood"]["symbol"]["semantic_path"],
                        "helper",
                    ),
                ),
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

    def test_list_routes_params_to_core(self) -> None:
        helper_read = self.helper_read()
        helper_context = self.helper_neighborhood_context()
        source = "def helper(value: int) -> int:\n    return value + 2\n"
        cases = [
            {
                "core_method": "list_symbols_json",
                "rpc_method": "arborist/list_symbols",
                "request_id": 60,
                "params": {
                    "workspace_root": ".",
                    "limit": 25,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
                "payload": self.make_list_result(),
                "expected_call": (
                    ".",
                    25,
                    "symbols.db",
                    "graph",
                    "function_definition",
                ),
                "check": lambda result: (
                    self.assertEqual(result["total_symbols"], 1),
                    self.assertFalse(result["truncated"]),
                    self.assertEqual(result["symbols"][0]["semantic_path"], "helper"),
                ),
            },
            {
                "core_method": "list_symbols_json",
                "rpc_method": "arborist/list_symbols",
                "request_id": 175,
                "params": {
                    "workspace_root": ".",
                    "limit": 25,
                    "file_path": "graph_b.py",
                    "source": source,
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
                "payload": self.make_list_result(),
                "expected_call": (
                    ".",
                    25,
                    None,
                    "graph",
                    "function_definition",
                    "graph_b.py",
                    source,
                ),
                "check": lambda result: (
                    self.assertEqual(result["total_symbols"], 1),
                    self.assertEqual(result["symbols"][0]["semantic_path"], "helper"),
                ),
            },
            {
                "core_method": "list_symbols_context_json",
                "rpc_method": "arborist/list_symbols_context",
                "request_id": 61,
                "params": {
                    "workspace_root": ".",
                    "limit": 25,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
                "payload": {
                    "list": self.make_list_result(),
                    "reads": [helper_read],
                },
                "expected_call": (
                    ".",
                    25,
                    "symbols.db",
                    "graph",
                    "function_definition",
                ),
                "check": lambda result: (
                    self.assertEqual(result["list"]["total_symbols"], 1),
                    self.assertEqual(
                        result["list"]["symbols"][0]["semantic_path"], "helper"
                    ),
                    self.assertIn("def helper()", result["reads"][0]["source"]),
                ),
            },
            {
                "core_method": "list_symbols_neighborhood_context_json",
                "rpc_method": "arborist/list_symbols_neighborhood_context",
                "request_id": 81,
                "params": {
                    "workspace_root": ".",
                    "limit": 25,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
                "payload": {
                    "list": self.make_list_result(),
                    "contexts": [helper_context],
                },
                "expected_call": (
                    ".",
                    25,
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    "graph",
                    "function_definition",
                ),
                "check": lambda result: (
                    self.assertEqual(result["list"]["total_symbols"], 1),
                    self.assertEqual(
                        result["contexts"][0]["neighborhood"]["symbol"]["semantic_path"],
                        "helper",
                    ),
                    self.assertIn(
                        "def helper()",
                        result["contexts"][0]["reads"][0]["source"],
                    ),
                ),
            },
            {
                "core_method": "list_symbols_discovery_context_json",
                "rpc_method": "arborist/list_symbols_discovery_context",
                "request_id": 87,
                "params": {
                    "workspace_root": ".",
                    "limit": 25,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                    "file_path_contains": "graph",
                    "node_kind": "function_definition",
                },
                "payload": {
                    "list": self.make_list_result(),
                    "reads": [helper_read],
                    "contexts": [helper_context],
                },
                "expected_call": (
                    ".",
                    25,
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    "graph",
                    "function_definition",
                ),
                "check": lambda result: (
                    self.assertEqual(result["list"]["total_symbols"], 1),
                    self.assertEqual(
                        result["reads"][0]["symbol"]["semantic_path"], "helper"
                    ),
                    self.assertEqual(
                        result["contexts"][0]["neighborhood"]["symbol"]["semantic_path"],
                        "helper",
                    ),
                ),
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

    def test_read_routes_params_to_core(self) -> None:
        helper_read = self.helper_read()
        helper_trace = self.helper_trace_context()
        helper_context = self.helper_neighborhood_context()
        helper_read_graph = self.helper_read(file_path="graph_b.py")
        helper_trace_graph = self.helper_trace_context(file_path="graph_b.py")
        helper_context_graph = self.helper_neighborhood_context(file_path="graph_b.py")
        source = "def helper(value: int) -> int:\n    return value + 2\n"
        cases = [
            {
                "core_method": "read_symbol_json",
                "rpc_method": "arborist/read_symbol",
                "request_id": 176,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "file_path": "graph_b.py",
                    "source": source,
                },
                "payload": helper_read,
                "expected_call": (".", "helper", None, "graph_b.py", source),
                "check": lambda result: (
                    self.assertEqual(result["symbol"]["semantic_path"], "helper"),
                    self.assertIn("def helper()", result["source"]),
                ),
            },
            {
                "core_method": "read_symbol_json",
                "rpc_method": "arborist/read_symbol",
                "request_id": 61,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "index_db_path": "symbols.db",
                },
                "payload": helper_read,
                "expected_call": (".", "helper", "symbols.db"),
                "check": lambda result: (
                    self.assertEqual(result["symbol"]["semantic_path"], "helper"),
                    self.assertIn("def helper()", result["source"]),
                ),
            },
            {
                "core_method": "read_symbol_at_position_json",
                "rpc_method": "arborist/read_symbol_at_position",
                "request_id": 62,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "index_db_path": "symbols.db",
                },
                "payload": helper_read_graph,
                "expected_call": (".", "graph_b.py", 0, 5, None, "symbols.db"),
                "check": lambda result: (
                    self.assertEqual(result["symbol"]["semantic_path"], "helper"),
                    self.assertIn("def helper()", result["source"]),
                ),
            },
            {
                "core_method": "trace_symbol_graph_json",
                "rpc_method": "arborist/trace_symbol_graph",
                "request_id": 177,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "file_path": "graph_b.py",
                    "source": source,
                },
                "payload": helper_trace,
                "expected_call": (
                    ".",
                    "helper",
                    "callers",
                    None,
                    "graph_b.py",
                    source,
                    None,
                ),
                "check": lambda result: (
                    self.assertEqual(result["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(
                        result["callers"][0]["semantic_path"], "orchestrate"
                    ),
                ),
            },
            {
                "core_method": "trace_symbol_graph_json",
                "rpc_method": "arborist/trace_symbol_graph",
                "request_id": 60,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "index_db_path": "symbols.db",
                },
                "payload": helper_trace,
                "expected_call": (".", "helper", "callers", "symbols.db", None, None, None),
                "check": lambda result: (
                    self.assertEqual(result["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(
                        result["callers"][0]["semantic_path"], "orchestrate"
                    ),
                ),
            },
            {
                "core_method": "trace_symbol_graph_at_position_json",
                "rpc_method": "arborist/trace_symbol_graph_at_position",
                "request_id": 65,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "index_db_path": "symbols.db",
                },
                "payload": helper_trace_graph,
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    None,
                    "symbols.db",
                    None,
                ),
                "check": lambda result: (
                    self.assertEqual(result["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(
                        result["callers"][0]["semantic_path"], "orchestrate"
                    ),
                ),
            },
            {
                "core_method": "trace_symbol_neighborhood_json",
                "rpc_method": "arborist/trace_symbol_neighborhood",
                "request_id": 66,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                "payload": helper_context["neighborhood"],
                "expected_call": (
                    ".",
                    "helper",
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    None,
                    None,
                    None,
                ),
                "check": lambda result: (
                    self.assertEqual(result["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(result["direction"], "callers"),
                    self.assertEqual(
                        result["nodes"][1]["symbol"]["semantic_path"], "orchestrate"
                    ),
                    self.assertEqual(result["edges"][0]["to_symbol_id"], "helper"),
                ),
            },
            {
                "core_method": "trace_symbol_neighborhood_at_position_json",
                "rpc_method": "arborist/trace_symbol_neighborhood_at_position",
                "request_id": 67,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                "payload": helper_context_graph["neighborhood"],
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    2,
                    10,
                    None,
                    "symbols.db",
                    None,
                ),
                "check": lambda result: (
                    self.assertEqual(result["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(result["direction"], "callers"),
                    self.assertEqual(
                        result["nodes"][1]["symbol"]["semantic_path"], "orchestrate"
                    ),
                    self.assertEqual(result["edges"][0]["to_symbol_id"], "helper"),
                ),
            },
            {
                "core_method": "read_symbol_context_json",
                "rpc_method": "arborist/read_symbol_context",
                "request_id": 178,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "file_path": "graph_b.py",
                    "source": source,
                },
                "payload": {"read": helper_read, "trace": helper_trace},
                "expected_call": (
                    ".",
                    "helper",
                    "callers",
                    None,
                    "graph_b.py",
                    source,
                ),
                "check": lambda result: (
                    self.assertEqual(result["read"]["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(
                        result["trace"]["callers"][0]["semantic_path"], "orchestrate"
                    ),
                ),
            },
            {
                "core_method": "read_symbol_context_json",
                "rpc_method": "arborist/read_symbol_context",
                "request_id": 63,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "index_db_path": "symbols.db",
                },
                "payload": {"read": helper_read, "trace": helper_trace},
                "expected_call": (".", "helper", "callers", "symbols.db"),
                "check": lambda result: (
                    self.assertEqual(result["read"]["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(
                        result["trace"]["callers"][0]["semantic_path"], "orchestrate"
                    ),
                ),
            },
            {
                "core_method": "read_symbol_context_at_position_json",
                "rpc_method": "arborist/read_symbol_context_at_position",
                "request_id": 64,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "index_db_path": "symbols.db",
                },
                "payload": {"read": helper_read_graph, "trace": helper_trace_graph},
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    None,
                    "symbols.db",
                ),
                "check": lambda result: (
                    self.assertEqual(result["read"]["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(
                        result["trace"]["callers"][0]["semantic_path"], "orchestrate"
                    ),
                ),
            },
            {
                "core_method": "read_symbol_neighborhood_context_json",
                "rpc_method": "arborist/read_symbol_neighborhood_context",
                "request_id": 179,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "file_path": "graph_b.py",
                    "source": source,
                },
                "payload": helper_context,
                "expected_call": (
                    ".",
                    "helper",
                    "callers",
                    2,
                    10,
                    None,
                    "graph_b.py",
                    source,
                ),
                "check": lambda result: (
                    self.assertEqual(
                        result["neighborhood"]["symbol"]["semantic_path"], "helper"
                    ),
                    self.assertEqual(
                        result["reads"][1]["symbol"]["semantic_path"], "orchestrate"
                    ),
                ),
            },
            {
                "core_method": "read_symbol_neighborhood_context_json",
                "rpc_method": "arborist/read_symbol_neighborhood_context",
                "request_id": 72,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                "payload": helper_context,
                "expected_call": (".", "helper", "callers", 2, 10, "symbols.db"),
                "check": lambda result: (
                    self.assertEqual(
                        result["neighborhood"]["symbol"]["semantic_path"], "helper"
                    ),
                    self.assertEqual(
                        result["reads"][1]["symbol"]["semantic_path"], "orchestrate"
                    ),
                    self.assertIn("def helper()", result["reads"][0]["source"]),
                ),
            },
            {
                "core_method": "read_symbol_neighborhood_context_at_position_json",
                "rpc_method": "arborist/read_symbol_neighborhood_context_at_position",
                "request_id": 73,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                "payload": helper_context_graph,
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    2,
                    10,
                    None,
                    "symbols.db",
                ),
                "check": lambda result: (
                    self.assertEqual(
                        result["neighborhood"]["symbol"]["semantic_path"], "helper"
                    ),
                    self.assertEqual(
                        result["reads"][1]["symbol"]["semantic_path"], "orchestrate"
                    ),
                ),
            },
            {
                "core_method": "read_symbol_discovery_context_json",
                "rpc_method": "arborist/read_symbol_discovery_context",
                "request_id": 180,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "file_path": "graph_b.py",
                    "source": source,
                },
                "payload": {
                    "read": helper_read,
                    "trace": helper_trace,
                    "neighborhood_context": helper_context,
                },
                "expected_call": (
                    ".",
                    "helper",
                    "callers",
                    2,
                    10,
                    None,
                    "graph_b.py",
                    source,
                ),
                "check": lambda result: (
                    self.assertEqual(result["read"]["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(result["trace"]["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(
                        result["neighborhood_context"]["reads"][1]["symbol"][
                            "semantic_path"
                        ],
                        "orchestrate",
                    ),
                ),
            },
            {
                "core_method": "read_symbol_discovery_context_json",
                "rpc_method": "arborist/read_symbol_discovery_context",
                "request_id": 74,
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                "payload": {
                    "read": helper_read,
                    "trace": helper_trace,
                    "neighborhood_context": helper_context,
                },
                "expected_call": (".", "helper", "callers", 2, 10, "symbols.db"),
                "check": lambda result: (
                    self.assertEqual(result["read"]["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(result["trace"]["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(
                        result["neighborhood_context"]["reads"][1]["symbol"][
                            "semantic_path"
                        ],
                        "orchestrate",
                    ),
                ),
            },
            {
                "core_method": "read_symbol_discovery_context_at_position_json",
                "rpc_method": "arborist/read_symbol_discovery_context_at_position",
                "request_id": 75,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                "payload": {
                    "read": helper_read_graph,
                    "trace": helper_trace_graph,
                    "neighborhood_context": helper_context_graph,
                },
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    2,
                    10,
                    None,
                    "symbols.db",
                ),
                "check": lambda result: (
                    self.assertEqual(result["read"]["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(result["trace"]["symbol"]["semantic_path"], "helper"),
                    self.assertEqual(
                        result["neighborhood_context"]["reads"][1]["symbol"][
                            "semantic_path"
                        ],
                        "orchestrate",
                    ),
                ),
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

    def test_position_routes_source_params_to_core(self) -> None:
        helper_read_graph = self.helper_read(file_path="graph_b.py")
        helper_trace_graph = self.helper_trace_context(file_path="graph_b.py")
        helper_context_graph = self.helper_neighborhood_context(file_path="graph_b.py")
        source = "def helper(value: int) -> int:\n    return value + 2\n"
        cases = [
            {
                "core_method": "read_symbol_at_position_json",
                "rpc_method": "arborist/read_symbol_at_position",
                "request_id": 168,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "source": source,
                },
                "payload": helper_read_graph,
                "expected_call": (".", "graph_b.py", 0, 5, source, None),
            },
            {
                "core_method": "trace_symbol_graph_at_position_json",
                "rpc_method": "arborist/trace_symbol_graph_at_position",
                "request_id": 169,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "source": source,
                },
                "payload": helper_trace_graph,
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    source,
                    None,
                    None,
                ),
            },
            {
                "core_method": "trace_symbol_neighborhood_at_position_json",
                "rpc_method": "arborist/trace_symbol_neighborhood_at_position",
                "request_id": 170,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "source": source,
                },
                "payload": helper_context_graph["neighborhood"],
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    2,
                    10,
                    source,
                    None,
                    None,
                ),
            },
            {
                "core_method": "read_symbol_context_at_position_json",
                "rpc_method": "arborist/read_symbol_context_at_position",
                "request_id": 171,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "source": source,
                },
                "payload": {"read": helper_read_graph, "trace": helper_trace_graph},
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    source,
                    None,
                ),
            },
            {
                "core_method": "read_symbol_neighborhood_context_at_position_json",
                "rpc_method": "arborist/read_symbol_neighborhood_context_at_position",
                "request_id": 172,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "source": source,
                },
                "payload": helper_context_graph,
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    2,
                    10,
                    source,
                    None,
                ),
            },
            {
                "core_method": "read_symbol_discovery_context_at_position_json",
                "rpc_method": "arborist/read_symbol_discovery_context_at_position",
                "request_id": 173,
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "source": source,
                },
                "payload": {
                    "read": helper_read_graph,
                    "trace": helper_trace_graph,
                    "neighborhood_context": helper_context_graph,
                },
                "expected_call": (
                    ".",
                    "graph_b.py",
                    0,
                    5,
                    "callers",
                    2,
                    10,
                    source,
                    None,
                ),
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
                    check_result=lambda result: self.assertTrue(result),
                )

    def test_patch_at_position_routes_params_to_core(self) -> None:
        cases = [
            {
                "core_method": "patch_ast_node_at_position_json",
                "rpc_method": "arborist/patch_ast_node_at_position",
                "request_id": 96,
                "params": {
                    "file_path": "sample.py",
                    "position": {"row": 3, "column": 1},
                    "new_code": "def helper() -> int:\n    return 2\n",
                    "source": "def helper() -> int:\n    return 1\n",
                    "bypass_reason": "known-safe",
                },
                "expected_call": (
                    "sample.py",
                    3,
                    1,
                    "def helper() -> int:\n    return 2\n",
                    "def helper() -> int:\n    return 1\n",
                    "known-safe",
                ),
            },
            {
                "core_method": "patch_virtual_ast_node_at_position_json",
                "rpc_method": "arborist/patch_virtual_ast_node_at_position",
                "request_id": 97,
                "params": {
                    "file_path": "sample.py",
                    "position": {"row": 3, "column": 1},
                    "new_code": "def helper() -> int:\n    return 2\n",
                    "bypass_reason": "known-safe",
                },
                "expected_call": (
                    "sample.py",
                    3,
                    1,
                    "def helper() -> int:\n    return 2\n",
                    "known-safe",
                ),
            },
        ]

        for case in cases:
            with self.subTest(method=case["rpc_method"]):
                self.assert_routed_json(
                    core_method=case["core_method"],
                    rpc_method=case["rpc_method"],
                    params=case["params"],
                    payload={},
                    request_id=case["request_id"],
                    expected_call=case["expected_call"],
                    check_result=lambda result: self.assertEqual(result, {}),
                )

    def test_context_validation_routes_params_to_core(self) -> None:
        updated_source = self.orchestrate_updated_source()
        cases = [
            {
                "core_method": "validate_patch_with_graph_context_json",
                "rpc_method": "arborist/validate_patch_with_graph_context",
                "request_id": 70,
                "payload": self.make_graph_context_payload(),
                "check": lambda result: (
                    self.assertTrue(result["patch"]["applied"]),
                    self.assertEqual(
                        result["trace"]["symbol"]["semantic_path"], "orchestrate"
                    ),
                    self.assertEqual(
                        result["neighborhood"]["nodes"][1]["symbol"]["semantic_path"],
                        "helper",
                    ),
                    self.assertTrue(result["trace_validation"]["allowed"]),
                ),
            },
            {
                "core_method": "validate_patch_with_neighborhood_context_json",
                "rpc_method": "arborist/validate_patch_with_neighborhood_context",
                "request_id": 75,
                "payload": self.make_neighborhood_context_payload(),
                "check": lambda result: (
                    self.assertTrue(result["patch"]["applied"]),
                    self.assertEqual(
                        result["trace"]["symbol"]["semantic_path"], "orchestrate"
                    ),
                    self.assertEqual(
                        result["neighborhood_context"]["neighborhood"]["nodes"][1][
                            "symbol"
                        ]["semantic_path"],
                        "helper",
                    ),
                    self.assertEqual(
                        result["neighborhood_context"]["reads"][1]["symbol"][
                            "semantic_path"
                        ],
                        "helper",
                    ),
                    self.assertTrue(result["trace_validation"]["allowed"]),
                ),
            },
            {
                "core_method": "validate_patch_with_discovery_context_json",
                "rpc_method": "arborist/validate_patch_with_discovery_context",
                "request_id": 79,
                "payload": self.make_discovery_context_payload(),
                "check": lambda result: (
                    self.assertTrue(result["patch"]["applied"]),
                    self.assertEqual(
                        result["trace"]["symbol"]["semantic_path"], "orchestrate"
                    ),
                    self.assertEqual(
                        result["read"]["symbol"]["semantic_path"], "orchestrate"
                    ),
                    self.assertEqual(
                        result["neighborhood_context"]["reads"][1]["symbol"][
                            "semantic_path"
                        ],
                        "helper",
                    ),
                    self.assertTrue(result["trace_validation"]["allowed"]),
                ),
            },
        ]

        for case in cases:
            with self.subTest(method=case["rpc_method"]):
                self.assert_routed_json(
                    core_method=case["core_method"],
                    rpc_method=case["rpc_method"],
                    params={
                        "workspace_root": ".",
                        "file_path": "caller.py",
                        "semantic_path": "orchestrate",
                        "new_code": updated_source,
                        "direction": "both",
                        "max_depth": 2,
                        "max_nodes": 10,
                    },
                    payload=case["payload"],
                    request_id=case["request_id"],
                    expected_call=(
                        ".",
                        "caller.py",
                        "orchestrate",
                        updated_source,
                        None,
                        None,
                        "both",
                        2,
                        10,
                        None,
                    ),
                    check_result=case["check"],
                )

    def test_context_validation_at_position_routes_params_to_core(self) -> None:
        updated_source = self.orchestrate_updated_source()
        cases = [
            {
                "core_method": "validate_patch_with_trace_context_at_position_json",
                "rpc_method": "arborist/validate_patch_with_trace_context_at_position",
                "request_id": 98,
                "expected_call": (
                    ".",
                    "caller.py",
                    3,
                    1,
                    updated_source,
                    "def orchestrate(value: int) -> int:\n    return value + 1\n",
                    "known-safe",
                    "callers",
                    None,
                ),
            },
            {
                "core_method": "validate_patch_with_graph_context_at_position_json",
                "rpc_method": "arborist/validate_patch_with_graph_context_at_position",
                "request_id": 99,
                "expected_call": (
                    ".",
                    "caller.py",
                    3,
                    1,
                    updated_source,
                    "def orchestrate(value: int) -> int:\n    return value + 1\n",
                    "known-safe",
                    "callers",
                    2,
                    10,
                    None,
                ),
            },
            {
                "core_method": "validate_patch_with_neighborhood_context_at_position_json",
                "rpc_method": "arborist/validate_patch_with_neighborhood_context_at_position",
                "request_id": 100,
                "expected_call": (
                    ".",
                    "caller.py",
                    3,
                    1,
                    updated_source,
                    "def orchestrate(value: int) -> int:\n    return value + 1\n",
                    "known-safe",
                    "callers",
                    2,
                    10,
                    None,
                ),
            },
            {
                "core_method": "validate_patch_with_discovery_context_at_position_json",
                "rpc_method": "arborist/validate_patch_with_discovery_context_at_position",
                "request_id": 101,
                "expected_call": (
                    ".",
                    "caller.py",
                    3,
                    1,
                    updated_source,
                    "def orchestrate(value: int) -> int:\n    return value + 1\n",
                    "known-safe",
                    "callers",
                    2,
                    10,
                    None,
                ),
            },
        ]

        for case in cases:
            with self.subTest(method=case["rpc_method"]):
                params = {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "position": {"row": 3, "column": 1},
                    "new_code": updated_source,
                    "source": "def orchestrate(value: int) -> int:\n    return value + 1\n",
                    "bypass_reason": "known-safe",
                    "direction": "callers",
                }
                if case["core_method"] != "validate_patch_with_trace_context_at_position_json":
                    params["max_depth"] = 2
                    params["max_nodes"] = 10
                self.assert_routed_json(
                    core_method=case["core_method"],
                    rpc_method=case["rpc_method"],
                    params=params,
                    payload={},
                    request_id=case["request_id"],
                    expected_call=case["expected_call"],
                    check_result=lambda result: self.assertEqual(result, {}),
                )

    def test_trace_context_returns_trace_error_when_patch_has_syntax_errors(self) -> None:
        with self.temp_workspace(
            {
                "caller.py": "def orchestrate(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            caller = workspace.joinpath("caller.py")
            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/validate_patch_with_trace_context",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller),
                        "semantic_path": "orchestrate",
                        "new_code": (
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(\n"
                        ),
                        "direction": "both",
                    },
                    request_id=42,
                ),
                request_id=42,
            )

        assert isinstance(result, dict)
        self.assertFalse(result["patch"]["applied"])
        self.assertEqual(result["trace_target"], result["patch"]["resolved_symbol_id"])
        self.assertTrue(result["patch"]["validation"]["syntax_errors"])
        self.assertIsNone(result["trace"])
        self.assertIsNone(result["trace_validation"])
        self.assertEqual(
            result["trace_error"],
            "trace skipped because patch validation reported syntax errors",
        )

    def test_trace_context_accepts_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/validate_patch_with_trace_context",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return value + 1\n"
                        ),
                        "semantic_path": "orchestrate",
                        "new_code": (
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "direction": "both",
                    },
                    request_id=43,
                ),
                request_id=43,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertTrue(result["patch"]["applied"])
            self.assertEqual(result["patch"]["file"], expected_file)
            self.assertEqual(result["trace_target"], result["patch"]["resolved_symbol_id"])
            self.assertIsNone(result["trace_error"])
            self.assertTrue(result["trace_validation"]["allowed"])
            self.assertEqual(result["trace_validation"]["replay_status"], "matched")
            self.assertEqual(result["trace"]["symbol"]["semantic_path"], "orchestrate")
            self.assertEqual(result["trace"]["symbol"]["file_path"], expected_file)
            self.assertTrue(
                any(symbol["semantic_path"] == "helper" for symbol in result["trace"]["callees"])
            )

    def test_graph_context_accepts_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
                "entry.py": (
                    "from caller import orchestrate\n\n\n"
                    "def entrypoint(value: int) -> int:\n"
                    "    return orchestrate(value)\n"
                ),
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/validate_patch_with_graph_context",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return value + 1\n"
                        ),
                        "semantic_path": "orchestrate",
                        "new_code": (
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "direction": "both",
                        "max_depth": 2,
                        "max_nodes": 10,
                    },
                    request_id=71,
                ),
                request_id=71,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertTrue(result["patch"]["applied"])
            self.assertEqual(result["patch"]["file"], expected_file)
            self.assertIsNone(result["trace_error"])
            self.assertTrue(result["trace_validation"]["allowed"])
            self.assertEqual(result["trace"]["symbol"]["semantic_path"], "orchestrate")
            self.assertEqual(result["trace"]["symbol"]["file_path"], expected_file)
            self.assertEqual(result["neighborhood"]["symbol"]["semantic_path"], "orchestrate")
            self.assertTrue(
                any(
                    node["symbol"]["semantic_path"] == "helper"
                    for node in result["neighborhood"]["nodes"]
                )
            )

    def test_neighborhood_context_accepts_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
                "entry.py": (
                    "from caller import orchestrate\n\n\n"
                    "def entrypoint(value: int) -> int:\n"
                    "    return orchestrate(value)\n"
                ),
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/validate_patch_with_neighborhood_context",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return value + 1\n"
                        ),
                        "semantic_path": "orchestrate",
                        "new_code": (
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "direction": "both",
                        "max_depth": 2,
                        "max_nodes": 10,
                    },
                    request_id=76,
                ),
                request_id=76,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertTrue(result["patch"]["applied"])
            self.assertEqual(result["trace"]["symbol"]["semantic_path"], "orchestrate")
            self.assertEqual(
                result["neighborhood_context"]["neighborhood"]["symbol"]["semantic_path"],
                "orchestrate",
            )
            self.assertTrue(
                any(
                    read["symbol"]["semantic_path"] == "helper"
                    for read in result["neighborhood_context"]["reads"]
                )
            )

    def test_discovery_context_accepts_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
                "entry.py": (
                    "from caller import orchestrate\n\n\n"
                    "def entrypoint(value: int) -> int:\n"
                    "    return orchestrate(value)\n"
                ),
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/validate_patch_with_discovery_context",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return value + 1\n"
                        ),
                        "semantic_path": "orchestrate",
                        "new_code": (
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "direction": "both",
                        "max_depth": 2,
                        "max_nodes": 10,
                    },
                    request_id=80,
                ),
                request_id=80,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertTrue(result["patch"]["applied"])
            self.assertEqual(result["trace"]["symbol"]["semantic_path"], "orchestrate")
            self.assertEqual(result["read"]["symbol"]["semantic_path"], "orchestrate")
            self.assertTrue(
                any(
                    read["symbol"]["semantic_path"] == "helper"
                    for read in result["neighborhood_context"]["reads"]
                )
            )

    def test_trace_context_accepts_index_db_path_with_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
                "caller.py": "def orchestrate(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            caller = workspace.joinpath("caller.py")
            db_path = workspace.joinpath("symbols.db")

            rebuild = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/rebuild_symbol_index",
                    {
                        "workspace_root": str(workspace),
                        "db_path": str(db_path),
                    },
                    request_id=180,
                ),
                request_id=180,
            )

            assert isinstance(rebuild, dict)
            self.assertEqual(rebuild["indexed_files"], 2)

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/validate_patch_with_trace_context",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return value + 1\n"
                        ),
                        "semantic_path": "orchestrate",
                        "new_code": (
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "direction": "both",
                        "index_db_path": str(db_path),
                    },
                    request_id=181,
                ),
                request_id=181,
            )

            assert isinstance(result, dict)
            self.assertTrue(result["patch"]["applied"])
            self.assertIsNone(result["trace_error"])
            self.assertTrue(result["trace_validation"]["allowed"])
            self.assertEqual(result["trace"]["symbol"]["semantic_path"], "orchestrate")
            self.assertTrue(
                any(symbol["semantic_path"] == "helper" for symbol in result["trace"]["callees"])
            )

    def test_trace_symbol_graph_accepts_index_db_path_with_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
                "caller.py": "def orchestrate(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            caller = workspace.joinpath("caller.py")
            db_path = workspace.joinpath("symbols.db")

            rebuild = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/rebuild_symbol_index",
                    {
                        "workspace_root": str(workspace),
                        "db_path": str(db_path),
                    },
                    request_id=182,
                ),
                request_id=182,
            )

            assert isinstance(rebuild, dict)
            self.assertEqual(rebuild["indexed_files"], 2)

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/trace_symbol_graph",
                    {
                        "workspace_root": str(workspace),
                        "symbol_path": "orchestrate",
                        "direction": "both",
                        "file_path": str(caller),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "index_db_path": str(db_path),
                    },
                    request_id=183,
                ),
                request_id=183,
            )

            assert isinstance(result, dict)
            self.assertEqual(result["symbol"]["semantic_path"], "orchestrate")
            self.assertTrue(
                any(symbol["semantic_path"] == "helper" for symbol in result["callees"])
            )
            self.assertIn("return value + 1", caller.read_text(encoding="utf-8"))

    def test_search_symbols_accepts_index_db_path_with_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper() -> int:\n    return 1\n",
            }
        ) as workspace:
            helper = workspace.joinpath("helper.py")
            db_path = workspace.joinpath("symbols.db")

            rebuild = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/rebuild_symbol_index",
                    {
                        "workspace_root": str(workspace),
                        "db_path": str(db_path),
                    },
                    request_id=184,
                ),
                request_id=184,
            )

            assert isinstance(rebuild, dict)
            self.assertEqual(rebuild["indexed_files"], 1)

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/search_symbols",
                    {
                        "workspace_root": str(workspace),
                        "query": "helper_alias",
                        "limit": 10,
                        "file_path": str(helper),
                        "source": (
                            "def helper() -> int:\n"
                            "    return 1\n\n\n"
                            "def helper_alias() -> int:\n"
                            "    return helper()\n"
                        ),
                        "index_db_path": str(db_path),
                    },
                    request_id=185,
                ),
                request_id=185,
            )

            assert isinstance(result, dict)
            self.assertEqual(result["total_matches"], 1)
            self.assertEqual(result["matches"][0]["semantic_path"], "helper_alias")
            self.assertNotIn("helper_alias", helper.read_text(encoding="utf-8"))

    def test_read_at_position_accepts_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/read_symbol_at_position",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "position": {"row": 3, "column": 5},
                    },
                    request_id=81,
                ),
                request_id=81,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertEqual(result["symbol"]["semantic_path"], "orchestrate")
            self.assertEqual(result["symbol"]["file_path"], expected_file)
            self.assertEqual(result["source"], "def orchestrate(value: int) -> int:\n    return helper(value)")

    def test_trace_graph_at_position_accepts_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/trace_symbol_graph_at_position",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "position": {"row": 3, "column": 5},
                        "direction": "both",
                    },
                    request_id=82,
                ),
                request_id=82,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertEqual(result["symbol"]["semantic_path"], "orchestrate")
            self.assertEqual(result["symbol"]["file_path"], expected_file)
            self.assertTrue(
                any(symbol["semantic_path"] == "helper" for symbol in result["callees"])
            )

    def test_discovery_context_at_position_accepts_unsaved_source(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
                "entry.py": (
                    "from caller import orchestrate\n\n\n"
                    "def entrypoint(value: int) -> int:\n"
                    "    return orchestrate(value)\n"
                ),
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/read_symbol_discovery_context_at_position",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "position": {"row": 3, "column": 5},
                        "direction": "both",
                        "max_depth": 2,
                        "max_nodes": 10,
                    },
                    request_id=83,
                ),
                request_id=83,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertEqual(result["read"]["symbol"]["semantic_path"], "orchestrate")
            self.assertEqual(result["read"]["symbol"]["file_path"], expected_file)
            self.assertEqual(result["trace"]["symbol"]["file_path"], expected_file)
            self.assertTrue(
                any(
                    read["symbol"]["semantic_path"] == "helper"
                    for read in result["neighborhood_context"]["reads"]
                )
            )

    def test_read_symbol_accepts_unsaved_source_with_file_anchor(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/read_symbol",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "symbol_path": "orchestrate",
                    },
                    request_id=84,
                ),
                request_id=84,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertEqual(result["symbol"]["semantic_path"], "orchestrate")
            self.assertEqual(result["symbol"]["file_path"], expected_file)
            self.assertEqual(
                result["source"],
                "def orchestrate(value: int) -> int:\n    return helper(value)",
            )

    def test_trace_symbol_graph_accepts_unsaved_source_with_file_anchor(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/trace_symbol_graph",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "symbol_path": "orchestrate",
                        "direction": "both",
                    },
                    request_id=85,
                ),
                request_id=85,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertEqual(result["symbol"]["semantic_path"], "orchestrate")
            self.assertEqual(result["symbol"]["file_path"], expected_file)
            self.assertTrue(
                any(symbol["semantic_path"] == "helper" for symbol in result["callees"])
            )

    def test_list_symbols_accepts_unsaved_source_with_file_anchor(self) -> None:
        with self.temp_workspace(
            {
                "helper.py": "def helper(value: int) -> int:\n    return value + 1\n",
            }
        ) as workspace:
            nested = workspace.joinpath("child")
            caller = workspace.joinpath("caller.py")
            nested.mkdir()
            caller_alias = nested.joinpath("..", "caller.py")
            expected_file = str(caller).replace("\\", "/")

            result = self.assert_jsonrpc_ok(
                self.call_gateway(
                    self.make_live_gateway(),
                    "arborist/list_symbols",
                    {
                        "workspace_root": str(workspace),
                        "file_path": str(caller_alias),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "limit": 10,
                        "file_path_contains": "caller",
                    },
                    request_id=86,
                ),
                request_id=86,
            )

            assert isinstance(result, dict)
            self.assertFalse(caller.exists())
            self.assertEqual(result["total_symbols"], 1)
            self.assertEqual(result["symbols"][0]["semantic_path"], "orchestrate")
            self.assertEqual(result["symbols"][0]["file_path"], expected_file)


_LIVE_CORE_TEST_NAMES = (
    "test_trace_context_returns_trace_error_when_patch_gate_rejects",
    "test_trace_context_returns_trace_error_when_patch_has_syntax_errors",
    "test_trace_context_accepts_unsaved_source",
    "test_graph_context_accepts_unsaved_source",
    "test_neighborhood_context_accepts_unsaved_source",
    "test_discovery_context_accepts_unsaved_source",
    "test_trace_context_accepts_index_db_path_with_unsaved_source",
    "test_trace_symbol_graph_accepts_index_db_path_with_unsaved_source",
    "test_search_symbols_accepts_index_db_path_with_unsaved_source",
    "test_read_at_position_accepts_unsaved_source",
    "test_trace_graph_at_position_accepts_unsaved_source",
    "test_discovery_context_at_position_accepts_unsaved_source",
    "test_read_symbol_accepts_unsaved_source_with_file_anchor",
    "test_trace_symbol_graph_accepts_unsaved_source_with_file_anchor",
    "test_list_symbols_accepts_unsaved_source_with_file_anchor",
)


def _extract_live_core_tests() -> dict[str, object]:
    extracted: dict[str, object] = {}
    for test_name in _LIVE_CORE_TEST_NAMES:
        extracted[test_name] = getattr(GatewaySymbolRouteTests, test_name)
        delattr(GatewaySymbolRouteTests, test_name)
    return extracted


LIVE_CORE_TESTS = _extract_live_core_tests()
