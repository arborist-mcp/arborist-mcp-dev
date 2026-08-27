import json
import unittest

from arborist_mcp.batch_tools import _validate_batch_calls
from arborist_mcp.jsonrpc import error_response, is_valid_request_id, serialize_response
from arborist_mcp.tool_param_specs import MAX_BATCH_CALLS, MAX_WORKSPACE_SCAN_TIMEOUT_MS
from arborist_mcp.tool_result_schemas import JsonRpcError


def _read_symbol_call(**arguments: object) -> dict[str, object]:
    return {"name": "arborist/read_symbol", "arguments": arguments}


class ValidateBatchCallsTests(unittest.TestCase):
    def test_rejects_missing_non_list_and_empty_calls(self) -> None:
        with self.assertRaisesRegex(JsonRpcError, "missing required array param: calls"):
            _validate_batch_calls({})
        with self.assertRaisesRegex(JsonRpcError, "missing required array param: calls"):
            _validate_batch_calls({"calls": "nope"})
        with self.assertRaisesRegex(JsonRpcError, "calls must not be empty"):
            _validate_batch_calls({"calls": []})

    def test_rejects_calls_above_limit_with_entry_count_message(self) -> None:
        calls = [
            {"name": "arborist/read_symbol", "arguments": {"path": f"src/{i}.py"}}
            for i in range(MAX_BATCH_CALLS + 1)
        ]
        with self.assertRaisesRegex(
            JsonRpcError, f"calls must contain at most {MAX_BATCH_CALLS} entries"
        ):
            _validate_batch_calls({"calls": calls})

    def test_rejects_non_object_call_entries(self) -> None:
        for index, entry in enumerate((3, "call", None)):
            with self.subTest(entry=entry):
                with self.assertRaisesRegex(
                    JsonRpcError, "invalid params: calls\\[0\\] must be an object"
                ):
                    _validate_batch_calls({"calls": [entry]})

    def test_rejects_blank_unknown_nested_and_write_only_names(self) -> None:
        with self.assertRaisesRegex(
            JsonRpcError, r"missing required string param: calls\[0\]\.name"
        ):
            _validate_batch_calls({"calls": [{"name": "   "}]})
        with self.assertRaisesRegex(JsonRpcError, "unknown batch tool: arborist/nope"):
            _validate_batch_calls({"calls": [{"name": "arborist/nope"}]})
        with self.assertRaisesRegex(
            JsonRpcError, "batch calls may not include arborist/batch"
        ):
            _validate_batch_calls({"calls": [{"name": "arborist/batch"}]})
        with self.assertRaisesRegex(
            JsonRpcError,
            "batch only supports read-only tools: arborist/patch_ast_node",
        ):
            _validate_batch_calls({"calls": [{"name": "arborist/patch_ast_node"}]})

    def test_rejects_call_level_unexpected_params_and_bad_arguments(self) -> None:
        with self.assertRaisesRegex(JsonRpcError, "unexpected param: extra"):
            _validate_batch_calls(
                {"calls": [{"name": "arborist/read_symbol", "extra": 1}]}
            )
        with self.assertRaisesRegex(
            JsonRpcError, r"calls\[0\]\.arguments must be an object"
        ):
            _validate_batch_calls({"calls": [{"name": "arborist/read_symbol", "arguments": []}]})
        with self.assertRaisesRegex(JsonRpcError, "unexpected param: bogus"):
            _validate_batch_calls({"calls": [_read_symbol_call(bogus=1)]})

    def test_inner_timeout_ms_validation_edges(self) -> None:
        path = r"calls\[0\]\.arguments\.timeout_ms"
        for value in (True, 1.5, "soon"):
            with self.subTest(value=value):
                with self.assertRaisesRegex(JsonRpcError, f"invalid int param: {path}"):
                    _validate_batch_calls({"calls": [_read_symbol_call(timeout_ms=value)]})
        for value in (0, -5):
            with self.subTest(value=value):
                with self.assertRaisesRegex(
                    JsonRpcError, f"invalid positive int param: {path}"
                ):
                    _validate_batch_calls({"calls": [_read_symbol_call(timeout_ms=value)]})
        with self.assertRaisesRegex(JsonRpcError, f"{path} exceeds maximum"):
            _validate_batch_calls(
                {
                    "calls": [
                        _read_symbol_call(timeout_ms=MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1)
                    ]
                }
            )

    def test_valid_batch_returns_structured_calls_and_defaults_arguments(self) -> None:
        validated = _validate_batch_calls(
            {
                "calls": [
                    {"name": "arborist/read_symbol"},
                    _read_symbol_call(symbol_path="Type::method"),
                ]
            }
        )

        self.assertEqual([call.name for call in validated], ["arborist/read_symbol"] * 2)
        self.assertEqual(validated[0].arguments, {})
        self.assertEqual(
            validated[1].arguments, {"symbol_path": "Type::method"}
        )
        self.assertIsNone(validated[0].timeout_ms)
        self.assertIsNone(validated[1].timeout_ms)

    def test_valid_inner_timeout_ms_is_preserved(self) -> None:
        validated = _validate_batch_calls(
            {"calls": [_read_symbol_call(timeout_ms=MAX_WORKSPACE_SCAN_TIMEOUT_MS)]}
        )

        self.assertEqual(validated[0].timeout_ms, MAX_WORKSPACE_SCAN_TIMEOUT_MS)


class ErrorResponseContractTests(unittest.TestCase):
    def test_error_response_keeps_valid_ids_and_coerces_invalid_ones(self) -> None:
        for request_id in (7, "abc", None):
            response = error_response(request_id, -32602, "bad params")
            self.assertEqual(response["id"], request_id)
        for invalid_id in ([1], {"x": 1}, True, 2.5):
            response = error_response(invalid_id, -32602, "bad params")
            self.assertIsNone(response["id"])

    def test_is_valid_request_id_accepts_json_rpc_id_shapes(self) -> None:
        self.assertTrue(is_valid_request_id(None))
        self.assertTrue(is_valid_request_id(0))
        self.assertTrue(is_valid_request_id("id"))
        self.assertFalse(is_valid_request_id(True))
        self.assertFalse(is_valid_request_id(1.5))
        self.assertFalse(is_valid_request_id([]))

    def test_serialize_response_falls_back_on_nan_payload(self) -> None:
        payload = serialize_response(
            {"jsonrpc": "2.0", "id": 1, "result": float("nan")}
        )

        self.assertIn("-32000", payload)
        self.assertIn("failed to serialize response", payload)

    def test_serialize_response_fallback_is_utf8_safe_for_lone_surrogates(self) -> None:
        payload = serialize_response(
            {"jsonrpc": "2.0", "id": 2, "result": {"value": "\ud800"}}
        )

        payload.encode("utf-8")
        response = json.loads(payload)
        self.assertEqual(response["error"]["code"], -32000)
        self.assertIn("failed to serialize response", response["error"]["message"])

    def test_serialize_response_round_trips_normal_payloads(self) -> None:
        response = {"jsonrpc": "2.0", "id": 3, "result": {"ok": True}}
        self.assertEqual(
            serialize_response(response),
            json.dumps(response, ensure_ascii=False, allow_nan=False),
        )


if __name__ == "__main__":
    unittest.main()
