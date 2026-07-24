from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from tests import GROUPS as TEST_GROUPS
from tests import SUITES as TEST_SUITES

from scripts import json_strict


def _load_manifest() -> dict[str, object]:
    manifest_path = Path(__file__).with_name("check_profiles.json")
    try:
        raw = json_strict.loads(manifest_path.read_text(encoding="utf-8"))
    except ValueError as exc:
        raise RuntimeError(f"invalid check profile manifest JSON: {exc}") from exc
    if not isinstance(raw, dict) or "profiles" not in raw or "ci_profiles" not in raw:
        raise RuntimeError(
            "check profile manifest must define object keys 'profiles' and 'ci_profiles'"
        )
    return raw


def _resolve_test_target(selection_name: str) -> dict[str, object]:
    if selection_name in TEST_SUITES:
        metadata = TEST_SUITES[selection_name]
        return {
            "target_type": "suite",
            "requires_extension": bool(metadata["requires_extension"]),
        }
    if selection_name in TEST_GROUPS:
        metadata = TEST_GROUPS[selection_name]
        return {
            "target_type": "group",
            "requires_extension": bool(metadata["requires_extension"]),
        }
    raise RuntimeError(f"unknown test suite or group '{selection_name}' referenced by check profiles")


def _load_profiles(manifest: dict[str, object]) -> dict[str, dict[str, object]]:
    raw = manifest["profiles"]
    if not isinstance(raw, dict) or not raw:
        raise RuntimeError("check profile manifest 'profiles' must be a non-empty JSON object")

    profiles: dict[str, dict[str, object]] = {}
    for profile_name, metadata in raw.items():
        if not isinstance(profile_name, str) or not profile_name.strip():
            raise RuntimeError("check profile names must be non-empty strings")
        if not isinstance(metadata, dict):
            raise RuntimeError(f"check profile '{profile_name}' metadata must be a JSON object")

        description = metadata.get("description")
        if not isinstance(description, str) or not description.strip():
            raise RuntimeError(
                f"check profile '{profile_name}' must define a non-empty description"
            )

        entries = metadata.get("entries")
        if entries is None:
            handler = metadata.get("handler")
            if not isinstance(handler, str) or not handler.strip():
                raise RuntimeError(
                    f"check profile '{profile_name}' must define a non-empty handler"
                )
            profile: dict[str, object] = {
                "description": description,
                "entries": (),
                "handler": handler,
            }

            if handler == "sanity":
                profile["needs_python"] = True
                profile["needs_rust"] = False
            elif handler in {"rust", "fuzz-manifest"}:
                profile["needs_python"] = False
                profile["needs_rust"] = True
            elif handler == "gateway-smoke":
                prepare_extension = metadata.get("prepare_extension", False)
                if not isinstance(prepare_extension, bool):
                    raise RuntimeError(
                        f"check profile '{profile_name}' prepare_extension must be a boolean"
                    )
                profile["prepare_extension"] = prepare_extension
                profile["needs_python"] = True
                profile["needs_rust"] = prepare_extension
            elif handler == "suite":
                suite = metadata.get("suite")
                if not isinstance(suite, str) or not suite.strip():
                    raise RuntimeError(
                        f"check profile '{profile_name}' suite must be a non-empty string"
                    )
                resolved_target = _resolve_test_target(suite)
                prepare_extension = metadata.get("prepare_extension", False)
                sync_extension = metadata.get("sync_extension")
                if sync_extension is None:
                    sync_extension = "auto"
                if sync_extension not in {"auto", "always", "never"}:
                    raise RuntimeError(
                        f"check profile '{profile_name}' sync_extension must be one of auto, always, never"
                    )
                if not isinstance(prepare_extension, bool):
                    raise RuntimeError(
                        f"check profile '{profile_name}' prepare_extension must be a boolean"
                    )
                target_requires_extension = bool(resolved_target["requires_extension"])
                if target_requires_extension and sync_extension == "never" and not prepare_extension:
                    raise RuntimeError(
                        f"check profile '{profile_name}' disables extension sync for test target '{suite}' without pre-building it"
                    )
                profile["suite"] = suite
                profile["suite_target_type"] = resolved_target["target_type"]
                profile["suite_requires_extension"] = target_requires_extension
                profile["sync_extension"] = sync_extension
                profile["prepare_extension"] = prepare_extension
                profile["needs_python"] = True
                profile["needs_rust"] = prepare_extension or (
                    target_requires_extension and sync_extension != "never"
                )
            else:
                raise RuntimeError(
                    f"check profile '{profile_name}' uses unsupported handler '{handler}'"
                )
            profiles[profile_name] = profile
            continue

        if not isinstance(entries, list) or not entries:
            raise RuntimeError(
                f"check profile '{profile_name}' entries must be a non-empty JSON array"
            )
        normalized_entries: list[str] = []
        for entry in entries:
            if not isinstance(entry, str) or not entry.strip():
                raise RuntimeError(
                    f"check profile '{profile_name}' entries must be non-empty strings"
                )
            normalized_entries.append(entry)

        profiles[profile_name] = {
            "description": description,
            "entries": tuple(normalized_entries),
        }

    return profiles


