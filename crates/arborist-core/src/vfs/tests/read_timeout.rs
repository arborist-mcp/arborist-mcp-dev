use std::cell::Cell;

use super::*;
use crate::MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS;
use crate::deadline::DeadlineCheck;

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

struct FailOnNthPhase {
    phase: &'static str,
    remaining: Cell<usize>,
}

impl FailOnNthPhase {
    fn new(phase: &'static str, occurrence: usize) -> Self {
        Self {
            phase,
            remaining: Cell::new(occurrence),
        }
    }
}

impl DeadlineCheck for FailOnNthPhase {
    fn check(&self, phase: &str) -> anyhow::Result<()> {
        if phase != self.phase {
            return Ok(());
        }
        let remaining = self.remaining.get();
        if remaining == 1 {
            anyhow::bail!("test deadline expired during {phase}");
        }
        self.remaining.set(remaining - 1);
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
fn virtual_read_operations_reject_invalid_timeouts_before_file_work() {
    let mut vfs = VirtualFileSystem::new();

    let open_zero = vfs
        .open_file_with_timeout(Path::new(""), None, Some(0))
        .expect_err("zero open timeout should fail before path validation");
    assert!(
        open_zero
            .to_string()
            .contains("invalid virtual file open timeout_ms: value must be greater than zero")
    );
    let open_excessive = vfs
        .open_file_with_timeout(
            Path::new("buffer.py"),
            None,
            Some(MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS + 1),
        )
        .expect_err("excessive open timeout should fail");
    assert!(open_excessive.to_string().contains(&format!(
        "must not exceed {MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS}"
    )));

    let read_zero = vfs
        .read_file_with_timeout(Path::new(""), Some(0))
        .expect_err("zero read timeout should fail before path validation");
    assert!(
        read_zero
            .to_string()
            .contains("invalid virtual file read timeout_ms: value must be greater than zero")
    );

    let list_zero = vfs
        .virtual_file_statuses_with_timeout(false, Some(0))
        .expect_err("zero listing timeout should fail before enumeration");
    assert!(
        list_zero
            .to_string()
            .contains("invalid virtual file listing timeout_ms: value must be greater than zero")
    );
    let list_excessive = vfs
        .virtual_file_statuses_with_timeout(false, Some(MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS + 1))
        .expect_err("excessive listing timeout should fail");
    assert!(list_excessive.to_string().contains(&format!(
        "must not exceed {MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS}"
    )));
}

#[test]
fn open_timeout_restores_existing_entry_after_source_override() {
    let disk_source = "def value() -> int:\n    return 1\n";
    let file = temp_file(disk_source);
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();
    let deadline = FailOnPhase {
        phase: "virtual source refresh",
    };

    let error = vfs
        .open_file_with_deadline(
            &file,
            Some("def value() -> int:\n    return 9\n"),
            &deadline,
        )
        .expect_err("open should fail after preparing the source override");

    assert!(error.to_string().contains("virtual source refresh"));
    assert_eq!(vfs.read_file(&file).unwrap(), initial);
    assert_eq!(fs::read_to_string(&file).unwrap(), disk_source);
}

#[test]
fn open_timeout_removes_entry_inserted_only_for_failed_request() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();
    let deadline = FailOnPhase {
        phase: "virtual source refresh",
    };

    vfs.open_file_with_deadline(&file, None, &deadline)
        .expect_err("open should fail after inserting the loaded source");

    assert!(vfs.virtual_file_statuses(false).unwrap().is_empty());
}

#[test]
fn read_timeout_restores_entry_refreshed_from_disk() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();
    fs::write(&file, "def value() -> int:\n    return 2\n").unwrap();
    let deadline = FailOnPhase {
        phase: "virtual file result validation",
    };

    let error = vfs
        .read_file_with_deadline(&file, &deadline)
        .expect_err("read should fail after refreshing the source");

