from __future__ import annotations

import json
import math
import sys
import time
from pathlib import Path
from typing import Any, Callable, Protocol, TextIO

from ._version import __version__
from .index_watch_cli_args import (
    _bounded_positive_int,
    _positive_float,
    _positive_int,
    build_parser,
)
from .index_watch_config import (
    IndexWatchError,
    IndexWatchTarget,
    _decode_object,
    _ordered_watch_targets,
    _resolve_path,
    _target_sort_key,
    load_watch_config,
)
from .tool_specs import (
    MAX_INDEX_WATCH_CONFIG_BYTES,
    MAX_INDEX_WATCH_TARGETS,
    MAX_WORKSPACE_SCAN_FILE_BYTES,
    MAX_WORKSPACE_SCAN_FILES,
    MAX_WORKSPACE_SCAN_TIMEOUT_MS,
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
    try:
        if timeout_ms is None:
            health_payload = core.inspect_symbol_index_json(db_path)
        else:
            health_payload = core.inspect_symbol_index_json(db_path, timeout_ms)
        health = _decode_object(health_payload, "inspect_symbol_index")
    except IndexWatchError:
        raise
    except Exception as exc:  # noqa: BLE001
        raise IndexWatchError(f"failed to inspect symbol index: {exc}") from exc

    if health.get("ok") is True:
        return {
            "status": "healthy",
            "db_path": db_path,
            "health": _health_summary(health),
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
                "health": _health_summary(health),
            }
        try:
            if timeout_ms is None:
                migration_payload = core.migrate_symbol_index_json(db_path)
            else:
                migration_payload = core.migrate_symbol_index_json(db_path, timeout_ms)
            migrated_health = _decode_object(
                migration_payload, "migrate_symbol_index"
            )
        except IndexWatchError:
            raise
        except Exception as exc:  # noqa: BLE001
            raise IndexWatchError(f"failed to migrate symbol index: {exc}") from exc

        return {
            "status": "migrated",
            "db_path": db_path,
            "health": _health_summary(health),
            "migrated_health": _health_summary(migrated_health),
        }

    if action != "rebuild" or has_unsupported_schema:
        reason = migration.get("reason") if isinstance(migration, dict) else None
        if not isinstance(reason, str) or not reason.strip():
            issues = health.get("issues")
            reason = issues[0] if isinstance(issues, list) and issues else "index is unhealthy"
        raise IndexWatchError(f"index watch cannot repair this index: {reason}")

    if dry_run:
        return {
            "status": "would_refresh",
            "db_path": db_path,
            "health": _health_summary(health),
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
        stats = _decode_object(refresh_payload, "refresh_symbol_index")
    except IndexWatchError:
        raise
    except Exception as exc:  # noqa: BLE001
        raise IndexWatchError(f"failed to refresh symbol index: {exc}") from exc

    return {
        "status": "refreshed",
        "db_path": db_path,
        "health": _health_summary(health),
        "stats": stats,
    }


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
    if not math.isfinite(interval_seconds) or interval_seconds <= 0:
        raise IndexWatchError(
            "index watch interval_seconds must be a finite number greater than zero"
        )
    ordered_targets = _ordered_watch_targets(targets)
    first_cycle = True
    while True:
        for target in ordered_targets:
            event = reconcile_index(
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
    all_healthy = True
    for target in _ordered_watch_targets(targets):
        event = reconcile_index(
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


def _load_core() -> IndexWatchCore:
    from ._arborist_core import ArboristCore

    return ArboristCore()


def run_cli(
    argv: list[str] | None = None,
    *,
    core_factory: Callable[[], IndexWatchCore] = _load_core,
    stdout: TextIO | None = None,
    stderr: TextIO | None = None,
    sleep: Callable[[float], None] = time.sleep,
) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    output = sys.stdout if stdout is None else stdout
    errors = sys.stderr if stderr is None else stderr

    def emit(event: dict[str, Any]) -> None:
        print(json.dumps(event, ensure_ascii=False, allow_nan=False), file=output)

    try:
        if args.check and args.dry_run:
            raise IndexWatchError("--check cannot be combined with --dry-run")
        if args.check and args.interval_seconds != 1.0:
            raise IndexWatchError("--check cannot be combined with --interval-seconds")
        if args.config_path is not None:
            if args.workspace_root != Path("."):
                raise IndexWatchError(
                    "--workspace-root cannot be combined with --config"
                )
            targets = load_watch_config(args.config_path)
        else:
            current_directory = Path.cwd()
            targets = (
                IndexWatchTarget(
                    _resolve_path(str(args.workspace_root), current_directory),
                    _resolve_path(str(args.db_path), current_directory),
                ),
            )

        core = core_factory()
        if args.check:
            return int(
                not check_watch_targets(
                    core,
                    targets=targets,
                    max_files=args.max_files,
                    max_file_bytes=args.max_file_bytes,
                    timeout_ms=args.timeout_ms,
                    emit=emit,
                    include_workspace_root=args.config_path is not None,
                )
            )

        run_watch_targets(
            core,
            targets=targets,
            interval_seconds=args.interval_seconds,
            max_files=args.max_files,
            max_file_bytes=args.max_file_bytes,
            timeout_ms=args.timeout_ms,
            once=args.once,
            dry_run=args.dry_run,
            sleep=sleep,
            emit=emit,
            include_workspace_root=args.config_path is not None,
        )
    except KeyboardInterrupt:
        return 0
    except (IndexWatchError, OSError, RuntimeError) as exc:
        print(f"error: {exc}", file=errors)
        return 1
    return 0


def main() -> int:
    return run_cli()


if __name__ == "__main__":
    raise SystemExit(main())
