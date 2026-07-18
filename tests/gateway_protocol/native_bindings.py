from __future__ import annotations

import ast
from pathlib import Path
import unittest


_REPO_ROOT = Path(__file__).resolve().parents[2]
_GATEWAY_PATH = _REPO_ROOT / "python" / "arborist_mcp" / "gateway.py"


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


if __name__ == "__main__":
    unittest.main()
