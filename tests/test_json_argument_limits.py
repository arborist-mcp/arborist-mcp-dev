import unittest

from arborist_mcp.gateway_params import GatewayParameterValidation
from arborist_mcp.tool_result_schemas import JsonRpcError
from arborist_mcp.tool_specs import MAX_JSON_ARG_DEPTH


class JsonArgumentLimitTests(unittest.TestCase):
    def test_rejects_excessive_nesting(self) -> None:
        value: object = 0
        for _ in range(MAX_JSON_ARG_DEPTH + 1):
            value = [value]

        with self.assertRaisesRegex(JsonRpcError, "maximum nesting depth"):
            GatewayParameterValidation._encode_json_param(value, "payload")

