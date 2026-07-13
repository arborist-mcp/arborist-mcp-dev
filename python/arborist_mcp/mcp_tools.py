from __future__ import annotations

import json
from collections.abc import Callable
from typing import Any

from .mcp_validation import reject_unexpected_params
from .tool_manifest import build_tool_catalog
from .tool_result_schemas import JsonRpcError
from .tool_specs import (
    MCP_TOOL_CALL_PARAM_NAMES,
    MCP_TOOL_LIST_PARAM_NAMES,
    TOOL_SPECS_BY_NAME,
    tool_spec,
)

ToolExecutor = Callable[[str, dict[str, Any]], Any]


def tools_list(params: dict[str, Any]) -> dict[str, Any]:
    reject_unexpected_params(params, MCP_TOOL_LIST_PARAM_NAMES)
    cursor = params.get("cursor")
    if cursor is not None and not isinstance(cursor, str):
        raise JsonRpcError(-32602, "invalid params: cursor must be a string")
    return {"tools": build_tool_catalog()}


def tools_call(params: dict[str, Any], execute_tool: ToolExecutor) -> dict[str, Any]:
    reject_unexpected_params(params, MCP_TOOL_CALL_PARAM_NAMES)
    tool_name = params.get("name")
    if not isinstance(tool_name, str) or not tool_name.strip():
        raise JsonRpcError(-32602, "missing required string param: name")
    if tool_name not in TOOL_SPECS_BY_NAME:
        raise JsonRpcError(-32602, f"unknown tool: {tool_name}")
    arguments = params.get("arguments", {})
    if not isinstance(arguments, dict):
        raise JsonRpcError(-32602, "invalid params: arguments must be an object")

    try:
        spec = tool_spec(tool_name)
        reject_unexpected_params(arguments, spec.params)
        tool_result = execute_tool(tool_name, arguments)
    except JsonRpcError as exc:
        return mcp_tool_error(str(exc))
    except ValueError as exc:
        return mcp_tool_error(str(exc))
    except Exception as exc:  # noqa: BLE001
        return mcp_tool_error(str(exc))

    return mcp_tool_result(tool_result)


def mcp_tool_result(tool_result: Any) -> dict[str, Any]:
    return {
        "content": [
            {
                "type": "text",
                "text": json.dumps(tool_result, ensure_ascii=False, allow_nan=False),
            }
        ],
        "structuredContent": {"result": tool_result},
        "isError": False,
    }


def mcp_tool_error(message: str) -> dict[str, Any]:
    return {
        "content": [
            {
                "type": "text",
                "text": message,
            }
        ],
        "isError": True,
    }
