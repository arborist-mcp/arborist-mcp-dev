from __future__ import annotations

from typing import Any

from .tool_result_schemas import JsonRpcError, TOOL_RESULT_SCHEMAS
from .tool_specs import BATCH_ALLOWED_TOOLS


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
                if tool_name == "arborist/batch":
                    _validate_batch_item(item, index)
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


def _validate_batch_item(item: dict[str, Any], index: int) -> None:
    expected_fields = {"name", "result"}
    missing = sorted(expected_fields - set(item))
    if missing:
        raise JsonRpcError(
            -32000,
            f"invalid result from arborist/batch: object item at index {index} "
            f"missing field `{missing[0]}`",
        )
    unexpected = sorted(set(item) - expected_fields)
    if unexpected:
        raise JsonRpcError(
            -32000,
            f"invalid result from arborist/batch: object item at index {index} "
            f"has unexpected field `{unexpected[0]}`",
        )

    name = item["name"]
    if not isinstance(name, str) or not name.strip():
        raise JsonRpcError(
            -32000,
            f"invalid result from arborist/batch: object item at index {index} "
            "`name` must be a non-empty string",
        )
    if name not in BATCH_ALLOWED_TOOLS:
        raise JsonRpcError(
            -32000,
            f"invalid result from arborist/batch: object item at index {index} "
            f"has unsupported tool name `{name}`",
        )

    try:
        validate_tool_result_shape(name, item["result"])
    except JsonRpcError as exc:
        raise JsonRpcError(
            -32000,
            f"invalid result from arborist/batch: object item at index {index} "
            f"has invalid result: {exc}",
        ) from exc
