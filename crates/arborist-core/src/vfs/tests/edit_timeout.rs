use std::cell::Cell;

use super::*;
use crate::MAX_VIRTUAL_FILE_EDIT_TIMEOUT_MS;
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

struct RejectChecksAfterCommit {
    committed: Cell<bool>,
}

impl RejectChecksAfterCommit {
    fn new() -> Self {
        Self {
            committed: Cell::new(false),
        }
    }
}

impl DeadlineCheck for RejectChecksAfterCommit {
    fn check(&self, phase: &str) -> anyhow::Result<()> {
        if self.committed.get() {
            anyhow::bail!("unexpected deadline check after virtual edit commit: {phase}");
        }
        if phase == "virtual edit commit" {
            self.committed.set(true);
        }
        Ok(())
    }
}

#[test]
fn virtual_edit_operations_reject_invalid_timeouts_before_file_work() {
    let mut vfs = VirtualFileSystem::new();

    let byte_zero = vfs
        .apply_edit_with_timeout(Path::new(""), 0, 0, "x", Some(0))
        .expect_err("zero byte-edit timeout should fail before path validation");
    assert!(
        byte_zero
            .to_string()
            .contains("invalid virtual buffer edit timeout_ms: value must be greater than zero")
    );
    let byte_excessive = vfs
        .apply_edit_with_timeout(
            Path::new("buffer.py"),
            0,
            0,
            "x",
            Some(MAX_VIRTUAL_FILE_EDIT_TIMEOUT_MS + 1),
        )
        .expect_err("excessive byte-edit timeout should fail");
    assert!(byte_excessive.to_string().contains(&format!(
        "must not exceed {MAX_VIRTUAL_FILE_EDIT_TIMEOUT_MS}"
    )));

    let position_zero = vfs
        .apply_position_edits_with_timeout(Path::new(""), &[], Some(0))
        .expect_err("zero position-edit timeout should fail before validation");
    assert!(
        position_zero
            .to_string()
            .contains("invalid virtual position edits timeout_ms: value must be greater than zero")
    );
    let position_excessive = vfs
        .apply_position_edits_with_timeout(
            Path::new("buffer.py"),
            &[],
            Some(MAX_VIRTUAL_FILE_EDIT_TIMEOUT_MS + 1),
        )
        .expect_err("excessive position-edit timeout should fail");
    assert!(position_excessive.to_string().contains(&format!(
        "must not exceed {MAX_VIRTUAL_FILE_EDIT_TIMEOUT_MS}"
    )));
}

#[test]
fn byte_edit_timeout_restores_existing_dirty_entry() {
    let disk_source = "value = 1\n";
    let file = temp_file(disk_source);
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs
        .open_file(&file, Some("value = 2\n"))
        .expect("dirty virtual file should open");
    let digit = initial.source.find('2').unwrap();
    let deadline = FailOnPhase {
        phase: "virtual edit commit",
    };

    let error = vfs
        .apply_edit_with_deadline(&file, digit, digit + 1, "3", &deadline)
        .expect_err("edit should time out at the final mutation gate");

    assert!(error.to_string().contains("virtual edit commit"));
    assert_eq!(vfs.read_file(&file).unwrap(), initial);
    assert_eq!(fs::read_to_string(&file).unwrap(), disk_source);
}

#[test]
fn byte_edit_timeout_removes_entry_loaded_only_for_failed_request() {
    let file = temp_file("value = 1\n");
    let mut vfs = VirtualFileSystem::new();
    let deadline = FailOnPhase {
        phase: "virtual edit commit",
    };

    vfs.apply_edit_with_deadline(&file, 8, 9, "2", &deadline)
        .expect_err("failed edit should not retain a request-only entry");

    assert!(vfs.virtual_file_statuses(false).unwrap().is_empty());
    assert_eq!(fs::read_to_string(&file).unwrap(), "value = 1\n");
}

