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
from .index_watch_runtime import (
    IndexWatchCore,
    _check_watch_targets as _check_watch_targets_runtime,
    _health_summary,
    _reconcile_index as _reconcile_index_runtime,
    _run_watch_targets as _run_watch_targets_runtime,
)
from .tool_specs import (
    MAX_INDEX_WATCH_CONFIG_BYTES,
    MAX_INDEX_WATCH_TARGETS,
    MAX_WORKSPACE_SCAN_FILE_BYTES,
    MAX_WORKSPACE_SCAN_FILES,
    MAX_WORKSPACE_SCAN_TIMEOUT_MS,
)


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
    return _reconcile_index_runtime(
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
    _run_watch_targets_runtime(
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
    return _check_watch_targets_runtime(
        core,
        targets=targets,
        max_files=max_files,
        max_file_bytes=max_file_bytes,
        timeout_ms=timeout_ms,
        emit=emit,
        include_workspace_root=include_workspace_root,
        bindings=globals(),
    )


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
