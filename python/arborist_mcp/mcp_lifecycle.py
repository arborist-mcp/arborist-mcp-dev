from __future__ import annotations

from collections.abc import Callable, Sequence
from typing import Any

from .mcp_validation import reject_unexpected_params
from .tool_manifest import build_resource_catalog
from .tool_result_schemas import JsonRpcError
from .tool_specs import (
    MCP_INITIALIZED_PARAM_NAMES,
    MCP_INITIALIZE_MARKERS,
    MCP_INITIALIZE_PARAM_NAMES,
    MCP_PROTOCOL_VERSION,
    TOOL_NAMES,
)


def is_mcp_initialize(params: dict[str, Any]) -> bool:
    return bool(MCP_INITIALIZE_MARKERS & set(params))


def initialize(
    params: dict[str, Any],
    *,
    server_info: dict[str, Any],
    supported_languages: Callable[[], Sequence[str]],
) -> dict[str, Any]:
    if not is_mcp_initialize(params):
        reject_unexpected_params(params, ())
        return {
            "serverInfo": server_info,
            "capabilities": {
                "tools": list(TOOL_NAMES),
                "resources": build_resource_catalog(),
            },
            "supportedLanguages": list(supported_languages()),
        }

    reject_unexpected_params(params, MCP_INITIALIZE_PARAM_NAMES)
    _optional_string(
        params,
        "protocolVersion",
        default=MCP_PROTOCOL_VERSION,
    )
    capabilities = params.get("capabilities", {})
    if not isinstance(capabilities, dict):
        raise JsonRpcError(-32602, "invalid params: capabilities must be an object")
    client_info = params.get("clientInfo", {})
    if not isinstance(client_info, dict):
        raise JsonRpcError(-32602, "invalid params: clientInfo must be an object")

    return {
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "capabilities": {
            "tools": {
                "listChanged": False,
            },
            "resources": {
                "subscribe": False,
                "listChanged": False,
            },
        },
        "serverInfo": server_info,
        "instructions": (
            "Use tools/list to discover Arborist tools and tools/call with "
            "arguments matching each tool inputSchema."
        ),
        "supportedLanguages": list(supported_languages()),
    }


def initialized(params: dict[str, Any]) -> dict[str, Any]:
    reject_unexpected_params(params, MCP_INITIALIZED_PARAM_NAMES)
    return {}


def server_info(version: str) -> dict[str, Any]:
    return {
        "name": "arborist-mcp",
        "version": version,
    }


def _optional_string(
    params: dict[str, Any],
    key: str,
    default: str | None = None,
    allow_empty: bool = False,
) -> str | None:
    value = params[key] if key in params else default
    if value is None:
        if key in params and default is not None:
            raise JsonRpcError(-32602, f"invalid string param: {key}")
        return None
    if not isinstance(value, str) or (not allow_empty and not value.strip()):
        raise JsonRpcError(-32602, f"invalid string param: {key}")
    return value