#[test]
fn byte_edit_timeout_restores_clean_entry_refreshed_from_disk() {
    let file = temp_file("value = 1\n");
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();
    fs::write(&file, "value = 2\n").unwrap();
    let deadline = FailOnPhase {
        phase: "virtual edit commit",
    };

    vfs.apply_edit_with_deadline(&file, 8, 9, "3", &deadline)
        .expect_err("failed edit should roll back the clean-buffer refresh");

    let entry = vfs.entries.get(&initial.file).unwrap();
    assert_eq!(entry.source, initial.source);
    assert_eq!(entry.disk_source, initial.disk_source);
    assert_eq!(entry.version, initial.version);
    assert!(!entry.dirty);
    assert_eq!(fs::read_to_string(&file).unwrap(), "value = 2\n");
}

#[test]
fn position_edit_timeout_rolls_back_edits_applied_earlier_in_batch() {
    let file = temp_file("value = 12\n");
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();
    let deadline = FailOnNthPhase::new("virtual edit commit", 2);
    let edits = [
        PositionEdit {
            start: Position { row: 0, column: 8 },
            end: Position { row: 0, column: 9 },
            new_text: "3".to_string(),
        },
        PositionEdit {
            start: Position { row: 0, column: 9 },
            end: Position { row: 0, column: 10 },
            new_text: "4".to_string(),
        },
    ];

    let error = vfs
        .apply_position_edits_with_deadline(&file, &edits, &deadline)
        .expect_err("second edit should time out after the first edit commits");

    assert!(
        error
            .to_string()
            .contains("failed to apply position edit at index 1")
    );
    assert!(format!("{error:#}").contains("virtual edit commit"));
    assert_eq!(vfs.read_file(&file).unwrap(), initial);
}

#[test]
fn empty_position_edit_timeout_rolls_back_loaded_entry() {
    let file = temp_file("value = 1\n");
    let mut vfs = VirtualFileSystem::new();
    let deadline = FailOnPhase {
        phase: "virtual edit result validation",
    };

    vfs.apply_position_edits_with_deadline(&file, &[], &deadline)
        .expect_err("empty edit result should still honor the shared deadline");

    assert!(vfs.virtual_file_statuses(false).unwrap().is_empty());
}

#[test]
fn timed_byte_and_position_edits_succeed_and_legacy_defaults_remain_supported() {
    let file = temp_file("value = 12\n");
    let mut vfs = VirtualFileSystem::new();

    let byte = vfs
        .apply_edit_with_timeout(&file, 8, 9, "3", Some(MAX_VIRTUAL_FILE_EDIT_TIMEOUT_MS))
        .expect("timed byte edit should succeed");
    assert_eq!(byte.source, "value = 32\n");
    assert_eq!(byte.version, 1);

    let position = vfs
        .apply_position_edits_with_timeout(
            &file,
            &[PositionEdit {
                start: Position { row: 0, column: 9 },
                end: Position { row: 0, column: 10 },
                new_text: "4".to_string(),
            }],
            Some(MAX_VIRTUAL_FILE_EDIT_TIMEOUT_MS),
        )
        .expect("timed position edit should succeed");
    assert_eq!(position.source, "value = 34\n");
    assert_eq!(position.version, 2);

    let legacy = vfs
        .apply_edit_with_timeout(&file, 8, 10, "56", None)
        .expect("missing timeout should preserve legacy byte edits");
    assert_eq!(legacy.source, "value = 56\n");
    assert_eq!(legacy.version, 3);
}

#[test]
fn byte_edit_does_not_check_deadline_after_final_commit_gate() {
    let file = temp_file("value = 1\n");
    let mut vfs = VirtualFileSystem::new();
    let deadline = RejectChecksAfterCommit::new();

    let result = vfs
        .apply_edit_with_deadline(&file, 8, 9, "2", &deadline)
        .expect("byte edit should not check the deadline after mutating state");

    assert!(deadline.committed.get());
    assert_eq!(result.source, "value = 2\n");
    assert_eq!(vfs.read_file(&file).unwrap().source, result.source);
}
