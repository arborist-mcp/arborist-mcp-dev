from __future__ import annotations

from arborist_mcp import gateway as gateway_module

from tests.gateway_protocol.helpers import GatewayProtocolTestCase
from tests.gateway_protocol.request_validation_context import (
    GatewayContextRequestValidationMixin,
)
from tests.gateway_protocol.request_validation_metadata import (
    GatewayMetadataRequestValidationMixin,
)
from tests.gateway_protocol.request_validation_params import (
    GatewayParameterRequestValidationMixin,
)
from tests.gateway_protocol.request_validation_source import (
    GatewaySourceRequestValidationMixin,
)
from tests.gateway_protocol.request_validation_edits import (
    GatewayEditRequestValidationMixin,
)
from tests.gateway_protocol.request_validation_timeouts import (
    GatewayTimeoutRequestValidationMixin,
)
from tests.gateway_protocol.request_validation_trace import (
    GatewayTraceRequestValidationMixin,
)

SUITE_NAME = "gateway-request-validation"
REQUIRES_EXTENSION = False
COVERED_TOOLS = (
    "arborist/apply_buffer_edit",
    "arborist/commit_virtual_file",
    "arborist/did_change",
    "arborist/did_close",
    "arborist/did_open",
    "arborist/discard_virtual_file",
    "arborist/get_semantic_skeleton",
    "arborist/list_symbol_indexes",
    "arborist/list_symbols",
    "arborist/list_symbols_discovery_context",
    "arborist/list_symbols_neighborhood_context",
    "arborist/list_virtual_files",
    "arborist/patch_ast_node_at_position",
    "arborist/patch_virtual_ast_node_at_position",
    "arborist/read_virtual_file",
    "arborist/read_symbol_at_position",
    "arborist/read_symbol_context",
    "arborist/read_symbol_context_at_position",
    "arborist/read_symbol_discovery_context",
    "arborist/read_symbol_discovery_context_at_position",
    "arborist/read_symbol_neighborhood_context",
    "arborist/read_symbol_neighborhood_context_at_position",
    "arborist/search_symbols",
    "arborist/search_symbols_discovery_context",
    "arborist/trace_symbol_graph_at_position",
    "arborist/trace_symbol_graph",
    "arborist/trace_symbol_neighborhood_at_position",
    "arborist/trace_symbol_neighborhood",
    "arborist/validate_patch_with_discovery_context",
    "arborist/validate_patch_with_discovery_context_at_position",
    "arborist/validate_patch_with_graph_context",
    "arborist/validate_patch_with_graph_context_at_position",
    "arborist/validate_patch_with_neighborhood_context",
    "arborist/validate_patch_with_neighborhood_context_at_position",
    "arborist/validate_patch_with_trace_context",
    "arborist/validate_patch_with_trace_context_at_position",
)


