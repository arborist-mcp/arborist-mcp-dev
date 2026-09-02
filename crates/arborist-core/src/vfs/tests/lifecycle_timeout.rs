use std::cell::Cell;

use super::*;
use crate::deadline::DeadlineCheck;
use crate::{MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS, read_symbol_from_index};

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

struct RejectChecksAfterPhase {
    phase: &'static str,
    seen: Cell<bool>,
}

impl RejectChecksAfterPhase {
    fn new(phase: &'static str) -> Self {
        Self {
            phase,
            seen: Cell::new(false),
        }
    }
}

impl DeadlineCheck for RejectChecksAfterPhase {
    fn check(&self, phase: &str) -> anyhow::Result<()> {
        if self.seen.get() {
            anyhow::bail!("unexpected deadline check after {}: {phase}", self.phase);
        }
        if phase == self.phase {
            self.seen.set(true);
        }
        Ok(())
    }
}

#[test]
fn virtual_lifecycle_operations_reject_invalid_timeouts_before_file_work() {
    let mut vfs = VirtualFileSystem::new();

    let discard_zero = vfs
        .discard_file_with_timeout(Path::new(""), Some(0))
        .expect_err("zero discard timeout should fail before path validation");
    assert!(
        discard_zero
            .to_string()
            .contains("invalid virtual file discard timeout_ms: value must be greater than zero")
    );
    let discard_excessive = vfs
        .discard_file_with_timeout(
            Path::new("buffer.py"),
            Some(MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS + 1),
        )
        .expect_err("excessive discard timeout should fail");
    assert!(discard_excessive.to_string().contains(&format!(
        "must not exceed {MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS}"
    )));

    let close_zero = vfs
        .close_file_with_timeout(Path::new(""), true, Some(0))
        .expect_err("zero close timeout should fail before path validation");
    assert!(
        close_zero
            .to_string()
            .contains("invalid virtual file close timeout_ms: value must be greater than zero")
    );
    let close_excessive = vfs
        .close_file_with_timeout(
            Path::new("buffer.py"),
            false,
            Some(MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS + 1),
        )
        .expect_err("excessive close timeout should fail");
    assert!(close_excessive.to_string().contains(&format!(
        "must not exceed {MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS}"
    )));
}

#[test]
fn discard_timeout_restores_existing_dirty_entry_and_disk() {
    let disk_source = "def value() -> int:\n    return 1\n";
    let dirty_source = "def value() -> int:\n    return 9\n";
    let file = temp_file(disk_source);
    let mut vfs = VirtualFileSystem::new();
    let before = vfs.open_file(&file, Some(dirty_source)).unwrap();
    let deadline = FailOnPhase {
        phase: "virtual source replacement",
    };

    let error = vfs
        .discard_file_with_deadline(&file, &deadline)
        .expect_err("discard should fail before replacing the virtual source");

    assert!(error.to_string().contains("virtual source replacement"));
    assert_eq!(vfs.read_file(&file).unwrap(), before);
    assert_eq!(fs::read_to_string(&file).unwrap(), disk_source);
}

#[test]
fn discard_timeout_removes_entry_loaded_only_for_failed_request() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();
    let deadline = FailOnPhase {
        phase: "disk source read",
    };

    vfs.discard_file_with_deadline(&file, &deadline)
        .expect_err("discard should fail after loading the source");

    assert!(vfs.virtual_file_statuses(false).unwrap().is_empty());
}

#[test]
fn timed_discard_replaces_dirty_source_without_writing_disk() {
    let disk_source = "def value() -> int:\n    return 1\n";
    let file = temp_file(disk_source);
    let mut vfs = VirtualFileSystem::new();
    let dirty = vfs
        .open_file(&file, Some("def value() -> int:\n    return 9\n"))
        .unwrap();

    let discarded = vfs
        .discard_file_with_timeout(&file, Some(MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS))
        .expect("timed discard should succeed");

    assert!(!discarded.dirty);
    assert_eq!(discarded.source, disk_source);
    assert_eq!(discarded.disk_source, disk_source);
    assert_eq!(discarded.version, dirty.version + 1);
    assert_eq!(fs::read_to_string(&file).unwrap(), disk_source);
}

#[test]
fn discard_does_not_check_deadline_after_replacing_virtual_source() {
    let disk_source = "def value() -> int:\n    return 1\n";
    let file = temp_file(disk_source);
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&file, Some("def value() -> int:\n    return 9\n"))
        .unwrap();
    let deadline = RejectChecksAfterPhase::new("virtual source replacement");

    let discarded = vfs
        .discard_file_with_deadline(&file, &deadline)
        .expect("discard should not check the deadline after replacing state");

    assert!(deadline.seen.get());
    assert!(!discarded.dirty);
    assert_eq!(discarded.source, disk_source);
}

#[test]
fn close_persist_timeout_keeps_dirty_entry_open_and_disk_unchanged() {
    let disk_source = "def value() -> int:\n    return 1\n";
    let dirty_source = "def value() -> int:\n    return 9\n";
    let file = temp_file(disk_source);
    let mut vfs = VirtualFileSystem::new();
    let before = vfs.open_file(&file, Some(dirty_source)).unwrap();
    let deadline = FailOnPhase {
        phase: "commit persistence",
    };

    let error = vfs
        .close_file_with_deadline(&file, true, &deadline)
        .expect_err("close should fail before persistence");

    assert!(error.to_string().contains("commit persistence"));
    assert_eq!(vfs.read_file(&file).unwrap(), before);
    assert_eq!(fs::read_to_string(&file).unwrap(), disk_source);
}

