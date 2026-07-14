from __future__ import annotations

from typing import Any

from .tool_result_schemas import OBJECT_RESULT_SCHEMA, TOOL_RESULT_SCHEMAS
from .tool_specs import (
    NON_MUTATING_STATE_TOOLS,
    READ_ONLY_CATEGORIES,
    TOOL_CATALOG_RESOURCE_MIME_TYPE,
    TOOL_CATALOG_RESOURCE_URI,
    TOOL_NAMES,
    MUTATING_TOOLS,
    WRITING_TOOLS,
    tool_param_spec,
    tool_spec,
)


def build_tool_catalog() -> list[dict[str, Any]]:
    return [build_tool_descriptor(tool_name) for tool_name in TOOL_NAMES]


def build_resource_catalog() -> list[dict[str, Any]]:
    return [
        {
            "uri": TOOL_CATALOG_RESOURCE_URI,
            "name": "Arborist tool catalog",
            "description": "Generated MCP tools/list snapshot for this Arborist gateway.",
            "mimeType": TOOL_CATALOG_RESOURCE_MIME_TYPE,
        }
    ]


def build_tool_descriptor(tool_name: str) -> dict[str, Any]:
    spec = tool_spec(tool_name)
    category = spec.category
    return {
        "name": tool_name,
        "title": _tool_title(tool_name),
        "description": _tool_description(tool_name, category),
        "inputSchema": build_tool_input_schema(tool_name),
        "outputSchema": build_tool_output_schema_for_tool(tool_name),
        "annotations": {
            "readOnlyHint": category in READ_ONLY_CATEGORIES
            or tool_name in NON_MUTATING_STATE_TOOLS,
            "destructiveHint": tool_name in WRITING_TOOLS,
        },
        "metadata": {
            "category": category,
            "legacyMethod": tool_name,
            "mutatesState": tool_name in MUTATING_TOOLS,
        },
    }


def build_tool_output_schema_for_tool(tool_name: str) -> dict[str, Any]:
    result_schema = TOOL_RESULT_SCHEMAS.get(tool_name, OBJECT_RESULT_SCHEMA)
    return {
        "type": "object",
        "properties": {
            "result": result_schema,
        },
        "required": ["result"],
        "additionalProperties": False,
    }


def build_tool_input_schema(tool_name: str) -> dict[str, Any]:
    properties: dict[str, Any] = {}
    for param_name in tool_spec(tool_name).params:
        param_schema = dict(tool_param_spec(param_name).schema)
        default = tool_param_default(tool_name, param_name)
        if default is not None:
            param_schema["default"] = default
        properties[param_name] = param_schema

    return {
        "type": "object",
        "properties": properties,
        "required": list(required_tool_params(tool_name)),
        "additionalProperties": False,
    }


def required_tool_params(tool_name: str) -> tuple[str, ...]:
    return tuple(
        param_name
        for param_name in tool_spec(tool_name).params
        if not tool_param_spec(param_name).optional
        and not (
            param_name == "file_path"
            and tool_name in tool_param_spec("file_path").source_anchored_optional_tools
        )
    )


def tool_param_default(tool_name: str, param_name: str) -> Any:
    default = tool_param_spec(param_name).default
    if isinstance(default, dict):
        if tool_name.startswith("arborist/list_symbols"):
            return default["list"]
        if tool_name.startswith("arborist/search_symbols"):
            return default["search"]
        return None
    return default


def _tool_title(tool_name: str) -> str:
    return tool_name.removeprefix("arborist/").replace("_", " ").title()


def _tool_description(tool_name: str, category: str) -> str:
    method_name = tool_name.removeprefix("arborist/")
    category_descriptions = {
        "read": "Read semantic source information without writing project files.",
        "write": "Patch persisted source files through Arborist semantic targeting.",
        "vfs": "Manage or inspect Arborist's session-scoped virtual-file state.",
        "index": "Build, refresh, register, or inspect persisted symbol indexes.",
        "trace": "Read trace, graph, or trace-backed validation context.",
    }
    return f"{category_descriptions[category]} Legacy JSON-RPC method: arborist/{method_name}."
