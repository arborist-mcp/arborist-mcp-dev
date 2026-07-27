from __future__ import annotations

import ast
from pathlib import Path
import tempfile
import unittest

from arborist_mcp import gateway as gateway_module


_REPO_ROOT = Path(__file__).resolve().parents[2]
_GATEWAY_PATH = _REPO_ROOT / "python" / "arborist_mcp" / "gateway.py"

SUITE_NAME = "gateway-native-bindings"
REQUIRES_EXTENSION = True
COVERED_TOOLS = tuple(gateway_module.TOOL_NAMES)


def _is_core_receiver(node: ast.expr) -> bool:
    if isinstance(node, ast.Name):
        return node.id == "core"

    return (
        isinstance(node, ast.Call)
        and isinstance(node.func, ast.Attribute)
        and node.func.attr == "_require_core"
        and isinstance(node.func.value, ast.Name)
        and node.func.value.id == "self"
    )


def _gateway_core_method_names() -> set[str]:
    tree = ast.parse(_GATEWAY_PATH.read_text(encoding="utf-8"))
    method_names: set[str] = set()

    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue

        if isinstance(node.func, ast.Attribute) and _is_core_receiver(node.func.value):
            method_names.add(node.func.attr)
            continue

        if (
            isinstance(node.func, ast.Name)
            and node.func.id == "getattr"
            and len(node.args) >= 2
            and _is_core_receiver(node.args[0])
            and isinstance(node.args[1], ast.Constant)
            and isinstance(node.args[1].value, str)
        ):
            method_names.add(node.args[1].value)

    return method_names


