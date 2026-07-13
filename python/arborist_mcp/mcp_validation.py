from __future__ import annotations

from typing import Any

from .tool_result_schemas import JsonRpcError


def reject_unexpected_params(params: dict[str, Any], allowed_keys: tuple[str, ...]) -> None:
    unexpected_keys = set(params) - set(allowed_keys)
    if unexpected_keys:
        key = sorted(unexpected_keys)[0]
        raise JsonRpcError(-32602, f"unexpected param: {key}")
