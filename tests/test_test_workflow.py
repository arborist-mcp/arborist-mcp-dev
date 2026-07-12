from __future__ import annotations

import json
from pathlib import Path
import shutil
import subprocess
import sys
import unittest

from tests import GROUP_MODULES, build_manifest_snapshot
from tests.gateway_protocol import GROUP_MODULES as GATEWAY_GROUP_MODULES

POWERSHELL = shutil.which("powershell") or shutil.which("pwsh")


class TestWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.repo_root = Path(__file__).resolve().parents[1]

    def test_python_manifest_snapshot_exposes_combined_suite_graph(self) -> None:
        snapshot = build_manifest_snapshot()

        self.assertIn("check-workflow", snapshot["suites"])
        self.assertIn("test-workflow", snapshot["suites"])
        self.assertIn("gateway-request-validation", snapshot["suites"])
        self.assertIn("python-fast", snapshot["groups"])
        self.assertIn("python-native", snapshot["groups"])
        self.assertIn("python", snapshot["groups"])

    def test_python_fast_group_extends_gateway_fast_with_workflow_tests(self) -> None:
        self.assertEqual(
            GROUP_MODULES["python-fast"],
            (
                "tests.test_check_workflow",
                "tests.test_test_workflow",
                *GATEWAY_GROUP_MODULES["gateway-fast"],
            ),
        )

    def test_python_group_covers_all_manifest_modules(self) -> None:
        self.assertEqual(
            GROUP_MODULES["python"],
            (
                "tests.test_check_workflow",
                "tests.test_test_workflow",
                *GATEWAY_GROUP_MODULES["gateway"],
            ),
        )

    def test_python_suite_manifest_cli_emits_snapshot(self) -> None:
        script_path = self.repo_root / "scripts" / "python_suite_manifest.py"
        completed = subprocess.run(
            [sys.executable, str(script_path)],
            cwd=self.repo_root,
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertEqual(json.loads(completed.stdout), build_manifest_snapshot())

    def test_python_suite_manifest_cli_emits_deduplicated_execution_plan(self) -> None:
        script_path = self.repo_root / "scripts" / "python_suite_manifest.py"
        completed = subprocess.run(
            [sys.executable, str(script_path), "--plan", "rust", "inner-loop", "gateway-fast"],
            cwd=self.repo_root,
            check=True,
            capture_output=True,
            text=True,
        )

        plan = json.loads(completed.stdout)
        self.assertEqual(plan["selection_names"], ["rust", "inner-loop", "gateway-fast"])
        self.assertEqual([step["kind"] for step in plan["steps"]], ["rust", "python"])
        self.assertEqual(plan["steps"][0]["selection_names"], ["rust"])
        self.assertEqual(
            plan["steps"][1]["selection_names"],
            ["python-fast", "gateway-fast"],
        )
        self.assertEqual(
            plan["steps"][1]["module_names"],
            list(GROUP_MODULES["python-fast"]),
        )
        self.assertFalse(plan["steps"][1]["requires_extension"])

    @unittest.skipUnless(POWERSHELL, "PowerShell is required for test.ps1 contract checks")
    def test_test_script_lists_suites_from_snapshot(self) -> None:
        snapshot = build_manifest_snapshot()
        completed = subprocess.run(
            [POWERSHELL, "-File", "scripts/test.ps1", "-ListSuites"],
            cwd=self.repo_root,
            check=True,
            capture_output=True,
            text=True,
        )
        lines = [line.rstrip() for line in completed.stdout.splitlines() if line.strip()]

        expected = [
            f"{'rust':<32} Run all Rust tests via cargo test --locked.",
            f"{'python':<32} {snapshot['groups']['python']['description']}",
            f"{'inner-loop':<32} Run Rust tests plus the python-fast group for the default local loop.",
            f"{'all':<32} Run Rust tests plus the full Python suite set.",
        ]
        expected.extend(
            f"{group_name:<32} {metadata['description']}"
            for group_name, metadata in snapshot["groups"].items()
            if group_name != "python"
        )
        expected.extend(
            f"{suite_name:<32} {metadata['description']}"
            for suite_name, metadata in snapshot["suites"].items()
        )
        self.assertEqual(lines, expected)

    @unittest.skipUnless(POWERSHELL, "PowerShell is required for test.ps1 contract checks")
    def test_test_script_show_plan_reports_deduplicated_execution_steps(self) -> None:
        completed = subprocess.run(
            [
                POWERSHELL,
                "-File",
                "scripts/test.ps1",
                "-Suite",
                "rust,inner-loop,gateway-fast",
                "-ShowPlan",
            ],
            cwd=self.repo_root,
            check=True,
            capture_output=True,
            text=True,
        )
        lines = [line.rstrip() for line in completed.stdout.splitlines() if line.strip()]
        self.assertEqual(lines[0], "rust    <- rust")
        self.assertEqual(lines[1], "python  <- python-fast, gateway-fast [pure-python; 5 module(s)]")
        self.assertEqual(
            lines[2:],
            [f"          {module_name}" for module_name in GROUP_MODULES["python-fast"]],
        )

    def test_readme_documents_python_suite_groups(self) -> None:
        readme = (self.repo_root / "README.md").read_text(encoding="utf-8")
        for suite_name in ("python-fast", "python-native", "python"):
            with self.subTest(suite=suite_name):
                self.assertIn(f".\\scripts\\test.ps1 -Suite {suite_name}", readme)
        self.assertIn("scripts/python_suite_manifest.py", readme)
        self.assertIn(".\\scripts\\test.ps1 -Suite rust,inner-loop -ShowPlan", readme)


if __name__ == "__main__":
    unittest.main()
