import json
import os
import unittest

from arborist_mcp.index_watch import (
    IndexWatchError,
    IndexWatchTarget,
    _ordered_watch_targets,
    check_watch_targets,
    reconcile_index,
    run_watch_targets,
)
from arborist_mcp.index_watch_runtime import _health_summary


def health_payload(
    *,
    ok: bool,
    action: str,
    reason: str = "current",
) -> str:
    return json.dumps(
        {
            "ok": ok,
            "exists": True,
            "schema_version": "1",
            "expected_schema_version": "1",
            "issues": [] if ok else [reason],
            "stale_files": [],
            "missing_files": [],
            "unreadable_files": [],
            "unindexed_files": [],
            "migration": {"action": action, "reason": reason},
        }
    )


class FailingCore:
    """A core stub whose native calls raise, simulating a broken extension."""

    def __init__(self, method_name: str, message: str) -> None:
        self._method_name = method_name
        self._message = message

    def inspect_symbol_index_json(
        self, db_path: str, timeout_ms: int | None = None
    ) -> str:
        if self._method_name == "inspect":
            raise RuntimeError(self._message)
        raise AssertionError("unexpected call")

    def migrate_symbol_index_json(
        self, db_path: str, timeout_ms: int | None = None
    ) -> str:
        raise RuntimeError(f"{self._method_name}: {self._message}")

    def refresh_symbol_index_json(self, *args: object) -> str:
        raise RuntimeError(f"{self._method_name}: {self._message}")


class HealthSummaryTests(unittest.TestCase):
    def test_summary_counts_well_formed_lists(self) -> None:
        summary = _health_summary(
            {
                "ok": True,
                "issues": ["a"],
                "stale_files": ["s1", "s2"],
                "missing_files": [],
                "unreadable_files": ["u"],
                "unindexed_files": [],
            }
        )

        self.assertEqual(
            summary,
            {
                "ok": True,
                "issues": 1,
                "stale_files": 2,
                "missing_files": 0,
                "unreadable_files": 1,
                "unindexed_files": 0,
            },
        )

    def test_summary_reports_none_for_non_list_fields_and_defaults(self) -> None:
        malformed = {
            "ok": False,
            "issues": "schema drift",
            "stale_files": 3,
            "missing_files": {"x": 1},
            "unreadable_files": True,
            "unindexed_files": ("tuple",),
        }
        self.assertEqual(
            _health_summary(malformed),
            {
                "ok": False,
                "issues": None,
                "stale_files": None,
                "missing_files": None,
                "unreadable_files": None,
                "unindexed_files": None,
            },
        )
        self.assertEqual(
            _health_summary({}),
            {
                "ok": None,
                "issues": None,
                "stale_files": None,
                "missing_files": None,
                "unreadable_files": None,
                "unindexed_files": None,
            },
        )


