from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Callable


def build_parser(version: str) -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="MCP-compatible stdio JSON-RPC gateway for the Arborist Rust core."
    )
    parser.add_argument(
        "--version",
        action="version",
        version=f"%(prog)s {version}",
    )
    parser.add_argument(
        "--once",
        type=Path,
        help="Read one request from a JSON file and print the response.",
    )
    parser.add_argument(
        "--dump-tool-catalog",
        action="store_true",
        help="Print the generated MCP tool catalog as JSON and exit.",
    )
    return parser


def run_stdio(
    *,
    gateway_factory: Callable[[], Any],
    parse_request: Callable[[str], tuple[Any | None, dict[str, Any] | None]],
    is_notification: Callable[[Any], bool],
    serialize_response: Callable[[dict[str, Any], int | None], str],
    write_response: Callable[[str], bool],
) -> int:
    gateway: Any | None = None

    for raw_line in sys.stdin:
        line = raw_line.strip()
        if not line:
            continue

        request, response = parse_request(line)
        if response is None:
            if gateway is None:
                gateway = gateway_factory()
            response = gateway.handle_request(request)

        if response is not None and not is_notification(request):
            if not write_response(serialize_response(response) + "\n"):
                return 0

    return 0


def main(
    *,
    argv: list[str] | None,
    version: str,
    gateway_factory: Callable[[], Any],
    build_tool_catalog: Callable[[], list[dict[str, Any]]],
    parse_request: Callable[[str], tuple[Any | None, dict[str, Any] | None]],
    is_notification: Callable[[Any], bool],
    serialize_response: Callable[[dict[str, Any], int | None], str],
    print_response: Callable[[str], bool],
    run_stdio: Callable[[], int],
) -> int:
    parser = build_parser(version)
    args = parser.parse_args(argv)

    if args.dump_tool_catalog:
        if not print_response(
            json.dumps(build_tool_catalog(), ensure_ascii=False, allow_nan=False, indent=2)
        ):
            return 0
        return 0

    if args.once:
        try:
            raw_request = args.once.read_text(encoding="utf-8")
        except (OSError, UnicodeError) as exc:
            print(
                f"error: failed to read request file {args.once}: {exc}",
                file=sys.stderr,
            )
            return 1
        request, response = parse_request(raw_request)
        if response is None:
            response = gateway_factory().handle_request(request)
        if response is not None and not is_notification(request):
            if not print_response(serialize_response(response, indent=2)):
                return 0
        return 0

    return run_stdio()
