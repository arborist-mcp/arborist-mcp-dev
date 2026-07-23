from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
EXPECTED_PROTOCOL_VERSION = "2025-06-18"
EXPECTED_TOOL = "arborist/get_semantic_skeleton"


def _run_gateway(
    python: str,
    launcher: str,
    *arguments: str,
    input_text: str | None = None,
) -> subprocess.CompletedProcess[str]:
    command = (
        [python, "-m", "arborist_mcp.gateway", *arguments]
        if launcher == "module"
        else ["arborist-mcp", *arguments]
    )
    return subprocess.run(
        command,
        cwd=REPO_ROOT,
        input=input_text,
        check=True,
        capture_output=True,
        text=True,
    )


def _reject_nonstandard_json_constant(name: str) -> Any:
    raise ValueError(f"non-standard JSON constant: {name}")


def _load_json(payload: str, description: str) -> Any:
    try:
        return json.loads(
            payload,
            parse_constant=_reject_nonstandard_json_constant,
        )
    except (json.JSONDecodeError, ValueError) as exc:
        raise RuntimeError(f"{description} returned invalid JSON: {exc}") from exc


def _assert_jsonrpc_ok(response: Any, description: str, request_id: int) -> dict[str, Any]:
    if not isinstance(response, dict):
        raise RuntimeError(f"{description} returned a non-object JSON-RPC response")
    if response.get("jsonrpc") != "2.0":
        raise RuntimeError(f"{description} returned unexpected jsonrpc value: {response!r}")
    if response.get("id") != request_id:
        raise RuntimeError(f"{description} returned unexpected id: {response!r}")
    if "error" in response:
        raise RuntimeError(f"{description} returned JSON-RPC error: {response['error']!r}")
    result = response.get("result")
    if not isinstance(result, dict):
        raise RuntimeError(f"{description} returned a non-object result: {response!r}")
    return result


def _request_once(
    python: str,
    launcher: str,
    request: dict[str, Any],
    description: str,
) -> dict[str, Any]:
    with tempfile.NamedTemporaryFile("w", encoding="utf-8", suffix=".json", delete=False) as handle:
        request_path = Path(handle.name)
        json.dump(request, handle, ensure_ascii=False, allow_nan=False)
        handle.write("\n")

    try:
        completed = _run_gateway(python, launcher, "--once", str(request_path))
    finally:
        request_path.unlink(missing_ok=True)

    response = _load_json(completed.stdout, description)
    request_id = request.get("id")
    if not isinstance(request_id, int):
        raise RuntimeError(f"{description} test request must use an integer id")
    return _assert_jsonrpc_ok(response, description, request_id)


def _request_stdio(
    python: str,
    launcher: str,
    request: dict[str, Any],
    description: str,
) -> dict[str, Any]:
    completed = _run_gateway(
        python,
        launcher,
        input_text=json.dumps(request, ensure_ascii=False, allow_nan=False) + "\n",
    )
    response = _load_json(completed.stdout, description)
    request_id = request.get("id")
    if not isinstance(request_id, int):
        raise RuntimeError(f"{description} test request must use an integer id")
    return _assert_jsonrpc_ok(response, description, request_id)


def check_cli(python: str, launcher: str) -> None:
    _run_gateway(python, launcher, "--help")
    _run_gateway(python, launcher, "--version")


def check_tool_catalog_dump(python: str, launcher: str) -> None:
    completed = _run_gateway(python, launcher, "--dump-tool-catalog")
    catalog = _load_json(completed.stdout, "gateway tool catalog dump")
    if not isinstance(catalog, list) or not catalog:
        raise RuntimeError("gateway tool catalog dump returned no tools")
    tool_names = {
        tool.get("name")
        for tool in catalog
        if isinstance(tool, dict) and isinstance(tool.get("name"), str)
    }
    if EXPECTED_TOOL not in tool_names:
        raise RuntimeError(f"gateway tool catalog dump did not include {EXPECTED_TOOL}")


def check_tools_list(python: str, launcher: str) -> None:
    result = _request_stdio(
        python,
        launcher,
        {"jsonrpc": "2.0", "id": 3, "method": "tools/list", "params": {}},
        "MCP tools/list",
    )
    tools = result.get("tools")
    if not isinstance(tools, list) or not tools:
        raise RuntimeError("MCP tools/list returned no tools")
    tool_names = {
        tool.get("name")
        for tool in tools
        if isinstance(tool, dict) and isinstance(tool.get("name"), str)
    }
    if EXPECTED_TOOL not in tool_names:
        raise RuntimeError(f"MCP tools/list did not include {EXPECTED_TOOL}")


def check_initialize(python: str, launcher: str) -> None:
    legacy = _request_once(
        python,
        launcher,
        {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}},
        "legacy gateway initialize",
    )
    supported_languages = legacy.get("supportedLanguages")
    if not isinstance(supported_languages, list):
        raise RuntimeError("legacy gateway initialize did not return supportedLanguages")
    for language in ("python", "c"):
        if language not in supported_languages:
            raise RuntimeError(f"legacy gateway initialize did not report {language} support")

    mcp = _request_once(
        python,
        launcher,
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "initialize",
            "params": {
                "protocolVersion": EXPECTED_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "gateway_smoke.py", "version": "0.1.0"},
            },
        },
        "MCP initialize",
    )
    if mcp.get("protocolVersion") != EXPECTED_PROTOCOL_VERSION:
        raise RuntimeError(
            "MCP initialize returned unexpected protocolVersion "
            f"{mcp.get('protocolVersion')!r}"
        )
    capabilities = mcp.get("capabilities")
    if not isinstance(capabilities, dict) or not isinstance(capabilities.get("tools"), dict):
        raise RuntimeError("MCP initialize did not report tools capability")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Run cross-platform Arborist gateway smoke checks."
    )
    parser.add_argument(
        "--python",
        default=sys.executable,
        help="Python executable used with --launcher module.",
    )
    parser.add_argument(
        "--launcher",
        choices=("module", "console"),
        default="module",
        help=(
            "Launch mode: 'module' runs python -m arborist_mcp.gateway; "
            "'console' runs the installed arborist-mcp entry point."
        ),
    )
    parser.add_argument(
        "--require-core",
        action="store_true",
        help="Also run initialize checks that load the native Arborist core.",
    )
    args = parser.parse_args(argv)

    check_cli(args.python, args.launcher)
    check_tool_catalog_dump(args.python, args.launcher)
    check_tools_list(args.python, args.launcher)
    if args.require_core:
        check_initialize(args.python, args.launcher)
    print("Gateway smoke checks passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
