from __future__ import annotations

import json
from typing import Any, Callable, TypeVar

T = TypeVar("T")

MAX_JSON_DEPTH = 64


def reject_nonstandard_json_constant(name: str) -> Any:
    raise ValueError(f"non-standard JSON constant: {name}")


def reject_duplicate_object_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    obj: dict[str, Any] = {}
    for key, value in pairs:
        if key in obj:
            raise ValueError(f"duplicate JSON object key: {key}")
        obj[key] = value
    return obj


def loads(payload: str) -> Any:
    """Parse JSON while rejecting NaN/Infinity constants and duplicate object keys."""
    try:
        value = json.loads(
            payload,
            parse_constant=reject_nonstandard_json_constant,
            object_pairs_hook=reject_duplicate_object_keys,
        )
    except RecursionError as exc:
        raise ValueError(
            f"JSON exceeds maximum nesting depth of {MAX_JSON_DEPTH}"
        ) from exc

    pending = [(value, 0)]
    while pending:
        current, depth = pending.pop()
        if not isinstance(current, (list, dict)):
            continue
        if depth >= MAX_JSON_DEPTH:
            raise ValueError(
                f"JSON exceeds maximum nesting depth of {MAX_JSON_DEPTH}"
            )
        child_depth = depth + 1
        if isinstance(current, list):
            pending.extend((item, child_depth) for item in current)
        else:
            pending.extend((item, child_depth) for item in current.values())
    return value


def load_text(
    payload: str,
    *,
    on_error: Callable[[Exception], T] | None = None,
) -> Any | T:
    """Like loads(), but optionally map parse failures through on_error."""
    try:
        return loads(payload)
    except (json.JSONDecodeError, ValueError, TypeError) as exc:
        if on_error is None:
            raise
        return on_error(exc)