def _load_ci_profiles(
    manifest: dict[str, object], *, profiles: dict[str, dict[str, object]]
) -> tuple[str, ...]:
    raw = manifest["ci_profiles"]
    if not isinstance(raw, list) or not raw:
        raise RuntimeError("check profile manifest 'ci_profiles' must be a non-empty JSON array")

    ci_profiles: list[str] = []
    seen_profiles: set[str] = set()
    for profile_name in raw:
        if not isinstance(profile_name, str) or not profile_name.strip():
            raise RuntimeError("check profile manifest ci_profiles must contain non-empty strings")
        if profile_name not in profiles:
            raise RuntimeError(f"unknown check profile '{profile_name}' listed in ci_profiles")
        if profile_name in seen_profiles:
            raise RuntimeError(f"duplicate check profile '{profile_name}' listed in ci_profiles")
        if profiles[profile_name]["entries"]:
            raise RuntimeError(f"CI check profile '{profile_name}' must not be an aggregate profile")
        seen_profiles.add(profile_name)
        ci_profiles.append(profile_name)

    return tuple(ci_profiles)


def _expand_profile(
    profile_name: str,
    *,
    profiles: dict[str, dict[str, object]],
    stack: tuple[str, ...] = (),
) -> tuple[str, ...]:
    if profile_name not in profiles:
        raise RuntimeError(f"unknown check profile '{profile_name}'")
    if profile_name in stack:
        cycle = " -> ".join((*stack, profile_name))
        raise RuntimeError(f"check profile cycle detected: {cycle}")

    metadata = profiles[profile_name]
    entries = metadata["entries"]
    assert isinstance(entries, tuple)
    if not entries:
        return (profile_name,)

    ordered_profiles: list[str] = []
    for entry in entries:
        for nested in _expand_profile(entry, profiles=profiles, stack=(*stack, profile_name)):
            if nested not in ordered_profiles:
                ordered_profiles.append(nested)
    return tuple(ordered_profiles)


