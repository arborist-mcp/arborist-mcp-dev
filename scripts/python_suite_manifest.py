from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tests import build_manifest_snapshot


def build_selection_descriptions(snapshot: dict[str, object] | None = None) -> list[tuple[str, str]]:
    resolved = build_manifest_snapshot() if snapshot is None else snapshot
    groups = resolved["groups"]
    suites = resolved["suites"]
    assert isinstance(groups, dict)
    assert isinstance(suites, dict)

    descriptions: list[tuple[str, str]] = [
        ("rust", "Run all Rust tests via cargo test --locked."),
        ("python", str(groups["python"]["description"])),
        ("inner-loop", "Run Rust tests plus the python-fast group for the default local loop."),
        ("all", "Run Rust tests plus the full Python suite set."),
    ]

    descriptions.extend(
        (group_name, str(metadata["description"]))
        for group_name, metadata in groups.items()
        if group_name != "python"
    )
    descriptions.extend(
        (suite_name, str(metadata["description"])) for suite_name, metadata in suites.items()
    )
    return descriptions


def _python_fragment(
    selection_name: str,
    *,
    module_names: list[str],
    requires_extension: bool,
    target_type: str,
) -> dict[str, object]:
    return {
        "kind": "python",
        "selection_name": selection_name,
        "target_type": target_type,
        "module_names": list(module_names),
        "requires_extension": requires_extension,
    }


def _expand_selection(
    selection_name: str,
    *,
    snapshot: dict[str, object],
) -> list[dict[str, object]]:
    groups = snapshot["groups"]
    suites = snapshot["suites"]
    assert isinstance(groups, dict)
    assert isinstance(suites, dict)

    if selection_name == "rust":
        return [{"kind": "rust", "selection_name": "rust"}]
    if selection_name == "inner-loop":
        return _expand_selection("rust", snapshot=snapshot) + _expand_selection(
            "python-fast", snapshot=snapshot
        )
    if selection_name == "all":
        return _expand_selection("rust", snapshot=snapshot) + _expand_selection(
            "python", snapshot=snapshot
        )

    group_metadata = groups.get(selection_name)
    if isinstance(group_metadata, dict):
        module_names = group_metadata["module_names"]
        requires_extension = group_metadata["requires_extension"]
        assert isinstance(module_names, list)
        assert isinstance(requires_extension, bool)
        return [
            _python_fragment(
                selection_name,
                module_names=[str(module_name) for module_name in module_names],
                requires_extension=requires_extension,
                target_type="group",
            )
        ]

    suite_metadata = suites.get(selection_name)
    if isinstance(suite_metadata, dict):
        module_name = suite_metadata["module"]
        requires_extension = suite_metadata["requires_extension"]
        assert isinstance(module_name, str)
        assert isinstance(requires_extension, bool)
        return [
            _python_fragment(
                selection_name,
                module_names=[module_name],
                requires_extension=requires_extension,
                target_type="suite",
            )
        ]

    raise RuntimeError(f"unknown Python test suite or group '{selection_name}'")


def build_execution_plan(selection_names: list[str]) -> dict[str, object]:
    if not selection_names:
        raise RuntimeError("execution plan requires at least one suite or group selection")

    snapshot = build_manifest_snapshot()
    steps: list[dict[str, object]] = []
    rust_step: dict[str, object] | None = None
    python_step: dict[str, object] | None = None
    seen_python_modules: set[str] = set()
    seen_python_selections: set[str] = set()

    for selection_name in selection_names:
        for fragment in _expand_selection(selection_name, snapshot=snapshot):
            kind = fragment["kind"]
            assert isinstance(kind, str)
            if kind == "rust":
                if rust_step is None:
                    rust_step = {
                        "key": "rust",
                        "kind": "rust",
                        "selection_names": [],
                    }
                    steps.append(rust_step)

                selection_label = str(fragment["selection_name"])
                if selection_label not in rust_step["selection_names"]:
                    rust_step["selection_names"].append(selection_label)
                continue

            if python_step is None:
                python_step = {
                    "key": "python",
                    "kind": "python",
                    "selection_names": [],
                    "module_names": [],
                    "requires_extension": False,
                }
                steps.append(python_step)

            selection_label = str(fragment["selection_name"])
            if selection_label not in seen_python_selections:
                python_step["selection_names"].append(selection_label)
                seen_python_selections.add(selection_label)

            requires_extension = bool(fragment["requires_extension"])
            python_step["requires_extension"] = bool(python_step["requires_extension"]) or requires_extension

            module_names = fragment["module_names"]
            assert isinstance(module_names, list)
            for module_name in module_names:
                normalized_module = str(module_name)
                if normalized_module in seen_python_modules:
                    continue
                python_step["module_names"].append(normalized_module)
                seen_python_modules.add(normalized_module)

    for step in steps:
        if step["kind"] == "python":
            step["module_count"] = len(step["module_names"])

    return {
        "selection_names": list(selection_names),
        "steps": steps,
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Emit normalized Arborist Python test-suite metadata."
    )
    parser.add_argument(
        "--plan",
        action="store_true",
        help="Emit the deduplicated execution plan for the provided suite selections.",
    )
    parser.add_argument(
        "--descriptions",
        action="store_true",
        help="Emit the ordered suite/group description table used by test.ps1 -ListSuites.",
    )
    parser.add_argument("selections", nargs="*", help="Suite or group selections for --plan.")
    args = parser.parse_args()

    if args.plan:
        payload = build_execution_plan(args.selections)
    elif args.descriptions:
        payload = [
            {"name": name, "description": description}
            for name, description in build_selection_descriptions()
        ]
    else:
        payload = build_manifest_snapshot()
    json.dump(payload, sys.stdout, ensure_ascii=False)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
