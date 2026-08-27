from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Callable

from .tool_specs import MAX_REQUEST_BYTES


def _read_request_file(path: Path) -> str:
    try:
        file_size = path.stat().st_size
    except OSError:
        file_size = None
    if file_size is not None and file_size > MAX_REQUEST_BYTES:
        raise ValueError(
            f"request file exceeds maximum size of {MAX_REQUEST_BYTES} bytes"
        )

    try:
        with path.open("rb") as request_file:
            raw_request_bytes = request_file.read(MAX_REQUEST_BYTES + 1)
    except FileNotFoundError:
        # Keep the normal Path.read_text error surface for missing files and
        # compatibility with callers that provide a virtual Path implementation.
        raw_request = path.read_text(encoding="utf-8")
        if len(raw_request.encode("utf-8")) > MAX_REQUEST_BYTES:
            raise ValueError(
                f"request file exceeds maximum size of {MAX_REQUEST_BYTES} bytes"
            )
        return raw_request

    if len(raw_request_bytes) > MAX_REQUEST_BYTES:
        raise ValueError(
            f"request file exceeds maximum size of {MAX_REQUEST_BYTES} bytes"
        )
    return raw_request_bytes.decode("utf-8")


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
    parse_error_response: Callable[[str], dict[str, Any]],
    is_notification: Callable[[Any], bool],
    serialize_response: Callable[[dict[str, Any], int | None], str],
    write_response: Callable[[str], bool],
) -> int:
    gateway: Any | None = None

    while True:
        try:
            raw_line = _read_stdio_line()
        except UnicodeDecodeError:
            # A text decoder may consume the malformed byte sequence while
            # raising, so the stream can no longer be safely re-synchronized.
            # Emit the protocol error before stopping instead of leaking the
            # decoder exception to the process boundary.
            response = parse_error_response(
                "invalid JSON: request is not valid UTF-8 text"
            )
            if not write_response(serialize_response(response) + "\n"):
                return 0
            return 0
        if raw_line is None:
            break
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


def _read_stdio_line() -> str | None:
    # Text streams impose character rather than byte limits. This cap bounds
    # memory even for four-byte UTF-8 characters; parse_request still enforces
    # the protocol's byte limit below.
    read_limit = MAX_REQUEST_BYTES + 2
    raw_line = sys.stdin.readline(read_limit)
    if raw_line == "":
        return None

    if len(raw_line) == read_limit and not raw_line.endswith("\n"):
        _discard_stdio_line_remainder()
    return raw_line


def _discard_stdio_line_remainder() -> None:
    while True:
        remainder = sys.stdin.readline(MAX_REQUEST_BYTES + 2)
        if remainder == "" or remainder.endswith("\n"):
            return


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
            raw_request = _read_request_file(args.once)
        except (OSError, UnicodeError, ValueError) as exc:
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
