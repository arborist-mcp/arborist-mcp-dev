from __future__ import annotations

import json
from typing import Any

from .jsonrpc import loads_strict
from .tool_result_schemas import JsonRpcError


class GatewayCoreHelpers:
    """Shared native-core invocation and payload decoding helpers."""

    @staticmethod
    def _call_with_optional_timeout(
        method: Any,
        args: tuple[Any, ...],
        timeout_ms: int | None,
        *,
        omitted_before_timeout: tuple[Any, ...] = (),
    ) -> Any:
        if timeout_ms is None:
            return method(*args)
        return method(*args, *omitted_before_timeout, timeout_ms)

    @staticmethod
    def _decode_core_payload(payload: str) -> Any:
        try:
            return loads_strict(payload)
        except (json.JSONDecodeError, ValueError) as exc:
            raise JsonRpcError(-32000, f"invalid JSON from arborist core: {exc}") from exc

    @staticmethod
    def _decode_core_object(payload: str) -> dict[str, Any]:
        value = GatewayCoreHelpers._decode_core_payload(payload)
        if not isinstance(value, dict):
            raise JsonRpcError(
                -32000,
                "invalid JSON from arborist core: expected object payload",
            )
        return value

    @staticmethod
    def _decode_core_object_array(payload: str) -> list[dict[str, Any]]:
        value = GatewayCoreHelpers._decode_core_payload(payload)
        if not isinstance(value, list):
            raise JsonRpcError(
                -32000,
                "invalid JSON from arborist core: expected array payload",
            )
        for index, item in enumerate(value):
            if not isinstance(item, dict):
                raise JsonRpcError(
                    -32000,
                    f"invalid JSON from arborist core: expected object item at index {index}",
                )
        return value
