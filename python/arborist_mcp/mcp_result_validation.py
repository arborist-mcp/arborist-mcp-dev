from __future__ import annotations

import math
from typing import Any

from .tool_result_schemas import JsonRpcError, TOOL_RESULT_SCHEMAS
from .tool_specs import BATCH_ALLOWED_TOOLS


class _ResultSchemaValidationError(ValueError):
    """Internal failure raised while checking a nested result schema."""


def validate_tool_result_shape(
    tool_name: str,
    tool_result: Any,
    *,
    deep: bool = False,
) -> None:
    """Keep structured results aligned with their advertised top-level type."""
    if deep:
        _validate_shallow_result_shape(tool_name, tool_result)
        schema = TOOL_RESULT_SCHEMAS.get(tool_name, {"type": "object"})
        try:
            _validate_result_schema(schema, tool_result, "result")
        except _ResultSchemaValidationError as exc:
            raise JsonRpcError(
                -32000,
                f"invalid result from {tool_name}: {exc}",
            ) from exc
        return

    _validate_shallow_result_shape(tool_name, tool_result)


def _validate_shallow_result_shape(tool_name: str, tool_result: Any) -> None:
    """Check only the compatibility-preserving top-level result shape."""
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


def _validate_result_schema(
    schema: dict[str, Any],
    value: Any,
    path: str,
) -> None:
    any_of = schema.get("anyOf")
    if isinstance(any_of, list):
        for branch in any_of:
            if not isinstance(branch, dict):
                continue
            try:
                _validate_result_schema(branch, value, path)
            except _ResultSchemaValidationError:
                continue
            return
        raise _ResultSchemaValidationError(
            f"{path} does not match any allowed result shape"
        )

    expected_type = schema.get("type")
    if isinstance(expected_type, str) and not _matches_json_type(expected_type, value):
        raise _ResultSchemaValidationError(
            f"{path} must be {_json_type_label(expected_type)}"
        )

    if "enum" in schema and value not in schema["enum"]:
        raise _ResultSchemaValidationError(
            f"{path} must be one of {schema['enum']}"
        )

    if isinstance(value, dict):
        _validate_result_object(schema, value, path)
    elif isinstance(value, list):
        _validate_result_array(schema, value, path)
    elif isinstance(value, str):
        min_length = schema.get("minLength")
        max_length = schema.get("maxLength")
        if isinstance(min_length, int) and len(value) < min_length:
            raise _ResultSchemaValidationError(
                f"{path} must contain at least {min_length} characters"
            )
        if isinstance(max_length, int) and len(value) > max_length:
            raise _ResultSchemaValidationError(
                f"{path} must contain at most {max_length} characters"
            )
    elif type(value) is int:
        minimum = schema.get("minimum")
        maximum = schema.get("maximum")
        if isinstance(minimum, int) and value < minimum:
            raise _ResultSchemaValidationError(
                f"{path} must be at least {minimum}"
            )
        if isinstance(maximum, int) and value > maximum:
            raise _ResultSchemaValidationError(
                f"{path} must be at most {maximum}"
            )
    elif type(value) is float and not math.isfinite(value):
        raise _ResultSchemaValidationError(f"{path} must be a finite number")


def _validate_result_object(
    schema: dict[str, Any],
    value: dict[Any, Any],
    path: str,
) -> None:
    if any(not isinstance(key, str) for key in value):
        raise _ResultSchemaValidationError(f"{path} must use string field names")

    properties = schema.get("properties", {})
    if not isinstance(properties, dict):
        properties = {}

    required = schema.get("required", [])
    if isinstance(required, list):
        for field_name in required:
            if field_name not in value:
                raise _ResultSchemaValidationError(
                    f"{path} is missing required field `{field_name}`"
                )

    additional_properties = schema.get("additionalProperties", True)
    if additional_properties is False:
        unexpected = sorted(set(value) - set(properties))
        if unexpected:
            raise _ResultSchemaValidationError(
                f"{path} has unexpected field `{unexpected[0]}`"
            )

    for field_name, field_value in value.items():
        field_schema = properties.get(field_name)
        if isinstance(field_schema, dict):
            _validate_result_schema(field_schema, field_value, f"{path}.{field_name}")
        elif isinstance(additional_properties, dict):
            _validate_result_schema(
                additional_properties,
                field_value,
                f"{path}.{field_name}",
            )


def _validate_result_array(
    schema: dict[str, Any],
    value: list[Any],
    path: str,
) -> None:
    min_items = schema.get("minItems")
    max_items = schema.get("maxItems")
    if isinstance(min_items, int) and len(value) < min_items:
        raise _ResultSchemaValidationError(
            f"{path} must contain at least {min_items} items"
        )
    if isinstance(max_items, int) and len(value) > max_items:
        raise _ResultSchemaValidationError(
            f"{path} must contain at most {max_items} items"
        )

    item_schema = schema.get("items")
    if isinstance(item_schema, dict):
        for index, item in enumerate(value):
            _validate_result_schema(item_schema, item, f"{path}[{index}]")


def _matches_json_type(expected_type: str, value: Any) -> bool:
    return {
        "object": isinstance(value, dict),
        "array": isinstance(value, list),
        "string": isinstance(value, str),
        "integer": type(value) is int,
        "number": (type(value) is int or type(value) is float),
        "boolean": type(value) is bool,
        "null": value is None,
    }.get(expected_type, False)


def _json_type_label(expected_type: str) -> str:
    return {
        "object": "an object",
        "array": "an array",
        "string": "a string",
        "integer": "an integer",
        "number": "a number",
        "boolean": "a boolean",
        "null": "null",
    }.get(expected_type, expected_type)


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
        validate_tool_result_shape(name, item["result"], deep=True)
    except JsonRpcError as exc:
        raise JsonRpcError(
            -32000,
            f"invalid result from arborist/batch: object item at index {index} "
            f"has invalid result: {exc}",
        ) from exc
