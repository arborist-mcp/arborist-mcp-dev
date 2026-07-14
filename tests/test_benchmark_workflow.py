from __future__ import annotations

import importlib.util
import json
from io import StringIO
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


def _load_benchmark_module():
    repo_root = Path(__file__).resolve().parents[1]
    module_path = repo_root / "scripts" / "benchmark_core.py"
    spec = importlib.util.spec_from_file_location("benchmark_core", module_path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class BenchmarkWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.module = _load_benchmark_module()

    def test_parse_max_median_threshold_accepts_valid_input(self) -> None:
        threshold = self.module.parse_max_median_threshold("trace_symbol_graph=25.5")
        self.assertEqual(threshold.workflow, "trace_symbol_graph")
        self.assertEqual(threshold.max_median_ms, 25.5)

    def test_parse_max_median_threshold_rejects_invalid_shape(self) -> None:
        with self.assertRaises(Exception) as context:
            self.module.parse_max_median_threshold("trace_symbol_graph")
        self.assertIn("WORKFLOW=MS", str(context.exception))

    def test_parse_max_median_threshold_rejects_non_positive_limit(self) -> None:
        with self.assertRaises(Exception) as context:
            self.module.parse_max_median_threshold("trace_symbol_graph=0")
        self.assertIn("greater than zero", str(context.exception))

    def test_evaluate_thresholds_reports_unknown_workflow(self) -> None:
        results = [
            self.module.BenchmarkResult(
                name="trace_symbol_graph",
                iterations=3,
                median_ms=10.0,
                min_ms=9.0,
                max_ms=11.0,
                mean_ms=10.0,
            )
        ]
        failures = self.module.evaluate_thresholds(
            results,
            [self.module.MedianThreshold("missing_workflow", 5.0)],
        )
        self.assertEqual(failures, ["unknown workflow in --max-median: missing_workflow"])

    def test_evaluate_thresholds_reports_exceeded_median(self) -> None:
        results = [
            self.module.BenchmarkResult(
                name="trace_symbol_graph",
                iterations=3,
                median_ms=10.0,
                min_ms=9.0,
                max_ms=11.0,
                mean_ms=10.0,
            )
        ]
        failures = self.module.evaluate_thresholds(
            results,
            [self.module.MedianThreshold("trace_symbol_graph", 9.5)],
        )
        self.assertEqual(
            failures,
            ["trace_symbol_graph median 10.00ms exceeded limit 9.50ms"],
        )

    def test_main_returns_nonzero_when_threshold_fails(self) -> None:
        fake_results = [
            self.module.BenchmarkResult(
                name="trace_symbol_graph",
                iterations=1,
                median_ms=12.0,
                min_ms=12.0,
                max_ms=12.0,
                mean_ms=12.0,
            )
        ]
        with tempfile.TemporaryDirectory() as temp_dir:
            stderr = StringIO()
            stdout = StringIO()
            with (
                mock.patch.object(self.module, "run_benchmarks", return_value=fake_results),
                mock.patch("sys.stderr", stderr),
                mock.patch("sys.stdout", stdout),
            ):
                exit_code = self.module.main(
                    [
                        "--workspace",
                        temp_dir,
                        "--max-median",
                        "trace_symbol_graph=10",
                        "--json",
                    ]
                )

        self.assertEqual(exit_code, 1)
        self.assertIn("exceeded limit 10.00ms", stderr.getvalue())
        payload = json.loads(stdout.getvalue())
        self.assertEqual(payload["results"][0]["name"], "trace_symbol_graph")


if __name__ == "__main__":
    unittest.main()
