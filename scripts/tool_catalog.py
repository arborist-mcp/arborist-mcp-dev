from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from arborist_mcp.gateway import build_tool_catalog


DEFAULT_SNAPSHOT_PATH = REPO_ROOT / "docs" / "tool-catalog.json"


def _catalog_json() -> str:
    return json.dumps(build_tool_catalog(), ensure_ascii=False, indent=2) + "\n"


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Generate or check the Arborist MCP tool catalog snapshot."
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Fail if the snapshot does not match the generated catalog.",
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

    if args.check:
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
        return 0

    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload, encoding="utf-8", newline="\n")
        return 0

    sys.stdout.write(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