class ReconcileFailureWrappingTests(unittest.TestCase):
    def test_inspect_failure_is_wrapped_with_context(self) -> None:
        core = FailingCore("inspect", "disk unavailable")

        with self.assertRaisesRegex(
            IndexWatchError, "failed to inspect symbol index: disk unavailable"
        ):
            reconcile_index(
                core,
                workspace_root="workspace",
                db_path="symbols.db",
                max_files=20,
                max_file_bytes=None,
            )

    def test_migration_failure_is_wrapped_with_context(self) -> None:
        class MigratingCore(FailingCore):
            def __init__(self) -> None:
                super().__init__("migrate", "db locked")

            def inspect_symbol_index_json(
                self, db_path: str, timeout_ms: int | None = None
            ) -> str:
                return health_payload(ok=False, action="migrate", reason="old schema")

        with self.assertRaisesRegex(
            IndexWatchError, "failed to migrate symbol index: migrate: db locked"
        ):
            reconcile_index(
                MigratingCore(),
                workspace_root="workspace",
                db_path="symbols.db",
                max_files=20,
                max_file_bytes=None,
            )

    def test_migration_must_return_a_healthy_index(self) -> None:
        class UnhealthyMigrationCore(FailingCore):
            def __init__(self) -> None:
                super().__init__("migrate", "unused")

            def inspect_symbol_index_json(
                self, db_path: str, timeout_ms: int | None = None
            ) -> str:
                return health_payload(ok=False, action="migrate", reason="old schema")

            def migrate_symbol_index_json(
                self, db_path: str, timeout_ms: int | None = None
            ) -> str:
                return health_payload(
                    ok=False,
                    action="manual",
                    reason="post-migration validation failed",
                )

        with self.assertRaisesRegex(
            IndexWatchError,
            "migration completed unsuccessfully: post-migration validation failed",
        ):
            reconcile_index(
                UnhealthyMigrationCore(),
                workspace_root="workspace",
                db_path="symbols.db",
                max_files=20,
                max_file_bytes=None,
            )

    def test_migration_without_issues_uses_fail_closed_reason(self) -> None:
        class IncompleteMigrationCore(FailingCore):
            def __init__(self) -> None:
                super().__init__("migrate", "unused")

            def inspect_symbol_index_json(
                self, db_path: str, timeout_ms: int | None = None
            ) -> str:
                return health_payload(ok=False, action="migrate", reason="old schema")

            def migrate_symbol_index_json(
                self, db_path: str, timeout_ms: int | None = None
            ) -> str:
                return json.dumps({"ok": False, "issues": []})

        with self.assertRaisesRegex(
            IndexWatchError,
            "migration completed unsuccessfully: migration did not produce a healthy index",
        ):
            reconcile_index(
                IncompleteMigrationCore(),
                workspace_root="workspace",
                db_path="symbols.db",
                max_files=20,
                max_file_bytes=None,
            )

    def test_refresh_failure_is_wrapped_with_context(self) -> None:
        class RebuildableCore(FailingCore):
            def __init__(self) -> None:
                super().__init__("refresh", "io error")

            def inspect_symbol_index_json(
                self, db_path: str, timeout_ms: int | None = None
            ) -> str:
                return health_payload(ok=False, action="rebuild", reason="stale")

        with self.assertRaisesRegex(
            IndexWatchError, "failed to refresh symbol index: refresh: io error"
        ):
            reconcile_index(
                RebuildableCore(),
                workspace_root="workspace",
                db_path="symbols.db",
                max_files=20,
                max_file_bytes=None,
            )


class OrderedWatchTargetTests(unittest.TestCase):
    def test_rejects_malformed_target_fields_before_path_normalization(self) -> None:
        malformed_targets = (
            (IndexWatchTarget("", "symbols.db"), "workspace_root"),
            (IndexWatchTarget("workspace", ""), "db_path"),
            (IndexWatchTarget("workspace", 7), "db_path"),
        )

        for target, field_name in malformed_targets:
            with self.subTest(field_name=field_name):
                with self.assertRaisesRegex(
                    IndexWatchError,
                    rf"index watch target 0\.{field_name} must be a non-empty string",
                ):
                    _ordered_watch_targets((target,))

    def test_rejects_duplicate_workspace_roots(self) -> None:
        targets = (
            IndexWatchTarget("workspace", "one.db"),
            IndexWatchTarget("workspace", "two.db"),
        )

        with self.assertRaisesRegex(
            IndexWatchError, "duplicate workspace_root `workspace`"
        ):
            _ordered_watch_targets(targets)

    def test_rejects_duplicate_database_paths(self) -> None:
        targets = (
            IndexWatchTarget("one", "symbols.db"),
            IndexWatchTarget("two", "symbols.db"),
        )

        with self.assertRaisesRegex(IndexWatchError, "duplicate db_path `symbols.db`"):
            _ordered_watch_targets(targets)

    def test_rejects_path_aliases_for_duplicate_targets(self) -> None:
        targets = (
            IndexWatchTarget("workspace", "indexes/../symbols.db"),
            IndexWatchTarget("workspace/.", "symbols.db"),
        )

        with self.assertRaisesRegex(IndexWatchError, "duplicate workspace_root"):
            _ordered_watch_targets(targets)

    def test_rejects_database_path_aliases(self) -> None:
        targets = (
            IndexWatchTarget("workspace-a", "indexes/../symbols.db"),
            IndexWatchTarget("workspace-b", "symbols.db"),
        )

        with self.assertRaisesRegex(IndexWatchError, "duplicate db_path"):
            _ordered_watch_targets(targets)

    @unittest.skipUnless(
        os.name == "nt", "case-insensitive path semantics require Windows"
    )
    def test_rejects_case_insensitive_duplicate_database_paths_on_windows(self) -> None:
        targets = (
            IndexWatchTarget("one", "Symbols.db"),
            IndexWatchTarget("two", "symbols.db"),
        )

        with self.assertRaisesRegex(IndexWatchError, "duplicate db_path `symbols.db`"):
            _ordered_watch_targets(targets)

    def test_unrepairable_index_falls_back_to_issue_then_default_reason(self) -> None:
        class UnrepairableCore(FailingCore):
            def __init__(self, payload: str) -> None:
                super().__init__("none", "unused")
                self._payload = payload

            def inspect_symbol_index_json(
                self, db_path: str, timeout_ms: int | None = None
            ) -> str:
                return self._payload

        with self.assertRaisesRegex(
            IndexWatchError,
            "cannot repair this index: first issue",
        ):
            reconcile_index(
                UnrepairableCore(
                    json.dumps(
                        {
                            "ok": False,
                            "exists": False,
                            "migration": {"action": "manual", "reason": ""},
                            "issues": ["first issue"],
                        }
                    )
                ),
                workspace_root="workspace",
                db_path="symbols.db",
                max_files=20,
                max_file_bytes=None,
            )
        with self.assertRaisesRegex(
            IndexWatchError,
            "cannot repair this index: index is unhealthy$",
        ):
            reconcile_index(
                UnrepairableCore(
                    json.dumps(
                        {
                            "ok": False,
                            "exists": False,
                            "migration": {},
                            "issues": [],
                        }
                    )
                ),
                workspace_root="workspace",
                db_path="symbols.db",
                max_files=20,
                max_file_bytes=None,
            )


