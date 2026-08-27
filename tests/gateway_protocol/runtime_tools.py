from __future__ import annotations

from arborist_mcp import gateway as gateway_module
from arborist_mcp.batch_tools import batch_tools
from arborist_mcp.mcp_tools import tools_call
from arborist_mcp.tool_result_schemas import JsonRpcError

from tests.gateway_protocol.helpers import make_recording_json_core


def _valid_semantic_skeleton_result() -> dict[str, object]:
    return {
        "file": "sample.py",
        "skeleton": "",
        "available_paths": [],
        "available_symbols": [],
    }


def _valid_trace_symbol_graph_result() -> dict[str, object]:
    symbol = {
        "symbol_id": "top_level",
        "semantic_path": "top_level",
        "scope_path": None,
        "file_path": "sample.py",
        "node_kind": "function_definition",
        "origin_type": "local",
        "evidence_key": "sample.py::top_level",
        "byte_range": [0, 1],
        "signature": None,
        "parameters": [],
        "return_type": None,
        "docstring": None,
        "dependencies": [],
        "references": [],
    }
    return {
        "symbol": symbol,
        "callers": [],
        "callees": [],
        "evidence_keys": {
            "symbol": "sample.py::top_level",
            "callers": [],
            "callees": [],
        },
        "indexed_files": 1,
    }


def _valid_trace_symbol_neighborhood_result() -> dict[str, object]:
    graph = _valid_trace_symbol_graph_result()
    return {
        "symbol": graph["symbol"],
        "direction": "both",
        "max_depth": 1,
        "max_nodes": 10,
        "truncated": False,
        "indexed_files": 1,
        "nodes": [],
        "edges": [],
    }


def _valid_patch_ast_node_result() -> dict[str, object]:
    return {
        "file": "sample.py",
        "target_path": "top_level",
        "resolved_path": "top_level",
        "resolved_symbol_id": "sample.py::top_level",
        "applied": True,
        "bypass_applied": False,
        "updated_source": "def top_level():\n    return 1\n",
        "validation": {
            "syntax_errors": [],
            "unresolved_identifiers": [],
            "resolved_identifiers": [],
            "ambiguous_identifiers": [],
            "binding_decisions": [],
            "commit_gate": {
                "status": "allowed",
                "allowed": True,
                "reason": "patch accepted",
                "bypass_reason": None,
                "blocking_decisions": [],
                "evidence_invariants": [],
                "syntax_error_count": 0,
            },
        },
    }


