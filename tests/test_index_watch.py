from __future__ import annotations

from contextlib import redirect_stderr
import io
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from arborist_mcp.index_watch import (
    IndexWatchError,
    IndexWatchTarget,
    check_watch_targets,
    load_watch_config,
    reconcile_index,
    run_cli,
    run_watch,
    run_watch_targets,
)


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
    def __init__(
        self,
        health: str,
        refresh: str = '{"indexed_files": 1}',
        migrate: str = '{"ok": true, "issues": []}',
    ) -> None:
        self.health = health
        self.refresh = refresh
        self.migrate = migrate
        self.inspect_calls: list[str] = []
        self.refresh_calls: list[tuple[object, ...]] = []
        self.migrate_calls: list[str] = []

    def inspect_symbol_index_json(
        self, db_path: str, timeout_ms: int | None = None
    ) -> str:
        self.inspect_calls.append(db_path)
        self.inspect_timeout_ms = timeout_ms
        return self.health

    def refresh_symbol_index_json(self, *args: object) -> str:
        self.refresh_calls.append(args)
        return self.refresh

    def migrate_symbol_index_json(self, db_path: str) -> str:
        self.migrate_calls.append(db_path)
        return self.migrate


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

    def test_reconcile_dry_run_reports_refresh_without_writing(self) -> None:
        core = StubCore(
            health_payload(ok=False, action="rebuild", reason="indexed file is stale")
        )

        event = reconcile_index(
            core,
            workspace_root="workspace",
            db_path="symbols.db",
            max_files=20,
            max_file_bytes=None,
            dry_run=True,
        )

        self.assertEqual(event["status"], "would_refresh")
        self.assertEqual(core.refresh_calls, [])
        self.assertEqual(core.migrate_calls, [])

    def test_reconcile_passes_optional_timeout_to_refresh(self) -> None:
        core = StubCore(
            health_payload(ok=False, action="rebuild", reason="indexed file is stale")
        )

        reconcile_index(
            core,
            workspace_root="workspace",
            db_path="symbols.db",
            max_files=20,
            max_file_bytes=None,
            timeout_ms=5000,
        )

        self.assertEqual(
            core.refresh_calls,
            [("workspace", "symbols.db", 20, None, 5000)],
        )
        self.assertEqual(core.inspect_timeout_ms, 5000)

    def test_reconcile_migrates_supported_schema_version(self) -> None:
        core = StubCore(
            health_payload(ok=False, action="migrate", reason="schema v1 can migrate"),
            migrate=health_payload(ok=True, action="none"),
        )

        event = reconcile_index(
            core,
            workspace_root="workspace",
            db_path="symbols.db",
            max_files=20,
            max_file_bytes=None,
        )

        self.assertEqual(event["status"], "migrated")
        self.assertEqual(core.migrate_calls, ["symbols.db"])
        self.assertEqual(core.refresh_calls, [])
        self.assertEqual(event["migrated_health"]["ok"], True)

    def test_reconcile_dry_run_reports_migration_without_writing(self) -> None:
        core = StubCore(
            health_payload(ok=False, action="migrate", reason="schema v1 can migrate")
        )

        event = reconcile_index(
            core,
            workspace_root="workspace",
            db_path="symbols.db",
            max_files=20,
            max_file_bytes=None,
            dry_run=True,
        )

        self.assertEqual(event["status"], "would_migrate")
        self.assertEqual(core.refresh_calls, [])
        self.assertEqual(core.migrate_calls, [])

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
        self.assertNotIn("workspace_root", events[0])

    def test_run_watch_targets_emits_deterministic_workspace_order(self) -> None:
        core = StubCore(health_payload(ok=True, action="none"))
        events: list[dict[str, object]] = []

        run_watch_targets(
            core,
            targets=(
                IndexWatchTarget("workspace-z", "z.db"),
                IndexWatchTarget("workspace-a", "a.db"),
            ),
            interval_seconds=1,
            max_files=20,
            max_file_bytes=None,
            once=True,
            emit=events.append,
        )

        self.assertEqual(
            [event["workspace_root"] for event in events],
            ["workspace-a", "workspace-z"],
        )
        self.assertEqual(core.inspect_calls, ["a.db", "z.db"])

    def test_check_watch_targets_reports_all_targets_without_writing(self) -> None:
        core = StubCore(
            health_payload(ok=False, action="rebuild", reason="indexed file is stale")
        )
        events: list[dict[str, object]] = []

        healthy = check_watch_targets(
            core,
            targets=(
                IndexWatchTarget("workspace-z", "z.db"),
                IndexWatchTarget("workspace-a", "a.db"),
            ),
            max_files=20,
            max_file_bytes=None,
            emit=events.append,
        )

        self.assertFalse(healthy)
        self.assertEqual(
            [event["workspace_root"] for event in events],
            ["workspace-a", "workspace-z"],
        )
        self.assertEqual([event["status"] for event in events], ["would_refresh", "would_refresh"])
        self.assertEqual(core.refresh_calls, [])
        self.assertEqual(core.migrate_calls, [])

    def test_load_watch_config_resolves_and_orders_targets(self) -> None:
        with TemporaryDirectory() as temporary_directory:
            config_path = Path(temporary_directory).joinpath("watch.json")
            config_path.write_text(
                json.dumps(
                    {
                        "indexes": [
                            {"workspace_root": "z", "db_path": "z/symbols.db"},
                            {"workspace_root": "a", "db_path": "a/symbols.db"},
                        ]
                    }
                ),
                encoding="utf-8",
            )

            targets = load_watch_config(config_path)

            self.assertEqual(
                targets,
                (
                    IndexWatchTarget(
                        str(config_path.parent.joinpath("a").resolve()),
                        str(config_path.parent.joinpath("a", "symbols.db").resolve()),
                    ),
                    IndexWatchTarget(
                        str(config_path.parent.joinpath("z").resolve()),
                        str(config_path.parent.joinpath("z", "symbols.db").resolve()),
                    ),
                ),
            )

    def test_load_watch_config_rejects_unknown_fields_and_duplicate_workspaces(self) -> None:
        with TemporaryDirectory() as temporary_directory:
            config_path = Path(temporary_directory).joinpath("watch.json")
            config_path.write_text(
                json.dumps(
                    {
                        "indexes": [
                            {
                                "workspace_root": ".",
                                "db_path": "symbols.db",
                                "extra": True,
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(IndexWatchError, "unexpected field `extra`"):
                load_watch_config(config_path)

            config_path.write_text(
                json.dumps(
                    {
                        "indexes": [
                            {"workspace_root": ".", "db_path": "a.db"},
                            {"workspace_root": ".", "db_path": "b.db"},
                        ]
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(IndexWatchError, "duplicate workspace_root"):
                load_watch_config(config_path)

            config_path.write_text(
                json.dumps(
                    {
                        "indexes": [
                            {"workspace_root": "a", "db_path": "symbols.db"},
                            {"workspace_root": "b", "db_path": "symbols.db"},
                        ]
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(IndexWatchError, "duplicate db_path"):
                load_watch_config(config_path)

            config_path.write_text(
                '{"indexes":[{"workspace_root":"a","workspace_root":"b","db_path":"a.db"}]}',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(IndexWatchError, "duplicate object key"):
                load_watch_config(config_path)

    def test_cli_once_emits_json_event(self) -> None:
        core = StubCore(health_payload(ok=False, action="rebuild", reason="missing index"))
        stdout = io.StringIO()
        stderr = io.StringIO()
        current_directory = Path.cwd()

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
        event = json.loads(stdout.getvalue())
        self.assertEqual(event["status"], "refreshed")
        self.assertEqual(event["db_path"], str(current_directory.joinpath("symbols.db").resolve()))
        self.assertEqual(
            core.inspect_calls,
            [str(current_directory.joinpath("symbols.db").resolve())],
        )
        self.assertEqual(
            core.refresh_calls,
            [
                (
                    str(current_directory.joinpath("workspace").resolve()),
                    str(current_directory.joinpath("symbols.db").resolve()),
                    20_000,
                    None,
                )
            ],
        )

    def test_cli_dry_run_emits_planned_action_without_writing(self) -> None:
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
                "--dry-run",
            ],
            core_factory=lambda: core,
            stdout=stdout,
            stderr=stderr,
        )

        self.assertEqual(result, 0)
        self.assertEqual(stderr.getvalue(), "")
        self.assertEqual(json.loads(stdout.getvalue())["status"], "would_refresh")
        self.assertEqual(core.refresh_calls, [])

    def test_cli_check_returns_nonzero_for_repairable_index_without_writing(self) -> None:
        core = StubCore(health_payload(ok=False, action="rebuild", reason="missing index"))
        stdout = io.StringIO()
        stderr = io.StringIO()

        result = run_cli(
            ["--workspace-root", "workspace", "--db-path", "symbols.db", "--check"],
            core_factory=lambda: core,
            stdout=stdout,
            stderr=stderr,
        )

        self.assertEqual(result, 1)
        self.assertEqual(stderr.getvalue(), "")
        self.assertEqual(json.loads(stdout.getvalue())["status"], "would_refresh")
        self.assertEqual(core.refresh_calls, [])

    def test_cli_check_returns_zero_for_healthy_index(self) -> None:
        core = StubCore(health_payload(ok=True, action="none"))
        stdout = io.StringIO()

        result = run_cli(
            ["--db-path", "symbols.db", "--check"],
            core_factory=lambda: core,
            stdout=stdout,
        )

        self.assertEqual(result, 0)
        self.assertEqual(json.loads(stdout.getvalue())["status"], "healthy")

    def test_cli_config_dry_run_preserves_target_context_without_writing(self) -> None:
        core = StubCore(health_payload(ok=False, action="rebuild", reason="missing index"))
        stdout = io.StringIO()
        stderr = io.StringIO()

        with TemporaryDirectory() as temporary_directory:
            config_path = Path(temporary_directory).joinpath("watch.json")
            config_path.write_text(
                json.dumps(
                    {
                        "indexes": [
                            {"workspace_root": "workspace", "db_path": "symbols.db"}
                        ]
                    }
                ),
                encoding="utf-8",
            )
            result = run_cli(
                ["--config", str(config_path), "--once", "--dry-run"],
                core_factory=lambda: core,
                stdout=stdout,
                stderr=stderr,
            )

        self.assertEqual(result, 0)
        self.assertEqual(stderr.getvalue(), "")
        event = json.loads(stdout.getvalue())
        self.assertEqual(event["status"], "would_refresh")
        self.assertEqual(event["workspace_root"], str(config_path.parent.joinpath("workspace").resolve()))
        self.assertEqual(core.refresh_calls, [])

    def test_cli_once_reads_multi_index_watch_config(self) -> None:
        core = StubCore(health_payload(ok=True, action="none"))
        stdout = io.StringIO()
        stderr = io.StringIO()

        with TemporaryDirectory() as temporary_directory:
            config_path = Path(temporary_directory).joinpath("watch.json")
            config_path.write_text(
                json.dumps(
                    {
                        "indexes": [
                            {"workspace_root": "workspace-b", "db_path": "b.db"},
                            {"workspace_root": "workspace-a", "db_path": "a.db"},
                        ]
                    }
                ),
                encoding="utf-8",
            )

            result = run_cli(
                ["--config", str(config_path), "--once"],
                core_factory=lambda: core,
                stdout=stdout,
                stderr=stderr,
            )

        self.assertEqual(result, 0)
        self.assertEqual(stderr.getvalue(), "")
        events = [json.loads(line) for line in stdout.getvalue().splitlines()]
        self.assertEqual([event["status"] for event in events], ["healthy", "healthy"])
        self.assertEqual([event["db_path"] for event in events], [
            str(config_path.parent.joinpath("a.db").resolve()),
            str(config_path.parent.joinpath("b.db").resolve()),
        ])

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
            with self.assertRaises(SystemExit):
                run_cli(["--db-path", "symbols.db", "--timeout-ms", "300001"])

    def test_pyproject_registers_index_watch_console_script(self) -> None:
        pyproject = Path(__file__).resolve().parents[1].joinpath("pyproject.toml")
        self.assertIn(
            'arborist-index-watch = "arborist_mcp.index_watch:main"',
            pyproject.read_text(encoding="utf-8"),
        )


if __name__ == "__main__":
    unittest.main()
