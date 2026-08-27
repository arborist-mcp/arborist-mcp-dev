from __future__ import annotations

import json
from collections.abc import Callable
from typing import Any

from .mcp_validation import reject_unexpected_params
from .tool_manifest import build_tool_catalog
from .tool_result_schemas import JsonRpcError, TOOL_RESULT_SCHEMAS
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
        _validate_tool_result_shape(tool_name, tool_result)
        return mcp_tool_result(tool_result)
    except JsonRpcError as exc:
        return mcp_tool_error(str(exc))
    except ValueError as exc:
        return mcp_tool_error(str(exc))
    except Exception as exc:  # noqa: BLE001
        return mcp_tool_error(str(exc))


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


def _validate_tool_result_shape(tool_name: str, tool_result: Any) -> None:
    """Keep MCP structured results aligned with their advertised top-level type."""
    schema = TOOL_RESULT_SCHEMAS.get(tool_name)
    expected_type = schema.get("type", "object") if schema is not None else "object"

    if expected_type == "object":
        if not isinstance(tool_result, dict):
            raise JsonRpcError(
                -32000,
                f"invalid result from {tool_name}: expected object payload",
            )
        return

    if expected_type == "array":
        if not isinstance(tool_result, list):
            raise JsonRpcError(
                -32000,
                f"invalid result from {tool_name}: expected array payload",
            )
        item_schema = schema.get("items") if schema is not None else None
        if isinstance(item_schema, dict) and item_schema.get("type") == "object":
            for index, item in enumerate(tool_result):
                if not isinstance(item, dict):
                    raise JsonRpcError(
                        -32000,
                        f"invalid result from {tool_name}: expected object item at index {index}",
                    )
        return

    if expected_type == "boolean" and type(tool_result) is not bool:
        raise JsonRpcError(
            -32000,
            f"invalid result from {tool_name}: expected boolean payload",
        )

    if expected_type == "null" and tool_result is not None:
        raise JsonRpcError(
            -32000,
            f"invalid result from {tool_name}: expected null payload",
        )


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