class GatewayRuntimeToolsTestsMixin:
    def test_tools_call_returns_mcp_error_for_non_serializable_tool_result(self) -> None:
        result = tools_call(
            {
                "name": "arborist/export_patch_diagnostics_sarif",
                "arguments": {},
            },
            lambda _tool_name, _arguments: {
                "version": "2.1.0",
                "$schema": "https://example.test/sarif.json",
                "runs": [{}],
                "invalid": float("nan"),
            },
        )

        self.assertTrue(result["isError"])
        self.assertNotIn("structuredContent", result)
        self.assertIn("Out of range float values", result["content"][0]["text"])

    def test_tools_call_rejects_malformed_direct_result_schema(self) -> None:
        cases = (
            (
                "arborist/get_semantic_skeleton",
                {},
                "result is missing required field `file`",
            ),
            (
                "arborist/register_symbol_index",
                {"workspace_root": ".", "db_path": "symbols.db", "extra": True},
                "result has unexpected field `extra`",
            ),
            (
                "arborist/trace_symbol_graph",
                {**_valid_trace_symbol_graph_result(), "indexed_files": -1},
                "result.indexed_files must be at least 0",
            ),
        )

        for tool_name, tool_result, expected_message in cases:
            with self.subTest(tool_name=tool_name):
                result = tools_call(
                    {"name": tool_name, "arguments": {}},
                    lambda _tool_name, _arguments, result=tool_result: result,
                )

                self.assertTrue(result["isError"])
                self.assertIn(expected_message, result["content"][0]["text"])
                self.assertNotIn("structuredContent", result)

    def test_tools_call_rejects_result_shape_mismatch(self) -> None:
        cases = (
            (
                "arborist/get_semantic_skeleton",
                {"file_path": "sample.py"},
                [],
                "expected object payload",
            ),
            (
                "arborist/list_symbol_indexes",
                {},
                {},
                "expected array payload",
            ),
            (
                "arborist/unregister_symbol_index",
                {},
                1,
                "expected boolean payload",
            ),
            (
                "arborist/list_symbol_indexes",
                {},
                [None],
                "expected object item at index 0",
            ),
        )

        for tool_name, arguments, tool_result, expected_message in cases:
            with self.subTest(tool_name=tool_name, tool_result=tool_result):
                result = tools_call(
                    {"name": tool_name, "arguments": arguments},
                    lambda _tool_name, _arguments, result=tool_result: result,
                )

                self.assertTrue(result["isError"])
                self.assertIn(expected_message, result["content"][0]["text"])
                self.assertNotIn("structuredContent", result)

    def test_tools_call_rejects_malformed_batch_result_items(self) -> None:
        cases = (
            (
                [{"name": "arborist/get_semantic_skeleton"}],
                "missing field `result`",
            ),
            (
                [
                    {
                        "name": "arborist/get_semantic_skeleton",
                        "result": {},
                        "extra": True,
                    }
                ],
                "has unexpected field `extra`",
            ),
            (
                [{"name": "arborist/batch", "result": []}],
                "has unsupported tool name `arborist/batch`",
            ),
            (
                [{"name": "arborist/get_semantic_skeleton", "result": []}],
                "has invalid result",
            ),
        )

        for tool_result, expected_message in cases:
            with self.subTest(tool_result=tool_result):
                result = tools_call(
                    {"name": "arborist/batch", "arguments": {"calls": []}},
                    lambda _tool_name, _arguments, result=tool_result: result,
                )

                self.assertTrue(result["isError"])
                self.assertIn(expected_message, result["content"][0]["text"])
                self.assertNotIn("structuredContent", result)

    def test_tools_call_rejects_malformed_batch_nested_result_schema(self) -> None:
        cases = (
            (
                [{"name": "arborist/get_semantic_skeleton", "result": {}}],
                "result is missing required field `file`",
            ),
            (
                [
                    {
                        "name": "arborist/get_semantic_skeleton",
                        "result": {
                            **_valid_semantic_skeleton_result(),
                            "future_field": True,
                        },
                    }
                ],
                "result has unexpected field `future_field`",
            ),
            (
                [
                    {
                        "name": "arborist/get_semantic_skeleton",
                        "result": {
                            **_valid_semantic_skeleton_result(),
                            "available_paths": [1],
                        },
                    }
                ],
                "result.available_paths[0] must be a string",
            ),
            (
                [
                    {
                        "name": "arborist/get_semantic_skeleton",
                        "result": {
                            **_valid_semantic_skeleton_result(),
                            "available_symbols": [
                                {
                                    "symbol_id": "top_level",
                                    "semantic_path": "top_level",
                                    "scope_path": None,
                                    "node_kind": "function_definition",
                                    "byte_range": [0],
                                    "signature": None,
                                    "parameters": [],
                                    "return_type": None,
                                    "docstring": None,
                                }
                            ],
                        },
                    }
                ],
                "result.available_symbols[0].byte_range must contain at least 2 items",
            ),
            (
                [
                    {
                        "name": "arborist/trace_symbol_neighborhood",
                        "result": {
                            **_valid_trace_symbol_neighborhood_result(),
                            "direction": "sideways",
                        },
                    }
                ],
                "result.direction must be one of ['callers', 'callees', 'both']",
            ),
        )

        for tool_result, expected_message in cases:
            with self.subTest(tool_result=tool_result):
                result = tools_call(
                    {"name": "arborist/batch", "arguments": {"calls": []}},
                    lambda _tool_name, _arguments, result=tool_result: result,
                )

                self.assertTrue(result["isError"])
                self.assertIn(expected_message, result["content"][0]["text"])
                self.assertNotIn("structuredContent", result)

    def test_batch_rejects_inner_result_shape_mismatch(self) -> None:
        with self.assertRaisesRegex(
            JsonRpcError,
            "invalid result from arborist/get_semantic_skeleton: expected object payload",
        ):
            batch_tools(
                {
                    "calls": [
                        {
                            "name": "arborist/get_semantic_skeleton",
                            "arguments": {"file_path": "sample.py"},
                        }
                    ]
                },
                lambda _tool_name, _arguments: [],
            )

    def test_tools_call_invokes_read_tool(self) -> None:
        skeleton = _valid_semantic_skeleton_result()
        core = make_recording_json_core(get_semantic_skeleton_json=skeleton)

        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(core),
                "tools/call",
                {
                    "name": "arborist/get_semantic_skeleton",
                    "arguments": {"file_path": "sample.py"},
                },
                request_id=103,
            ),
            request_id=103,
        )

        assert isinstance(result, dict)
        self.assertFalse(result["isError"])
        self.assertEqual(result["structuredContent"]["result"], skeleton)
        self.assertEqual(core.calls_for("get_semantic_skeleton_json"), [("sample.py", None, 2, None)])

    def test_tools_call_invokes_write_tool(self) -> None:
        patch = _valid_patch_ast_node_result()
        core = make_recording_json_core(patch_ast_node_json=patch)

        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(core),
                "tools/call",
                {
                    "name": "arborist/patch_ast_node",
                    "arguments": {
                        "file_path": "sample.py",
                        "semantic_path": "top_level",
                        "new_code": "def top_level():\n    return 1\n",
                    },
                },
                request_id=104,
            ),
            request_id=104,
        )

        assert isinstance(result, dict)
        self.assertFalse(result["isError"])
        self.assertEqual(result["structuredContent"]["result"], patch)
        self.assertEqual(
            core.calls_for("patch_ast_node_json"),
            [("sample.py", "top_level", "def top_level():\n    return 1\n", None, None)],
        )

    def test_tools_call_invokes_index_tool(self) -> None:
        registered = {"workspace_root": ".", "db_path": "symbols.db"}
        core = make_recording_json_core(register_symbol_index_json=registered)

        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(core),
                "tools/call",
                {
                    "name": "arborist/register_symbol_index",
                    "arguments": {"workspace_root": ".", "db_path": "symbols.db"},
                },
                request_id=105,
            ),
            request_id=105,
        )

        assert isinstance(result, dict)
        self.assertFalse(result["isError"])
        self.assertEqual(result["structuredContent"]["result"], registered)
        self.assertEqual(core.calls_for("register_symbol_index_json"), [(".", "symbols.db")])

    def test_tools_call_invokes_trace_tool(self) -> None:
        trace = _valid_trace_symbol_graph_result()
        core = make_recording_json_core(trace_symbol_graph_json=trace)

        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(core),
                "tools/call",
                {
                    "name": "arborist/trace_symbol_graph",
                    "arguments": {"workspace_root": ".", "symbol_path": "top_level"},
                },
                request_id=106,
            ),
            request_id=106,
        )

        assert isinstance(result, dict)
        self.assertFalse(result["isError"])
        self.assertEqual(result["structuredContent"]["result"], trace)
        self.assertEqual(
            core.calls_for("trace_symbol_graph_json"),
            [(".", "top_level", "both", None, None, None, None)],
        )

    def test_tools_call_invokes_read_only_batch(self) -> None:
        skeleton = _valid_semantic_skeleton_result()
        trace = _valid_trace_symbol_graph_result()
        core = make_recording_json_core(
            get_semantic_skeleton_json=skeleton,
            trace_symbol_graph_json=trace,
        )

        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(core),
                "tools/call",
                {
                    "name": "arborist/batch",
                    "arguments": {
                        "calls": [
                            {
                                "name": "arborist/get_semantic_skeleton",
                                "arguments": {"file_path": "sample.py"},
                            },
                            {
                                "name": "arborist/trace_symbol_graph",
                                "arguments": {
                                    "workspace_root": ".",
                                    "symbol_path": "top_level",
                                },
                            },
                        ]
                    },
                },
                request_id=113,
            ),
            request_id=113,
        )

        assert isinstance(result, dict)
        self.assertFalse(result["isError"])
        self.assertEqual(
            result["structuredContent"]["result"],
            [
                {
                    "name": "arborist/get_semantic_skeleton",
                    "result": skeleton,
                },
                {
                    "name": "arborist/trace_symbol_graph",
                    "result": trace,
                },
            ],
        )
        self.assertEqual(core.calls_for("get_semantic_skeleton_json"), [("sample.py", None, 2, None)])
        self.assertEqual(
            core.calls_for("trace_symbol_graph_json"),
            [(".", "top_level", "both", None, None, None, None)],
        )

    def test_tools_call_propagates_batch_timeout_to_inner_tool(self) -> None:
        core = make_recording_json_core(
            get_semantic_skeleton_json=_valid_semantic_skeleton_result()
        )

        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(core),
                "tools/call",
                {
                    "name": "arborist/batch",
                    "arguments": {
                        "calls": [
                            {
                                "name": "arborist/get_semantic_skeleton",
                                "arguments": {"file_path": "sample.py"},
                            }
                        ],
                        "timeout_ms": 5000,
                    },
                },
                request_id=117,
            ),
            request_id=117,
        )

        assert isinstance(result, dict)
        self.assertFalse(result["isError"])
        timeout_ms = core.calls_for("get_semantic_skeleton_json")[0][-1]
        self.assertIsInstance(timeout_ms, int)
        self.assertGreater(timeout_ms, 0)
        self.assertLessEqual(timeout_ms, 5000)

    def test_batch_shared_timeout_propagates_remaining_budget(self) -> None:
        calls = {
            "calls": [
                {
                    "name": "arborist/get_semantic_skeleton",
                    "arguments": {"file_path": "sample.py"},
                },
                {
                    "name": "arborist/trace_symbol_graph",
                    "arguments": {
                        "workspace_root": ".",
                        "symbol_path": "top_level",
                        "timeout_ms": 3,
                    },
                },
                {"name": "arborist/list_symbol_indexes", "arguments": {}},
            ]
        }
        observed: list[tuple[str, dict[str, object]]] = []
        clock_values = iter(
            (0, 1_100_000, 2_000_000, 3_000_000, 4_000_000, 5_000_000, 6_000_000)
        )

        def execute(name: str, arguments: dict[str, object]) -> object:
            observed.append((name, arguments))
            if name == "arborist/list_symbol_indexes":
                return []
            if name == "arborist/get_semantic_skeleton":
                return _valid_semantic_skeleton_result()
            return _valid_trace_symbol_graph_result()

        result = batch_tools(
            calls,
            execute,
            timeout_ms=10,
            monotonic_ns=lambda: next(clock_values),
        )

        self.assertEqual(
            [entry["name"] for entry in result],
            [call["name"] for call in calls["calls"]],
        )
        self.assertEqual(observed[0][1]["timeout_ms"], 9)
        self.assertEqual(observed[1][1]["timeout_ms"], 3)
        self.assertEqual(observed[2][1]["timeout_ms"], 5)
        self.assertNotIn("timeout_ms", calls["calls"][0]["arguments"])
        self.assertEqual(calls["calls"][1]["arguments"]["timeout_ms"], 3)

    def test_batch_shared_timeout_stops_before_next_call(self) -> None:
        observed: list[str] = []
        clock_values = iter((0, 0, 4_000_000, 5_000_000))

        with self.assertRaises(JsonRpcError) as context:
            batch_tools(
                {
                    "calls": [
                        {"name": "arborist/list_symbol_indexes"},
                        {"name": "arborist/list_symbol_indexes"},
                    ]
                },
                lambda name, _arguments: observed.append(name) or [],
                timeout_ms=5,
                monotonic_ns=lambda: next(clock_values),
            )

        self.assertEqual(context.exception.code, -32000)
        self.assertIn("before calls[1]", str(context.exception))
        self.assertEqual(observed, ["arborist/list_symbol_indexes"])

    def test_batch_validates_inner_timeout_before_execution(self) -> None:
        observed: list[str] = []

        with self.assertRaises(JsonRpcError) as context:
            batch_tools(
                {
                    "calls": [
                        {"name": "arborist/list_symbol_indexes"},
                        {
                            "name": "arborist/get_semantic_skeleton",
                            "arguments": {
                                "file_path": "sample.py",
                                "timeout_ms": (
                                    gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1
                                ),
                            },
                        },
                    ]
                },
                lambda name, _arguments: observed.append(name),
                timeout_ms=5,
                monotonic_ns=lambda: 0,
            )

        self.assertEqual(context.exception.code, -32602)
        self.assertIn("calls[1].arguments.timeout_ms", str(context.exception))
        self.assertEqual(observed, [])

    def test_batch_rejects_write_tool(self) -> None:
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(),
                "tools/call",
                {
                    "name": "arborist/batch",
                    "arguments": {
                        "calls": [
                            {
                                "name": "arborist/patch_ast_node",
                                "arguments": {
                                    "file_path": "sample.py",
                                    "semantic_path": "top_level",
                                    "new_code": "def top_level():\n    return 1\n",
                                },
                            }
                        ]
                    },
                },
                request_id=114,
            ),
            request_id=114,
        )

        assert isinstance(result, dict)
        self.assertTrue(result["isError"])
        self.assertIn("batch only supports read-only tools", result["content"][0]["text"])

    def test_batch_rejects_unknown_tool(self) -> None:
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(),
                "tools/call",
                {
                    "name": "arborist/batch",
                    "arguments": {
                        "calls": [
                            {"name": "arborist/missing", "arguments": {}},
                        ]
                    },
                },
                request_id=115,
            ),
            request_id=115,
        )

        assert isinstance(result, dict)
        self.assertTrue(result["isError"])
        self.assertIn("unknown batch tool", result["content"][0]["text"])

    def test_batch_rejects_nested_batch(self) -> None:
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(),
                "tools/call",
                {
                    "name": "arborist/batch",
                    "arguments": {
                        "calls": [
                            {"name": "arborist/batch", "arguments": {"calls": []}},
                        ]
                    },
                },
                request_id=116,
            ),
            request_id=116,
        )

        assert isinstance(result, dict)
        self.assertTrue(result["isError"])
        self.assertIn("may not include arborist/batch", result["content"][0]["text"])

    def test_tools_call_rejects_unknown_tool(self) -> None:
        response = self.call_gateway(
            self.make_gateway(),
            "tools/call",
            {"name": "arborist/missing", "arguments": {}},
            request_id=107,
        )

        self.assert_jsonrpc_error(
            response,
            request_id=107,
            code=-32602,
            contains="unknown tool",
        )

    def test_tools_call_reports_missing_tool_argument_as_tool_error(self) -> None:
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(),
                "tools/call",
                {"name": "arborist/get_semantic_skeleton", "arguments": {}},
                request_id=108,
            ),
            request_id=108,
        )

        assert isinstance(result, dict)
        self.assertTrue(result["isError"])
        self.assertIn("missing required string param: file_path", result["content"][0]["text"])

    def test_tools_call_rejects_non_object_arguments(self) -> None:
        response = self.call_gateway(
            self.make_gateway(),
            "tools/call",
            {"name": "arborist/get_semantic_skeleton", "arguments": []},
            request_id=109,
        )

        self.assert_jsonrpc_error(
            response,
            request_id=109,
            code=-32602,
            contains="arguments must be an object",
        )

    def test_tools_call_reports_argument_type_error_as_tool_error(self) -> None:
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(),
                "tools/call",
                {
                    "name": "arborist/get_semantic_skeleton",
                    "arguments": {"file_path": "sample.py", "depth_limit": "two"},
                },
                request_id=110,
            ),
            request_id=110,
        )

        assert isinstance(result, dict)
        self.assertTrue(result["isError"])
        self.assertIn("invalid int param: depth_limit", result["content"][0]["text"])

    def test_rejects_nonstandard_json_from_core(self) -> None:
        class StubCore:
            def list_symbol_indexes_json(self) -> str:
                return '[{"workspace_root": NaN}]'

        response = self.call_gateway(
            self.make_gateway(StubCore()),
            "arborist/list_symbol_indexes",
            {},
            request_id=34,
        )

        self.assert_jsonrpc_error(
            response,
            request_id=34,
            code=-32000,
            contains="invalid JSON from arborist core",
        )
        self.assertIn("non-standard JSON constant", response["error"]["message"])

    def test_rejects_malformed_json_from_core(self) -> None:
        class StubCore:
            def list_symbol_indexes_json(self) -> str:
                return '[{"workspace_root": "."}'

        response = self.call_gateway(
            self.make_gateway(StubCore()),
            "arborist/list_symbol_indexes",
            {},
            request_id=35,
        )

        self.assert_jsonrpc_error(
            response,
            request_id=35,
            code=-32000,
            contains="invalid JSON from arborist core",
        )

    def test_rejects_duplicate_json_keys_from_core(self) -> None:
        class StubCore:
            def list_symbol_indexes_json(self) -> str:
                return '[{"workspace_root": "a", "workspace_root": "b"}]'

        response = self.call_gateway(
            self.make_gateway(StubCore()),
            "arborist/list_symbol_indexes",
            {},
            request_id=50,
        )

        self.assert_jsonrpc_error(
            response,
            request_id=50,
            code=-32000,
            contains="invalid JSON from arborist core",
        )
        self.assertIn("duplicate JSON object key", response["error"]["message"])

    def test_rejects_object_core_payload_with_wrong_shape(self) -> None:
        class StubCore:
            def get_semantic_skeleton_json(self, *args: object) -> str:
                return "[]"

        response = self.call_gateway(
            self.make_gateway(StubCore()),
            "arborist/get_semantic_skeleton",
            {"file_path": "sample.py"},
            request_id=52,
        )

        self.assert_jsonrpc_error(
            response,
            request_id=52,
            code=-32000,
            contains="invalid JSON from arborist core",
        )
        self.assertIn("expected object", response["error"]["message"])

    def test_rejects_list_core_payload_with_wrong_shape(self) -> None:
        class StubCore:
            def list_symbol_indexes_json(self) -> str:
                return "{}"

        response = self.call_gateway(
            self.make_gateway(StubCore()),
            "arborist/list_symbol_indexes",
            {},
            request_id=53,
        )

        self.assert_jsonrpc_error(
            response,
            request_id=53,
            code=-32000,
            contains="invalid JSON from arborist core",
        )
        self.assertIn("expected array", response["error"]["message"])

    def test_rejects_list_core_payload_with_non_object_items(self) -> None:
        class StubCore:
            def execute_tree_query_json(self, *args: object) -> str:
                return "[null]"

        response = self.call_gateway(
            self.make_gateway(StubCore()),
            "arborist/execute_tree_query",
            {"file_path": "sample.py", "query": "(module) @module"},
            request_id=54,
        )

        self.assert_jsonrpc_error(
            response,
            request_id=54,
            code=-32000,
            contains="invalid JSON from arborist core",
        )
        self.assertIn("expected object item", response["error"]["message"])
