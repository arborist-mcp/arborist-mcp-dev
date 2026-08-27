from __future__ import annotations

import json
import math
import time
from typing import Any, Callable, Mapping, Protocol

from .index_watch_config import (
    IndexWatchError,
    IndexWatchTarget,
    _decode_object,
    _ordered_watch_targets,
    _target_identity_path,
)


_SYMBOL_INDEX_HEALTH_RESPONSE_SCHEMA_VERSION = "4"

_HEALTH_FIELDS = frozenset(
    {
        "response_schema_version",
        "db_path",
        "exists",
        "ok",
        "schema_version",
        "expected_schema_version",
        "migration",
        "workspace_root",
        "indexed_files",
        "indexed_symbols",
        "file_state_entries",
        "fresh_file_count",
        "stale_files",
        "missing_files",
        "unreadable_files",
        "unindexed_files",
        "issues",
    }
)
_MIGRATION_FIELDS = frozenset({"required", "action", "reason"})
_REFRESH_STATS_FIELDS = frozenset(
    {
        "db_path",
        "indexed_files",
        "indexed_symbols",
        "rebuilt_files",
        "reused_files",
    }
)


class IndexWatchCore(Protocol):
    def inspect_symbol_index_json(
        self, db_path: str, timeout_ms: int | None = None
    ) -> str: ...

    def migrate_symbol_index_json(
        self, db_path: str, timeout_ms: int | None = None
    ) -> str: ...

    def refresh_symbol_index_json(
        self,
        workspace_root: str,
        db_path: str,
        max_files: int,
        max_file_bytes: int | None,
        timeout_ms: int | None,
    ) -> str: ...


def _reject_unknown_fields(
    payload: dict[str, Any],
    expected_fields: frozenset[str],
    operation: str,
    object_name: str,
) -> None:
    unexpected = sorted(set(payload) - expected_fields)
    if unexpected:
        raise IndexWatchError(
            f"invalid {object_name} payload from {operation}: "
            f"unexpected field `{unexpected[0]}`"
        )


def _reject_missing_fields(
    payload: dict[str, Any],
    expected_fields: frozenset[str],
    operation: str,
    object_name: str,
) -> None:
    missing = sorted(expected_fields - set(payload))
    if missing:
        raise IndexWatchError(
            f"invalid {object_name} payload from {operation}: "
            f"missing field `{missing[0]}`"
        )


