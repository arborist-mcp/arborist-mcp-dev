from __future__ import annotations

import argparse
from pathlib import Path
import re
import sys

try:
    import tomllib
except ModuleNotFoundError:
    tomllib = None


REPO_ROOT = Path(__file__).resolve().parents[1]
WORKSPACE_CRATE_NAMES = ("arborist-core", "arborist-py")


def _read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def _load_toml(path: Path) -> object | None:
    if tomllib is None:
        return None
    return tomllib.loads(_read_text(path))


def _extract_regex_value(path: Path, pattern: str, description: str) -> str:
    match = re.search(pattern, _read_text(path), flags=re.MULTILINE | re.DOTALL)
    if match is None:
        raise RuntimeError(f"Could not read {description} from {path}.")
    return match.group(1)


def _read_pyproject_version(path: Path) -> str:
    parsed = _load_toml(path)
    if isinstance(parsed, dict):
        project = parsed.get("project")
        if isinstance(project, dict):
            version = project.get("version")
            if isinstance(version, str) and version:
                return version
        raise RuntimeError(f"Could not read pyproject version from {path}.")
    return _extract_regex_value(
        path,
        r'^\[project\]\s*(?:(?!^\[).)*?^version\s*=\s*"([^"]+)"',
        "pyproject version",
    )


def _read_cargo_workspace_version(path: Path) -> str:
    parsed = _load_toml(path)
    if isinstance(parsed, dict):
        workspace = parsed.get("workspace")
        if isinstance(workspace, dict):
            package = workspace.get("package")
            if isinstance(package, dict):
                version = package.get("version")
                if isinstance(version, str) and version:
                    return version
        raise RuntimeError(f"Could not read Cargo workspace version from {path}.")
    return _extract_regex_value(
        path,
        r'^\[workspace\.package\]\s*(?:(?!^\[).)*?^version\s*=\s*"([^"]+)"',
        "Cargo workspace version",
    )


def _read_python_package_version(path: Path) -> str:
    return _extract_regex_value(
        path,
        r'^__version__\s*=\s*"([^"]+)"',
        "Python package version",
    )


def _read_cargo_lock_package_version(path: Path, package_name: str) -> str:
    parsed = _load_toml(path)
    if isinstance(parsed, dict):
        packages = parsed.get("package")
        if isinstance(packages, list):
            for package in packages:
                if not isinstance(package, dict):
                    continue
                if package.get("name") == package_name:
                    version = package.get("version")
                    if isinstance(version, str) and version:
                        return version
                    break
        raise RuntimeError(f"Could not read Cargo.lock version for package {package_name} from {path}.")

    pattern = (
        r'^\[\[package\]\]\s*(?:(?!^\[\[package\]\]).)*?^name\s*=\s*"'
        + re.escape(package_name)
        + r'"\s*(?:(?!^\[\[package\]\]).)*?^version\s*=\s*"([^"]+)"'
    )
    return _extract_regex_value(
        path,
        pattern,
        f"Cargo.lock version for package {package_name}",
    )


def collect_versions(repo_root: Path) -> dict[str, str]:
    cargo_lock = repo_root / "Cargo.lock"
    cargo_toml = repo_root / "Cargo.toml"
    pyproject = repo_root / "pyproject.toml"
    version_py = repo_root / "python" / "arborist_mcp" / "_version.py"

    versions = {
        "pyproject": _read_pyproject_version(pyproject),
        "cargo_workspace": _read_cargo_workspace_version(cargo_toml),
        "python_package": _read_python_package_version(version_py),
    }
    for package_name in WORKSPACE_CRATE_NAMES:
        versions[f"cargo_lock:{package_name}"] = _read_cargo_lock_package_version(
            cargo_lock, package_name
        )
    return versions


def check_repo(repo_root: Path) -> None:
    versions = collect_versions(repo_root)
    workspace_version = versions["cargo_workspace"]
    if versions["pyproject"] != workspace_version or versions["python_package"] != workspace_version:
        raise RuntimeError(
            "Version mismatch: "
            f"pyproject={versions['pyproject']} "
            f"Cargo={workspace_version} "
            f"package={versions['python_package']}."
        )

    for package_name in WORKSPACE_CRATE_NAMES:
        lock_version = versions[f"cargo_lock:{package_name}"]
        if lock_version != workspace_version:
            raise RuntimeError(
                "Version mismatch: "
                f"Cargo workspace={workspace_version} "
                f"Cargo.lock {package_name}={lock_version}."
            )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Check version consistency across Arborist manifests."
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=REPO_ROOT,
        help="Repository root to validate. Defaults to the current Arborist checkout.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        check_repo(args.repo_root.resolve())
    except RuntimeError as exc:
        print(str(exc), file=sys.stderr)
        return 1
    print("Version consistency checks passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
