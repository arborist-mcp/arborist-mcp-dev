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

    def test_invalid_jsonrpc_version_with_array_id_returns_null_id(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

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
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {"id": True, "method": "arborist/list_symbol_indexes", "params": {}}
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32600)
        self.assertIn("jsonrpc", response["error"]["message"])

    def test_rejects_array_request_id_as_invalid_request(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

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
        gateway = ArboristGateway.__new__(ArboristGateway)

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
        gateway = ArboristGateway.__new__(ArboristGateway)

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
        gateway = ArboristGateway.__new__(ArboristGateway)

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
        gateway = ArboristGateway.__new__(ArboristGateway)

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
        gateway = ArboristGateway.__new__(ArboristGateway)

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

    def test_rejects_non_json_serializable_edits_as_invalid_params(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 10,
                "method": "arborist/did_change",
                "params": {
                    "file_path": "sample.py",
                    "edits": [{"new_text": {1, 2, 3}}],
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 10)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("edits", response["error"]["message"])

    def test_rejects_non_finite_edits_as_invalid_params(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 12,
                "method": "arborist/did_change",
                "params": {
                    "file_path": "sample.py",
                    "edits": [{"start": {"row": float("nan"), "column": 0}}],
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 12)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("edits", response["error"]["message"])

    def test_rejects_negative_position_edit_coordinates(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 28,
                "method": "arborist/did_change",
                "params": {
                    "file_path": "sample.py",
                    "edits": [
                        {
                            "start": {"row": -1, "column": 0},
                            "end": {"row": 0, "column": 0},
                            "new_text": "x",
                        }
                    ],
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 28)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("edits[0].start.row", response["error"]["message"])

    def test_rejects_missing_position_edit_new_text(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 29,
                "method": "arborist/did_change",
                "params": {
                    "file_path": "sample.py",
                    "edits": [
                        {
                            "start": {"row": 0, "column": 0},
                            "end": {"row": 0, "column": 0},
                        }
                    ],
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 29)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("edits[0].new_text", response["error"]["message"])

    def test_rejects_reversed_position_edit_range(self) -> None:
        class StubCore:
            def apply_position_edits_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 37,
                "method": "arborist/did_change",
                "params": {
                    "file_path": "sample.py",
                    "edits": [
                        {
                            "start": {"row": 2, "column": 0},
                            "end": {"row": 1, "column": 9},
                            "new_text": "x",
                        }
                    ],
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 37)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("edits[0].start", response["error"]["message"])

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

    def test_rejects_negative_optional_int_params(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 14,
                "method": "arborist/get_semantic_skeleton",
                "params": {"file_path": "sample.py", "depth_limit": -1},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 14)
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

    def test_rejects_null_string_param_with_default(self) -> None:
        class StubCore:
            def trace_symbol_graph_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 38,
                "method": "arborist/trace_symbol_graph",
                "params": {"workspace_root": None, "symbol_path": "top_level"},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 38)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("workspace_root", response["error"]["message"])

    def test_rejects_invalid_trace_direction_as_invalid_params(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 16,
                "method": "arborist/trace_symbol_graph",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "orchestrate",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 16)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_invalid_trace_context_direction_as_invalid_params(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 17,
                "method": "arborist/validate_patch_with_trace_context",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.c",
                    "semantic_path": "orchestrate",
                    "new_code": "int orchestrate(void) { return 0; }",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 17)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_core_invalid_query_maps_to_invalid_params(self) -> None:
        gateway = ArboristGateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 18,
                "method": "arborist/execute_tree_query",
                "params": {
                    "file_path": "tests/fixtures/sample.py",
                    "query": "(function_definition @",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 18)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("query", response["error"]["message"].lower())

    def test_rejects_reversed_buffer_edit_range(self) -> None:
        class StubCore:
            def apply_buffer_edit_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 19,
                "method": "arborist/apply_buffer_edit",
                "params": {
                    "file_path": "tests/fixtures/sample.py",
                    "start_byte": 10,
                    "old_end_byte": 2,
                    "new_text": "x",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 19)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("start_byte", response["error"]["message"])

    def test_rejects_negative_buffer_edit_offsets(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 27,
                "method": "arborist/apply_buffer_edit",
                "params": {
                    "file_path": "tests/fixtures/sample.py",
                    "start_byte": -1,
                    "old_end_byte": 2,
                    "new_text": "x",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 27)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("start_byte", response["error"]["message"])

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
                "id": 20,
                "method": "arborist/get_semantic_skeleton",
                "params": {"file_path": "sample.py"},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 20)
        self.assertEqual(response["result"], {})
        self.assertEqual(core.args, ("sample.py", None, 2, None))

    def test_allows_null_nullable_string_params(self) -> None:
        class StubCore:
            def patch_ast_node_json(
                self,
                file_path: str,
                semantic_path: str,
                new_code: str,
                source: str | None,
                bypass_reason: str | None,
            ) -> str:
                self.args = (file_path, semantic_path, new_code, source, bypass_reason)
                return "{}"

        core = StubCore()
        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = core

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 39,
                "method": "arborist/patch_ast_node",
                "params": {
                    "file_path": "sample.py",
                    "semantic_path": "top_level",
                    "new_code": "def top_level():\n    return 1\n",
                    "source": None,
                    "bypass_reason": None,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 39)
        self.assertEqual(response["result"], {})
        self.assertEqual(
            core.args,
            (
                "sample.py",
                "top_level",
                "def top_level():\n    return 1\n",
                None,
                None,
            ),
        )

    def test_rejects_blank_expand_node_selectors(self) -> None:
        class StubCore:
            def get_semantic_skeleton_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = StubCore()

        for selector in ("", " \t"):
            with self.subTest(selector=selector):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 36,
                        "method": "arborist/get_semantic_skeleton",
                        "params": {"file_path": "sample.py", "expand_nodes": [selector]},
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 36)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn("expand_nodes", response["error"]["message"])

    def test_passes_valid_position_edits_to_core(self) -> None:
        class StubCore:
            def apply_position_edits_json(self, file_path: str, edits_json: str) -> str:
                self.args = (file_path, edits_json)
                return "{}"

        core = StubCore()
        gateway = ArboristGateway.__new__(ArboristGateway)
        gateway._core = core

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 30,
                "method": "arborist/did_change",
                "params": {
                    "file_path": "sample.py",
                    "edits": [
                        {
                            "start": {"row": 0, "column": 0},
                            "end": {"row": 0, "column": 0},
                            "new_text": "x",
                        }
                    ],
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 30)
        self.assertEqual(response["result"], {})
        self.assertEqual(core.args[0], "sample.py")
        self.assertIn('"new_text": "x"', core.args[1])

    def test_rejects_non_json_serializable_patch_object_as_invalid_params(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 21,
                "method": "arborist/replay_patch_evidence_against_trace",
                "params": {
                    "patch": {"binding_decisions": {1, 2}},
                    "trace": {},
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 21)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("patch", response["error"]["message"])

    def test_rejects_non_finite_patch_object_as_invalid_params(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 26,
                "method": "arborist/replay_patch_evidence_against_trace",
                "params": {
                    "patch": {"confidence": float("inf")},
                    "trace": {},
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 26)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("patch", response["error"]["message"])

    def test_rejects_non_json_serializable_trace_object_as_invalid_params(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 22,
                "method": "arborist/validate_patch_commit_with_trace",
                "params": {
                    "patch": {},
                    "trace": {"callee_keys": {1, 2}},
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 22)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("trace", response["error"]["message"])

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

    def test_rejects_nonstandard_json_from_core(self) -> None:
        class StubCore:
            def list_symbol_indexes_json(self) -> str:
                return '[{"workspace_root": NaN}]'

        gateway = ArboristGateway()
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 34,
                "method": "arborist/list_symbol_indexes",
                "params": {},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 34)
        self.assertEqual(response["error"]["code"], -32000)
        self.assertIn("invalid JSON from arborist core", response["error"]["message"])
        self.assertIn("non-standard JSON constant", response["error"]["message"])

    def test_rejects_malformed_json_from_core(self) -> None:
        class StubCore:
            def list_symbol_indexes_json(self) -> str:
                return '[{"workspace_root": "."}'

        gateway = ArboristGateway()
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 35,
                "method": "arborist/list_symbol_indexes",
                "params": {},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 35)
        self.assertEqual(response["error"]["code"], -32000)
        self.assertIn("invalid JSON from arborist core", response["error"]["message"])

    def test_execute_tree_query_preserves_owner_metadata_from_core(self) -> None:
        gateway = ArboristGateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 23,
                "method": "arborist/execute_tree_query",
                "params": {
                    "file_path": "tests/fixtures/sample.py",
                    "source": "@logged\ndef top_level(value):\n    return value\n",
                    "query": "(decorator (identifier) @decorator)",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 23)
        self.assertEqual(len(response["result"]), 1)
        self.assertEqual(response["result"][0]["capture_name"], "decorator")
        self.assertEqual(response["result"][0]["text"], "logged")
        self.assertEqual(response["result"][0]["owner_symbol_id"], "top_level")
        self.assertEqual(response["result"][0]["owner_semantic_path"], "top_level")
        self.assertIsNone(response["result"][0]["owner_scope_path"])

    def test_gateway_initialization_loads_core_lazily(self) -> None:
        class StubCore:
            def __init__(self) -> None:
                self.loaded = True

        with mock.patch.object(gateway_module, "_load_core_class", return_value=StubCore) as loader:
            gateway = gateway_module.ArboristGateway()
            self.assertIsNone(gateway._core)
            loader.assert_not_called()
            self.assertIsInstance(gateway._require_core(), StubCore)
            loader.assert_called_once()

    def test_require_core_handles_new_only_gateway_instance(self) -> None:
        class StubCore:
            pass

        gateway = gateway_module.ArboristGateway.__new__(gateway_module.ArboristGateway)

        with mock.patch.object(gateway_module, "_load_core_class", return_value=StubCore):
            core = gateway._require_core()

        self.assertIsInstance(core, StubCore)
        self.assertIs(gateway._core, core)

    def test_initialize_reports_core_load_failure_as_jsonrpc_error(self) -> None:
        gateway = gateway_module.ArboristGateway()

        with mock.patch.object(gateway_module, "_load_core_class", side_effect=ImportError("boom")):
            response = gateway.handle_request(
                {"jsonrpc": "2.0", "id": 24, "method": "initialize", "params": {}}
            )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 24)
        self.assertEqual(response["error"]["code"], -32000)
        self.assertIn("failed to load arborist core", response["error"]["message"])

    def test_once_valid_request_with_core_load_failure_prints_error_response(self) -> None:
        with mock.patch.object(gateway_module, "_load_core_class", side_effect=ImportError("boom")):
            with mock.patch(
                "pathlib.Path.read_text",
                return_value='{"jsonrpc":"2.0","id":25,"method":"initialize","params":{}}',
            ):
                with mock.patch("builtins.print") as mock_print:
                    exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 0)
        mock_print.assert_called_once()
        response = gateway_module.json.loads(mock_print.call_args.args[0])
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 25)
        self.assertEqual(response["error"]["code"], -32000)
        self.assertIn("failed to load arborist core", response["error"]["message"])

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

    def test_stdio_invalid_json_emits_parse_error_response(self) -> None:
        stdin = io.StringIO("{bad json}\n")
        stdout = io.StringIO()

        with mock.patch.object(gateway_module.ArboristGateway, "__init__", return_value=None):
            with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)
        response = gateway_module.json.loads(stdout.getvalue())
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32700)
        self.assertIn("invalid JSON", response["error"]["message"])

    def test_stdio_invalid_no_id_request_emits_error_response(self) -> None:
        stdin = io.StringIO('{"method":"arborist/list_symbol_indexes","params":{}}\n')
        stdout = io.StringIO()

        with mock.patch.object(gateway_module.ArboristGateway, "__init__", return_value=None):
            with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)
        response = gateway_module.json.loads(stdout.getvalue())
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32600)
        self.assertIn("jsonrpc", response["error"]["message"])

    def test_stdio_invalid_json_does_not_initialize_gateway(self) -> None:
        stdin = io.StringIO("{bad json}\n")
        stdout = io.StringIO()

        with mock.patch.object(
            gateway_module.ArboristGateway,
            "__init__",
            side_effect=AssertionError("gateway should not initialize"),
        ):
            with mock.patch.object(
                gateway_module,
                "_load_core_class",
                side_effect=AssertionError("core loader should not run"),
            ):
                with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                    exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)
        response = gateway_module.json.loads(stdout.getvalue())
        self.assertEqual(response["error"]["code"], -32700)

    def test_once_invalid_json_prints_parse_error_response(self) -> None:
        with mock.patch.object(gateway_module.ArboristGateway, "__init__", return_value=None):
            with mock.patch("pathlib.Path.read_text", return_value="{bad json}"):
                with mock.patch("builtins.print") as mock_print:
                    exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 0)
        mock_print.assert_called_once()
        response = gateway_module.json.loads(mock_print.call_args.args[0])
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32700)
        self.assertIn("invalid JSON", response["error"]["message"])

    def test_once_invalid_no_id_request_prints_error_response(self) -> None:
        with mock.patch.object(gateway_module.ArboristGateway, "__init__", return_value=None):
            with mock.patch(
                "pathlib.Path.read_text",
                return_value='{"method":"arborist/list_symbol_indexes","params":{}}',
            ):
                with mock.patch("builtins.print") as mock_print:
                    exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 0)
        mock_print.assert_called_once()
        response = gateway_module.json.loads(mock_print.call_args.args[0])
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32600)
        self.assertIn("jsonrpc", response["error"]["message"])

    def test_once_invalid_json_does_not_initialize_gateway(self) -> None:
        with mock.patch.object(
            gateway_module.ArboristGateway,
            "__init__",
            side_effect=AssertionError("gateway should not initialize"),
        ):
            with mock.patch.object(
                gateway_module,
                "_load_core_class",
                side_effect=AssertionError("core loader should not run"),
            ):
                with mock.patch("pathlib.Path.read_text", return_value="{bad json}"):
                    with mock.patch("builtins.print") as mock_print:
                        exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 0)
        mock_print.assert_called_once()
        response = gateway_module.json.loads(mock_print.call_args.args[0])
        self.assertEqual(response["error"]["code"], -32700)

    def test_once_missing_request_file_reports_cli_error(self) -> None:
        stderr = io.StringIO()

        with mock.patch.object(
            gateway_module.ArboristGateway,
            "__init__",
            side_effect=AssertionError("gateway should not initialize"),
        ):
            with mock.patch.object(
                gateway_module,
                "_load_core_class",
                side_effect=AssertionError("core loader should not run"),
            ):
                with mock.patch(
                    "pathlib.Path.read_text",
                    side_effect=FileNotFoundError("missing request file"),
                ):
                    with mock.patch("sys.stderr", stderr):
                        exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 1)
        self.assertIn("failed to read request file", stderr.getvalue())
        self.assertIn("dummy.json", stderr.getvalue())

    def test_once_invalid_request_encoding_reports_cli_error(self) -> None:
        stderr = io.StringIO()

        with mock.patch(
            "pathlib.Path.read_text",
            side_effect=UnicodeDecodeError("utf-8", b"\xff", 0, 1, "invalid start byte"),
        ):
            with mock.patch("sys.stderr", stderr):
                exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 1)
        self.assertIn("failed to read request file", stderr.getvalue())
        self.assertIn("dummy.json", stderr.getvalue())

    def test_stdio_broken_pipe_exits_cleanly(self) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                self.request = request
                return {"jsonrpc": "2.0", "id": 1, "result": {"ok": True}}

        class BrokenStdout(io.StringIO):
            def write(self, text: str) -> int:
                raise BrokenPipeError("pipe closed")

        fake_gateway = StubGateway()
        stdin = io.StringIO('{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}\n')
        stdout = BrokenStdout()

        with mock.patch.object(gateway_module, "ArboristGateway", return_value=fake_gateway):
            with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)

    def test_stdio_nonstandard_response_value_emits_internal_error(self) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                self.request = request
                return {"jsonrpc": "2.0", "id": 31, "result": {"value": float("nan")}}

        fake_gateway = StubGateway()
        stdin = io.StringIO('{"jsonrpc":"2.0","id":31,"method":"initialize","params":{}}\n')
        stdout = io.StringIO()

        with mock.patch.object(gateway_module, "ArboristGateway", return_value=fake_gateway):
            with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)
        response = gateway_module.json.loads(stdout.getvalue())
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 31)
        self.assertEqual(response["error"]["code"], -32000)
        self.assertIn("failed to serialize response", response["error"]["message"])

    def test_stdio_unserializable_response_id_falls_back_to_null(self) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                self.request = request
                return {"jsonrpc": "2.0", "id": object(), "result": {"value": object()}}

        fake_gateway = StubGateway()
        stdin = io.StringIO('{"jsonrpc":"2.0","id":33,"method":"initialize","params":{}}\n')
        stdout = io.StringIO()

        with mock.patch.object(gateway_module, "ArboristGateway", return_value=fake_gateway):
            with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)
        response = gateway_module.json.loads(stdout.getvalue())
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32000)
        self.assertIn("failed to serialize response", response["error"]["message"])

    def test_once_broken_pipe_exits_cleanly(self) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                self.request = request
                return {"jsonrpc": "2.0", "id": 1, "result": {"ok": True}}

        fake_gateway = StubGateway()

        with mock.patch.object(gateway_module, "ArboristGateway", return_value=fake_gateway):
            with mock.patch(
                "pathlib.Path.read_text",
                return_value='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}',
            ):
                with mock.patch("builtins.print", side_effect=BrokenPipeError("pipe closed")):
                    exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 0)

    def test_once_nonstandard_response_value_prints_internal_error(self) -> None:
        class StubGateway:
            def handle_request(self, request: object) -> dict[str, object]:
                self.request = request
                return {"jsonrpc": "2.0", "id": 32, "result": {"value": float("inf")}}

        fake_gateway = StubGateway()

        with mock.patch.object(gateway_module, "ArboristGateway", return_value=fake_gateway):
            with mock.patch(
                "pathlib.Path.read_text",
                return_value='{"jsonrpc":"2.0","id":32,"method":"initialize","params":{}}',
            ):
                with mock.patch("builtins.print") as mock_print:
                    exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 0)
        mock_print.assert_called_once()
        response = gateway_module.json.loads(mock_print.call_args.args[0])
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 32)
        self.assertEqual(response["error"]["code"], -32000)
        self.assertIn("failed to serialize response", response["error"]["message"])

    def test_stdio_rejects_nan_as_parse_error(self) -> None:
        stdin = io.StringIO(
            '{"jsonrpc":"2.0","id":NaN,"method":"arborist/list_symbol_indexes","params":{}}\n'
        )
        stdout = io.StringIO()

        with mock.patch.object(gateway_module.ArboristGateway, "__init__", return_value=None):
            with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", stdout):
                exit_code = gateway_module.run_stdio()

        self.assertEqual(exit_code, 0)
        response = gateway_module.json.loads(stdout.getvalue())
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32700)
        self.assertIn("non-standard JSON constant", response["error"]["message"])

    def test_once_rejects_infinity_as_parse_error(self) -> None:
        with mock.patch.object(gateway_module.ArboristGateway, "__init__", return_value=None):
            with mock.patch(
                "pathlib.Path.read_text",
                return_value='{"jsonrpc":"2.0","id":Infinity,"method":"initialize","params":{}}',
            ):
                with mock.patch("builtins.print") as mock_print:
                    exit_code = gateway_module.main(["--once", "dummy.json"])

        self.assertEqual(exit_code, 0)
        mock_print.assert_called_once()
        response = gateway_module.json.loads(mock_print.call_args.args[0])
        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertIsNone(response["id"])
        self.assertEqual(response["error"]["code"], -32700)
        self.assertIn("non-standard JSON constant", response["error"]["message"])


if __name__ == "__main__":
    unittest.main()