class WatchLoopSemanticsTests(unittest.TestCase):
    def test_multi_cycle_watch_suppresses_repeat_healthy_events(self) -> None:
        core_calls: list[str] = []

        class HealthyCore:
            def inspect_symbol_index_json(
                self, db_path: str, timeout_ms: int | None = None
            ) -> str:
                core_calls.append(db_path)
                return health_payload(ok=True, action="none")

            def migrate_symbol_index_json(
                self, db_path: str, timeout_ms: int | None = None
            ) -> str:
                raise AssertionError("unexpected migration")

            def refresh_symbol_index_json(self, *args: object) -> str:
                raise AssertionError("unexpected refresh")

        class CycleLimitReached(Exception):
            pass

        events: list[dict[str, object]] = []
        sleeps: list[float] = []

        def stop_after_second_cycle(seconds: float) -> None:
            sleeps.append(seconds)
            if len(sleeps) >= 2:
                raise CycleLimitReached

        with self.assertRaises(CycleLimitReached):
            run_watch_targets(
                HealthyCore(),
                targets=(IndexWatchTarget("workspace", "symbols.db"),),
                interval_seconds=5,
                max_files=20,
                max_file_bytes=None,
                once=False,
                sleep=stop_after_second_cycle,
                emit=events.append,
            )

        # Two cycles ran; only the first healthy event was emitted.
        self.assertEqual(len(core_calls), 2)
        self.assertEqual(len(sleeps), 2)
        self.assertEqual([event["status"] for event in events], ["healthy"])

    def test_check_watch_targets_returns_true_when_all_targets_healthy(self) -> None:
        class HealthyCore:
            def inspect_symbol_index_json(
                self, db_path: str, timeout_ms: int | None = None
            ) -> str:
                return health_payload(ok=True, action="none")

            def migrate_symbol_index_json(
                self, db_path: str, timeout_ms: int | None = None
            ) -> str:
                raise AssertionError("unexpected migration")

            def refresh_symbol_index_json(self, *args: object) -> str:
                raise AssertionError("unexpected refresh")

        events: list[dict[str, object]] = []

        healthy = check_watch_targets(
            HealthyCore(),
            targets=(IndexWatchTarget("workspace-a", "a.db"),),
            max_files=20,
            max_file_bytes=None,
            emit=events.append,
        )

        self.assertTrue(healthy)
        self.assertEqual([event["status"] for event in events], ["healthy"])

    def test_ordered_watch_targets_requires_at_least_one_target(self) -> None:
        with self.assertRaisesRegex(IndexWatchError, "at least one target"):
            _ordered_watch_targets(())


if __name__ == "__main__":
    unittest.main()
