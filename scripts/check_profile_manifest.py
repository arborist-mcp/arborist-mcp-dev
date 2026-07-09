from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys


def _load_manifest() -> dict[str, object]:
    manifest_path = Path(__file__).with_name("check_profiles.json")
    raw = json.loads(manifest_path.read_text(encoding="utf-8"))
    if not isinstance(raw, dict) or "profiles" not in raw or "ci_profiles" not in raw:
        raise RuntimeError(
            "check profile manifest must define object keys 'profiles' and 'ci_profiles'"
        )
    return raw


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
            needs_python = metadata.get("needs_python")
            needs_rust = metadata.get("needs_rust")
            if not isinstance(needs_python, bool):
                raise RuntimeError(
                    f"check profile '{profile_name}' must define a boolean needs_python flag"
                )
            if not isinstance(needs_rust, bool):
                raise RuntimeError(
                    f"check profile '{profile_name}' must define a boolean needs_rust flag"
                )
            profiles[profile_name] = {
                "description": description,
                "entries": (),
                "needs_python": needs_python,
                "needs_rust": needs_rust,
            }
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
    for profile_name in raw:
        if not isinstance(profile_name, str) or not profile_name.strip():
            raise RuntimeError("check profile manifest ci_profiles must contain non-empty strings")
        if profile_name not in profiles:
            raise RuntimeError(f"unknown check profile '{profile_name}' listed in ci_profiles")
        if profiles[profile_name]["entries"]:
            raise RuntimeError(f"CI check profile '{profile_name}' must not be an aggregate profile")
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


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Emit normalized Arborist check-profile metadata."
    )
    parser.add_argument(
        "--github-matrix",
        action="store_true",
        help="Emit the GitHub Actions matrix JSON instead of the full profile snapshot.",
    )
    args = parser.parse_args()

    snapshot = build_snapshot()
    payload = build_github_matrix(snapshot) if args.github_matrix else snapshot
    json.dump(payload, sys.stdout, ensure_ascii=False)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
