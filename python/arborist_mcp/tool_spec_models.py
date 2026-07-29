from __future__ import annotations

from typing import Any, NamedTuple


class ToolSpec(NamedTuple):
    name: str
    handler: str
    params: tuple[str, ...]
    category: str
    result_schema: str = "object"


class ToolParamSpec(NamedTuple):
    schema: dict[str, Any]
    optional: bool = False
    default: Any = None
    string_max_length: int | None = None
    int_max_value: int | None = None
    source_anchored_optional_tools: frozenset[str] = frozenset()
    string_max_bytes: int | None = None


def _schema(
    schema_type: str,
    description: str,
    *,
    default: Any = None,
    enum: tuple[str, ...] | None = None,
    minimum: int | None = None,
    maximum: int | None = None,
    min_items: int | None = None,
    max_length: int | None = None,
    allow_empty: bool = False,
) -> dict[str, Any]:
    result: dict[str, Any] = {"type": schema_type, "description": description}
    if default is not None:
        result["default"] = default
    if enum is not None:
        result["enum"] = list(enum)
    if minimum is not None:
        result["minimum"] = minimum
    if maximum is not None:
        result["maximum"] = maximum
    if min_items is not None:
        result["minItems"] = min_items
    if max_length is not None:
        result["maxLength"] = max_length
    if schema_type == "string" and not allow_empty:
        result["minLength"] = 1
    return result


# Preserve the established import and pickle identity after moving definitions here.
ToolSpec.__module__ = "arborist_mcp.tool_specs"
ToolParamSpec.__module__ = "arborist_mcp.tool_specs"
