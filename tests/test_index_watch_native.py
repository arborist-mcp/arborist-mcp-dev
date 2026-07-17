from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import unittest

from tests.gateway_protocol.helpers import temp_workspace


class IndexWatchNativeTests(unittest.TestCase):
    def test_once_rebuilds_missing_index_then_reports_healthy(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        with temp_workspace({"helper.py": "def helper() -> int:\n    return 1\n"}) as workspace:
            db_path = workspace.joinpath("symbols.db")
            command = [
                sys.executable,
                "-m",
                "arborist_mcp.index_watch",
                "--workspace-root",
                str(workspace),
                "--db-path",
                str(db_path),
                "--once",
            ]

            refreshed = subprocess.run(
                command,
                cwd=repo_root,
                check=True,
                capture_output=True,
                text=True,
            )
            healthy = subprocess.run(
                command,
                cwd=repo_root,
                check=True,
                capture_output=True,
                text=True,
            )
            workspace.joinpath("helper.py").write_text(
                "def helper() -> int:\n    return 2\n", encoding="utf-8"
            )
            refreshed_after_change = subprocess.run(
                command,
                cwd=repo_root,
                check=True,
                capture_output=True,
                text=True,
            )

            self.assertEqual(json.loads(refreshed.stdout)["status"], "refreshed")
            self.assertEqual(json.loads(healthy.stdout)["status"], "healthy")
            self.assertEqual(
                json.loads(refreshed_after_change.stdout)["status"], "refreshed"
            )
            self.assertTrue(db_path.exists())

    def test_registered_refresh_updates_external_disk_changes(self) -> None:
        from arborist_mcp._arborist_core import ArboristCore

        with temp_workspace({"helper.py": "def helper() -> int:\n    return 1\n"}) as workspace:
            db_path = workspace.joinpath("symbols.db")
            core = ArboristCore()

            initial = json.loads(
                core.register_symbol_index_json(str(workspace), str(db_path))
            )
            workspace.joinpath("helper.py").write_text(
                "def helper() -> int:\n    return 2\n", encoding="utf-8"
            )
            refreshed = json.loads(core.refresh_registered_symbol_indexes_json())

            self.assertEqual(initial["indexed_files"], 1)
            self.assertEqual(len(refreshed), 1)
            self.assertEqual(refreshed[0]["db_path"], str(db_path).replace("\\", "/"))
            self.assertEqual(refreshed[0]["rebuilt_files"], 1)
            self.assertEqual(refreshed[0]["reused_files"], 0)

    def test_once_reconciles_watch_config(self) -> None:
        repo_root = Path(__file__).resolve().parents[1]
        with temp_workspace({"helper.py": "def helper() -> int:\n    return 1\n"}) as workspace:
            db_path = workspace.joinpath("symbols.db")
            config_path = workspace.joinpath("watch.json")
            config_path.write_text(
                json.dumps(
                    {
                        "indexes": [
                            {
                                "workspace_root": str(workspace),
                                "db_path": str(db_path),
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            completed = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "arborist_mcp.index_watch",
                    "--config",
                    str(config_path),
                    "--once",
                ],
                cwd=repo_root,
                check=True,
                capture_output=True,
                text=True,
            )

            event = json.loads(completed.stdout)
            self.assertEqual(event["status"], "refreshed")
            self.assertEqual(event["workspace_root"], str(workspace))
            self.assertTrue(db_path.exists())


if __name__ == "__main__":
    unittest.main()
