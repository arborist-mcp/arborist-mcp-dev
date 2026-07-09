from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import unittest


def _load_check_profile_module():
    repo_root = Path(__file__).resolve().parents[1]
    module_path = repo_root / "scripts" / "check_profile_manifest.py"
    spec = importlib.util.spec_from_file_location("check_profile_manifest", module_path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class CheckWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.repo_root = Path(__file__).resolve().parents[1]
        cls.module = _load_check_profile_module()

    def test_check_profile_snapshot_has_expected_profile_order(self) -> None:
        snapshot = self.module.build_snapshot()
        self.assertEqual(
            snapshot["profile_order"],
            [
                "sanity",
                "rust",
                "gateway-fast",
                "python-fast",
                "gateway-native",
                "python-discovery",
                "gateway-smoke",
                "python-native",
                "full",
            ],
        )

    def test_check_profile_snapshot_marks_aggregate_profiles(self) -> None:
        snapshot = self.module.build_snapshot()
        profiles = snapshot["profiles"]

        self.assertEqual(profiles["sanity"]["handler"], "sanity")
        self.assertEqual(profiles["python-fast"]["handler"], "suite")
        self.assertEqual(profiles["python-fast"]["suite"], "python-fast")
        self.assertFalse(profiles["python-fast"]["prepare_extension"])
        self.assertEqual(profiles["gateway-native"]["handler"], "suite")
        self.assertTrue(profiles["gateway-native"]["prepare_extension"])
        self.assertEqual(profiles["python-discovery"]["suite"], "python")
        self.assertEqual(profiles["gateway-smoke"]["handler"], "gateway-smoke")

        self.assertEqual(
            profiles["python-native"]["leaf_profiles"],
            ["gateway-native", "python-discovery", "gateway-smoke"],
        )
        self.assertFalse(profiles["python-native"]["leaf"])
        self.assertTrue(profiles["python-native"]["needs_python"])
        self.assertTrue(profiles["python-native"]["needs_rust"])

        self.assertEqual(
            profiles["full"]["leaf_profiles"],
            ["sanity", "rust", "gateway-native", "python-discovery", "gateway-smoke"],
        )
        self.assertFalse(profiles["full"]["leaf"])
        self.assertTrue(profiles["full"]["needs_python"])
        self.assertTrue(profiles["full"]["needs_rust"])

    def test_check_profile_manifest_cli_emits_snapshot(self) -> None:
        script_path = self.repo_root / "scripts" / "check_profile_manifest.py"
        completed = subprocess.run(
            [sys.executable, str(script_path)],
            cwd=self.repo_root,
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertEqual(json.loads(completed.stdout), self.module.build_snapshot())

    def test_check_profile_manifest_cli_emits_github_matrix(self) -> None:
        script_path = self.repo_root / "scripts" / "check_profile_manifest.py"
        completed = subprocess.run(
            [sys.executable, str(script_path), "--github-matrix"],
            cwd=self.repo_root,
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertEqual(json.loads(completed.stdout), self.module.build_github_matrix())

    def test_check_script_lists_profiles_from_snapshot(self) -> None:
        snapshot = self.module.build_snapshot()
        completed = subprocess.run(
            ["powershell", "-File", "scripts/check.ps1", "-ListProfiles"],
            cwd=self.repo_root,
            check=True,
            capture_output=True,
            text=True,
        )
        lines = [line.rstrip() for line in completed.stdout.splitlines() if line.strip()]
        expected = [
            f"{profile_name:<16} {snapshot['profiles'][profile_name]['description']}"
            for profile_name in snapshot["profile_order"]
        ]
        self.assertEqual(lines, expected)

    def test_check_workflow_uses_shared_matrix_helper(self) -> None:
        workflow = (self.repo_root / ".github" / "workflows" / "check.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("define-check-matrix:", workflow)
        self.assertIn(
            'python3 scripts/check_profile_manifest.py --github-matrix',
            workflow,
        )
        self.assertIn(
            "matrix: ${{ fromJson(needs.define-check-matrix.outputs.matrix) }}",
            workflow,
        )

    def test_ci_profiles_promote_python_fast_over_gateway_fast(self) -> None:
        snapshot = self.module.build_snapshot()
        self.assertIn("python-fast", snapshot["ci_profiles"])
        self.assertNotIn("gateway-fast", snapshot["ci_profiles"])

    def test_readme_documents_split_native_profiles(self) -> None:
        readme = (self.repo_root / "README.md").read_text(encoding="utf-8")
        for profile_name in (
            "python-fast",
            "gateway-fast",
            "gateway-native",
            "python-discovery",
            "gateway-smoke",
            "python-native",
        ):
            with self.subTest(profile=profile_name):
                self.assertIn(f".\\scripts\\check.ps1 -Profile {profile_name}", readme)


if __name__ == "__main__":
    unittest.main()
