from __future__ import annotations

import json
from pathlib import Path


def _load_suite_modules() -> dict[str, str]:
    manifest_path = Path(__file__).with_name("suites.json")
    raw = json.loads(manifest_path.read_text(encoding="utf-8"))
    if not isinstance(raw, dict):
        raise RuntimeError("gateway protocol suite manifest must be a JSON object")

    suite_modules: dict[str, str] = {}
    for suite_name, module_name in raw.items():
        if not isinstance(suite_name, str) or not suite_name.strip():
            raise RuntimeError("gateway protocol suite names must be non-empty strings")
        if not isinstance(module_name, str) or not module_name.strip():
            raise RuntimeError("gateway protocol suite modules must be non-empty strings")
        suite_modules[suite_name] = module_name

    return suite_modules


SUITE_MODULES = _load_suite_modules()
MODULE_NAMES = tuple(SUITE_MODULES.values())