def _validate_health_payload(
    health: dict[str, Any],
    operation: str,
    *,
    expected_db_path: str | None = None,
) -> None:
    _reject_unknown_fields(health, _HEALTH_FIELDS, operation, "health")
    if not isinstance(health.get("ok"), bool):
        raise IndexWatchError(
            f"invalid health payload from {operation}: `ok` must be a boolean"
        )
    exists = health.get("exists")
    if not isinstance(exists, bool):
        raise IndexWatchError(
            f"invalid health payload from {operation}: `exists` must be a boolean"
        )
    schema_version = health.get("schema_version")
    if schema_version is not None and not isinstance(schema_version, str):
        raise IndexWatchError(
            f"invalid health payload from {operation}: `schema_version` must be a string or null"
        )
    expected_schema_version = health.get("expected_schema_version")
    if not isinstance(expected_schema_version, str) or not expected_schema_version.strip():
        raise IndexWatchError(
            f"invalid health payload from {operation}: `expected_schema_version` must be a non-empty string"
        )
    for field_name in (
        "issues",
        "stale_files",
        "missing_files",
        "unreadable_files",
        "unindexed_files",
    ):
        values = health.get(field_name)
        if not isinstance(values, list) or any(
            not isinstance(value, str) or not value.strip() for value in values
        ):
            raise IndexWatchError(
                f"invalid health payload from {operation}: "
                f"`{field_name}` must be a list of strings"
            )
    migration = health.get("migration")
    if not isinstance(migration, dict):
        raise IndexWatchError(
            f"invalid health payload from {operation}: `migration` must be an object"
        )
    _reject_unknown_fields(
        migration,
        _MIGRATION_FIELDS,
        operation,
        "health migration",
    )
    action = migration.get("action")
    if not isinstance(action, str) or not action.strip():
        raise IndexWatchError(
            f"invalid health payload from {operation}: "
            "`migration.action` must be a non-empty string"
        )
    if action not in {"none", "migrate", "rebuild", "manual"}:
        raise IndexWatchError(
            f"invalid health payload from {operation}: "
            f"unsupported `migration.action`: {action}"
        )
    reason = migration.get("reason")
    if not isinstance(reason, str) or not reason.strip():
        raise IndexWatchError(
            f"invalid health payload from {operation}: "
            "`migration.reason` must be a non-empty string"
        )
    required = migration.get("required")
    if not isinstance(required, bool):
        raise IndexWatchError(
            f"invalid health payload from {operation}: "
            "`migration.required` must be a boolean"
        )
    if required is False and action != "none":
        raise IndexWatchError(
            f"invalid health payload from {operation}: "
            "optional migration must use action `none`"
        )
    if required is True and action == "none":
        raise IndexWatchError(
            f"invalid health payload from {operation}: "
            "required migration must use a concrete action"
        )
    if health["ok"] is True and (not exists or action != "none"):
        raise IndexWatchError(
            f"invalid health payload from {operation}: "
            "healthy indexes must be existing indexes with migration action `none`"
        )
    response_schema_version = health.get("response_schema_version")
    if response_schema_version != _SYMBOL_INDEX_HEALTH_RESPONSE_SCHEMA_VERSION:
        raise IndexWatchError(
            f"invalid health payload from {operation}: "
            "`response_schema_version` does not match the supported response schema"
        )
    db_path = health.get("db_path")
    if not isinstance(db_path, str) or not db_path.strip():
        raise IndexWatchError(
            f"invalid health payload from {operation}: `db_path` must be a non-empty string"
        )
    if expected_db_path is not None:
        try:
            paths_match = _target_identity_path(db_path) == _target_identity_path(
                expected_db_path
            )
        except (OSError, RuntimeError, ValueError):
            paths_match = False
        if not paths_match:
            raise IndexWatchError(
                f"invalid health payload from {operation}: "
                "`db_path` does not match the requested database path"
            )

    for field_name in (
        "workspace_root",
        "indexed_files",
        "indexed_symbols",
        "file_state_entries",
        "fresh_file_count",
    ):
        value = health.get(field_name)
        if field_name == "workspace_root":
            valid = value is None or (isinstance(value, str) and bool(value.strip()))
            expected = "a non-empty string or null"
        else:
            valid = value is None or (
                isinstance(value, int) and not isinstance(value, bool) and value >= 0
            )
            expected = "a non-negative integer or null"
        if not valid:
            raise IndexWatchError(
                f"invalid health payload from {operation}: "
                f"`{field_name}` must be {expected}"
            )

    if health["ok"] is True and (
        health.get("issues")
        or required
        or health.get("schema_version") != expected_schema_version
        or health.get("workspace_root") is None
        or any(
            health.get(field_name) is None
            for field_name in (
                "indexed_files",
                "indexed_symbols",
                "file_state_entries",
                "fresh_file_count",
            )
        )
    ):
        raise IndexWatchError(
            f"invalid health payload from {operation}: "
            "healthy indexes must report a complete current inspection"
        )
    if health["ok"] is False and not health["issues"]:
        raise IndexWatchError(
            f"invalid health payload from {operation}: "
            "unhealthy indexes must report at least one issue"
        )
    indexed_files = health.get("indexed_files")
    file_state_entries = health.get("file_state_entries")
    if (
        indexed_files is not None
        and file_state_entries is not None
        and indexed_files != file_state_entries
    ):
        raise IndexWatchError(
            f"invalid health payload from {operation}: "
            "`indexed_files` must equal `file_state_entries`"
        )
    if not exists and (
        health.get("schema_version") is not None
        or health.get("workspace_root") is not None
        or any(
            health.get(field_name) is not None
            for field_name in (
                "indexed_files",
                "indexed_symbols",
                "file_state_entries",
                "fresh_file_count",
            )
        )
        or any(
            health.get(field_name)
            for field_name in (
                "stale_files",
                "missing_files",
                "unreadable_files",
                "unindexed_files",
            )
        )
    ):
        raise IndexWatchError(
            f"invalid health payload from {operation}: "
            "missing indexes must not report loaded metadata"
        )
    if not exists and action != "rebuild":
        raise IndexWatchError(
            f"invalid health payload from {operation}: "
            "missing indexes must recommend rebuild"
        )
    fresh_file_count = health.get("fresh_file_count")
    file_state_entries = health.get("file_state_entries")
    if fresh_file_count is not None:
        if file_state_entries is None:
            raise IndexWatchError(
                f"invalid health payload from {operation}: "
                "`fresh_file_count` requires `file_state_entries`"
            )
        if (
            fresh_file_count
            + len(health["stale_files"])
            + len(health["missing_files"])
            + len(health["unreadable_files"])
            != file_state_entries
        ):
            raise IndexWatchError(
                f"invalid health payload from {operation}: "
                "freshness counts must equal `file_state_entries`"
            )

    freshness_paths: set[str] = set()
    for field_name in (
        "stale_files",
        "missing_files",
        "unreadable_files",
        "unindexed_files",
    ):
        for index, value in enumerate(health[field_name]):
            try:
                path_identity = _target_identity_path(value)
            except (OSError, RuntimeError, ValueError):
                raise IndexWatchError(
                    f"invalid health payload from {operation}: "
                    f"`{field_name}[{index}]` must be a valid path"
                ) from None
            if path_identity in freshness_paths:
                raise IndexWatchError(
                    f"invalid health payload from {operation}: "
                    "freshness file paths must be unique"
                )
            freshness_paths.add(path_identity)
    _reject_missing_fields(health, _HEALTH_FIELDS, operation, "health")


