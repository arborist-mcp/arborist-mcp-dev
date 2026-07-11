from __future__ import annotations

import json
import sys
from typing import Any


def is_notification_request(request: Any) -> bool:
    return (
        isinstance(request, dict)
        and request.get("jsonrpc") == "2.0"
        and "id" not in request
        and isinstance(request.get("method"), str)
        and bool(request.get("method"))
    )


def is_valid_request_id(request_id: Any) -> bool:
    if request_id is None or isinstance(request_id, str):
        return True

    if isinstance(request_id, bool):
        return False

    if isinstance(request_id, int):
        return True

    return False


def error_response(request_id: Any, code: int, message: str) -> dict[str, Any]:
    return {
        "jsonrpc": "2.0",
        "id": request_id if is_valid_request_id(request_id) else None,
        "error": {
            "code": code,
            "message": message,
        },
    }


def _reject_nonstandard_json_constant(name: str) -> Any:
    raise ValueError(f"non-standard JSON constant: {name}")


def _reject_duplicate_object_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    obj: dict[str, Any] = {}
    for key, value in pairs:
        if key in obj:
            raise ValueError(f"duplicate JSON object key: {key}")
        obj[key] = value
    return obj


def parse_request_json(raw_request: str) -> tuple[Any | None, dict[str, Any] | None]:
    try:
        return json.loads(
            raw_request,
            parse_constant=_reject_nonstandard_json_constant,
            object_pairs_hook=_reject_duplicate_object_keys,
        ), None
    except (json.JSONDecodeError, ValueError) as exc:
        return None, error_response(None, -32700, f"invalid JSON: {exc}")


def serialize_response(response: dict[str, Any], indent: int | None = None) -> str:
    try:
        return json.dumps(response, ensure_ascii=False, allow_nan=False, indent=indent)
    except (TypeError, ValueError) as exc:
        fallback = error_response(
            response.get("id"),
            -32000,
            f"failed to serialize response: {exc}",
        )
        return json.dumps(fallback, ensure_ascii=False, allow_nan=False, indent=indent)


def write_response(payload: str) -> bool:
    try:
        sys.stdout.write(payload)
        sys.stdout.flush()
    except BrokenPipeError:
        return False
    return True


def print_response(payload: str) -> bool:
    try:
        print(payload)
    except BrokenPipeError:
        return False
    return True
