from __future__ import annotations

import argparse
from collections import Counter
import json
from pathlib import Path
import re
import sys

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from arborist_mcp.tool_manifest import build_tool_catalog


DEFAULT_SNAPSHOT_PATH = REPO_ROOT / "docs" / "tool-catalog.json"


def _catalog_json() -> str:
    return json.dumps(build_tool_catalog(), ensure_ascii=False, allow_nan=False, indent=2) + "\n"


def _documentation_errors(catalog: list[dict[str, object]]) -> list[str]:
    counts = Counter(
        tool.get("metadata", {}).get("category")
        for tool in catalog
        if isinstance(tool.get("metadata"), dict)
    )
    labels = {"read": "Read", "write": "Write", "vfs": "VFS", "index": "Index", "trace": "Trace"}
    errors: list[str] = []
    for relative_path in ("README.md", "docs/tools.md"):
        path = REPO_ROOT / relative_path
        try:
            document = path.read_text(encoding="utf-8")
        except OSError as exc:
            errors.append(f"{relative_path}: unable to read documentation: {exc}")
            continue

        total_match = re.search(r"returns\s+(\d+)\s+tools:", document)
        if total_match is None:
            errors.append(f"{relative_path}: missing generated tool count")
        elif int(total_match.group(1)) != len(catalog):
            errors.append(
                f"{relative_path}: documented tool count {total_match.group(1)} "
                f"does not match generated count {len(catalog)}"
            )

        for category, label in labels.items():
            category_match = re.search(rf"- {label} tools:\s+(\d+)", document)
            expected = counts.get(category, 0)
            if category_match is None:
                errors.append(f"{relative_path}: missing {label} tool count")
            elif int(category_match.group(1)) != expected:
                errors.append(
                    f"{relative_path}: documented {label} count {category_match.group(1)} "
                    f"does not match generated count {expected}"
                )

    protocol_path = REPO_ROOT / "docs" / "protocol.md"
    try:
        protocol = protocol_path.read_text(encoding="utf-8")
    except OSError as exc:
        errors.append(f"docs/protocol.md: unable to read documentation: {exc}")
    else:
        required_fragments = (
            '"tools/list"',
            '"tools/call"',
            '"resources/list"',
            '"resources/read"',
            "arborist/*",
            "python scripts/tool_catalog.py --check",
        )
        for fragment in required_fragments:
            if fragment not in protocol:
                errors.append(
                    f"docs/protocol.md: missing protocol/catalog reference {fragment!r}"
                )
    return errors


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Generate or check the Arborist MCP tool catalog snapshot."
    )
    parser.add_argument(
        "--check",
        nargs="?",
        const=DEFAULT_SNAPSHOT_PATH,
        type=Path,
        metavar="SNAPSHOT",
        help="Fail if the snapshot does not match the generated catalog. Defaults to docs/tool-catalog.json.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Write the generated catalog to this path instead of stdout.",
    )
    parser.add_argument(
        "--snapshot",
        type=Path,
        default=DEFAULT_SNAPSHOT_PATH,
        help="Snapshot path used by --check. Defaults to docs/tool-catalog.json.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    payload = _catalog_json()

    if args.check is not None:
        snapshot_path = args.check
        if snapshot_path == DEFAULT_SNAPSHOT_PATH and args.snapshot != DEFAULT_SNAPSHOT_PATH:
            snapshot_path = args.snapshot
        try:
            existing = snapshot_path.read_text(encoding="utf-8")
        except FileNotFoundError:
            print(f"tool catalog snapshot is missing: {snapshot_path}", file=sys.stderr)
            return 1
        if existing != payload:
            print(f"tool catalog snapshot is out of date: {snapshot_path}", file=sys.stderr)
            print(
                f"regenerate it with: {Path(__file__).name} --output {snapshot_path}",
                file=sys.stderr,
            )
            return 1
        documentation_errors = _documentation_errors(build_tool_catalog())
        if documentation_errors:
            print("tool documentation is out of date:", file=sys.stderr)
            for error in documentation_errors:
                print(f"- {error}", file=sys.stderr)
            return 1
        return 0

    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload, encoding="utf-8", newline="\n")
        return 0

    sys.stdout.write(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
