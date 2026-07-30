from __future__ import annotations

import argparse
import math
from pathlib import Path

from ._version import __version__
from .tool_specs import (
    MAX_WORKSPACE_SCAN_FILE_BYTES,
    MAX_WORKSPACE_SCAN_FILES,
    MAX_WORKSPACE_SCAN_TIMEOUT_MS,
)


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
    if not math.isfinite(parsed) or parsed <= 0:
        raise argparse.ArgumentTypeError(
            "value must be a finite number greater than zero"
        )
    return parsed


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Poll and incrementally refresh an Arborist SQLite symbol index."
    )
    parser.add_argument(
        "--version",
        action="version",
        version=f"%(prog)s {__version__}",
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
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--once",
        action="store_true",
        help="Inspect and reconcile once, then exit.",
    )
    mode.add_argument(
        "--check",
        action="store_true",
        help="Check configured targets without writing; exit nonzero unless all are healthy.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Report refresh or migration actions without writing the index.",
    )
    return parser
