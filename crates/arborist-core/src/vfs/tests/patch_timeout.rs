use std::cell::Cell;

use super::super::patch_context::VirtualPatchTarget;
use super::*;
use crate::deadline::DeadlineCheck;
use crate::symbol_trace::TraceQueryDeadline;
use crate::{MAX_PATCH_TIMEOUT_MS, read_symbol_from_index};

struct FailOnPhase {
    phase: &'static str,
}

impl DeadlineCheck for FailOnPhase {
    fn check(&self, phase: &str) -> anyhow::Result<()> {
        if phase == self.phase {
            anyhow::bail!("test deadline expired during {phase}");
        }
        Ok(())
    }
}

#[derive(Default)]
struct RejectChecksAfterSourceWrite {
    source_write_seen: Cell<bool>,
}

impl DeadlineCheck for RejectChecksAfterSourceWrite {
    fn check(&self, phase: &str) -> anyhow::Result<()> {
        if self.source_write_seen.get() {
            anyhow::bail!("unexpected deadline check after source write: {phase}");
        }
        if phase == "source write" {
            self.source_write_seen.set(true);
        }
        Ok(())
    }
}

#[test]
fn virtual_patch_rejects_invalid_timeout_before_file_work() {
    let mut vfs = VirtualFileSystem::new();
    let path = Path::new("");
    let position = Position { row: 0, column: 0 };
    let replacement = "def value() -> int:\n    return 2\n";
    let errors = [
        vfs.patch_node_with_timeout(path, "value", replacement, None, Some(0))
            .expect_err("semantic virtual patch should reject zero timeout"),
        vfs.patch_node_at_position_with_timeout(path, &position, replacement, None, Some(0))
            .expect_err("position virtual patch should reject zero timeout"),
        vfs.patch_node_and_commit_with_timeout(path, "value", replacement, None, Some(0))
            .expect_err("semantic committed patch should reject zero timeout"),
        vfs.patch_node_at_position_and_commit_with_timeout(
            path,
            &position,
            replacement,
            None,
            Some(0),
        )
        .expect_err("position committed patch should reject zero timeout"),
    ];

    for error in errors {
        assert!(
            error
                .to_string()
                .contains("invalid patch timeout_ms: value must be greater than zero")
        );
    }

    let excessive = vfs
        .patch_node_with_timeout(
            Path::new("buffer.py"),
            "value",
            replacement,
            None,
            Some(MAX_PATCH_TIMEOUT_MS + 1),
        )
        .expect_err("excessive virtual patch timeout should fail");
    assert!(
        excessive
            .to_string()
            .contains(&format!("must not exceed {MAX_PATCH_TIMEOUT_MS}"))
    );
}

#[test]
fn timed_virtual_patch_and_commit_preserves_existing_dirty_source() {
    let disk_source = "def helper() -> int:\n    return 1\n\ndef value() -> int:\n    return 1\n";
    let unsaved_source =
        "def helper() -> int:\n    return 2\n\ndef value() -> int:\n    return 1\n";
    let replacement = "def value() -> int:\n    return helper()\n";
    let file = temp_file(disk_source);
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&file, Some(unsaved_source)).unwrap();

    let result = vfs
        .patch_node_and_commit_with_timeout(
            &file,
            "value",
            replacement,
            None,
            Some(MAX_PATCH_TIMEOUT_MS),
        )
        .expect("timed virtual patch should commit");

    assert!(result.applied);
    let snapshot = vfs.read_file(&file).unwrap();
    assert!(!snapshot.dirty);
    assert_eq!(snapshot.source, result.updated_source);
    assert!(snapshot.source.contains("return 2"));
    assert_eq!(fs::read_to_string(&file).unwrap(), result.updated_source);
}

#[test]
fn timed_position_virtual_patch_and_commit_writes_source() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    let result = vfs
        .patch_node_at_position_and_commit_with_timeout(
            &file,
            &Position { row: 0, column: 4 },
            "def value() -> int:\n    return 2\n",
            None,
            Some(MAX_PATCH_TIMEOUT_MS),
        )
        .expect("timed position patch should commit");

    assert!(result.applied);
    assert_eq!(fs::read_to_string(&file).unwrap(), result.updated_source);
    assert!(!vfs.read_file(&file).unwrap().dirty);
}

#[test]
fn blocked_committed_patch_preserves_dirty_entry_and_disk() {
    let disk_source = "def value() -> int:\n    return 1\n";
    let unsaved_source = "def value() -> int:\n    return 2\n";
    let file = temp_file(disk_source);
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.open_file(&file, Some(unsaved_source)).unwrap();

    let result = vfs
        .patch_node_and_commit_with_timeout(
            &file,
            "value",
            "def value() -> int:\n    return missing_value\n",
            None,
            Some(MAX_PATCH_TIMEOUT_MS),
        )
        .expect("invalid patch should return a blocked result");

    assert!(!result.applied);
    let snapshot = vfs.read_file(&file).unwrap();
    assert_eq!(snapshot.source, initial.source);
    assert_eq!(snapshot.disk_source, initial.disk_source);
    assert_eq!(snapshot.version, initial.version);
    assert_eq!(snapshot.dirty, initial.dirty);
    assert_eq!(fs::read_to_string(&file).unwrap(), disk_source);
}