class NativeBindingsTests(unittest.TestCase):
    def test_gateway_core_methods_are_registered_by_native_extension(self) -> None:
        from arborist_mcp._arborist_core import ArboristCore

        missing = sorted(
            method_name
            for method_name in _gateway_core_method_names()
            if not hasattr(ArboristCore, method_name)
        )

        self.assertFalse(missing, f"native extension is missing gateway methods: {missing}")

    def test_semantic_skeleton_timeout_reaches_native_parameter(self) -> None:
        gateway = gateway_module.ArboristGateway()
        result = gateway._get_semantic_skeleton(
            {
                "file_path": "unsaved.py",
                "source": "def sample():\n    return 1\n",
                "timeout_ms": 300000,
            }
        )

        self.assertIn("sample", result["available_paths"])

    def test_workspace_edit_preview_timeout_reaches_native_parameter(self) -> None:
        gateway = gateway_module.ArboristGateway()
        result = gateway._preview_workspace_position_edits(
            {
                "files": [
                    {
                        "file_path": "unsaved.py",
                        "source": "value = 1\n",
                        "edits": [],
                    }
                ],
                "timeout_ms": 300000,
            }
        )

        self.assertFalse(result["changed"])
        self.assertEqual(result["files"][0]["source"], "value = 1\n")

    def test_trace_context_patch_timeouts_reach_native_timeout_parameter(self) -> None:
        gateway = gateway_module.ArboristGateway()
        source = "def target() -> int:\n    return 1\n"
        replacement = "def target() -> int:\n    return 2\n"
        with tempfile.TemporaryDirectory() as temp_dir:
            index_db_path = str(Path(temp_dir) / "missing.db")
            cases = (
                (
                    gateway._validate_patch_with_trace_context,
                    {"semantic_path": "target"},
                ),
                (
                    gateway._validate_patch_with_trace_context_at_position,
                    {"position": {"row": 0, "column": 5}},
                ),
            )

            for handler, target_params in cases:
                with self.subTest(handler=handler.__name__):
                    params = {
                        "workspace_root": temp_dir,
                        "file_path": str(Path(temp_dir) / "target.py"),
                        "new_code": replacement,
                        "source": source,
                        "index_db_path": index_db_path,
                        "timeout_ms": 300000,
                        **target_params,
                    }
                    with self.assertRaisesRegex(ValueError, "does not exist"):
                        handler(params)

    def test_graph_context_patch_timeouts_reach_native_timeout_parameter(self) -> None:
        gateway = gateway_module.ArboristGateway()
        source = "def target() -> int:\n    return 1\n"
        replacement = "def target() -> int:\n    return 2\n"
        with tempfile.TemporaryDirectory() as temp_dir:
            index_db_path = str(Path(temp_dir) / "missing.db")
            cases = (
                (
                    gateway._validate_patch_with_graph_context,
                    {"semantic_path": "target"},
                ),
                (
                    gateway._validate_patch_with_graph_context_at_position,
                    {"position": {"row": 0, "column": 5}},
                ),
            )

            for handler, target_params in cases:
                with self.subTest(handler=handler.__name__):
                    params = {
                        "workspace_root": temp_dir,
                        "file_path": str(Path(temp_dir) / "target.py"),
                        "new_code": replacement,
                        "source": source,
                        "index_db_path": index_db_path,
                        "timeout_ms": 300000,
                        **target_params,
                    }
                    with self.assertRaisesRegex(ValueError, "does not exist"):
                        handler(params)

    def test_rich_context_patch_timeouts_reach_native_timeout_parameter(self) -> None:
        gateway = gateway_module.ArboristGateway()
        source = "def target() -> int:\n    return 1\n"
        replacement = "def target() -> int:\n    return 2\n"
        with tempfile.TemporaryDirectory() as temp_dir:
            index_db_path = str(Path(temp_dir) / "missing.db")
            cases = (
                (
                    gateway._validate_patch_with_neighborhood_context,
                    {"semantic_path": "target"},
                ),
                (
                    gateway._validate_patch_with_neighborhood_context_at_position,
                    {"position": {"row": 0, "column": 5}},
                ),
                (
                    gateway._validate_patch_with_discovery_context,
                    {"semantic_path": "target"},
                ),
                (
                    gateway._validate_patch_with_discovery_context_at_position,
                    {"position": {"row": 0, "column": 5}},
                ),
            )

            for handler, target_params in cases:
                with self.subTest(handler=handler.__name__):
                    params = {
                        "workspace_root": temp_dir,
                        "file_path": str(Path(temp_dir) / "target.py"),
                        "new_code": replacement,
                        "source": source,
                        "index_db_path": index_db_path,
                        "timeout_ms": 300000,
                        **target_params,
                    }
                    with self.assertRaisesRegex(ValueError, "does not exist"):
                        handler(params)

    def test_direct_read_timeouts_reach_native_timeout_parameter(self) -> None:
        gateway = gateway_module.ArboristGateway()
        with tempfile.TemporaryDirectory() as temp_dir:
            index_db_path = str(Path(temp_dir) / "missing.db")
            cases = (
                (gateway._read_symbol, {"symbol_path": "missing"}),
                (
                    gateway._read_symbol_at_position,
                    {"file_path": "missing.py", "position": {"row": 0, "column": 0}},
                ),
                (gateway._read_symbol_context, {"symbol_path": "missing"}),
                (
                    gateway._read_symbol_context_at_position,
                    {"file_path": "missing.py", "position": {"row": 0, "column": 0}},
                ),
                (gateway._read_symbol_discovery_context, {"symbol_path": "missing"}),
                (
                    gateway._read_symbol_discovery_context_at_position,
                    {"file_path": "missing.py", "position": {"row": 0, "column": 0}},
                ),
            )

            for handler, required_params in cases:
                with self.subTest(handler=handler.__name__):
                    params = {
                        "workspace_root": ".",
                        "index_db_path": index_db_path,
                        "timeout_ms": 300000,
                        **required_params,
                    }
                    with self.assertRaisesRegex(ValueError, "does not exist"):
                        handler(params)

    def test_list_and_search_timeouts_reach_native_timeout_parameter(self) -> None:
        gateway = gateway_module.ArboristGateway()
        with tempfile.TemporaryDirectory() as temp_dir:
            index_db_path = str(Path(temp_dir) / "missing.db")
            cases = (
                (gateway._list_symbols, {}),
                (gateway._list_symbols_context, {}),
                (gateway._list_symbols_neighborhood_context, {}),
                (gateway._list_symbols_discovery_context, {}),
                (gateway._search_symbols, {"query": "missing"}),
                (gateway._search_symbols_context, {"query": "missing"}),
                (
                    gateway._search_symbols_neighborhood_context,
                    {"query": "missing"},
                ),
                (gateway._search_symbols_discovery_context, {"query": "missing"}),
            )

            for handler, required_params in cases:
                with self.subTest(handler=handler.__name__):
                    params = {
                        "workspace_root": ".",
                        "index_db_path": index_db_path,
                        "timeout_ms": 300000,
                        **required_params,
                    }
                    with self.assertRaisesRegex(ValueError, "does not exist"):
                        handler(params)


if __name__ == "__main__":
    unittest.main()