def build_snapshot() -> dict[str, object]:
    manifest = _load_manifest()
    profiles = _load_profiles(manifest)
    ci_profiles = _load_ci_profiles(manifest, profiles=profiles)

    snapshot_profiles: dict[str, dict[str, object]] = {}
    for profile_name, metadata in profiles.items():
        leaf_profiles = _expand_profile(profile_name, profiles=profiles)
        needs_python = any(bool(profiles[name].get("needs_python")) for name in leaf_profiles)
        needs_rust = any(bool(profiles[name].get("needs_rust")) for name in leaf_profiles)
        entries = metadata["entries"]
        assert isinstance(entries, tuple)
        snapshot_profiles[profile_name] = {
            "description": metadata["description"],
            "entries": list(entries),
            "leaf_profiles": list(leaf_profiles),
            "leaf": not entries,
            "needs_python": needs_python,
            "needs_rust": needs_rust,
            "ci_job": profile_name in ci_profiles,
        }
        if not entries:
            snapshot_profiles[profile_name]["handler"] = metadata["handler"]
            if "suite" in metadata:
                snapshot_profiles[profile_name]["suite"] = metadata["suite"]
            if "suite_target_type" in metadata:
                snapshot_profiles[profile_name]["suite_target_type"] = metadata["suite_target_type"]
            if "suite_requires_extension" in metadata:
                snapshot_profiles[profile_name]["suite_requires_extension"] = metadata[
                    "suite_requires_extension"
                ]
            if "sync_extension" in metadata:
                snapshot_profiles[profile_name]["sync_extension"] = metadata["sync_extension"]
            if "prepare_extension" in metadata:
                snapshot_profiles[profile_name]["prepare_extension"] = metadata["prepare_extension"]

    return {
        "profile_order": list(profiles),
        "profiles": snapshot_profiles,
        "ci_profiles": list(ci_profiles),
    }


def build_github_matrix(snapshot: dict[str, object] | None = None) -> dict[str, object]:
    resolved = build_snapshot() if snapshot is None else snapshot
    profiles = resolved["profiles"]
    ci_profiles = resolved["ci_profiles"]
    assert isinstance(profiles, dict)
    assert isinstance(ci_profiles, list)

    return {
        "include": [
            {
                "job_name": profile_name,
                "profile": profile_name,
                "needs_python": profiles[profile_name]["needs_python"],
                "needs_rust": profiles[profile_name]["needs_rust"],
            }
            for profile_name in ci_profiles
        ]
    }


def build_execution_plan(
    profile_names: list[str], snapshot: dict[str, object] | None = None
) -> dict[str, object]:
    if not profile_names:
        raise RuntimeError("execution plan requires at least one check profile")

    resolved = build_snapshot() if snapshot is None else snapshot
    profiles = resolved["profiles"]
    assert isinstance(profiles, dict)

    ordered_leaf_profiles: list[str] = []
    for profile_name in profile_names:
        if profile_name not in profiles:
            raise RuntimeError(f"unknown check profile '{profile_name}'")
        leaf_profiles = profiles[profile_name]["leaf_profiles"]
        assert isinstance(leaf_profiles, list)
        for leaf_name in leaf_profiles:
            normalized_leaf = str(leaf_name)
            if normalized_leaf not in ordered_leaf_profiles:
                ordered_leaf_profiles.append(normalized_leaf)

    return {
        "profile_names": list(profile_names),
        "steps": [
            {
                "profile": leaf_name,
                "handler": profiles[leaf_name]["handler"],
                "needs_python": profiles[leaf_name]["needs_python"],
                "needs_rust": profiles[leaf_name]["needs_rust"],
                "description": profiles[leaf_name]["description"],
                **({"suite": profiles[leaf_name]["suite"]} if "suite" in profiles[leaf_name] else {}),
                **(
                    {"prepare_extension": profiles[leaf_name]["prepare_extension"]}
                    if "prepare_extension" in profiles[leaf_name]
                    else {}
                ),
            }
            for leaf_name in ordered_leaf_profiles
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Emit normalized Arborist check-profile metadata."
    )
    parser.add_argument(
        "--github-matrix",
        action="store_true",
        help="Emit the GitHub Actions matrix JSON instead of the full profile snapshot.",
    )
    parser.add_argument(
        "--plan",
        action="store_true",
        help="Emit the deduplicated execution plan for the provided check profiles.",
    )
    parser.add_argument("profiles", nargs="*", help="Profile selections for --plan.")
    args = parser.parse_args()

    snapshot = build_snapshot()
    if args.github_matrix:
        payload = build_github_matrix(snapshot)
    elif args.plan:
        payload = build_execution_plan(args.profiles, snapshot)
    else:
        payload = snapshot
    json.dump(payload, sys.stdout, ensure_ascii=False, allow_nan=False)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