def _validate_refresh_stats_payload(
    stats: dict[str, Any],
    operation: str,
    *,
    expected_db_path: str | None = None,
) -> None:
    _reject_unknown_fields(stats, _REFRESH_STATS_FIELDS, operation, "stats")
    db_path = stats.get("db_path")
    if not isinstance(db_path, str) or not db_path.strip():
        raise IndexWatchError(
            f"invalid stats payload from {operation}: `db_path` must be a non-empty string"
        )
    if expected_db_path is not None:
        try:
            paths_match = _target_identity_path(db_path) == _target_identity_path(
                expected_db_path
            )
        except (OSError, RuntimeError, ValueError):
            paths_match = False
        if not paths_match:
            raise IndexWatchError(
                f"invalid stats payload from {operation}: "
                "`db_path` does not match the requested database path"
            )
    counts: dict[str, int] = {}
    for field_name in (
        "indexed_files",
        "indexed_symbols",
        "rebuilt_files",
        "reused_files",
    ):
        value = stats.get(field_name)
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            raise IndexWatchError(
                f"invalid stats payload from {operation}: "
                f"`{field_name}` must be a non-negative integer"
            )
        counts[field_name] = value
    if counts["rebuilt_files"] + counts["reused_files"] != counts["indexed_files"]:
        raise IndexWatchError(
            f"invalid stats payload from {operation}: "
            "`indexed_files` must equal `rebuilt_files` + `reused_files`"
        )


def _health_summary(health: dict[str, Any]) -> dict[str, Any]:
    return {
        "ok": health.get("ok"),
        "issues": len(health.get("issues", []))
        if isinstance(health.get("issues"), list)
        else None,
        "stale_files": len(health.get("stale_files", []))
        if isinstance(health.get("stale_files"), list)
        else None,
        "missing_files": len(health.get("missing_files", []))
        if isinstance(health.get("missing_files"), list)
        else None,
        "unreadable_files": len(health.get("unreadable_files", []))
        if isinstance(health.get("unreadable_files"), list)
        else None,
        "unindexed_files": len(health.get("unindexed_files", []))
        if isinstance(health.get("unindexed_files"), list)
        else None,
    }


