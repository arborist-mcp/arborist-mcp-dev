from __future__ import annotations

import importlib
import io
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
from unittest import mock

import arborist_mcp
from arborist_mcp import gateway as gateway_module
from arborist_mcp import _version as version_module
from arborist_mcp.gateway import ArboristGateway

from tests.gateway_protocol.helpers import (
    GatewayProtocolTestCase,
    make_recording_json_core,
)
from tests.gateway_protocol import (
    GROUP_MODULES,
    GROUP_SUITES,
    GROUPS,
    MANIFEST,
    SUITE_MODULES,
    SUITES,
    build_manifest_snapshot,
)

SUITE_NAME = "gateway-request-validation"
REQUIRES_EXTENSION = False
COVERED_TOOLS = (
    "arborist/apply_buffer_edit",
    "arborist/did_change",
    "arborist/did_close",
    "arborist/get_semantic_skeleton",
    "arborist/list_symbol_indexes",
    "arborist/list_symbols",
    "arborist/list_symbols_discovery_context",
    "arborist/list_symbols_neighborhood_context",
    "arborist/list_virtual_files",
    "arborist/patch_ast_node_at_position",
    "arborist/patch_virtual_ast_node_at_position",
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


class GatewayRequestValidationTests(GatewayProtocolTestCase):
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

    def test_gateway_reuses_package_version(self) -> None:
        self.assertEqual(gateway_module.__version__, arborist_mcp.__version__)
        self.assertEqual(gateway_module.__version__, version_module.__version__)

    def test_cli_version_reports_package_version(self) -> None:
        stdout = io.StringIO()

        with mock.patch("sys.stdout", stdout):
            with self.assertRaises(SystemExit) as context:
                gateway_module.main(["--version"])

        self.assertEqual(context.exception.code, 0)
        self.assertIn(gateway_module.__version__, stdout.getvalue())

    def test_advertised_tools_have_gateway_handlers(self) -> None:
        self.assertEqual(gateway_module.TOOL_NAMES, tuple(gateway_module.TOOL_HANDLERS))
        for handler_name in gateway_module.TOOL_HANDLERS.values():
            with self.subTest(handler_name=handler_name):
                self.assertTrue(callable(getattr(ArboristGateway, handler_name, None)))

    def test_advertised_tools_have_param_specs(self) -> None:
        self.assertEqual(
            set(gateway_module.TOOL_HANDLERS),
            set(gateway_module.TOOL_PARAM_NAMES),
        )

    def test_advertised_tool_params_have_schema_specs(self) -> None:
        expected_params = {
            param_name
            for param_names in gateway_module.TOOL_PARAM_NAMES.values()
            for param_name in param_names
        }

        self.assertEqual(expected_params, set(gateway_module.TOOL_PARAM_SCHEMAS))

    def test_generated_tool_catalog_matches_gateway_specs(self) -> None:
        catalog = gateway_module.build_tool_catalog()

        self.assertEqual(len(catalog), len(gateway_module.TOOL_NAMES))
        for tool in catalog:
            with self.subTest(tool=tool["name"]):
                tool_name = tool["name"]
                self.assertIn(tool_name, gateway_module.TOOL_HANDLERS)
                self.assertEqual(
                    tool["metadata"]["category"],
                    gateway_module.TOOL_CATEGORIES[tool_name],
                )
                self.assertEqual(tool["metadata"]["legacyMethod"], tool_name)
                self.assertEqual(
                    tool["metadata"]["mutatesState"],
                    tool_name in gateway_module.MUTATING_TOOLS,
                )
                self.assertEqual(
                    set(tool["inputSchema"]["properties"]),
                    set(gateway_module.TOOL_PARAM_NAMES[tool_name]),
                )
                self.assertEqual(
                    tool["inputSchema"]["required"],
                    list(gateway_module.required_tool_params(tool_name)),
                )
                self.assertEqual(tool["inputSchema"]["additionalProperties"], False)
                self.assertEqual(tool["outputSchema"]["required"], ["result"])
                expected_result_schema = gateway_module.TOOL_RESULT_SCHEMAS.get(
                    tool_name,
                    gateway_module.OBJECT_RESULT_SCHEMA,
                )
                self.assertEqual(
                    tool["outputSchema"]["properties"]["result"],
                    expected_result_schema,
                )

    def test_tool_catalog_script_and_snapshot_match_generated_catalog(self) -> None:
        repo_root = Path(__file__).resolve().parents[2]
        script_path = repo_root / "scripts" / "tool_catalog.py"
        snapshot_path = repo_root / "docs" / "tool-catalog.json"

        completed = subprocess.run(
            [sys.executable, str(script_path)],
            cwd=repo_root,
            check=True,
            capture_output=True,
            text=True,
        )

        generated = gateway_module.build_tool_catalog()
        self.assertEqual(json.loads(completed.stdout), generated)
        check_completed = subprocess.run(
            [sys.executable, str(script_path), "--check"],
            cwd=repo_root,
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertEqual(check_completed.stdout, "")
        self.assertEqual(
            json.loads(snapshot_path.read_text(encoding="utf-8")),
            generated,
        )

    def test_tool_catalog_script_reports_outdated_snapshot(self) -> None:
        repo_root = Path(__file__).resolve().parents[2]
        script_path = repo_root / "scripts" / "tool_catalog.py"

        with tempfile.TemporaryDirectory() as temp_dir:
            snapshot_path = Path(temp_dir) / "tool-catalog.json"
            snapshot_path.write_text("[]\n", encoding="utf-8", newline="\n")

            completed = subprocess.run(
                [
                    sys.executable,
                    str(script_path),
                    "--check",
                    "--snapshot",
                    str(snapshot_path),
                ],
                cwd=repo_root,
                capture_output=True,
                text=True,
            )

        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("out of date", completed.stderr)

    def test_readme_tool_counts_match_generated_catalog(self) -> None:
        readme = Path(__file__).resolve().parents[2].joinpath("README.md").read_text(
            encoding="utf-8"
        )

        total_match = re.search(r"returns (\d+) tools", readme)
        self.assertIsNotNone(total_match)
        assert total_match is not None
        self.assertEqual(int(total_match.group(1)), len(gateway_module.TOOL_NAMES))

        expected_counts = {
            "Read": 0,
            "Write": 0,
            "VFS": 0,
            "Index": 0,
            "Trace": 0,
        }
        for category in gateway_module.TOOL_CATEGORIES.values():
            expected_counts[category.upper() if category == "vfs" else category.title()] += 1

        for label, expected_count in expected_counts.items():
            count_match = re.search(rf"{label} tools: (\d+)", readme)
            self.assertIsNotNone(count_match, msg=f"README missing {label} tool count")
            assert count_match is not None
            self.assertEqual(int(count_match.group(1)), expected_count)

    def test_gateway_suite_metadata_covers_all_advertised_tools(self) -> None:
        suite_manifest = MANIFEST["suites"]
        assert isinstance(suite_manifest, dict)

        expected_tools = set(gateway_module.TOOL_HANDLERS)
        covered_tools: set[str] = set()

        for suite_name in suite_manifest:
            module = importlib.import_module(SUITE_MODULES[suite_name])
            self.assertEqual(module.SUITE_NAME, suite_name)
            self.assertEqual(
                module.REQUIRES_EXTENSION,
                suite_manifest[suite_name]["requires_extension"],
            )

            module_tools = set(module.COVERED_TOOLS)
            self.assertTrue(module_tools, msg=f"{suite_name} must declare covered tools")
            self.assertTrue(
                module_tools <= expected_tools,
                msg=f"{suite_name} declares unknown tools: {sorted(module_tools - expected_tools)}",
            )
            covered_tools.update(module_tools)

        self.assertEqual(
            covered_tools,
            expected_tools,
            msg=(
                "gateway suite tool coverage drifted; missing="
                f"{sorted(expected_tools - covered_tools)}, extra={sorted(covered_tools - expected_tools)}"
            ),
        )

    def test_gateway_groups_match_extension_requirements(self) -> None:
        suite_manifest = MANIFEST["suites"]
        assert isinstance(suite_manifest, dict)

        for suite_name in GROUP_SUITES["gateway-fast"]:
            with self.subTest(group="gateway-fast", suite=suite_name):
                self.assertFalse(suite_manifest[suite_name]["requires_extension"])

        for suite_name in GROUP_SUITES["gateway-native"]:
            with self.subTest(group="gateway-native", suite=suite_name):
                self.assertTrue(suite_manifest[suite_name]["requires_extension"])

    def test_gateway_manifest_snapshot_matches_loaded_metadata(self) -> None:
        snapshot = build_manifest_snapshot()
        self.assertEqual(snapshot["suites"], SUITES)

        snapshot_groups = snapshot["groups"]
        assert isinstance(snapshot_groups, dict)
        self.assertEqual(set(snapshot_groups), set(GROUPS))
        for group_name, metadata in snapshot_groups.items():
            with self.subTest(group=group_name):
                self.assertEqual(tuple(metadata["suite_names"]), GROUP_SUITES[group_name])
                self.assertEqual(tuple(metadata["module_names"]), GROUP_MODULES[group_name])
                self.assertEqual(metadata["requires_extension"], GROUPS[group_name]["requires_extension"])

    def test_gateway_manifest_cli_emits_normalized_metadata(self) -> None:
        repo_root = Path(__file__).resolve().parents[2]
        script_path = repo_root / "scripts" / "gateway_suite_manifest.py"
        completed = subprocess.run(
            [sys.executable, str(script_path)],
            cwd=repo_root,
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertEqual(json.loads(completed.stdout), build_manifest_snapshot())

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

    def test_rejects_non_json_serializable_edits_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

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
        gateway = self.make_gateway()

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
        gateway = self.make_gateway()

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
        gateway = self.make_gateway()

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

        gateway = self.make_gateway()
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

    def test_rejects_unknown_position_edit_fields(self) -> None:
        class StubCore:
            def apply_position_edits_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 40,
                "method": "arborist/did_change",
                "params": {
                    "file_path": "sample.py",
                    "edits": [
                        {
                            "start": {"row": 0, "column": 0},
                            "end": {"row": 0, "column": 0},
                            "new_text": "x",
                            "newText": "x",
                        }
                    ],
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 40)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("edits[0].newText", response["error"]["message"])

    def test_rejects_unknown_position_fields(self) -> None:
        class StubCore:
            def apply_position_edits_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 41,
                "method": "arborist/did_change",
                "params": {
                    "file_path": "sample.py",
                    "edits": [
                        {
                            "start": {"row": 0, "column": 0, "character": 0},
                            "end": {"row": 0, "column": 0},
                            "new_text": "x",
                        }
                    ],
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 41)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("edits[0].start.character", response["error"]["message"])

    def test_rejects_string_bool_params(self) -> None:
        gateway = self.make_gateway()

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
        gateway = self.make_gateway()

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

    def test_rejects_bool_int_params(self) -> None:
        class StubCore:
            def get_semantic_skeleton_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

            def apply_buffer_edit_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        cases = [
            (
                "arborist/get_semantic_skeleton",
                {"file_path": "sample.py", "depth_limit": True},
                "depth_limit",
            ),
            (
                "arborist/apply_buffer_edit",
                {
                    "file_path": "sample.py",
                    "start_byte": True,
                    "old_end_byte": 1,
                    "new_text": "x",
                },
                "start_byte",
            ),
        ]

        for method, params, expected_message in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 42,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 42)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn(expected_message, response["error"]["message"])

    def test_rejects_negative_optional_int_params(self) -> None:
        gateway = self.make_gateway()

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

    def test_rejects_negative_search_limit(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 55,
                "method": "arborist/search_symbols",
                "params": {"workspace_root": ".", "query": "helper", "limit": -1},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 55)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("limit", response["error"]["message"])

    def test_rejects_non_string_optional_params(self) -> None:
        gateway = self.make_gateway()

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

    def test_rejects_blank_required_string_params(self) -> None:
        class StubCore:
            def get_semantic_skeleton_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 39,
                "method": "arborist/get_semantic_skeleton",
                "params": {"file_path": "   "},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 39)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("file_path", response["error"]["message"])

    def test_rejects_blank_optional_string_params(self) -> None:
        class StubCore:
            def trace_symbol_graph_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 40,
                "method": "arborist/trace_symbol_graph",
                "params": {"workspace_root": "   ", "symbol_path": "top_level"},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 40)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("workspace_root", response["error"]["message"])

    def test_rejects_blank_search_query(self) -> None:
        class StubCore:
            def search_symbols_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 56,
                "method": "arborist/search_symbols",
                "params": {"workspace_root": ".", "query": "   "},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 56)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("query", response["error"]["message"])

    def test_rejects_blank_search_filters(self) -> None:
        class StubCore:
            def search_symbols_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 58,
                "method": "arborist/search_symbols",
                "params": {
                    "workspace_root": ".",
                    "query": "helper",
                    "file_path_contains": "   ",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 58)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("file_path_contains", response["error"]["message"])

    def test_rejects_blank_list_symbols_filters(self) -> None:
        class StubCore:
            def list_symbols_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 59,
                "method": "arborist/list_symbols",
                "params": {"workspace_root": ".", "node_kind": "   "},
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 59)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("node_kind", response["error"]["message"])

    def test_rejects_null_string_param_with_default(self) -> None:
        class StubCore:
            def trace_symbol_graph_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
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
        gateway = self.make_gateway()

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

    def test_rejects_invalid_trace_symbol_neighborhood_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 64,
                "method": "arborist/trace_symbol_neighborhood",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "orchestrate",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 64)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_invalid_trace_symbol_graph_at_position_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 97,
                "method": "arborist/trace_symbol_graph_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 97)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_negative_trace_symbol_neighborhood_limits(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 65,
                "method": "arborist/trace_symbol_neighborhood",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "orchestrate",
                    "max_depth": -1,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 65)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_depth", response["error"]["message"])

    def test_rejects_zero_trace_symbol_neighborhood_max_nodes(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 67,
                "method": "arborist/trace_symbol_neighborhood",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "orchestrate",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 67)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_trace_symbol_neighborhood_at_position_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 98,
                "method": "arborist/trace_symbol_neighborhood_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 98)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_trace_symbol_neighborhood_at_position_max_nodes(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 99,
                "method": "arborist/trace_symbol_neighborhood_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_b.py",
                    "position": {"row": 0, "column": 5},
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 99)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

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

    def test_rejects_invalid_read_symbol_at_position_position_as_invalid_params(self) -> None:
        class StubCore:
            def read_symbol_at_position_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        cases = [
            (
                "arborist/read_symbol_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": -1},
                },
                "position.column",
            ),
            (
                "arborist/read_symbol_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "character": 2},
                },
                "position.character",
            ),
            (
                "arborist/read_symbol_neighborhood_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": "1:2",
                },
                "position",
            ),
            (
                "arborist/read_symbol_discovery_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": True, "column": 2},
                },
                "position.row",
            ),
        ]

        for method, params, expected_key in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 86,
                        "method": method,
                        "params": params,
                    }
                )

                self.assertEqual(response["jsonrpc"], "2.0")
                self.assertEqual(response["id"], 86)
                self.assertEqual(response["error"]["code"], -32602)
                self.assertIn(expected_key, response["error"]["message"])

    def test_position_entrypoints_allow_source_with_index_db_path(self) -> None:
        source = "def helper() -> int:\n    return 1\n"
        core = make_recording_json_core(
            read_symbol_at_position_json={},
            read_symbol_context_at_position_json={},
            read_symbol_neighborhood_context_at_position_json={},
            read_symbol_discovery_context_at_position_json={},
            trace_symbol_graph_at_position_json={},
            trace_symbol_neighborhood_at_position_json={},
        )
        gateway = self.make_gateway(core)

        cases = [
            (
                "read_symbol_at_position_json",
                "arborist/read_symbol_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 2},
                    "source": source,
                    "index_db_path": "symbols.db",
                },
                (".", "graph_a.py", 1, 2, source, "symbols.db"),
            ),
            (
                "read_symbol_context_at_position_json",
                "arborist/read_symbol_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 2},
                    "direction": "callers",
                    "source": source,
                    "index_db_path": "symbols.db",
                },
                (".", "graph_a.py", 1, 2, "callers", source, "symbols.db"),
            ),
            (
                "read_symbol_neighborhood_context_at_position_json",
                "arborist/read_symbol_neighborhood_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 2},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "source": source,
                    "index_db_path": "symbols.db",
                },
                (".", "graph_a.py", 1, 2, "callers", 2, 10, source, "symbols.db"),
            ),
            (
                "read_symbol_discovery_context_at_position_json",
                "arborist/read_symbol_discovery_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 2},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "source": source,
                    "index_db_path": "symbols.db",
                },
                (".", "graph_a.py", 1, 2, "callers", 2, 10, source, "symbols.db"),
            ),
            (
                "trace_symbol_graph_at_position_json",
                "arborist/trace_symbol_graph_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 2},
                    "direction": "callers",
                    "source": source,
                    "index_db_path": "symbols.db",
                },
                (".", "graph_a.py", 1, 2, "callers", source, "symbols.db"),
            ),
            (
                "trace_symbol_neighborhood_at_position_json",
                "arborist/trace_symbol_neighborhood_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 2},
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "source": source,
                    "index_db_path": "symbols.db",
                },
                (".", "graph_a.py", 1, 2, "callers", 2, 10, source, "symbols.db"),
            ),
        ]

        for core_method, rpc_method, params, expected_call in cases:
            with self.subTest(method=rpc_method):
                result = self.assert_jsonrpc_ok(
                    self.call_gateway(gateway, rpc_method, params, request_id=111),
                    request_id=111,
                )
                self.assertEqual(result, {})
                self.assertEqual(core.calls_for(core_method), [expected_call])

    def test_rejects_invalid_patch_at_position_position_as_invalid_params(self) -> None:
        class StubCore:
            def patch_ast_node_at_position_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

            def patch_virtual_ast_node_at_position_json(self, *args: object) -> str:
                raise AssertionError("core should not be called")

            def validate_patch_with_trace_context_at_position_json(
                self, *args: object
            ) -> str:
                raise AssertionError("core should not be called")

            def validate_patch_with_graph_context_at_position_json(
                self, *args: object
            ) -> str:
                raise AssertionError("core should not be called")

            def validate_patch_with_neighborhood_context_at_position_json(
                self, *args: object
            ) -> str:
                raise AssertionError("core should not be called")

            def validate_patch_with_discovery_context_at_position_json(
                self, *args: object
            ) -> str:
                raise AssertionError("core should not be called")

        gateway = self.make_gateway()
        gateway._core = StubCore()

        cases = [
            (
                "arborist/patch_ast_node_at_position",
                {
                    "file_path": "sample.py",
                    "position": {"row": 1, "column": -1},
                    "new_code": "def helper() -> int:\n    return 2\n",
                },
                "position.column",
            ),
            (
                "arborist/patch_virtual_ast_node_at_position",
                {
                    "file_path": "sample.py",
                    "position": {"row": 1, "character": 2},
                    "new_code": "def helper() -> int:\n    return 2\n",
                },
                "position.character",
            ),
            (
                "arborist/validate_patch_with_trace_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "position": "1:2",
                    "new_code": "def helper() -> int:\n    return 2\n",
                },
                "position",
            ),
            (
                "arborist/validate_patch_with_graph_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "position": {"row": True, "column": 2},
                    "new_code": "def helper() -> int:\n    return 2\n",
                },
                "position.row",
            ),
            (
                "arborist/validate_patch_with_neighborhood_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "position": {"row": 1, "column": -1},
                    "new_code": "def helper() -> int:\n    return 2\n",
                },
                "position.column",
            ),
            (
                "arborist/validate_patch_with_discovery_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "position": {"row": 1, "character": 2},
                    "new_code": "def helper() -> int:\n    return 2\n",
                },
                "position.character",
            ),
        ]

        for method, params, expected_key in cases:
            with self.subTest(method=method):
                response = gateway.handle_request(
                    {
                        "jsonrpc": "2.0",
                        "id": 88,
                        "method": method,
                        "params": params,
                    }
                )

    def test_path_and_workspace_entrypoints_allow_source_with_index_db_path(self) -> None:
        source = "def helper() -> int:\n    return 1\n"
        core = make_recording_json_core(
            read_symbol_json={},
            read_symbol_context_json={},
            read_symbol_neighborhood_context_json={},
            read_symbol_discovery_context_json={},
            trace_symbol_graph_json={},
            trace_symbol_neighborhood_json={},
            search_symbols_json={},
            search_symbols_context_json={},
            search_symbols_neighborhood_context_json={},
            search_symbols_discovery_context_json={},
            list_symbols_json={},
            list_symbols_context_json={},
            list_symbols_neighborhood_context_json={},
            list_symbols_discovery_context_json={},
        )
        gateway = self.make_gateway(core)

        shared = {
            "workspace_root": ".",
            "file_path": "graph_a.py",
            "source": source,
            "index_db_path": "symbols.db",
        }
        cases = [
            (
                "read_symbol_json",
                "arborist/read_symbol",
                {**shared, "symbol_path": "helper"},
                (".", "helper", "symbols.db", "graph_a.py", source),
            ),
            (
                "read_symbol_context_json",
                "arborist/read_symbol_context",
                {**shared, "symbol_path": "helper", "direction": "callers"},
                (".", "helper", "callers", "symbols.db", "graph_a.py", source),
            ),
            (
                "read_symbol_neighborhood_context_json",
                "arborist/read_symbol_neighborhood_context",
                {
                    **shared,
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
                (
                    ".",
                    "helper",
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    "graph_a.py",
                    source,
                ),
            ),
            (
                "read_symbol_discovery_context_json",
                "arborist/read_symbol_discovery_context",
                {
                    **shared,
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
                (
                    ".",
                    "helper",
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    "graph_a.py",
                    source,
                ),
            ),
            (
                "trace_symbol_graph_json",
                "arborist/trace_symbol_graph",
                {**shared, "symbol_path": "helper", "direction": "callers"},
                (".", "helper", "callers", "symbols.db", "graph_a.py", source),
            ),
            (
                "trace_symbol_neighborhood_json",
                "arborist/trace_symbol_neighborhood",
                {
                    **shared,
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
                (
                    ".",
                    "helper",
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    "graph_a.py",
                    source,
                ),
            ),
            (
                "search_symbols_json",
                "arborist/search_symbols",
                {**shared, "query": "helper", "limit": 5},
                (".", "helper", 5, "symbols.db", None, None, "graph_a.py", source),
            ),
            (
                "search_symbols_context_json",
                "arborist/search_symbols_context",
                {**shared, "query": "helper", "limit": 5},
                (".", "helper", 5, "symbols.db", None, None, "graph_a.py", source),
            ),
            (
                "search_symbols_neighborhood_context_json",
                "arborist/search_symbols_neighborhood_context",
                {
                    **shared,
                    "query": "helper",
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
                (
                    ".",
                    "helper",
                    5,
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    None,
                    None,
                    "graph_a.py",
                    source,
                ),
            ),
            (
                "search_symbols_discovery_context_json",
                "arborist/search_symbols_discovery_context",
                {
                    **shared,
                    "query": "helper",
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
                (
                    ".",
                    "helper",
                    5,
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    None,
                    None,
                    "graph_a.py",
                    source,
                ),
            ),
            (
                "list_symbols_json",
                "arborist/list_symbols",
                {**shared, "limit": 5},
                (".", 5, "symbols.db", None, None, "graph_a.py", source),
            ),
            (
                "list_symbols_context_json",
                "arborist/list_symbols_context",
                {**shared, "limit": 5},
                (".", 5, "symbols.db", None, None, "graph_a.py", source),
            ),
            (
                "list_symbols_neighborhood_context_json",
                "arborist/list_symbols_neighborhood_context",
                {
                    **shared,
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
                (
                    ".",
                    5,
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    None,
                    None,
                    "graph_a.py",
                    source,
                ),
            ),
            (
                "list_symbols_discovery_context_json",
                "arborist/list_symbols_discovery_context",
                {
                    **shared,
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
                (
                    ".",
                    5,
                    "callers",
                    2,
                    10,
                    "symbols.db",
                    None,
                    None,
                    "graph_a.py",
                    source,
                ),
            ),
        ]

        for core_method, rpc_method, params, expected_call in cases:
            with self.subTest(method=rpc_method):
                result = self.assert_jsonrpc_ok(
                    self.call_gateway(gateway, rpc_method, params, request_id=112),
                    request_id=112,
                )
                self.assertEqual(result, {})
                self.assertEqual(core.calls_for(core_method), [expected_call])

    def test_patch_context_entrypoints_allow_source_with_index_db_path(self) -> None:
        source = "def orchestrate(value: int) -> int:\n    return value + 1\n"
        new_code = "def orchestrate(value: int) -> int:\n    return helper(value)\n"
        core = make_recording_json_core(
            validate_patch_with_trace_context_json={},
            validate_patch_with_trace_context_at_position_json={},
            validate_patch_with_graph_context_json={},
            validate_patch_with_graph_context_at_position_json={},
            validate_patch_with_neighborhood_context_json={},
            validate_patch_with_neighborhood_context_at_position_json={},
            validate_patch_with_discovery_context_json={},
            validate_patch_with_discovery_context_at_position_json={},
        )
        gateway = self.make_gateway(core)

        cases = [
            (
                "validate_patch_with_trace_context_json",
                "arborist/validate_patch_with_trace_context",
                {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "semantic_path": "orchestrate",
                    "new_code": new_code,
                    "source": source,
                    "direction": "both",
                    "index_db_path": "symbols.db",
                },
                (".", "caller.py", "orchestrate", new_code, source, None, "both", "symbols.db"),
            ),
            (
                "validate_patch_with_trace_context_at_position_json",
                "arborist/validate_patch_with_trace_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "position": {"row": 1, "column": 4},
                    "new_code": new_code,
                    "source": source,
                    "direction": "both",
                    "index_db_path": "symbols.db",
                },
                (".", "caller.py", 1, 4, new_code, source, None, "both", "symbols.db"),
            ),
            (
                "validate_patch_with_graph_context_json",
                "arborist/validate_patch_with_graph_context",
                {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "semantic_path": "orchestrate",
                    "new_code": new_code,
                    "source": source,
                    "direction": "both",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                (
                    ".",
                    "caller.py",
                    "orchestrate",
                    new_code,
                    source,
                    None,
                    "both",
                    2,
                    10,
                    "symbols.db",
                ),
            ),
            (
                "validate_patch_with_graph_context_at_position_json",
                "arborist/validate_patch_with_graph_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "position": {"row": 1, "column": 4},
                    "new_code": new_code,
                    "source": source,
                    "direction": "both",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                (".", "caller.py", 1, 4, new_code, source, None, "both", 2, 10, "symbols.db"),
            ),
            (
                "validate_patch_with_neighborhood_context_json",
                "arborist/validate_patch_with_neighborhood_context",
                {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "semantic_path": "orchestrate",
                    "new_code": new_code,
                    "source": source,
                    "direction": "both",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                (
                    ".",
                    "caller.py",
                    "orchestrate",
                    new_code,
                    source,
                    None,
                    "both",
                    2,
                    10,
                    "symbols.db",
                ),
            ),
            (
                "validate_patch_with_neighborhood_context_at_position_json",
                "arborist/validate_patch_with_neighborhood_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "position": {"row": 1, "column": 4},
                    "new_code": new_code,
                    "source": source,
                    "direction": "both",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                (".", "caller.py", 1, 4, new_code, source, None, "both", 2, 10, "symbols.db"),
            ),
            (
                "validate_patch_with_discovery_context_json",
                "arborist/validate_patch_with_discovery_context",
                {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "semantic_path": "orchestrate",
                    "new_code": new_code,
                    "source": source,
                    "direction": "both",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                (
                    ".",
                    "caller.py",
                    "orchestrate",
                    new_code,
                    source,
                    None,
                    "both",
                    2,
                    10,
                    "symbols.db",
                ),
            ),
            (
                "validate_patch_with_discovery_context_at_position_json",
                "arborist/validate_patch_with_discovery_context_at_position",
                {
                    "workspace_root": ".",
                    "file_path": "caller.py",
                    "position": {"row": 1, "column": 4},
                    "new_code": new_code,
                    "source": source,
                    "direction": "both",
                    "max_depth": 2,
                    "max_nodes": 10,
                    "index_db_path": "symbols.db",
                },
                (".", "caller.py", 1, 4, new_code, source, None, "both", 2, 10, "symbols.db"),
            ),
        ]

        for core_method, rpc_method, params, expected_call in cases:
            with self.subTest(method=rpc_method):
                result = self.assert_jsonrpc_ok(
                    self.call_gateway(gateway, rpc_method, params, request_id=113),
                    request_id=113,
                )
                self.assertEqual(result, {})
                self.assertEqual(core.calls_for(core_method), [expected_call])

    def test_rejects_missing_file_path_for_source_backed_path_and_workspace_entrypoints(self) -> None:
        class StubCore:
            def __getattr__(self, name: str):
                if name.endswith("_json"):
                    def _unexpected(*args: object) -> str:
                        raise AssertionError(f"core should not be called: {name}")

                    return _unexpected
                raise AttributeError(name)

        gateway = self.make_gateway()
        gateway._core = StubCore()

        shared = {
            "workspace_root": ".",
            "source": "def helper() -> int:\n    return 1\n",
        }
        cases = [
            ("arborist/read_symbol", {**shared, "symbol_path": "helper"}),
            (
                "arborist/read_symbol_context",
                {**shared, "symbol_path": "helper", "direction": "callers"},
            ),
            (
                "arborist/read_symbol_neighborhood_context",
                {
                    **shared,
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            ),
            (
                "arborist/read_symbol_discovery_context",
                {
                    **shared,
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            ),
            (
                "arborist/trace_symbol_graph",
                {**shared, "symbol_path": "helper", "direction": "callers"},
            ),
            (
                "arborist/trace_symbol_neighborhood",
                {
                    **shared,
                    "symbol_path": "helper",
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            ),
            ("arborist/search_symbols", {**shared, "query": "helper", "limit": 5}),
            (
                "arborist/search_symbols_context",
                {**shared, "query": "helper", "limit": 5},
            ),
            (
                "arborist/search_symbols_neighborhood_context",
                {
                    **shared,
                    "query": "helper",
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            ),
            (
                "arborist/search_symbols_discovery_context",
                {
                    **shared,
                    "query": "helper",
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            ),
            ("arborist/list_symbols", {**shared, "limit": 5}),
            ("arborist/list_symbols_context", {**shared, "limit": 5}),
            (
                "arborist/list_symbols_neighborhood_context",
                {
                    **shared,
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            ),
            (
                "arborist/list_symbols_discovery_context",
                {
                    **shared,
                    "limit": 5,
                    "direction": "callers",
                    "max_depth": 2,
                    "max_nodes": 10,
                },
            ),
        ]

        for method, params in cases:
            with self.subTest(method=method):
                self.assert_invalid_params(
                    method,
                    params,
                    request_id=113,
                    contains="file_path is required when source is provided",
                    gateway=gateway,
                )

    def test_rejects_invalid_read_symbol_context_at_position_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 87,
                "method": "arborist/read_symbol_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 4},
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 87)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_invalid_read_symbol_neighborhood_context_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 70,
                "method": "arborist/read_symbol_neighborhood_context",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "orchestrate",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 70)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_read_symbol_neighborhood_context_max_nodes(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 71,
                "method": "arborist/read_symbol_neighborhood_context",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "orchestrate",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 71)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_read_symbol_neighborhood_context_at_position_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 88,
                "method": "arborist/read_symbol_neighborhood_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 4},
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 88)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_read_symbol_neighborhood_context_at_position_max_nodes(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 89,
                "method": "arborist/read_symbol_neighborhood_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 4},
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 89)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_read_symbol_discovery_context_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 72,
                "method": "arborist/read_symbol_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "orchestrate",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 72)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_read_symbol_discovery_context_max_nodes(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 73,
                "method": "arborist/read_symbol_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "symbol_path": "orchestrate",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 73)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_read_symbol_discovery_context_at_position_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 90,
                "method": "arborist/read_symbol_discovery_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 4},
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 90)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_read_symbol_discovery_context_at_position_max_nodes(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 91,
                "method": "arborist/read_symbol_discovery_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "graph_a.py",
                    "position": {"row": 1, "column": 4},
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 91)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_list_symbols_neighborhood_context_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 79,
                "method": "arborist/list_symbols_neighborhood_context",
                "params": {
                    "workspace_root": ".",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 79)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_list_symbols_neighborhood_context_max_nodes(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 80,
                "method": "arborist/list_symbols_neighborhood_context",
                "params": {
                    "workspace_root": ".",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 80)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_search_symbols_discovery_context_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 82,
                "method": "arborist/search_symbols_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "query": "helper",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 82)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_search_symbols_discovery_context_max_nodes(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 83,
                "method": "arborist/search_symbols_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "query": "helper",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 83)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_list_symbols_discovery_context_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 84,
                "method": "arborist/list_symbols_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 84)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_list_symbols_discovery_context_max_nodes(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 85,
                "method": "arborist/list_symbols_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 85)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_trace_context_direction_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

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

    def test_rejects_invalid_trace_context_at_position_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 89,
                "method": "arborist/validate_patch_with_trace_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.c",
                    "position": {"row": 0, "column": 4},
                    "new_code": "int orchestrate(void) { return 0; }",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 89)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_invalid_graph_context_direction_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 68,
                "method": "arborist/validate_patch_with_graph_context",
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
        self.assertEqual(response["id"], 68)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_invalid_graph_context_at_position_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 90,
                "method": "arborist/validate_patch_with_graph_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.c",
                    "position": {"row": 0, "column": 4},
                    "new_code": "int orchestrate(void) { return 0; }",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 90)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_graph_context_max_nodes_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 69,
                "method": "arborist/validate_patch_with_graph_context",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "semantic_path": "orchestrate",
                    "new_code": "def orchestrate() -> int:\n    return 1\n",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 69)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_zero_graph_context_at_position_max_nodes_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 91,
                "method": "arborist/validate_patch_with_graph_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "position": {"row": 0, "column": 4},
                    "new_code": "def orchestrate() -> int:\n    return 1\n",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 91)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_neighborhood_context_direction_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 73,
                "method": "arborist/validate_patch_with_neighborhood_context",
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
        self.assertEqual(response["id"], 73)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_invalid_neighborhood_context_at_position_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 92,
                "method": "arborist/validate_patch_with_neighborhood_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.c",
                    "position": {"row": 0, "column": 4},
                    "new_code": "int orchestrate(void) { return 0; }",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 92)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_neighborhood_context_max_nodes_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 74,
                "method": "arborist/validate_patch_with_neighborhood_context",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "semantic_path": "orchestrate",
                    "new_code": "def orchestrate() -> int:\n    return 1\n",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 74)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_zero_neighborhood_context_at_position_max_nodes_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 93,
                "method": "arborist/validate_patch_with_neighborhood_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "position": {"row": 0, "column": 4},
                    "new_code": "def orchestrate() -> int:\n    return 1\n",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 93)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_invalid_discovery_context_direction_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 77,
                "method": "arborist/validate_patch_with_discovery_context",
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
        self.assertEqual(response["id"], 77)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_invalid_discovery_context_at_position_direction_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 94,
                "method": "arborist/validate_patch_with_discovery_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.c",
                    "position": {"row": 0, "column": 4},
                    "new_code": "int orchestrate(void) { return 0; }",
                    "direction": "sideways",
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 94)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("direction", response["error"]["message"])

    def test_rejects_zero_discovery_context_max_nodes_as_invalid_params(self) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 78,
                "method": "arborist/validate_patch_with_discovery_context",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "semantic_path": "orchestrate",
                    "new_code": "def orchestrate() -> int:\n    return 1\n",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 78)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

    def test_rejects_zero_discovery_context_at_position_max_nodes_as_invalid_params(
        self,
    ) -> None:
        gateway = self.make_gateway()

        response = gateway.handle_request(
            {
                "jsonrpc": "2.0",
                "id": 95,
                "method": "arborist/validate_patch_with_discovery_context_at_position",
                "params": {
                    "workspace_root": ".",
                    "file_path": "sample.py",
                    "position": {"row": 0, "column": 4},
                    "new_code": "def orchestrate() -> int:\n    return 1\n",
                    "max_nodes": 0,
                },
            }
        )

        self.assertEqual(response["jsonrpc"], "2.0")
        self.assertEqual(response["id"], 95)
        self.assertEqual(response["error"]["code"], -32602)
        self.assertIn("max_nodes", response["error"]["message"])

