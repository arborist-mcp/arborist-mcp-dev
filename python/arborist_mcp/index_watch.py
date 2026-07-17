from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path
from typing import Any, Callable, Protocol, TextIO

from .tool_specs import MAX_WORKSPACE_SCAN_FILE_BYTES, MAX_WORKSPACE_SCAN_FILES


class IndexWatchCore(Protocol):
    def inspect_symbol_index_json(self, db_path: str) -> str: ...

    def migrate_symbol_index_json(self, db_path: str) -> str: ...

    def refresh_symbol_index_json(
        self,
        workspace_root: str,
        db_path: str,
        max_files: int,
        max_file_bytes: int | None,
    ) -> str: ...


class IndexWatchError(RuntimeError):
    pass


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
) -> dict[str, Any]:
    try:
        health = _decode_object(
            core.inspect_symbol_index_json(db_path), "inspect_symbol_index"
        )
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

    try:
        stats = _decode_object(
            core.refresh_symbol_index_json(
                workspace_root, db_path, max_files, max_file_bytes
            ),
            "refresh_symbol_index",
        )
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
    sleep: Callable[[float], None] = time.sleep,
    emit: Callable[[dict[str, Any]], None] = lambda event: print(
        json.dumps(event, ensure_ascii=False, allow_nan=False)
    ),
) -> None:
    first_cycle = True
    while True:
        event = reconcile_index(
            core,
            workspace_root=workspace_root,
            db_path=db_path,
            max_files=max_files,
            max_file_bytes=max_file_bytes,
        )
        if first_cycle or event["status"] != "healthy":
            emit(event)
        first_cycle = False
        if once:
            return
        sleep(interval_seconds)


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
    parser.add_argument(
        "--db-path",
        type=Path,
        required=True,
        help="SQLite symbol-index database path.",
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
        "--once",
        action="store_true",
        help="Inspect and reconcile once, then exit.",
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
        run_watch(
            core_factory(),
            workspace_root=str(args.workspace_root),
            db_path=str(args.db_path),
            interval_seconds=args.interval_seconds,
            max_files=args.max_files,
            max_file_bytes=args.max_file_bytes,
            once=args.once,
            sleep=sleep,
            emit=emit,
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
