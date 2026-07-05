from __future__ import annotations

import io
import unittest
from unittest import mock

from arborist_mcp import gateway as gateway_module
from arborist_mcp.gateway import ArboristGateway


class GatewayProtocolTests(unittest.TestCase):
    def test_rejects_non_object_request_without_calling_core(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(["initialize"])

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32600)
        self.assertIn("expected object", response["error"]["message"])

    def test_rejects_non_object_params_without_calling_core_method(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 7,
                "method": "arborist/get_semantic_skeleton",
                "params": [],
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 7)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("invalid params", response["error"]["message"])

    def test_rejects_missing_method_as_invalid_request(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request({"jsonrpc": "2.0", "id": 3, "params": {}})

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 3)
        self.assertEqual(response["error"]["code"], -32600)
        self.assertIn("missing method", response["error"]["message"])

    def test_reports_unknown_method_with_method_not_found_code(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {"jsonrpc": "2.0", "id": 5, "method": "arborist/nope", "params": {}}
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 5)
        self.assertEqual(response["error"]["code"], -32601)
        self.assertIn("method not found", response["error"]["message"])

    def test_rejects_missing_jsonrpc_version(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {"id": 6, "method": "arborist/list_symbol_indexes", "params": {}}
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 6)
        self.assertEqual(response["error"]["code"], -32600)
        self.assertIn("jsonrpc", response["error"]["message"])

    def test_rejects_non_2_0_jsonrpc_version(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "1.0",
                "id": 8,
                "method": "arborist/list_symbol_indexes",
                "params": {},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 8)
        self.assertEqual(response["error"]["code"], -32600)
        self.assertIn("jsonrpc", response["error"]["message"])

    def test_reports_missing_required_param_as_invalid_params(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 9,
                "method": "arborist/get_semantic_skeleton",
                "params": {},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 9)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("file_path", response["error"]["message"])

    def test_rejects_string_bool_params(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 11,
                "method": "arborist/list_virtual_files",
                "params": {"dirty_only": "false"},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 11)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("dirty_only", response["error"]["message"])

    def test_rejects_string_int_params(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 13,
                "method": "arborist/get_semantic_skeleton",
                "params": {"file_path": "sample.py", "depth_limit": "2"},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 13)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("depth_limit", response["error"]["message"])

    def test_rejects_non_string_optional_params(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 15,
                "method": "arborist/trace_symbol_graph",
                "params": {"workspace_root": 123, "symbol_path": "top_level"},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 15)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("workspace_root", response["error"]["message"])

    def test_passes_typed_optional_defaults_to_core(self) -> None:
        class StubCore:
            def get_semantic_skeleton_json(
                self,
                file_path: str,
                source: str | None,
                depth_limit: int,
                expand_nodes: list[str] | None,
            ) -> str:
                self.args = (file_path, source, depth_limit, expand_nodes)
                return "{}"

        core = StubCore()
        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = core

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 17,
                "method": "arborist/get_semantic_skeleton",
                "params": {"file_path": "sample.py"},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 17)
        self.assertEqual(response["result"], {})
        self.assertEqual(core.args, ("sample.py", None, 2, None))

    def test_initialize_still_reports_tools(self) -> None:
        class StubCore:
            def supported_languages(self) -> list[str]:
                return ["python", "c"]

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 1)
        self.assertEqual(response["result"]["supportedLanguages"], ["python", "c"])
        self.assertIn(
            "arborist/validate_patch_with_trace_context",
            response["result"]["capabilities"]["tools"],
        )

    def test_stdio_notification_does_not_emit_response(self) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                self.request = request
                return {"jsonrpc": "2.0", "id": None, "result": {"ok": True}}

        fake_gateway = StubGateway()
        stdin = io.StringIO(
            '{"jsonrpc":"2.0","method":"arborist/list_symbol_indexes","params":{}}\n'
        )
        stdout = io.StringIO()

        with mock.patch.object(gateway_module, "ArboristGateway", return_value=fake_gateway):
            with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)
        self.assertEqual(
            fake_gateway.request,
            {
                "jsonrpc": "2.0",
                "method": "arborist/list_symbol_indexes",
                "params": {},
            },
        )
        self.assertEqual(stdout.getvalue(), "")

    def test_once_notification_does_not_print_response(self) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                self.request = request
                return {"jsonrpc": "2.0", "id": None, "result": {"ok": True}}

        fake_gateway = StubGateway()

        with mock.patch.object(gateway_module, "ArboristGateway", return_value=fake_gateway):
            with mock.patch(
                "pathlib.Path.read_text",
                return_value='{"jsonrpc":"2.0","method":"arborist/list_symbol_indexes","params":{}}',
            ):
                with mock.patch("builtins.print") as mock_print:
                    exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 0)
        self.assertEqual(
            fake_gateway.request,
            {
                "jsonrpc": "2.0",
                "method": "arborist/list_symbol_indexes",
                "params": {},
            },
        )
        mock_print.assert_not_called()


if __name__ == "__main__":
    unittest.main()
