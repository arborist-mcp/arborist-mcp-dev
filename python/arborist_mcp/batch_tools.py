from __future__ import annotations

from typing import Any

from .mcp_tools import ToolExecutor
from .mcp_validation import reject_unexpected_params
from .tool_result_schemas import JsonRpcError
from .tool_specs import (
    BATCH_ALLOWED_TOOLS,
    MAX_BATCH_CALLS,
    TOOL_SPECS_BY_NAME,
    tool_spec,
)


def batch_tools(params: dict[str, Any], execute_tool: ToolExecutor) -> list[dict[str, Any]]:
    calls = params.get("calls")
    if not isinstance(calls, list):
        raise JsonRpcError(-32602, "missing required array param: calls")
    if not calls:
        raise JsonRpcError(-32602, "invalid params: calls must not be empty")
    if len(calls) > MAX_BATCH_CALLS:
        raise JsonRpcError(
            -32602,
            f"invalid params: calls must contain at most {MAX_BATCH_CALLS} entries",
        )

    results: list[dict[str, Any]] = []
    for index, call in enumerate(calls):
        if not isinstance(call, dict):
            raise JsonRpcError(
                -32602,
                f"invalid params: calls[{index}] must be an object",
            )
        reject_unexpected_params(call, ("name", "arguments"))
        tool_name = call.get("name")
        if not isinstance(tool_name, str) or not tool_name.strip():
            raise JsonRpcError(
                -32602,
                f"missing required string param: calls[{index}].name",
            )
        if tool_name not in TOOL_SPECS_BY_NAME:
            raise JsonRpcError(-32602, f"unknown batch tool: {tool_name}")
        if tool_name == "arborist/batch":
            raise JsonRpcError(-32602, "batch calls may not include arborist/batch")
        if tool_name not in BATCH_ALLOWED_TOOLS:
            raise JsonRpcError(
                -32602,
                f"batch only supports read-only tools: {tool_name}",
            )

        arguments = call.get("arguments", {})
        if not isinstance(arguments, dict):
            raise JsonRpcError(
                -32602,
                f"invalid params: calls[{index}].arguments must be an object",
            )
        spec = tool_spec(tool_name)
        reject_unexpected_params(arguments, spec.params)
        results.append({"name": tool_name, "result": execute_tool(tool_name, arguments)})

    return results
