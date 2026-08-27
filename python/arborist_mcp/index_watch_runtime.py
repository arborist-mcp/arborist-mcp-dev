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


def _validate_health_payload(
    health: dict[str, Any],
    operation: str,
) -> None:
    if not isinstance(health.get("ok"), bool):
        raise IndexWatchError(
            f"invalid health payload from {operation}: `ok` must be a boolean"
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
            not isinstance(value, str) for value in values
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
    action = migration.get("action")
    if not isinstance(action, str) or not action.strip():
        raise IndexWatchError(
            f"invalid health payload from {operation}: "
            "`migration.action` must be a non-empty string"
        )
    reason = migration.get("reason")
    if not isinstance(reason, str):
        raise IndexWatchError(
            f"invalid health payload from {operation}: "
            "`migration.reason` must be a string"
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
        bindings["_validate_health_payload"](health, "inspect_symbol_index")
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
                migrated_health, "migrate_symbol_index"
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
