from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
import os
import sys
import time
from pathlib import Path
from typing import Any, Callable, Protocol, TextIO

from .tool_specs import (
    MAX_WORKSPACE_SCAN_FILE_BYTES,
    MAX_WORKSPACE_SCAN_FILES,
    MAX_WORKSPACE_SCAN_TIMEOUT_MS,
)


class IndexWatchCore(Protocol):
    def inspect_symbol_index_json(
        self, db_path: str, timeout_ms: int | None = None
    ) -> str: ...

    def migrate_symbol_index_json(self, db_path: str) -> str: ...

    def refresh_symbol_index_json(
        self,
        workspace_root: str,
        db_path: str,
        max_files: int,
        max_file_bytes: int | None,
        timeout_ms: int | None,
    ) -> str: ...


class IndexWatchError(RuntimeError):
    pass


@dataclass(frozen=True)
class IndexWatchTarget:
    workspace_root: str
    db_path: str


def _reject_constant(value: str) -> None:
    raise ValueError(f"non-standard JSON constant: {value}")


def _decode_object(payload: str, operation: str) -> dict[str, Any]:
    try:
        value = json.loads(
            payload,
            parse_constant=_reject_constant,
            object_pairs_hook=_reject_duplicate_keys,
        )
    except (TypeError, ValueError) as exc:
        raise IndexWatchError(f"invalid JSON from {operation}: {exc}") from exc
    if not isinstance(value, dict):
        raise IndexWatchError(f"invalid JSON from {operation}: expected object payload")
    return value


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate object key: {key}")
        result[key] = value
    return result


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
        payload = config_path.read_text(encoding="utf-8")
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
            migrated_health = _decode_object(
                core.migrate_symbol_index_json(db_path), "migrate_symbol_index"
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


def _ordered_watch_targets(
    targets: tuple[IndexWatchTarget, ...],
) -> tuple[IndexWatchTarget, ...]:
    if not targets:
        raise IndexWatchError("index watch requires at least one target")
    return tuple(sorted(targets, key=_target_sort_key))


def _positive_int(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("value must be greater than zero")
    return parsed


def _bounded_positive_int(value: str, maximum: int) -> int:
    parsed = _positive_int(value)
    if parsed > maximum:
        raise argparse.ArgumentTypeError(f"value must not exceed {maximum}")
    return parsed


def _positive_float(value: str) -> float:
    parsed = float(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("value must be greater than zero")
    return parsed


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Poll and incrementally refresh an Arborist SQLite symbol index."
    )
    parser.add_argument(
        "--workspace-root",
        type=Path,
        default=Path("."),
        help="Workspace root to scan (default: current directory).",
    )
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument(
        "--db-path",
        type=Path,
        help="SQLite symbol-index database path for single-index watch mode.",
    )
    source.add_argument(
        "--config",
        type=Path,
        dest="config_path",
        help="JSON watch manifest containing multiple workspace/index pairs.",
    )
    parser.add_argument(
        "--interval-seconds",
        type=_positive_float,
        default=1.0,
        help="Polling interval in seconds (default: 1).",
    )
    parser.add_argument(
        "--max-files",
        type=lambda value: _bounded_positive_int(value, MAX_WORKSPACE_SCAN_FILES),
        default=20_000,
        help="Maximum source files to scan per refresh (default: 20000).",
    )
    parser.add_argument(
        "--max-file-bytes",
        type=lambda value: _bounded_positive_int(
            value, MAX_WORKSPACE_SCAN_FILE_BYTES
        ),
        default=None,
        help="Optional maximum source file size in bytes.",
    )
    parser.add_argument(
        "--timeout-ms",
        type=lambda value: _bounded_positive_int(value, MAX_WORKSPACE_SCAN_TIMEOUT_MS),
        default=None,
        help="Optional cooperative health and workspace scan timeout in milliseconds.",
    )
    parser.add_argument(
        "--once",
        action="store_true",
        help="Inspect and reconcile once, then exit.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Report refresh or migration actions without writing the index.",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Check configured targets without writing; exit nonzero unless all are healthy.",
    )
    return parser


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
