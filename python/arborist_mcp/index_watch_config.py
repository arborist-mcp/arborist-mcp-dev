from __future__ import annotations

from dataclasses import dataclass
import os
from pathlib import Path
from typing import Any

from .jsonrpc import loads_strict
from .tool_specs import MAX_INDEX_WATCH_CONFIG_BYTES, MAX_INDEX_WATCH_TARGETS


class IndexWatchError(RuntimeError):
    pass


@dataclass(frozen=True)
class IndexWatchTarget:
    workspace_root: str
    db_path: str


def _decode_object(payload: str, operation: str) -> dict[str, Any]:
    try:
        value = loads_strict(payload)
    except (TypeError, ValueError) as exc:
        detail = str(exc)
        if detail.startswith("duplicate JSON object key:"):
            detail = (
                detail.replace("duplicate JSON object key:", "duplicate object key:", 1)
                + " (duplicate JSON object key)"
            )
        raise IndexWatchError(f"invalid JSON from {operation}: {detail}") from exc
    if not isinstance(value, dict):
        raise IndexWatchError(f"invalid JSON from {operation}: expected object payload")
    return value


def _resolve_path(value: str, base_directory: Path) -> str:
    path = Path(value)
    if not path.is_absolute():
        path = base_directory / path
    return str(path.resolve(strict=False))


def _target_sort_key(target: IndexWatchTarget) -> tuple[str, str, str, str]:
    return (
        os.path.normcase(target.workspace_root),
        target.workspace_root,
        os.path.normcase(target.db_path),
        target.db_path,
    )


def load_watch_config(config_path: Path) -> tuple[IndexWatchTarget, ...]:
    try:
        with config_path.open("rb") as handle:
            raw_payload = handle.read(MAX_INDEX_WATCH_CONFIG_BYTES + 1)
        if len(raw_payload) > MAX_INDEX_WATCH_CONFIG_BYTES:
            raise IndexWatchError(
                f"watch config exceeds maximum size of {MAX_INDEX_WATCH_CONFIG_BYTES} bytes"
            )
        payload = raw_payload.decode("utf-8")
    except IndexWatchError:
        raise
    except (OSError, UnicodeError) as exc:
        raise IndexWatchError(
            f"failed to read watch config {config_path}: {exc}"
        ) from exc

    config = _decode_object(payload, f"watch config {config_path}")
    if set(config) != {"indexes"}:
        unexpected = sorted(set(config) - {"indexes"})
        missing = "indexes" not in config
        if missing:
            raise IndexWatchError("invalid watch config: missing `indexes`")
        raise IndexWatchError(
            f"invalid watch config: unexpected field `{unexpected[0]}`"
        )

    raw_indexes = config["indexes"]
    if not isinstance(raw_indexes, list) or not raw_indexes:
        raise IndexWatchError(
            "invalid watch config: `indexes` must be a non-empty list"
        )
    if len(raw_indexes) > MAX_INDEX_WATCH_TARGETS:
        raise IndexWatchError(
            "invalid watch config: `indexes` must contain at most "
            f"{MAX_INDEX_WATCH_TARGETS} entries"
        )

    targets: list[IndexWatchTarget] = []
    seen_workspaces: set[str] = set()
    seen_databases: set[str] = set()
    for index, raw_index in enumerate(raw_indexes):
        if not isinstance(raw_index, dict):
            raise IndexWatchError(
                f"invalid watch config: indexes[{index}] must be an object"
            )
        if set(raw_index) != {"workspace_root", "db_path"}:
            unexpected = sorted(set(raw_index) - {"workspace_root", "db_path"})
            if unexpected:
                raise IndexWatchError(
                    f"invalid watch config: indexes[{index}] has unexpected field `{unexpected[0]}`"
                )
            missing = sorted({"workspace_root", "db_path"} - set(raw_index))[0]
            raise IndexWatchError(
                f"invalid watch config: indexes[{index}] is missing `{missing}`"
            )

        values: dict[str, str] = {}
        for key in ("workspace_root", "db_path"):
            value = raw_index[key]
            if not isinstance(value, str) or not value.strip():
                raise IndexWatchError(
                    f"invalid watch config: indexes[{index}].{key} must be a non-empty string"
                )
            values[key] = _resolve_path(value, config_path.parent)

        target = IndexWatchTarget(values["workspace_root"], values["db_path"])
        workspace_key = os.path.normcase(target.workspace_root)
        if workspace_key in seen_workspaces:
            raise IndexWatchError(
                f"invalid watch config: duplicate workspace_root `{target.workspace_root}`"
            )
        database_key = os.path.normcase(target.db_path)
        if database_key in seen_databases:
            raise IndexWatchError(
                f"invalid watch config: duplicate db_path `{target.db_path}`"
            )
        seen_workspaces.add(workspace_key)
        seen_databases.add(database_key)
        targets.append(target)

    targets.sort(key=_target_sort_key)
    return tuple(targets)


def _ordered_watch_targets(
    targets: tuple[IndexWatchTarget, ...],
) -> tuple[IndexWatchTarget, ...]:
    if not targets:
        raise IndexWatchError("index watch requires at least one target")
    return tuple(sorted(targets, key=_target_sort_key))


# Preserve established import, introspection, and pickle identities after extraction.
for _compatibility_symbol in (
    IndexWatchError,
    IndexWatchTarget,
    _decode_object,
    _resolve_path,
    _target_sort_key,
    load_watch_config,
    _ordered_watch_targets,
):
    _compatibility_symbol.__module__ = "arborist_mcp.index_watch"
del _compatibility_symbol
