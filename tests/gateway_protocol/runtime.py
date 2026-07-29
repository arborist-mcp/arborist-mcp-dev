from __future__ import annotations

from arborist_mcp import gateway as gateway_module
from arborist_mcp.batch_tools import batch_tools
from arborist_mcp.mcp_tools import tools_call
from arborist_mcp.tool_result_schemas import JsonRpcError

from tests.gateway_protocol.helpers import GatewayProtocolTestCase, make_recording_json_core
from tests.gateway_protocol.runtime_catalog import (
    GatewayRuntimeCatalogTestsMixin,
)
from tests.gateway_protocol.runtime_transport import (
    GatewayRuntimeTransportTestsMixin,
)

SUITE_NAME = "gateway-runtime"
REQUIRES_EXTENSION = True
COVERED_TOOLS = (
    "arborist/batch",
    "arborist/execute_tree_query",
    "arborist/get_semantic_skeleton",
    "arborist/list_symbol_indexes",
)


class GatewayRuntimeTests(
    GatewayRuntimeCatalogTestsMixin,
    GatewayRuntimeTransportTestsMixin,
    GatewayProtocolTestCase,
):
    def test_tools_call_returns_mcp_error_for_non_serializable_tool_result(self) -> None:
        result = tools_call(
            {
                "name": "arborist/get_semantic_skeleton",
                "arguments": {"file_path": "sample.py"},
            },
            lambda _tool_name, _arguments: {"invalid": float("nan")},
        )

        self.assertTrue(result["isError"])
        self.assertNotIn("structuredContent", result)
        self.assertIn("Out of range float values", result["content"][0]["text"])

    def test_tools_call_invokes_read_tool(self) -> None:
        core = make_recording_json_core(get_semantic_skeleton_json={"kind": "module"})

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
        self.assertEqual(result["structuredContent"]["result"], {"kind": "module"})
        self.assertEqual(core.calls_for("get_semantic_skeleton_json"), [("sample.py", None, 2, None)])

    def test_tools_call_invokes_write_tool(self) -> None:
        core = make_recording_json_core(patch_ast_node_json={"patched": True})

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
        self.assertEqual(result["structuredContent"]["result"], {"patched": True})
        self.assertEqual(
            core.calls_for("patch_ast_node_json"),
            [("sample.py", "top_level", "def top_level():\n    return 1\n", None, None)],
        )

    def test_tools_call_invokes_index_tool(self) -> None:
        core = make_recording_json_core(register_symbol_index_json={"registered": True})

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
        self.assertEqual(result["structuredContent"]["result"], {"registered": True})
        self.assertEqual(core.calls_for("register_symbol_index_json"), [(".", "symbols.db")])

    def test_tools_call_invokes_trace_tool(self) -> None:
        core = make_recording_json_core(trace_symbol_graph_json={"symbol": "top_level"})

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
        self.assertEqual(result["structuredContent"]["result"], {"symbol": "top_level"})
        self.assertEqual(
            core.calls_for("trace_symbol_graph_json"),
            [(".", "top_level", "both", None, None, None, None)],
        )

    def test_tools_call_invokes_read_only_batch(self) -> None:
        core = make_recording_json_core(
            get_semantic_skeleton_json={"kind": "module"},
            trace_symbol_graph_json={"symbol": "top_level"},
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
                    "result": {"kind": "module"},
                },
                {
                    "name": "arborist/trace_symbol_graph",
                    "result": {"symbol": "top_level"},
                },
            ],
        )
        self.assertEqual(core.calls_for("get_semantic_skeleton_json"), [("sample.py", None, 2, None)])
        self.assertEqual(
            core.calls_for("trace_symbol_graph_json"),
            [(".", "top_level", "both", None, None, None, None)],
        )

    def test_tools_call_propagates_batch_timeout_to_inner_tool(self) -> None:
        core = make_recording_json_core(get_semantic_skeleton_json={"kind": "module"})

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

        result = batch_tools(
            calls,
            lambda name, arguments: observed.append((name, arguments))
            or {"ok": True},
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

    def test_execute_tree_query_passes_capture_limit_to_core(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.args: tuple[object, ...] | None = None

            def execute_tree_query_json(self, *args: object) -> str:
                self.args = args
                return "[]"

        core = StubCore()
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_gateway(core),
                "arborist/execute_tree_query",
                {
                    "file_path": "sample.py",
                    "query": "(module) @module",
                    "max_captures": 7,
                    "timeout_ms": 2500,
                },
                request_id=55,
            ),
            request_id=55,
        )

        self.assertEqual(result, [])
        self.assertEqual(core.args, ("sample.py", "(module) @module", None, 7, 2500))

    def test_execute_tree_query_rejects_invalid_timeout(self) -> None:
        class StubCore:
            def execute_tree_query_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        for timeout_ms in (0, gateway_module.MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1):
            with self.subTest(timeout_ms=timeout_ms):
                response = self.call_gateway(
                    self.make_gateway(StubCore()),
                    "arborist/execute_tree_query",
                    {
                        "file_path": "sample.py",
                        "query": "(module) @module",
                        "timeout_ms": timeout_ms,
                    },
                    request_id=58 + timeout_ms,
                )

                self.assert_jsonrpc_error(
                    response,
                    request_id=58 + timeout_ms,
                    code=-32602,
                    contains="timeout_ms",
                )

    def test_execute_tree_query_rejects_zero_capture_limit(self) -> None:
        class StubCore:
            def execute_tree_query_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        response = self.call_gateway(
            self.make_gateway(StubCore()),
            "arborist/execute_tree_query",
            {
                "file_path": "sample.py",
                "query": "(module) @module",
                "max_captures": 0,
            },
            request_id=56,
        )

        self.assert_jsonrpc_error(
            response, request_id=56, code=-32602, contains="max_captures"
        )

    def test_execute_tree_query_rejects_oversized_query_before_core(self) -> None:
        class StubCore:
            def execute_tree_query_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        response = self.call_gateway(
            self.make_gateway(StubCore()),
            "arborist/execute_tree_query",
            {
                "file_path": "sample.py",
                "query": "(" * (gateway_module.TREE_QUERY_MAX_LENGTH + 1),
            },
            request_id=57,
        )

        self.assert_jsonrpc_error(response, request_id=57, code=-32602, contains="query")
        self.assertIn("max length", response["error"]["message"])

    def test_execute_tree_query_preserves_owner_metadata_from_core(self) -> None:
        result = self.assert_jsonrpc_ok(
            self.call_gateway(
                self.make_live_gateway(),
                "arborist/execute_tree_query",
                {
                    "file_path": "tests/fixtures/sample.py",
                    "source": "@logged\ndef top_level(value):\n    return value\n",
                    "query": "(decorator (identifier) @decorator)",
                },
                request_id=23,
            ),
            request_id=23,
        )

        assert isinstance(result, list)
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]["capture_name"], "decorator")
        self.assertEqual(result[0]["text"], "logged")
        self.assertEqual(result[0]["owner_symbol_id"], "top_level")
        self.assertEqual(result[0]["owner_semantic_path"], "top_level")
        self.assertIsNone(result[0]["owner_scope_path"])
