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


def _load_suites(manifest: dict[str, object]) -> dict[str, dict[str, object]]:
    raw = manifest["suites"]
    if not isinstance(raw, dict):
        raise RuntimeError("gateway protocol suite manifest 'suites' must be a JSON object")
    suites: dict[str, dict[str, object]] = {}
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

        description = metadata.get("description")
        if not isinstance(description, str) or not description.strip():
            raise RuntimeError(
                f"gateway protocol suite '{suite_name}' must define a non-empty description"
            )

        requires_extension = metadata.get("requires_extension")
        if not isinstance(requires_extension, bool):
            raise RuntimeError(
                f"gateway protocol suite '{suite_name}' must define a boolean requires_extension flag"
            )

        suites[suite_name] = {
            "module": module_name,
            "description": description,
            "requires_extension": requires_extension,
        }

    return suites


def _load_groups(manifest: dict[str, object]) -> dict[str, dict[str, object]]:
    raw = manifest["groups"]
    if not isinstance(raw, dict):
        raise RuntimeError("gateway protocol suite manifest 'groups' must be a JSON object")

    groups: dict[str, dict[str, object]] = {}
    for group_name, metadata in raw.items():
        if not isinstance(group_name, str) or not group_name.strip():
            raise RuntimeError("gateway protocol group names must be non-empty strings")
        if not isinstance(metadata, dict):
            raise RuntimeError(
                f"gateway protocol group '{group_name}' metadata must be a JSON object"
            )

        description = metadata.get("description")
        if not isinstance(description, str) or not description.strip():
            raise RuntimeError(
                f"gateway protocol group '{group_name}' must define a non-empty description"
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

        groups[group_name] = {
            "description": description,
            "entries": tuple(normalized_entries),
        }

    return groups


def _expand_group(
    group_name: str,
    *,
    suites: dict[str, dict[str, object]],
    groups: dict[str, dict[str, object]],
    stack: tuple[str, ...] = (),
) -> tuple[str, ...]:
    if group_name not in groups:
        raise RuntimeError(f"unknown gateway protocol group '{group_name}'")
    if group_name in stack:
        cycle = " -> ".join((*stack, group_name))
        raise RuntimeError(f"gateway protocol group cycle detected: {cycle}")

    ordered_suite_names: list[str] = []
    entries = groups[group_name]["entries"]
    assert isinstance(entries, tuple)
    for entry in entries:
        if entry in suites:
            if entry not in ordered_suite_names:
                ordered_suite_names.append(entry)
            continue
        for nested in _expand_group(
            entry,
            suites=suites,
            groups=groups,
            stack=(*stack, group_name),
        ):
            if nested not in ordered_suite_names:
                ordered_suite_names.append(nested)

    return tuple(ordered_suite_names)


def _build_resolved_groups(
    *,
    suites: dict[str, dict[str, object]],
    groups: dict[str, dict[str, object]],
) -> dict[str, dict[str, object]]:
    resolved_groups: dict[str, dict[str, object]] = {}
    for group_name, metadata in groups.items():
        suite_names = _expand_group(group_name, suites=suites, groups=groups)
        module_names = tuple(suites[suite_name]["module"] for suite_name in suite_names)
        requires_extension = any(
            bool(suites[suite_name]["requires_extension"]) for suite_name in suite_names
        )
        resolved_groups[group_name] = {
            "description": metadata["description"],
            "entries": metadata["entries"],
            "suite_names": suite_names,
            "module_names": module_names,
            "requires_extension": requires_extension,
        }

    return resolved_groups


def build_manifest_snapshot() -> dict[str, object]:
    return {
        "suites": {
            suite_name: {
                "module": metadata["module"],
                "description": metadata["description"],
                "requires_extension": metadata["requires_extension"],
            }
            for suite_name, metadata in SUITES.items()
        },
        "groups": {
            group_name: {
                "description": metadata["description"],
                "entries": list(metadata["entries"]),
                "suite_names": list(metadata["suite_names"]),
                "module_names": list(metadata["module_names"]),
                "requires_extension": metadata["requires_extension"],
            }
            for group_name, metadata in GROUPS.items()
        },
    }


MANIFEST = _load_manifest()
SUITES = _load_suites(MANIFEST)
_GROUP_DEFINITIONS = _load_groups(MANIFEST)
SUITE_MODULES = {
    suite_name: str(metadata["module"]) for suite_name, metadata in SUITES.items()
}
GROUPS = _build_resolved_groups(suites=SUITES, groups=_GROUP_DEFINITIONS)
GROUP_SUITES = {
    group_name: tuple(metadata["suite_names"])
    for group_name, metadata in GROUPS.items()
}
GROUP_MODULES = {
    group_name: tuple(metadata["module_names"]) for group_name, metadata in GROUPS.items()
}
MODULE_NAMES = GROUP_MODULES["gateway"]
