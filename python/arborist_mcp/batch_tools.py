from __future__ import annotations

import time
from collections.abc import Callable
from dataclasses import dataclass
from typing import Any

from .mcp_result_validation import validate_tool_result_shape
from .mcp_tools import ToolExecutor
from .mcp_validation import reject_unexpected_params
from .tool_result_schemas import JsonRpcError
from .tool_specs import (
    BATCH_ALLOWED_TOOLS,
    MAX_BATCH_CALLS,
    MAX_WORKSPACE_SCAN_TIMEOUT_MS,
    TOOL_SPECS_BY_NAME,
    tool_spec,
)


_NANOSECONDS_PER_MILLISECOND = 1_000_000


@dataclass(frozen=True)
class _ValidatedBatchCall:
    name: str
    arguments: dict[str, Any]
    timeout_ms: int | None


class _BatchDeadline:
    def __init__(
        self,
        timeout_ms: int,
        monotonic_ns: Callable[[], int],
    ) -> None:
        self._monotonic_ns = monotonic_ns
        self._deadline_ns = (
            monotonic_ns() + timeout_ms * _NANOSECONDS_PER_MILLISECOND
        )

    def remaining_timeout_ms(self, call_index: int, phase: str) -> int:
        remaining_ns = self._deadline_ns - self._monotonic_ns()
        if remaining_ns <= 0:
            raise JsonRpcError(
                -32000,
                f"batch timeout exceeded {phase} calls[{call_index}]",
            )
        return (
            remaining_ns + _NANOSECONDS_PER_MILLISECOND - 1
        ) // _NANOSECONDS_PER_MILLISECOND


def batch_tools(
    params: dict[str, Any],
    execute_tool: ToolExecutor,
    timeout_ms: int | None = None,
    *,
    monotonic_ns: Callable[[], int] = time.monotonic_ns,
) -> list[dict[str, Any]]:
    deadline = (
        _BatchDeadline(timeout_ms, monotonic_ns) if timeout_ms is not None else None
    )
    calls = _validate_batch_calls(params)

    results: list[dict[str, Any]] = []
    for index, call in enumerate(calls):
        arguments = call.arguments
        if deadline is not None:
            remaining_timeout_ms = deadline.remaining_timeout_ms(index, "before")
            if "timeout_ms" in tool_spec(call.name).params:
                arguments = dict(arguments)
                arguments["timeout_ms"] = (
                    remaining_timeout_ms
                    if call.timeout_ms is None
                    else min(call.timeout_ms, remaining_timeout_ms)
                )

        result = execute_tool(call.name, arguments)
        validate_tool_result_shape(call.name, result)
        if deadline is not None:
            deadline.remaining_timeout_ms(index, "after")
        results.append({"name": call.name, "result": result})

    return results


def _validate_batch_calls(params: dict[str, Any]) -> list[_ValidatedBatchCall]:
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

    validated_calls: list[_ValidatedBatchCall] = []
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
        timeout_ms = _validate_inner_timeout_ms(arguments, index, spec.params)
        validated_calls.append(_ValidatedBatchCall(tool_name, arguments, timeout_ms))

    return validated_calls


def _validate_inner_timeout_ms(
    arguments: dict[str, Any],
    call_index: int,
    param_names: tuple[str, ...],
) -> int | None:
    if "timeout_ms" not in param_names or "timeout_ms" not in arguments:
        return None

    timeout_ms = arguments["timeout_ms"]
    param_path = f"calls[{call_index}].arguments.timeout_ms"
    if not isinstance(timeout_ms, int) or isinstance(timeout_ms, bool):
        raise JsonRpcError(-32602, f"invalid int param: {param_path}")
    if timeout_ms <= 0:
        raise JsonRpcError(-32602, f"invalid positive int param: {param_path}")
    if timeout_ms > MAX_WORKSPACE_SCAN_TIMEOUT_MS:
        raise JsonRpcError(
            -32602,
            f"invalid int param: {param_path} exceeds maximum "
            f"{MAX_WORKSPACE_SCAN_TIMEOUT_MS}",
        )
    return timeout_ms