class GatewayRequestValidationTests(
    GatewayMetadataRequestValidationMixin,
    GatewayContextRequestValidationMixin,
    GatewayEditRequestValidationMixin,
    GatewayParameterRequestValidationMixin,
    GatewaySourceRequestValidationMixin,
    GatewayTimeoutRequestValidationMixin,
    GatewayTraceRequestValidationMixin,
    GatewayProtocolTestCase,
):
    def assert_invalid_request(
        self,
        request: object,
        *,
        request_id: object,
        contains: str,
    ) -> None:
        response = self.make_gateway().handle_request(request)
        self.assert_jsonrpc_error(
            response,
            request_id=request_id,
            code=-32600,
            contains=contains,
        )

    def assert_invalid_params(
        self,
        method: str,
        params: object,
        *,
        request_id: object,
        contains: str,
        gateway: object | None = None,
    ) -> None:
        target_gateway = self.make_gateway() if gateway is None else gateway
        response = target_gateway.handle_request(
            self.request(method, params, request_id=request_id)
        )
        self.assert_jsonrpc_error(
            response,
            request_id=request_id,
            code=-32602,
            contains=contains,
        )

    def test_rejects_non_object_request_without_calling_core(self) -> None:
        self.assert_invalid_request(["initialize"], request_id=None, contains="expected object")

    def test_rejects_non_object_params_without_calling_core_method(self) -> None:
        self.assert_invalid_params(
            "arborist/get_semantic_skeleton",
            [],
            request_id=7,
            contains="invalid params",
        )

    def test_rejects_unexpected_legacy_initialize_params_without_calling_core(self) -> None:
        class StubCore:
            def supported_languages(self) -> list[str]:
                raise AssertionError("core should not be called")

        self.assert_invalid_params(
            "initialize",
            {"unexpected": {"name": "codex"}},
            request_id=8,
            contains="unexpected",
            gateway=self.make_gateway(StubCore()),
        )

    def test_rejects_missing_method_as_invalid_request(self) -> None:
        self.assert_invalid_request(
            {"jsonrpc": "2.0", "id": 3, "params": {}},
            request_id=3,
            contains="missing method",
        )

    def test_reports_unknown_method_with_method_not_found_code(self) -> None:
        response = self.call_gateway(
            self.make_gateway(),
            "arborist/nope",
            {},
            request_id=5,
        )
        self.assert_jsonrpc_error(
            response,
            request_id=5,
            code=-32601,
            contains="method not found",
        )

    def test_rejects_missing_jsonrpc_version(self) -> None:
        self.assert_invalid_request(
            {"id": 6, "method": "arborist/list_symbol_indexes", "params": {}},
            request_id=6,
            contains="jsonrpc",
        )

    def test_rejects_non_2_0_jsonrpc_version(self) -> None:
        self.assert_invalid_request(
            {
                "jsonrpc": "1.0",
                "id": 8,
                "method": "arborist/list_symbol_indexes",
                "params": {},
            },
            request_id=8,
            contains="jsonrpc",
        )

    def test_invalid_jsonrpc_version_with_array_id_returns_null_id(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "1.0",
                "id": [],
                "method": "arborist/list_symbol_indexes",
                "params": {},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32600)
        self.assertIn("jsonrpc", response["error"]["message"])

    def test_missing_jsonrpc_with_bool_id_returns_null_id(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {"id": True, "method": "arborist/list_symbol_indexes", "params": {}}
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32600)
        self.assertIn("jsonrpc", response["error"]["message"])

    def test_rejects_array_request_id_as_invalid_request(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": [],
                "method": "arborist/list_symbol_indexes",
                "params": {},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32600)
        self.assertIn("invalid id", response["error"]["message"])

    def test_rejects_bool_request_id_as_invalid_request(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": True,
                "method": "arborist/list_symbol_indexes",
                "params": {},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32600)
        self.assertIn("invalid id", response["error"]["message"])

    def test_rejects_nan_request_id_object_as_invalid_request(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": float("nan"),
                "method": "arborist/list_symbol_indexes",
                "params": {},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32600)
        self.assertIn("invalid id", response["error"]["message"])

    def test_rejects_infinite_request_id_object_as_invalid_request(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": float("inf"),
                "method": "arborist/list_symbol_indexes",
                "params": {},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32600)
        self.assertIn("invalid id", response["error"]["message"])

    def test_rejects_fractional_request_id_as_invalid_request(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 1.5,
                "method": "arborist/list_symbol_indexes",
                "params": {},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32600)
        self.assertIn("invalid id", response["error"]["message"])

    def test_rejects_float_request_id_as_invalid_request(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 1.0,
                "method": "arborist/list_symbol_indexes",
                "params": {},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32600)
        self.assertIn("invalid id", response["error"]["message"])

    def test_reports_missing_required_param_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

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

    def test_rejects_unexpected_top_level_params_without_calling_core(self) -> None:
        class StubCore:
            def list_symbol_indexes_json(self) -> str:
                raise AssertionError("core should not be called")

            def trace_symbol_graph_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

            def close_virtual_file_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        cases = [
            (
                "arborist/list_symbol_indexes",
                {"unexpected": True},
                "unexpected",
            ),
            (
                "arborist/trace_symbol_graph",
                {"symbol_path": "top_level", "workspaceRoot": "."},
                "workspaceRoot",
            ),
            (
                "arborist/did_close",
                {"file_path": "sample.py", "persist": False, "save": True},
                "save",
            ),
        ]

        for method, params, expected_key in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 44,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 44)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn(expected_key, response["error"]["message"])

    def test_rejects_invalid_read_symbol_context_direction_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 62,
                "method": "arborist/read_symbol_context",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "orchestrate",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 62)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])