def _reconcile_index(
    core: IndexWatchCore,
    *,
    workspace_root: str,
    db_path: str,
    max_files: int,
    max_file_bytes: int | None,
    timeout_ms: int | None,
    dry_run: bool,
    bindings: Mapping[str, Any],
) -> dict[str, Any]:
    try:
        if timeout_ms is None:
            health_payload = core.inspect_symbol_index_json(db_path)
        else:
            health_payload = core.inspect_symbol_index_json(db_path, timeout_ms)
        health = bindings["_decode_object"](
            health_payload, "inspect_symbol_index"
        )
        bindings["_validate_health_payload"](
            health,
            "inspect_symbol_index",
            expected_db_path=db_path,
        )
    except bindings["IndexWatchError"]:
        raise
    except Exception as exc:  # noqa: BLE001
        raise bindings["IndexWatchError"](
            f"failed to inspect symbol index: {exc}"
        ) from exc

    if health.get("ok") is True:
        return {
            "status": "healthy",
            "db_path": db_path,
            "health": bindings["_health_summary"](health),
        }

    migration = health.get("migration")
    action = migration.get("action") if isinstance(migration, dict) else None
    schema_version = health.get("schema_version")
    expected_schema_version = health.get("expected_schema_version")
    has_unsupported_schema = (
        health.get("exists") is True
        and isinstance(schema_version, str)
        and isinstance(expected_schema_version, str)
        and schema_version != expected_schema_version
    )
    if action == "migrate":
        if dry_run:
            return {
                "status": "would_migrate",
                "db_path": db_path,
                "health": bindings["_health_summary"](health),
            }
        try:
            if timeout_ms is None:
                migration_payload = core.migrate_symbol_index_json(db_path)
            else:
                migration_payload = core.migrate_symbol_index_json(db_path, timeout_ms)
            migrated_health = bindings["_decode_object"](
                migration_payload, "migrate_symbol_index"
            )
            bindings["_validate_health_payload"](
                migrated_health,
                "migrate_symbol_index",
                expected_db_path=db_path,
            )
        except bindings["IndexWatchError"]:
            raise
        except Exception as exc:  # noqa: BLE001
            raise bindings["IndexWatchError"](
                f"failed to migrate symbol index: {exc}"
            ) from exc

        if migrated_health.get("ok") is not True:
            issues = migrated_health.get("issues")
            reason = (
                issues[0]
                if isinstance(issues, list)
                and issues
                and isinstance(issues[0], str)
                else "migration did not produce a healthy index"
            )
            raise bindings["IndexWatchError"](
                f"index watch migration completed unsuccessfully: {reason}"
            )

        return {
            "status": "migrated",
            "db_path": db_path,
            "health": bindings["_health_summary"](health),
            "migrated_health": bindings["_health_summary"](migrated_health),
        }

    if action != "rebuild" or has_unsupported_schema:
        reason = migration.get("reason") if isinstance(migration, dict) else None
        if not isinstance(reason, str) or not reason.strip():
            issues = health.get("issues")
            reason = issues[0] if isinstance(issues, list) and issues else "index is unhealthy"
        raise bindings["IndexWatchError"](
            f"index watch cannot repair this index: {reason}"
        )

    if dry_run:
        return {
            "status": "would_refresh",
            "db_path": db_path,
            "health": bindings["_health_summary"](health),
        }

    try:
        if timeout_ms is None:
            refresh_payload = core.refresh_symbol_index_json(
                workspace_root, db_path, max_files, max_file_bytes
            )
        else:
            refresh_payload = core.refresh_symbol_index_json(
                workspace_root,
                db_path,
                max_files,
                max_file_bytes,
                timeout_ms,
            )
        stats = bindings["_decode_object"](
            refresh_payload, "refresh_symbol_index"
        )
        bindings["_validate_refresh_stats_payload"](
            stats,
            "refresh_symbol_index",
            expected_db_path=db_path,
        )
    except bindings["IndexWatchError"]:
        raise
    except Exception as exc:  # noqa: BLE001
        raise bindings["IndexWatchError"](
            f"failed to refresh symbol index: {exc}"
        ) from exc

    return {
        "status": "refreshed",
        "db_path": db_path,
        "health": bindings["_health_summary"](health),
        "stats": stats,
    }


def reconcile_index(
    core: IndexWatchCore,
    *,
    workspace_root: str,
    db_path: str,
    max_files: int,
    max_file_bytes: int | None,
    timeout_ms: int | None = None,
    dry_run: bool = False,
) -> dict[str, Any]:
    return _reconcile_index(
        core,
        workspace_root=workspace_root,
        db_path=db_path,
        max_files=max_files,
        max_file_bytes=max_file_bytes,
        timeout_ms=timeout_ms,
        dry_run=dry_run,
        bindings=globals(),
    )


def run_watch(
    core: IndexWatchCore,
    *,
    workspace_root: str,
    db_path: str,
    interval_seconds: float,
    max_files: int,
    max_file_bytes: int | None,
    once: bool,
    dry_run: bool = False,
    timeout_ms: int | None = None,
    sleep: Callable[[float], None] = time.sleep,
    emit: Callable[[dict[str, Any]], None] = lambda event: print(
        json.dumps(event, ensure_ascii=False, allow_nan=False)
    ),
) -> None:
    run_watch_targets(
        core,
        targets=(IndexWatchTarget(workspace_root, db_path),),
        interval_seconds=interval_seconds,
        max_files=max_files,
        max_file_bytes=max_file_bytes,
        timeout_ms=timeout_ms,
        dry_run=dry_run,
        once=once,
        sleep=sleep,
        emit=emit,
        include_workspace_root=False,
    )


