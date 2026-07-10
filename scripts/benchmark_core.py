from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
from pathlib import Path
import statistics
import sys
import tempfile
import time
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from arborist_mcp.gateway import ArboristGateway


@dataclass(frozen=True)
class BenchmarkResult:
    name: str
    iterations: int
    median_ms: float
    min_ms: float
    max_ms: float
    mean_ms: float


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Run lightweight Arborist core workflow benchmarks through the Python gateway. "
            "Requires the native _arborist_core module to be built, for example with "
            "`maturin develop --locked`."
        )
    )
    parser.add_argument(
        "--iterations",
        type=positive_int,
        default=5,
        help="Measured iterations per workflow. Defaults to 5.",
    )
    parser.add_argument(
        "--warmup",
        type=nonnegative_int,
        default=1,
        help="Warmup iterations per workflow. Defaults to 1.",
    )
    parser.add_argument(
        "--modules",
        type=positive_int,
        default=20,
        help="Number of generated Python modules in the benchmark workspace. Defaults to 20.",
    )
    parser.add_argument(
        "--workspace",
        type=Path,
        help="Use this workspace directory instead of a temporary directory.",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Print machine-readable JSON instead of a text table.",
    )
    parser.add_argument(
        "--keep-workspace",
        action="store_true",
        help="Keep the temporary workspace after the benchmark finishes.",
    )
    return parser


def positive_int(value: str) -> int:
    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("value must be greater than zero")
    return parsed


def nonnegative_int(value: str) -> int:
    parsed = int(value)
    if parsed < 0:
        raise argparse.ArgumentTypeError("value must not be negative")
    return parsed


def write_workspace(workspace: Path, modules: int, revision: int) -> Path:
    workspace.mkdir(parents=True, exist_ok=True)
    for index in range(modules):
        source = (
            f"def helper_{index}(value):\n"
            f"    return value + {index + revision}\n\n"
            f"def caller_{index}(value):\n"
            f"    return helper_{index}(value)\n"
        )
        (workspace / f"module_{index}.py").write_text(
            source, encoding="utf-8", newline="\n"
        )

    app_source = (
        "from module_0 import helper_0\n\n"
        "def orchestrate(value):\n"
        "    return helper_0(value)\n"
    )
    (workspace / "app.py").write_text(app_source, encoding="utf-8", newline="\n")
    return workspace / "module_0.py"


def call_gateway(
    gateway: ArboristGateway,
    method: str,
    params: dict[str, Any],
    request_id: int,
) -> Any:
    response = gateway.handle_request(
        {
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params,
        }
    )
    if "error" in response:
        message = response["error"].get("message", "unknown gateway error")
        raise RuntimeError(f"{method} failed: {message}")
    return response["result"]


def measure(
    name: str,
    iterations: int,
    warmup: int,
    run_once,
) -> BenchmarkResult:
    for _ in range(warmup):
        run_once()

    samples: list[float] = []
    for _ in range(iterations):
        start = time.perf_counter()
        run_once()
        samples.append((time.perf_counter() - start) * 1000.0)

    return BenchmarkResult(
        name=name,
        iterations=iterations,
        median_ms=statistics.median(samples),
        min_ms=min(samples),
        max_ms=max(samples),
        mean_ms=statistics.fmean(samples),
    )


def run_benchmarks(
    workspace: Path,
    modules: int,
    iterations: int,
    warmup: int,
) -> list[BenchmarkResult]:
    gateway = ArboristGateway()
    db_path = workspace / "symbols.db"
    refresh_target = write_workspace(workspace, modules, revision=0)
    request_id = 1

    def next_id() -> int:
        nonlocal request_id
        request_id += 1
        return request_id

    def rebuild() -> None:
        call_gateway(
            gateway,
            "arborist/rebuild_symbol_index",
            {
                "workspace_root": str(workspace),
                "db_path": str(db_path),
                "max_files": modules + 4,
            },
            next_id(),
        )

    rebuild()

    refresh_revision = 1

    def refresh() -> None:
        nonlocal refresh_revision, refresh_target
        refresh_target = write_workspace(workspace, modules, revision=refresh_revision)
        refresh_revision += 1
        call_gateway(
            gateway,
            "arborist/refresh_symbol_index_for_file",
            {
                "workspace_root": str(workspace),
                "db_path": str(db_path),
                "file_path": str(refresh_target),
                "max_files": modules + 4,
            },
            next_id(),
        )

    def trace() -> None:
        call_gateway(
            gateway,
            "arborist/trace_symbol_graph",
            {
                "workspace_root": str(workspace),
                "index_db_path": str(db_path),
                "symbol_path": "orchestrate",
                "direction": "both",
            },
            next_id(),
        )

    def search() -> None:
        call_gateway(
            gateway,
            "arborist/search_symbols",
            {
                "workspace_root": str(workspace),
                "index_db_path": str(db_path),
                "query": "helper",
                "limit": 50,
            },
            next_id(),
        )

    return [
        measure("rebuild_symbol_index", iterations, warmup, rebuild),
        measure("refresh_symbol_index_for_file", iterations, warmup, refresh),
        measure("trace_symbol_graph", iterations, warmup, trace),
        measure("search_symbols", iterations, warmup, search),
    ]


def print_table(results: list[BenchmarkResult], workspace: Path, modules: int) -> None:
    print(f"workspace: {workspace}")
    print(f"modules:   {modules}")
    print()
    print(
        f"{'workflow':34} {'iters':>5} {'median':>10} "
        f"{'mean':>10} {'min':>10} {'max':>10}"
    )
    for result in results:
        print(
            f"{result.name:34} {result.iterations:5d} "
            f"{result.median_ms:9.2f}ms {result.mean_ms:9.2f}ms "
            f"{result.min_ms:9.2f}ms {result.max_ms:9.2f}ms"
        )


def print_results(
    results: list[BenchmarkResult],
    workspace: Path,
    modules: int,
    as_json: bool,
) -> None:
    if as_json:
        payload = {
            "workspace": str(workspace),
            "modules": modules,
            "results": [result.__dict__ for result in results],
        }
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    else:
        print_table(results, workspace, modules)


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)

    if args.workspace is not None:
        workspace = args.workspace.resolve()
        results = run_benchmarks(workspace, args.modules, args.iterations, args.warmup)
        print_results(results, workspace, args.modules, args.json)
    else:
        with tempfile.TemporaryDirectory(prefix="arborist-bench-") as temp_dir:
            workspace = Path(temp_dir)
            results = run_benchmarks(workspace, args.modules, args.iterations, args.warmup)
            if args.keep_workspace:
                kept = Path(tempfile.mkdtemp(prefix="arborist-bench-kept-"))
                for path in workspace.iterdir():
                    path.replace(kept / path.name)
                workspace = kept
            print_results(results, workspace, args.modules, args.json)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
