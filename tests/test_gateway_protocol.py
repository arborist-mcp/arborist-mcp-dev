from __future__ import annotations

import unittest

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


if __name__ == "__main__":
    unittest.main()