#[test]
fn timeout_after_virtual_edit_restores_dirty_entry() {
    let disk_source = "def value() -> int:\n    return 1\n";
    let unsaved_source = "def value() -> int:\n    return 2\n";
    let file = temp_file(disk_source);
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.open_file(&file, Some(unsaved_source)).unwrap();
    let deadline = FailOnPhase {
        phase: "virtual source edit result",
    };

    let error = vfs
        .patch_node_with_deadline(
            &file,
            VirtualPatchTarget::Semantic("value"),
            "def value() -> int:\n    return 3\n",
            None,
            true,
            &deadline,
        )
        .expect_err("post-edit timeout should fail");

    assert!(error.to_string().contains("virtual source edit result"));
    let snapshot = vfs.read_file(&file).unwrap();
    assert_eq!(snapshot.source, initial.source);
    assert_eq!(snapshot.disk_source, initial.disk_source);
    assert_eq!(snapshot.version, initial.version);
    assert_eq!(snapshot.dirty, initial.dirty);
    assert_eq!(fs::read_to_string(&file).unwrap(), disk_source);
}

#[test]
fn timeout_before_source_write_restores_dirty_entry() {
    let disk_source = "def value() -> int:\n    return 1\n";
    let unsaved_source = "def value() -> int:\n    return 2\n";
    let file = temp_file(disk_source);
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.open_file(&file, Some(unsaved_source)).unwrap();
    let deadline = FailOnPhase {
        phase: "source write",
    };

    let error = vfs
        .patch_node_with_deadline(
            &file,
            VirtualPatchTarget::Semantic("value"),
            "def value() -> int:\n    return 3\n",
            None,
            true,
            &deadline,
        )
        .expect_err("pre-write timeout should fail");

    assert!(error.to_string().contains("source write"));
    let snapshot = vfs.read_file(&file).unwrap();
    assert_eq!(snapshot.source, initial.source);
    assert_eq!(snapshot.disk_source, initial.disk_source);
    assert_eq!(snapshot.version, initial.version);
    assert_eq!(snapshot.dirty, initial.dirty);
    assert_eq!(fs::read_to_string(&file).unwrap(), disk_source);
}

#[test]
fn committed_patch_does_not_check_deadline_after_source_write_starts() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();
    let deadline = RejectChecksAfterSourceWrite::default();

    let result = vfs
        .patch_node_with_deadline(
            &file,
            VirtualPatchTarget::Semantic("value"),
            "def value() -> int:\n    return 2\n",
            None,
            true,
            &deadline,
        )
        .expect("commit should not perform post-write deadline checks");

    assert!(deadline.source_write_seen.get());
    assert!(result.applied);
    assert_eq!(fs::read_to_string(&file).unwrap(), result.updated_source);
}

#[test]
fn committed_patch_preserves_persisted_state_when_index_sync_fails() {
    let workspace = temp_workspace();
    let file = workspace.join("buffer.py");
    let db_path = workspace.join("symbols.db");
    fs::write(&file, "def value() -> int:\n    return 1\n").unwrap();
    let mut vfs = VirtualFileSystem::new();
    vfs.register_symbol_index(&workspace, &db_path).unwrap();

    let workspace_key = vfs.registered_symbol_indexes()[0].workspace_root.clone();
    let invalid_db_path = workspace.join("invalid-index");
    fs::create_dir_all(&invalid_db_path).unwrap();
    vfs.symbol_indexes
        .insert(workspace_key.clone(), invalid_db_path);

    let error = vfs
        .patch_node_and_commit_with_timeout(
            &file,
            "value",
            "def value() -> int:\n    return 2\n",
            None,
            Some(MAX_PATCH_TIMEOUT_MS),
        )
        .expect_err("index sync failure should be reported after persistence");

    assert!(error.to_string().contains("failed to commit virtual patch"));
    let snapshot = vfs.read_file(&file).unwrap();
    assert!(!snapshot.dirty);
    assert!(snapshot.source.contains("return 2"));
    assert_eq!(fs::read_to_string(&file).unwrap(), snapshot.source);

    vfs.symbol_indexes.insert(workspace_key, db_path.clone());
    vfs.commit_file(&file)
        .expect("retry should synchronize the persisted patch");
    assert!(
        read_symbol_from_index(&db_path, "value")
            .unwrap()
            .source
            .contains("return 2")
    );
}

#[test]
fn trace_backed_virtual_patch_result_reuses_caller_deadline() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let workspace = file.parent().expect("temporary file should have a parent");
    let mut vfs = VirtualFileSystem::new();
    let patch = vfs
        .patch_node(&file, "value", "def value() -> int:\n    return 2\n", None)
        .expect("virtual patch should apply");
    let deadline = TraceQueryDeadline::expired_for_tests(1);

    let error = vfs
        .trace_backed_patch_result_with_deadline(workspace, &patch, TraceDirection::Both, &deadline)
        .expect_err("trace-backed virtual patch results should honor an expired caller deadline");

    assert!(error.to_string().contains("virtual patch overrides"));
}

#[test]
fn virtual_read_context_patch_results_reuse_caller_deadline() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let workspace = file.parent().expect("temporary file should have a parent");
    let mut vfs = VirtualFileSystem::new();
    let patch = vfs
        .patch_node(&file, "value", "def value() -> int:\n    return 2\n", None)
        .expect("virtual patch should apply");
    let deadline = TraceQueryDeadline::expired_for_tests(1);

    let neighborhood_error = vfs
        .neighborhood_context_patch_result_with_deadline(
            workspace,
            &patch,
            TraceDirection::Both,
            1,
            10,
            &deadline,
        )
        .expect_err("neighborhood patch results should honor an expired caller deadline");
    let discovery_error = vfs
        .discovery_context_patch_result_with_deadline(
            workspace,
            &patch,
            TraceDirection::Both,
            1,
            10,
            &deadline,
        )
        .expect_err("discovery patch results should honor an expired caller deadline");

    assert!(
        neighborhood_error
            .to_string()
            .contains("virtual neighborhood patch overrides")
    );
    assert!(
        discovery_error
            .to_string()
            .contains("virtual discovery patch overrides")
    );
}