def _run_watch_targets(
    core: IndexWatchCore,
    *,
    targets: tuple[IndexWatchTarget, ...],
    interval_seconds: float,
    max_files: int,
    max_file_bytes: int | None,
    once: bool,
    dry_run: bool,
    timeout_ms: int | None,
    sleep: Callable[[float], None],
    emit: Callable[[dict[str, Any]], None],
    include_workspace_root: bool,
    bindings: Mapping[str, Any],
) -> None:
    if not bindings["math"].isfinite(interval_seconds) or interval_seconds <= 0:
        raise bindings["IndexWatchError"](
            "index watch interval_seconds must be a finite number greater than zero"
        )
    ordered_targets = bindings["_ordered_watch_targets"](targets)
    first_cycle = True
    while True:
        for target in ordered_targets:
            event = bindings["reconcile_index"](
                core,
                workspace_root=target.workspace_root,
                db_path=target.db_path,
                max_files=max_files,
                max_file_bytes=max_file_bytes,
                timeout_ms=timeout_ms,
                dry_run=dry_run,
            )
            if include_workspace_root:
                event["workspace_root"] = target.workspace_root
            if first_cycle or event["status"] != "healthy":
                emit(event)
        first_cycle = False
        if once:
            return
        sleep(interval_seconds)


def run_watch_targets(
    core: IndexWatchCore,
    *,
    targets: tuple[IndexWatchTarget, ...],
    interval_seconds: float,
    max_files: int,
    max_file_bytes: int | None,
    once: bool,
    dry_run: bool = False,
    timeout_ms: int | None = None,
    sleep: Callable[[float], None] = time.sleep,
    emit: Callable[[dict[str, Any]], None] = lambda event: print(
        json.dumps(event, ensure_ascii=False, allow_nan=False)
    ),
    include_workspace_root: bool = True,
) -> None:
    _run_watch_targets(
        core,
        targets=targets,
        interval_seconds=interval_seconds,
        max_files=max_files,
        max_file_bytes=max_file_bytes,
        once=once,
        dry_run=dry_run,
        timeout_ms=timeout_ms,
        sleep=sleep,
        emit=emit,
        include_workspace_root=include_workspace_root,
        bindings=globals(),
    )


def _check_watch_targets(
    core: IndexWatchCore,
    *,
    targets: tuple[IndexWatchTarget, ...],
    max_files: int,
    max_file_bytes: int | None,
    timeout_ms: int | None,
    emit: Callable[[dict[str, Any]], None],
    include_workspace_root: bool,
    bindings: Mapping[str, Any],
) -> bool:
    all_healthy = True
    for target in bindings["_ordered_watch_targets"](targets):
        event = bindings["reconcile_index"](
            core,
            workspace_root=target.workspace_root,
            db_path=target.db_path,
            max_files=max_files,
            max_file_bytes=max_file_bytes,
            timeout_ms=timeout_ms,
            dry_run=True,
        )
        if include_workspace_root:
            event["workspace_root"] = target.workspace_root
        emit(event)
        all_healthy = all_healthy and event["status"] == "healthy"
    return all_healthy


def check_watch_targets(
    core: IndexWatchCore,
    *,
    targets: tuple[IndexWatchTarget, ...],
    max_files: int,
    max_file_bytes: int | None,
    timeout_ms: int | None = None,
    emit: Callable[[dict[str, Any]], None] = lambda event: print(
        json.dumps(event, ensure_ascii=False, allow_nan=False)
    ),
    include_workspace_root: bool = True,
) -> bool:
    return _check_watch_targets(
        core,
        targets=targets,
        max_files=max_files,
        max_file_bytes=max_file_bytes,
        timeout_ms=timeout_ms,
        emit=emit,
        include_workspace_root=include_workspace_root,
        bindings=globals(),
    )


# Preserve the protocol and shared helper's established facade identities.
IndexWatchCore.__module__ = "arborist_mcp.index_watch"
_health_summary.__module__ = "arborist_mcp.index_watch"
for _method_name in (
    "inspect_symbol_index_json",
    "migrate_symbol_index_json",
    "refresh_symbol_index_json",
):
    getattr(IndexWatchCore, _method_name).__module__ = "arborist_mcp.index_watch"
del _method_name
