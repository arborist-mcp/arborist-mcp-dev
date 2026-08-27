import unittest
from unittest.mock import patch

from arborist_mcp.gateway_params import GatewayParameterValidation
from arborist_mcp.jsonrpc import parse_request_json
from arborist_mcp.tool_result_schemas import JsonRpcError
from arborist_mcp.tool_specs import MAX_JSON_ARG_DEPTH


class JsonArgumentLimitTests(unittest.TestCase):
    def test_rejects_excessive_nesting(self) -> None:
        value: object = 0
        for _ in range(MAX_JSON_ARG_DEPTH + 1):
            value = [value]

        with self.assertRaisesRegex(JsonRpcError, "maximum nesting depth"):
            GatewayParameterValidation._encode_json_param(value, "payload")

    def test_rejects_oversized_json_rpc_request_before_decoding(self) -> None:
        with patch("arborist_mcp.jsonrpc.MAX_REQUEST_BYTES", 8):
            request, response = parse_request_json('{"jsonrpc":"2.0"}')

        self.assertIsNone(request)
        assert response is not None
        self.assertEqual(response["error"]["code"], -32600)
        self.assertIn("maximum size", response["error"]["message"])

    def test_rejects_non_utf8_text_without_raising(self) -> None:
        request, response = parse_request_json("\ud800")

        self.assertIsNone(request)
        assert response is not None
        self.assertEqual(response["error"]["code"], -32700)
        self.assertIn("not valid UTF-8 text", response["error"]["message"])

    def test_rejects_excessive_json_rpc_nesting(self) -> None:
        payload = "0"
        for _ in range(MAX_JSON_ARG_DEPTH + 1):
            payload = f"[{payload}]"

        request, response = parse_request_json(payload)

        self.assertIsNone(request)
        assert response is not None
        self.assertEqual(response["error"]["code"], -32700)
        self.assertIn("maximum nesting depth", response["error"]["message"])
