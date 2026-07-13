from __future__ import annotations

import json
from typing import Any

from .tool_manifest import build_resource_catalog, build_tool_catalog
from .tool_result_schemas import JsonRpcError
from .tool_specs import (
    MCP_RESOURCE_LIST_PARAM_NAMES,
    MCP_RESOURCE_READ_PARAM_NAMES,
    TOOL_CATALOG_RESOURCE_MIME_TYPE,
    TOOL_CATALOG_RESOURCE_URI,
)


def resources_list(params: dict[str, Any]) -> dict[str, Any]:
    _reject_unexpected_params(params, MCP_RESOURCE_LIST_PARAM_NAMES)
    cursor = params.get("cursor")
    if cursor is not None and not isinstance(cursor, str):
        raise JsonRpcError(-32602, "invalid params: cursor must be a string")
    return {"resources": build_resource_catalog()}


def resources_read(params: dict[str, Any]) -> dict[str, Any]:
    _reject_unexpected_params(params, MCP_RESOURCE_READ_PARAM_NAMES)
    uri = params.get("uri")
    if not isinstance(uri, str) or not uri.strip():
        raise JsonRpcError(-32602, "missing required string param: uri")
    if uri != TOOL_CATALOG_RESOURCE_URI:
        raise JsonRpcError(-32602, f"unknown resource: {uri}")
    return {
        "contents": [
            {
                "uri": TOOL_CATALOG_RESOURCE_URI,
                "mimeType": TOOL_CATALOG_RESOURCE_MIME_TYPE,
                "text": json.dumps(build_tool_catalog(), ensure_ascii=False, indent=2),
            }
        ]
    }


def _reject_unexpected_params(params: dict[str, Any], allowed_keys: tuple[str, ...]) -> None:
    unexpected_keys = set(params) - set(allowed_keys)
    if unexpected_keys:
        key = sorted(unexpected_keys)[0]
        raise JsonRpcError(-32602, f"unexpected param: {key}")
