from __future__ import annotations

from contextlib import redirect_stderr
import io
import json
from pathlib import Path
import unittest

from arborist_mcp.index_watch import IndexWatchError, reconcile_index, run_cli, run_watch


def health_payload(*, ok: bool, action: str, reason: str = "current") -> str:
    return json.dumps(
        {
            "ok": ok,
            "exists": True,
            "schema_version": "1",
            "expected_schema_version": "1",
            "issues": [] if ok else [reason],
            "stale_files": [],
            "missing_files": [],
            "unindexed_files": [],
            "migration": {"action": action, "reason": reason},
        }
    )


class StubCore:
    def __init__(self, health: str, refresh: str = '{"indexed_files": 1}') -> None:
        self.health = health
        self.refresh = refresh
        self.inspect_calls: list[str] = []
        self.refresh_calls: list[tuple[object, ...]] = []

    def inspect_symbol_index_json(self, db_path: str) -> str:
        self.inspect_calls.append(db_path)
        return self.health

    def refresh_symbol_index_json(self, *args: object) -> str:
        self.refresh_calls.append(args)
        return self.refresh


class IndexWatchTests(unittest.TestCase):
    def test_reconcile_leaves_healthy_index_unchanged(self) -> None:
        core = StubCore(health_payload(ok=True, action="none"))

        event = reconcile_index(
            core,
            workspace_root="workspace",
            db_path="symbols.db",
            max_files=20,
            max_file_bytes=None,
        )

        self.assertEqual(event["status"], "healthy")
        self.assertEqual(core.inspect_calls, ["symbols.db"])
        self.assertEqual(core.refresh_calls, [])

    def test_reconcile_refreshes_rebuildable_index(self) -> None:
        core = StubCore(
            health_payload(ok=False, action="rebuild", reason="indexed file is stale"),
            '{"indexed_files": 2, "rebuilt_files": 1}',
        )

        event = reconcile_index(
            core,
            workspace_root="workspace",
            db_path="symbols.db",
            max_files=20,
            max_file_bytes=4096,
        )

        self.assertEqual(event["status"], "refreshed")
        self.assertEqual(event["stats"]["indexed_files"], 2)
        self.assertEqual(
            core.refresh_calls,
            [("workspace", "symbols.db", 20, 4096)],
        )

    def test_reconcile_fails_closed_for_manual_action(self) -> None:
        core = StubCore(
            health_payload(ok=False, action="manual", reason="foreign database")
        )

        with self.assertRaisesRegex(IndexWatchError, "cannot repair"):
            reconcile_index(
                core,
                workspace_root="workspace",
                db_path="symbols.db",
                max_files=20,
                max_file_bytes=None,
            )

        self.assertEqual(core.refresh_calls, [])

    def test_reconcile_fails_closed_for_unsupported_existing_schema(self) -> None:
        payload = json.loads(
            health_payload(ok=False, action="rebuild", reason="schema is unsupported")
        )
        payload["schema_version"] = "99"
        core = StubCore(json.dumps(payload))

        with self.assertRaisesRegex(IndexWatchError, "cannot repair"):
            reconcile_index(
                core,
                workspace_root="workspace",
                db_path="symbols.db",
                max_files=20,
                max_file_bytes=None,
            )

        self.assertEqual(core.refresh_calls, [])

    def test_run_watch_emits_initial_healthy_event_once(self) -> None:
        core = StubCore(health_payload(ok=True, action="none"))
        events: list[dict[str, object]] = []

        run_watch(
            core,
            workspace_root="workspace",
            db_path="symbols.db",
            interval_seconds=1,
            max_files=20,
            max_file_bytes=None,
            once=True,
            emit=events.append,
        )

        self.assertEqual([event["status"] for event in events], ["healthy"])

    def test_cli_once_emits_json_event(self) -> None:
        core = StubCore(health_payload(ok=False, action="rebuild", reason="missing index"))
        stdout = io.StringIO()
        stderr = io.StringIO()

        result = run_cli(
            [
                "--workspace-root",
                "workspace",
                "--db-path",
                "symbols.db",
                "--once",
            ],
            core_factory=lambda: core,
            stdout=stdout,
            stderr=stderr,
        )

        self.assertEqual(result, 0)
        self.assertEqual(stderr.getvalue(), "")
        self.assertEqual(json.loads(stdout.getvalue())["status"], "refreshed")

    def test_cli_reports_manual_action_without_refreshing(self) -> None:
        core = StubCore(
            health_payload(ok=False, action="manual", reason="foreign database")
        )
        stdout = io.StringIO()
        stderr = io.StringIO()

        result = run_cli(
            ["--db-path", "symbols.db", "--once"],
            core_factory=lambda: core,
            stdout=stdout,
            stderr=stderr,
        )

        self.assertEqual(result, 1)
        self.assertEqual(stdout.getvalue(), "")
        self.assertIn("cannot repair", stderr.getvalue())
        self.assertEqual(core.refresh_calls, [])

    def test_cli_rejects_non_positive_limits(self) -> None:
        with redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit):
                run_cli(["--db-path", "symbols.db", "--interval-seconds", "0"])
            with self.assertRaises(SystemExit):
                run_cli(["--db-path", "symbols.db", "--max-files", "0"])
            with self.assertRaises(SystemExit):
                run_cli(["--db-path", "symbols.db", "--max-file-bytes", "0"])
            with self.assertRaises(SystemExit):
                run_cli(["--db-path", "symbols.db", "--max-files", "200001"])

    def test_pyproject_registers_index_watch_console_script(self) -> None:
        pyproject = Path(__file__).resolve().parents[1].joinpath("pyproject.toml")
        self.assertIn(
            'arborist-index-watch = "arborist_mcp.index_watch:main"',
            pyproject.read_text(encoding="utf-8"),
        )


if __name__ == "__main__":
    unittest.main()
