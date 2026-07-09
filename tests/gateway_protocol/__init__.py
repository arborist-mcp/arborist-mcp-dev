from __future__ import annotations

import json
from pathlib import Path


def _load_manifest() -> dict[str, object]:
    manifest_path = Path(__file__).with_name("suites.json")
    raw = json.loads(manifest_path.read_text(encoding="utf-8"))
    if not isinstance(raw, dict) or "suites" not in raw or "groups" not in raw:
        raise RuntimeError(
            "gateway protocol suite manifest must define object keys 'suites' and 'groups'"
        )
    return raw


def _load_suite_modules(manifest: dict[str, object]) -> dict[str, str]:
    raw = manifest["suites"]
    if not isinstance(raw, dict):
        raise RuntimeError("gateway protocol suite manifest 'suites' must be a JSON object")
    suite_modules: dict[str, str] = {}
    for suite_name, metadata in raw.items():
        if not isinstance(suite_name, str) or not suite_name.strip():
            raise RuntimeError("gateway protocol suite names must be non-empty strings")
        if not isinstance(metadata, dict):
            raise RuntimeError(
                f"gateway protocol suite '{suite_name}' metadata must be a JSON object"
            )

        module_name = metadata.get("module")
        if not isinstance(module_name, str) or not module_name.strip():
            raise RuntimeError(
                f"gateway protocol suite '{suite_name}' must define a non-empty module name"
            )

        requires_extension = metadata.get("requires_extension")
        if not isinstance(requires_extension, bool):
            raise RuntimeError(
                f"gateway protocol suite '{suite_name}' must define a boolean requires_extension flag"
            )

        suite_modules[suite_name] = module_name

    return suite_modules


def _load_group_entries(manifest: dict[str, object]) -> dict[str, tuple[str, ...]]:
    raw = manifest["groups"]
    if not isinstance(raw, dict):
        raise RuntimeError("gateway protocol suite manifest 'groups' must be a JSON object")

    group_entries: dict[str, tuple[str, ...]] = {}
    for group_name, metadata in raw.items():
        if not isinstance(group_name, str) or not group_name.strip():
            raise RuntimeError("gateway protocol group names must be non-empty strings")
        if not isinstance(metadata, dict):
            raise RuntimeError(
                f"gateway protocol group '{group_name}' metadata must be a JSON object"
            )

        entries = metadata.get("entries")
        if not isinstance(entries, list) or not entries:
            raise RuntimeError(
                f"gateway protocol group '{group_name}' must define a non-empty entries list"
            )
        normalized_entries: list[str] = []
        for entry in entries:
            if not isinstance(entry, str) or not entry.strip():
                raise RuntimeError(
                    f"gateway protocol group '{group_name}' entries must be non-empty strings"
                )
            normalized_entries.append(entry)

        group_entries[group_name] = tuple(normalized_entries)

    return group_entries


def _expand_group(
    group_name: str,
    *,
    suite_modules: dict[str, str],
    group_entries: dict[str, tuple[str, ...]],
    stack: tuple[str, ...] = (),
) -> tuple[str, ...]:
    if group_name not in group_entries:
        raise RuntimeError(f"unknown gateway protocol group '{group_name}'")
    if group_name in stack:
        cycle = " -> ".join((*stack, group_name))
        raise RuntimeError(f"gateway protocol group cycle detected: {cycle}")

    ordered_suite_names: list[str] = []
    for entry in group_entries[group_name]:
        if entry in suite_modules:
            if entry not in ordered_suite_names:
                ordered_suite_names.append(entry)
            continue
        for nested in _expand_group(
            entry,
            suite_modules=suite_modules,
            group_entries=group_entries,
            stack=(*stack, group_name),
        ):
            if nested not in ordered_suite_names:
                ordered_suite_names.append(nested)

    return tuple(ordered_suite_names)


MANIFEST = _load_manifest()
SUITE_MODULES = _load_suite_modules(MANIFEST)
GROUP_ENTRIES = _load_group_entries(MANIFEST)
GROUP_SUITES = {
    group_name: _expand_group(
        group_name,
        suite_modules=SUITE_MODULES,
        group_entries=GROUP_ENTRIES,
    )
    for group_name in GROUP_ENTRIES
}
MODULE_NAMES = tuple(SUITE_MODULES[suite_name] for suite_name in GROUP_SUITES["gateway"])
