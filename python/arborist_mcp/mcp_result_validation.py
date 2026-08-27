from __future__ import annotations

from typing import Any

from .tool_result_schemas import JsonRpcError, TOOL_RESULT_SCHEMAS


def validate_tool_result_shape(tool_name: str, tool_result: Any) -> None:
    """Keep structured results aligned with their advertised top-level type."""
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
