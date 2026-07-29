from __future__ import annotations

from arborist_mcp import gateway as gateway_module


class GatewayRuntimeTreeQueryTestsMixin:
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