    assert!(error.to_string().contains("virtual file result validation"));
    let entry = vfs.entries.get(&initial.file).unwrap();
    assert_eq!(entry.source, initial.source);
    assert_eq!(entry.disk_source, initial.disk_source);
    assert_eq!(entry.version, initial.version);
    assert!(!entry.dirty);
    assert!(fs::read_to_string(&file).unwrap().contains("return 2"));
}

#[test]
fn timed_open_and_read_apply_source_and_disk_refreshes() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    let opened = vfs
        .open_file_with_timeout(
            &file,
            Some("def value() -> int:\n    return 9\n"),
            Some(MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS),
        )
        .expect("timed open should succeed");
    assert!(opened.dirty);
    assert!(opened.source.contains("return 9"));

    vfs.discard_file(&file).unwrap();
    fs::write(&file, "def value() -> int:\n    return 2\n").unwrap();
    let read = vfs
        .read_file_with_timeout(&file, Some(MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS))
        .expect("timed read should refresh from disk");
    assert!(!read.dirty);
    assert!(read.source.contains("return 2"));
}

#[test]
fn listing_timeout_restores_all_entries_refreshed_before_collection() {
    let first = temp_file("def first() -> int:\n    return 1\n");
    let second = temp_file("def second() -> int:\n    return 2\n");
    let mut vfs = VirtualFileSystem::new();
    let first_initial = vfs.read_file(&first).unwrap();
    let second_initial = vfs.read_file(&second).unwrap();
    fs::write(&first, "def first() -> int:\n    return 3\n").unwrap();
    fs::write(&second, "def second() -> int:\n    return 4\n").unwrap();
    let deadline = FailOnNthPhase::new("virtual status collection", 2);

    let error = vfs
        .virtual_file_statuses_with_deadline(false, &deadline)
        .expect_err("listing should fail during status collection");

    assert!(error.to_string().contains("virtual status collection"));
    let first_entry = vfs.entries.get(&first_initial.file).unwrap();
    assert_eq!(first_entry.source, first_initial.source);
    assert_eq!(first_entry.version, first_initial.version);
    let second_entry = vfs.entries.get(&second_initial.file).unwrap();
    assert_eq!(second_entry.source, second_initial.source);
    assert_eq!(second_entry.version, second_initial.version);
}

#[test]
fn timed_listing_refreshes_entries_and_returns_sorted_statuses() {
    let first = temp_file("def first() -> int:\n    return 1\n");
    let second = temp_file("def second() -> int:\n    return 2\n");
    let mut vfs = VirtualFileSystem::new();
    vfs.read_file(&second).unwrap();
    let first_snapshot = vfs.read_file(&first).unwrap();
    fs::write(&first, "def first(\n").unwrap();

    let statuses = vfs
        .virtual_file_statuses_with_timeout(false, Some(MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS))
        .expect("timed listing should succeed");

    assert_eq!(statuses.len(), 2);
    assert!(statuses[0].file < statuses[1].file);
    let first_status = statuses
        .iter()
        .find(|status| status.file == first_snapshot.file)
        .unwrap();
    assert_eq!(first_status.version, 1);
    assert!(first_status.syntax_error_count > 0);
}

#[test]
fn read_does_not_check_deadline_after_result_validation() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();
    let deadline = RejectChecksAfterPhase::new("virtual file result validation");

    let snapshot = vfs
        .read_file_with_deadline(&file, &deadline)
        .expect("read should not check the deadline after validating its result");

    assert!(deadline.seen.get());
    assert!(!snapshot.dirty);
}

#[test]
fn listing_does_not_check_deadline_after_final_status_validation() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();
    vfs.read_file(&file).unwrap();
    let deadline = RejectChecksAfterPhase::new("virtual status validation");

    let statuses = vfs
        .virtual_file_statuses_with_deadline(false, &deadline)
        .expect("listing should not check the deadline after final validation");

    assert!(deadline.seen.get());
    assert_eq!(statuses.len(), 1);
}