#[test]
fn close_discard_timeout_keeps_dirty_entry_open() {
    let disk_source = "def value() -> int:\n    return 1\n";
    let file = temp_file(disk_source);
    let mut vfs = VirtualFileSystem::new();
    let before = vfs
        .open_file(&file, Some("def value() -> int:\n    return 9\n"))
        .unwrap();
    let deadline = FailOnPhase {
        phase: "virtual source replacement",
    };

    vfs.close_file_with_deadline(&file, false, &deadline)
        .expect_err("close should fail before discarding the dirty source");

    assert_eq!(vfs.read_file(&file).unwrap(), before);
    assert_eq!(fs::read_to_string(&file).unwrap(), disk_source);
}

#[test]
fn close_discard_timeout_removes_entry_loaded_only_for_failed_request() {
    let disk_source = "def value() -> int:\n    return 1\n";
    let file = temp_file(disk_source);
    let mut vfs = VirtualFileSystem::new();
    let deadline = FailOnPhase {
        phase: "disk source read",
    };

    vfs.close_file_with_deadline(&file, false, &deadline)
        .expect_err("close should fail after loading the source for discard");

    assert!(vfs.virtual_file_statuses(false).unwrap().is_empty());
}

#[test]
fn timed_close_persists_source_and_removes_virtual_entry() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&file, Some("def value() -> int:\n    return 9\n"))
        .unwrap();

    let closed = vfs
        .close_file_with_timeout(&file, true, Some(MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS))
        .expect("timed persisted close should succeed");

    assert!(!closed.dirty);
    assert!(closed.source.contains("return 9"));
    assert!(vfs.virtual_file_statuses(false).unwrap().is_empty());
    assert_eq!(fs::read_to_string(&file).unwrap(), closed.source);
}

#[test]
fn timed_close_discards_source_and_removes_virtual_entry() {
    let disk_source = "def value() -> int:\n    return 1\n";
    let file = temp_file(disk_source);
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&file, Some("def value() -> int:\n    return 9\n"))
        .unwrap();

    let closed = vfs
        .close_file_with_timeout(&file, false, Some(MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS))
        .expect("timed discarded close should succeed");

    assert!(!closed.dirty);
    assert_eq!(closed.source, disk_source);
    assert!(vfs.virtual_file_statuses(false).unwrap().is_empty());
    assert_eq!(fs::read_to_string(&file).unwrap(), disk_source);
}

#[test]
fn close_persist_keeps_persisted_entry_open_when_index_sync_fails() {
    let workspace = temp_workspace();
    let file = workspace.join("helper.py");
    let db_path = workspace.join("symbols.db");
    fs::write(
        &file,
        "def helper() -> int:
    return 1
",
    )
    .unwrap();
    let mut vfs = VirtualFileSystem::new();
    vfs.register_symbol_index(&workspace, &db_path).unwrap();
    let snapshot = vfs.read_file(&file).unwrap();
    let digit = snapshot.source.rfind('1').unwrap();
    vfs.apply_edit(&file, digit, digit + 1, "2").unwrap();

    let workspace_key = vfs.registered_symbol_indexes()[0].workspace_root.clone();
    let invalid_db_path = workspace.join("invalid-index");
    fs::create_dir_all(&invalid_db_path).unwrap();
    vfs.symbol_indexes
        .insert(workspace_key.clone(), invalid_db_path);

    vfs.close_file_with_timeout(&file, true, Some(MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS))
        .expect_err("index sync failure should keep the persisted entry open");

    let statuses = vfs.virtual_file_statuses(false).unwrap();
    assert_eq!(statuses.len(), 1);
    assert!(!statuses[0].dirty);
    assert!(fs::read_to_string(&file).unwrap().contains("return 2"));

    vfs.symbol_indexes.insert(workspace_key, db_path.clone());
    vfs.close_file_with_timeout(&file, true, Some(MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS))
        .expect("retry should synchronize the index and close the entry");

    assert!(vfs.virtual_file_statuses(false).unwrap().is_empty());
    assert!(
        read_symbol_from_index(&db_path, "helper")
            .unwrap()
            .source
            .contains("return 2")
    );
}

#[test]
fn close_does_not_check_deadline_after_persistence_starts() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&file, Some("def value() -> int:\n    return 9\n"))
        .unwrap();
    let deadline = RejectChecksAfterPhase::new("commit persistence");

    let closed = vfs
        .close_file_with_deadline(&file, true, &deadline)
        .expect("close should not check the deadline after persistence starts");

    assert!(deadline.seen.get());
    assert!(vfs.virtual_file_statuses(false).unwrap().is_empty());
    assert_eq!(fs::read_to_string(&file).unwrap(), closed.source);
}

#[test]
fn close_does_not_check_deadline_after_discard_replaces_state() {
    let disk_source = "def value() -> int:\n    return 1\n";
    let file = temp_file(disk_source);
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&file, Some("def value() -> int:\n    return 9\n"))
        .unwrap();
    let deadline = RejectChecksAfterPhase::new("virtual source replacement");

    let closed = vfs
        .close_file_with_deadline(&file, false, &deadline)
        .expect("close should not check the deadline after discard replaces state");

    assert!(deadline.seen.get());
    assert_eq!(closed.source, disk_source);
    assert!(vfs.virtual_file_statuses(false).unwrap().is_empty());
}
