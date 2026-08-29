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


def _contains_invalid_utf8_string(value: Any) -> bool:
    pending = [value]
    visited_containers: set[int] = set()
    while pending:
        current = pending.pop()
        if isinstance(current, str):
            try:
                current.encode("utf-8")
            except UnicodeEncodeError:
                return True
        elif isinstance(current, (list, dict)):
            identity = id(current)
            if identity in visited_containers:
                continue
            visited_containers.add(identity)
            if isinstance(current, list):
                pending.extend(current)
            else:
                pending.extend(current.keys())
                pending.extend(current.values())
    return False


def is_valid_request_id(request_id: Any) -> bool:
    if request_id is None:
        return True

    if isinstance(request_id, str):
        return not _contains_invalid_utf8_string(request_id)

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
            "message": safe_error_text(message),
        },
    }


def safe_error_text(message: str) -> str:
    """Keep protocol error messages writable even when an exception is malformed."""
    try:
        message.encode("utf-8")
    except UnicodeEncodeError:
        return message.encode("utf-8", "backslashreplace").decode("utf-8")
    return message


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
    if _contains_invalid_utf8_string(value):
        raise ValueError("JSON contains a string that is not valid UTF-8")
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
    try:
        request_size = len(raw_request.encode("utf-8"))
    except UnicodeEncodeError:
        return None, error_response(
            None,
            -32700,
            "invalid JSON: request is not valid UTF-8 text",
        )

    if request_size > MAX_REQUEST_BYTES:
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


def _ascii_escape(payload: str) -> str:
    escaped: list[str] = []
    for character in payload:
        codepoint = ord(character)
        if codepoint < 0x80:
            escaped.append(character)
        elif codepoint <= 0xFFFF:
            escaped.append(f"\\u{codepoint:04x}")
        else:
            codepoint -= 0x10000
            high_surrogate = 0xD800 + (codepoint >> 10)
            low_surrogate = 0xDC00 + (codepoint & 0x3FF)
            escaped.append(f"\\u{high_surrogate:04x}\\u{low_surrogate:04x}")
    return "".join(escaped)


def write_response(payload: str) -> bool:
    try:
        sys.stdout.write(payload)
        sys.stdout.flush()
    except BrokenPipeError:
        return False
    except UnicodeEncodeError:
        # Some hosts configure stdout with a legacy encoding. Retry the same
        # JSON document with equivalent ASCII \u escapes rather than losing
        # the protocol response at the text-stream boundary.
        try:
            sys.stdout.write(_ascii_escape(payload))
            sys.stdout.flush()
        except (BrokenPipeError, UnicodeEncodeError):
            return False
    return True


def print_response(payload: str) -> bool:
    try:
        print(payload)
    except BrokenPipeError:
        return False
    except UnicodeEncodeError:
        try:
            print(_ascii_escape(payload))
        except (BrokenPipeError, UnicodeEncodeError):
            return False
    return True
