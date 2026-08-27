from __future__ import annotations

import json
import sys
from typing import Any

from .tool_specs import MAX_JSON_ARG_DEPTH, MAX_REQUEST_BYTES


def is_notification_request(request: Any) -> bool:
    return (
        isinstance(request, dict)
        and request.get("jsonrpc") == "2.0"
        and "id" not in request
        and isinstance(request.get("method"), str)
        and bool(request.get("method"))
    )


def is_valid_request_id(request_id: Any) -> bool:
    if request_id is None:
        return True

    if isinstance(request_id, str):
        try:
            request_id.encode("utf-8")
        except UnicodeEncodeError:
            return False
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


def loads_strict(payload: str) -> Any:
    """Parse JSON while rejecting NaN/Infinity constants and duplicate object keys."""
    try:
        value = json.loads(
            payload,
            parse_constant=_reject_nonstandard_json_constant,
            object_pairs_hook=_reject_duplicate_object_keys,
        )
    except RecursionError as exc:
        raise ValueError(
            f"JSON exceeds maximum nesting depth of {MAX_JSON_ARG_DEPTH}"
        ) from exc

    _validate_json_depth(value)
    return value


def _validate_json_depth(value: Any) -> None:
    pending = [(value, 0)]
    while pending:
        current, depth = pending.pop()
        if not isinstance(current, (list, dict)):
            continue
        if depth >= MAX_JSON_ARG_DEPTH:
            raise ValueError(
                f"JSON exceeds maximum nesting depth of {MAX_JSON_ARG_DEPTH}"
            )
        child_depth = depth + 1
        if isinstance(current, list):
            pending.extend((item, child_depth) for item in current)
        else:
            pending.extend((item, child_depth) for item in current.values())


def parse_request_json(raw_request: str) -> tuple[Any | None, dict[str, Any] | None]:
    if len(raw_request.encode("utf-8")) > MAX_REQUEST_BYTES:
        return None, error_response(
            None,
            -32600,
            f"request exceeds maximum size of {MAX_REQUEST_BYTES} bytes",
        )
    try:
        return loads_strict(raw_request), None
    except (json.JSONDecodeError, ValueError) as exc:
        return None, error_response(None, -32700, f"invalid JSON: {exc}")


def serialize_response(response: dict[str, Any], indent: int | None = None) -> str:
    try:
        payload = json.dumps(response, ensure_ascii=False, allow_nan=False, indent=indent)
        payload.encode("utf-8")
        return payload
    except (RecursionError, TypeError, UnicodeEncodeError, ValueError) as exc:
        fallback = error_response(
            response.get("id"),
            -32000,
            f"failed to serialize response: {exc}",
        )
        # ASCII escaping keeps the recovery envelope writable even when the
        # original response contains an unpaired surrogate.
        return json.dumps(fallback, ensure_ascii=True, allow_nan=False, indent=indent)


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
