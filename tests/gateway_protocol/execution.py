from __future__ import annotations

import io
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import arborist_mcp
from arborist_mcp import gateway as gateway_module
from arborist_mcp import _version as version_module
from arborist_mcp.gateway import ArboristGateway


class GatewayExecutionTests(unittest.TestCase):
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

    def test_get_semantic_skeleton_accepts_unsaved_source(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            file_path = Path(temp_dir).joinpath("sample.py")
            gateway = ArboristGateway()

            response = gateway.handle_request(
                {
                    "jsonrpc": "2.0",
                    "id": 45,
                    "method": "arborist/get_semantic_skeleton",
                    "params": {
                        "file_path": str(file_path),
                        "source": "def top_level() -> int:\n    return 1\n",
                        "depth_limit": 2,
                    },
                }
            )

            self.assertEqual(response["jsonrpc"], "2.0")
            self.assertEqual(response["id"], 45)
            self.assertNotIn("error", response)
            self.assertFalse(file_path.exists())
            self.assertIn("top_level", response["result"]["available_paths"])
            self.assertTrue(
                any(
                    symbol["semantic_path"] == "top_level"
                    for symbol in response["result"]["available_symbols"]
                )
            )

    def test_source_backed_requests_return_normalized_file_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            nested = Path(temp_dir).joinpath("child")
            nested.mkdir()
            file_path = Path(temp_dir).joinpath("sample.py")
            alias_path = nested.joinpath("..", "sample.py")
            expected_file = str(file_path).replace("\\", "/")
            gateway = ArboristGateway()

            skeleton_response = gateway.handle_request(
                {
                    "jsonrpc": "2.0",
                    "id": 46,
                    "method": "arborist/get_semantic_skeleton",
                    "params": {
                        "file_path": str(alias_path),
                        "source": "def top_level() -> int:\n    return 1\n",
                    },
                }
            )
            patch_response = gateway.handle_request(
                {
                    "jsonrpc": "2.0",
                    "id": 47,
                    "method": "arborist/patch_ast_node",
                    "params": {
                        "file_path": str(alias_path),
                        "source": "def top_level() -> int:\n    return 1\n",
                        "semantic_path": "top_level",
                        "new_code": "def top_level() -> int:\n    return 2\n",
                    },
                }
            )

            self.assertEqual(skeleton_response["jsonrpc"], "2.0")
            self.assertEqual(skeleton_response["id"], 46)
            self.assertNotIn("error", skeleton_response)
            self.assertEqual(skeleton_response["result"]["file"], expected_file)
            self.assertEqual(patch_response["jsonrpc"], "2.0")
            self.assertEqual(patch_response["id"], 47)
            self.assertNotIn("error", patch_response)
            self.assertEqual(patch_response["result"]["file"], expected_file)
            self.assertFalse(file_path.exists())

    def test_execute_tree_query_source_returns_normalized_c_owner_path(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            nested = Path(temp_dir).joinpath("child")
            nested.mkdir()
            file_path = Path(temp_dir).joinpath("sample.c")
            alias_path = nested.joinpath("..", "sample.c")
            expected_file = str(file_path).replace("\\", "/")
            gateway = ArboristGateway()

            response = gateway.handle_request(
                {
                    "jsonrpc": "2.0",
                    "id": 48,
                    "method": "arborist/execute_tree_query",
                    "params": {
                        "file_path": str(alias_path),
                        "source": "static int orchestrate(int value) { return value + 1; }\n",
                        "query": (
                            "(function_definition declarator: "
                            "(function_declarator declarator: (identifier) @name))"
                        ),
                    },
                }
            )

            self.assertEqual(response["jsonrpc"], "2.0")
            self.assertEqual(response["id"], 48)
            self.assertNotIn("error", response)
            self.assertEqual(len(response["result"]), 1)
            self.assertEqual(
                response["result"][0]["owner_symbol_id"],
                f"{expected_file}::orchestrate",
            )
            self.assertFalse(file_path.exists())

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

    def test_patch_ast_node_accepts_unsaved_source_without_writing_disk(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            file_path = Path(temp_dir).joinpath("sample.py")
            gateway = ArboristGateway()

            response = gateway.handle_request(
                {
                    "jsonrpc": "2.0",
                    "id": 44,
                    "method": "arborist/patch_ast_node",
                    "params": {
                        "file_path": str(file_path),
                        "source": "def top_level() -> int:\n    return 1\n",
                        "semantic_path": "top_level",
                        "new_code": "def top_level() -> int:\n    return 2\n",
                    },
                }
            )

            self.assertEqual(response["jsonrpc"], "2.0")
            self.assertEqual(response["id"], 44)
            self.assertNotIn("error", response)
            self.assertFalse(file_path.exists())
            self.assertTrue(response["result"]["applied"])
            self.assertIn("return 2", response["result"]["updated_source"])

    def test_patch_ast_node_at_position_accepts_unsaved_source_without_writing_disk(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            file_path = Path(temp_dir).joinpath("sample.py")
            gateway = ArboristGateway()

            response = gateway.handle_request(
                {
                    "jsonrpc": "2.0",
                    "id": 102,
                    "method": "arborist/patch_ast_node_at_position",
                    "params": {
                        "file_path": str(file_path),
                        "source": (
                            "def decorator(func):\n"
                            "    return func\n\n"
                            "@decorator\n"
                            "def helper() -> int:\n"
                            "    return 1\n"
                        ),
                        "position": {"row": 3, "column": 1},
                        "new_code": "def helper() -> int:\n    return 2\n",
                    },
                }
            )

            self.assertEqual(response["jsonrpc"], "2.0")
            self.assertEqual(response["id"], 102)
            self.assertNotIn("error", response)
            self.assertFalse(file_path.exists())
            self.assertTrue(response["result"]["applied"])
            self.assertEqual(response["result"]["resolved_path"], "helper")
            self.assertIn("return 2", response["result"]["updated_source"])

    def test_trace_context_at_position_accepts_unsaved_source(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            workspace = Path(temp_dir)
            helper = workspace.joinpath("helper.py")
            caller = workspace.joinpath("caller.py")
            helper.write_text(
                "def helper(value: int) -> int:\n    return value + 1\n",
                encoding="utf-8",
            )

            gateway = ArboristGateway()
            response = gateway.handle_request(
                {
                    "jsonrpc": "2.0",
                    "id": 103,
                    "method": "arborist/validate_patch_with_trace_context_at_position",
                    "params": {
                        "workspace_root": str(workspace),
                        "file_path": str(caller),
                        "source": (
                            "from helper import helper\n\n\n"
                            "def orchestrate(value: int) -> int:\n"
                            "    return value + 1\n"
                        ),
                        "position": {"row": 3, "column": 5},
                        "new_code": (
                            "def orchestrate(value: int) -> int:\n"
                            "    return helper(value)\n"
                        ),
                        "direction": "both",
                    },
                }
            )

            self.assertEqual(response["jsonrpc"], "2.0")
            self.assertEqual(response["id"], 103)
            self.assertNotIn("error", response)
            self.assertFalse(caller.exists())
            self.assertTrue(response["result"]["patch"]["applied"])
            self.assertEqual(response["result"]["patch"]["resolved_path"], "orchestrate")
            self.assertEqual(response["result"]["trace_target"], "orchestrate")
            self.assertIsNone(response["result"]["trace_error"])
            self.assertTrue(response["result"]["trace_validation"]["allowed"])
            self.assertTrue(
                any(
                    symbol["semantic_path"] == "helper"
                    for symbol in response["result"]["trace"]["callees"]
                )
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

    def test_rejects_non_string_patch_object_keys_as_invalid_params(self) -> None:
        gateway = ArboristGateway.__new__(ArboristGateway)

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 50,
                "method": "arborist/replay_patch_evidence_against_trace",
                "params": {
                    "patch": {"file": "sample.py", 1: "coerces-to-string"},
                    "trace": {},
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 50)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("patch", response["error"]["message"])
