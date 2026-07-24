from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import shutil
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


def _load_version_consistency_module():
    repo_root = Path(__file__).resolve().parents[1]
    module_path = repo_root / "scripts" / "version_consistency.py"
    spec = importlib.util.spec_from_file_location("version_consistency", module_path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _load_tool_catalog_module():
    repo_root = Path(__file__).resolve().parents[1]
    module_path = repo_root / "scripts" / "tool_catalog.py"
    spec = importlib.util.spec_from_file_location("tool_catalog", module_path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _load_gateway_smoke_module():
    repo_root = Path(__file__).resolve().parents[1]
    module_path = repo_root / "scripts" / "gateway_smoke.py"
    spec = importlib.util.spec_from_file_location("gateway_smoke", module_path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


POWERSHELL = shutil.which("powershell") or shutil.which("pwsh")


class CheckWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.repo_root = Path(__file__).resolve().parents[1]
        cls.module = _load_check_profile_module()
        cls.version_module = _load_version_consistency_module()
        cls.tool_catalog_module = _load_tool_catalog_module()
        cls.gateway_smoke_module = _load_gateway_smoke_module()

    def test_check_profile_snapshot_has_expected_profile_order(self) -> None:
        snapshot = self.module.build_snapshot()
        self.assertEqual(
            snapshot["profile_order"],
            [
                "sanity",
                "rust",
                "fuzz-manifest",
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
        self.assertTrue(profiles["sanity"]["needs_python"])
        self.assertFalse(profiles["sanity"]["needs_rust"])
        self.assertEqual(profiles["rust"]["handler"], "rust")
        self.assertFalse(profiles["rust"]["needs_python"])
        self.assertTrue(profiles["rust"]["needs_rust"])
        self.assertEqual(profiles["fuzz-manifest"]["handler"], "fuzz-manifest")
        self.assertFalse(profiles["fuzz-manifest"]["needs_python"])
        self.assertTrue(profiles["fuzz-manifest"]["needs_rust"])
        self.assertEqual(profiles["python-fast"]["handler"], "suite")
        self.assertEqual(profiles["python-fast"]["suite"], "python-fast")
        self.assertEqual(profiles["python-fast"]["suite_target_type"], "group")
        self.assertFalse(profiles["python-fast"]["suite_requires_extension"])
        self.assertFalse(profiles["python-fast"]["prepare_extension"])
        self.assertEqual(profiles["gateway-native"]["handler"], "suite")
        self.assertEqual(profiles["gateway-native"]["suite_target_type"], "group")
        self.assertTrue(profiles["gateway-native"]["suite_requires_extension"])
        self.assertTrue(profiles["gateway-native"]["prepare_extension"])
        self.assertEqual(profiles["python-discovery"]["suite"], "python")
        self.assertEqual(profiles["python-discovery"]["suite_target_type"], "group")
        self.assertTrue(profiles["python-discovery"]["suite_requires_extension"])
        self.assertEqual(profiles["gateway-smoke"]["handler"], "gateway-smoke")
        self.assertTrue(profiles["gateway-smoke"]["prepare_extension"])
        self.assertTrue(profiles["gateway-smoke"]["needs_python"])
        self.assertTrue(profiles["gateway-smoke"]["needs_rust"])

        self.assertEqual(
            profiles["python-native"]["leaf_profiles"],
            ["gateway-native", "python-discovery", "gateway-smoke"],
        )
        self.assertFalse(profiles["python-native"]["leaf"])
        self.assertTrue(profiles["python-native"]["needs_python"])
        self.assertTrue(profiles["python-native"]["needs_rust"])

        self.assertEqual(
            profiles["full"]["leaf_profiles"],
            [
                "sanity",
                "rust",
                "fuzz-manifest",
                "gateway-native",
                "python-discovery",
                "gateway-smoke",
            ],
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

    def test_check_profile_manifest_rejects_duplicate_ci_profiles(self) -> None:
        profiles = {
            "rust": {
                "description": "Rust checks",
                "entries": (),
                "handler": "rust",
            }
        }

        with self.assertRaisesRegex(
            RuntimeError, "duplicate check profile 'rust' listed in ci_profiles"
        ):
            self.module._load_ci_profiles(
                {"ci_profiles": ["rust", "rust"]},
                profiles=profiles,
            )

    def test_check_profile_manifest_cli_emits_deduplicated_execution_plan(self) -> None:
        script_path = self.repo_root / "scripts" / "check_profile_manifest.py"
        completed = subprocess.run(
            [sys.executable, str(script_path), "--plan", "full", "python-native"],
            cwd=self.repo_root,
            check=True,
            capture_output=True,
            text=True,
        )

        plan = json.loads(completed.stdout)
        self.assertEqual(plan["profile_names"], ["full", "python-native"])
        self.assertEqual(
            [step["profile"] for step in plan["steps"]],
            [
                "sanity",
                "rust",
                "fuzz-manifest",
                "gateway-native",
                "python-discovery",
                "gateway-smoke",
            ],
        )
        self.assertEqual(plan["steps"][3]["suite"], "gateway-native")
        self.assertTrue(plan["steps"][5]["prepare_extension"])

    @unittest.skipUnless(POWERSHELL, "PowerShell is required for check.ps1 contract checks")
    def test_check_script_lists_profiles_from_snapshot(self) -> None:
        snapshot = self.module.build_snapshot()
        completed = subprocess.run(
            [POWERSHELL, "-File", "scripts/check.ps1", "-ListProfiles"],
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

    @unittest.skipUnless(POWERSHELL, "PowerShell is required for check.ps1 contract checks")
    def test_check_script_show_plan_reports_deduplicated_leaf_profiles(self) -> None:
        completed = subprocess.run(
            [
                POWERSHELL,
                "-File",
                "scripts/check.ps1",
                "-Profile",
                "full,python-native",
                "-ShowPlan",
            ],
            cwd=self.repo_root,
            check=True,
            capture_output=True,
            text=True,
        )
        lines = [line.rstrip() for line in completed.stdout.splitlines() if line.strip()]
        self.assertEqual(
            lines,
            [
                "sanity           sanity [python]",
                "rust             rust [rust]",
                "fuzz-manifest    fuzz-manifest [rust]",
                "gateway-native   suite -> gateway-native -> prepare-extension [rust+python]",
                "python-discovery suite -> python -> prepare-extension [rust+python]",
                "gateway-smoke    gateway-smoke -> prepare-extension [rust+python]",
            ],
        )

    def test_gateway_smoke_helper_runs_catalog_checks_without_native_core(self) -> None:
        script_path = self.repo_root / "scripts" / "gateway_smoke.py"
        completed = subprocess.run(
            [sys.executable, str(script_path)],
            cwd=self.repo_root,
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertIn("Gateway smoke checks passed.", completed.stdout)

    def test_gateway_smoke_load_json_rejects_duplicate_keys_and_nonstandard_constants(
        self,
    ) -> None:
        load_json = self.gateway_smoke_module._load_json
        with self.assertRaisesRegex(RuntimeError, "duplicate JSON object key"):
            load_json('{"a": 1, "a": 2}', "duplicate key payload")
        with self.assertRaisesRegex(RuntimeError, "non-standard JSON constant"):
            load_json('{"a": NaN}', "nonstandard constant payload")
        self.assertEqual(
            load_json('{"a": 1, "b": {"c": 2}}', "valid payload"),
            {"a": 1, "b": {"c": 2}},
        )

    def test_version_consistency_script_passes_for_repo_versions(self) -> None:
        script_path = self.repo_root / "scripts" / "version_consistency.py"
        completed = subprocess.run(
            [sys.executable, str(script_path)],
            cwd=self.repo_root,
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertIn("Version consistency checks passed.", completed.stdout)

    def test_version_consistency_module_reads_matching_repo_versions(self) -> None:
        versions = self.version_module.collect_versions(self.repo_root)
        workspace_version = versions["cargo_workspace"]
        self.assertEqual(versions["pyproject"], workspace_version)
        self.assertEqual(versions["python_package"], workspace_version)
        self.assertEqual(versions["cargo_lock:arborist-core"], workspace_version)
        self.assertEqual(versions["cargo_lock:arborist-py"], workspace_version)

    def test_check_script_and_linux_ci_share_gateway_smoke_helper(self) -> None:
        check_script = (self.repo_root / "scripts" / "check.ps1").read_text(encoding="utf-8")
        workflow = (self.repo_root / ".github" / "workflows" / "check.yml").read_text(
            encoding="utf-8"
        )

        self.assertIn("gateway_smoke.py", check_script)
        self.assertIn("--require-core", check_script)
        self.assertIn("python scripts/gateway_smoke.py", workflow)
        self.assertIn("python scripts/gateway_smoke.py --launcher console --require-core", workflow)
        self.assertIn("macos-basic:", workflow)
        self.assertNotIn("printf '%s\\n'", workflow)

    def test_check_script_and_unix_ci_share_cross_platform_metadata_checks(self) -> None:
        check_script = (self.repo_root / "scripts" / "check.ps1").read_text(encoding="utf-8")
        workflow = (self.repo_root / ".github" / "workflows" / "check.yml").read_text(
            encoding="utf-8"
        )

        self.assertIn("version_consistency.py", check_script)
        self.assertIn('Join-Path $RepoRoot "scripts\\version_consistency.py"', check_script)
        self.assertIn("python scripts/version_consistency.py", workflow)
        self.assertIn("python scripts/tool_catalog.py --check", workflow)

    def test_unix_ci_runs_rust_formatting_and_lint_checks(self) -> None:
        workflow = (self.repo_root / ".github" / "workflows" / "check.yml").read_text(
            encoding="utf-8"
        )

        self.assertIn("rustfmt", workflow)
        self.assertIn("clippy", workflow)
        self.assertIn("cargo fmt --check", workflow)
        self.assertIn("cargo clippy --locked --all-targets -- -D warnings", workflow)

    def test_fuzz_manifest_check_covers_every_declared_target(self) -> None:
        check_script = (self.repo_root / "scripts" / "check.ps1").read_text(
            encoding="utf-8"
        )
        fuzz_manifest = (self.repo_root / "fuzz" / "Cargo.toml").read_text(
            encoding="utf-8"
        )
        declared_targets: set[str] = set()
        in_bin = False
        for line in fuzz_manifest.splitlines():
            if line.strip() == "[[bin]]":
                in_bin = True
            elif line.startswith("[") and line.strip() != "[[bin]]":
                in_bin = False
            elif in_bin and line.startswith("name = "):
                declared_targets.add(line.split('"')[1])
        self.assertEqual(
            declared_targets,
            {
                "tree_query",
                "semantic_skeleton",
                "patch_preview",
                "workspace_edit_preview",
                "symbol_index_inspection",
                "symbol_index_queries",
                "source_overlay_queries",
                "workspace_edit_json",
            },
        )
        self.assertIn('Join-Path $RepoRoot "fuzz\\Cargo.toml"', check_script)
        self.assertNotIn(
            'foreach ($target in @("tree_query"',
            check_script,
        )

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

    def test_wheels_workflow_caches_cargo_builds_per_runner_and_lockfile(self) -> None:
        workflow = (self.repo_root / ".github" / "workflows" / "wheels.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("actions/cache@v4", workflow)
        self.assertIn("~/.cargo/registry", workflow)
        self.assertIn("~/.cargo/git", workflow)
        self.assertIn("target", workflow)
        self.assertIn(
            "key: ${{ runner.os }}-cargo-wheel-${{ hashFiles('**/Cargo.lock') }}",
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
        self.assertIn(".\\scripts\\check.ps1 -Profile full,python-native -ShowPlan", readme)

    def test_tool_counts_in_user_docs_match_generated_catalog(self) -> None:
        catalog = json.loads(
            (self.repo_root / "docs" / "tool-catalog.json").read_text(encoding="utf-8")
        )
        category_counts: dict[str, int] = {}
        for tool in catalog:
            category = tool["metadata"]["category"]
            category_counts[category] = category_counts.get(category, 0) + 1

        expected_counts = {
            "total": len(catalog),
            "read": category_counts["read"],
            "write": category_counts["write"],
            "vfs": category_counts["vfs"],
            "index": category_counts["index"],
            "trace": category_counts["trace"],
        }
        category_labels = {
            "read": "Read",
            "write": "Write",
            "vfs": "VFS",
            "index": "Index",
            "trace": "Trace",
        }
        for relative_path in ("README.md", "docs/tools.md"):
            document = (self.repo_root / relative_path).read_text(encoding="utf-8")
            with self.subTest(document=relative_path):
                self.assertRegex(
                    document,
                    rf"(?:currently returns|returns) {expected_counts['total']} tools:",
                )
                for category in ("read", "write", "vfs", "index", "trace"):
                    self.assertRegex(
                        document,
                        rf"- {category_labels[category]} tools: {expected_counts[category]}(?:,|\\.)",
                    )

    def test_tool_catalog_check_detects_stale_document_counts(self) -> None:
        catalog = json.loads(
            (self.repo_root / "docs" / "tool-catalog.json").read_text(encoding="utf-8")
        )
        catalog.append({"metadata": {"category": "read"}})

        errors = self.tool_catalog_module._documentation_errors(catalog)

        self.assertEqual(len(errors), 4)
        self.assertTrue(any("README.md" in error for error in errors))
        self.assertTrue(any("docs/tools.md" in error for error in errors))

    def test_tool_catalog_check_accepts_protocol_contract_references(self) -> None:
        catalog = json.loads(
            (self.repo_root / "docs" / "tool-catalog.json").read_text(encoding="utf-8")
        )

        errors = self.tool_catalog_module._documentation_errors(catalog)

        self.assertEqual(errors, [])


if __name__ == "__main__":
    unittest.main()
