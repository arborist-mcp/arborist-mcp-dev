use std::cell::Cell;

use super::*;
use crate::MAX_VIRTUAL_FILE_COMMIT_TIMEOUT_MS;
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

#[derive(Default)]
struct RejectChecksAfterPersistence {
    persistence_seen: Cell<bool>,
}

impl DeadlineCheck for RejectChecksAfterPersistence {
    fn check(&self, phase: &str) -> anyhow::Result<()> {
        if self.persistence_seen.get() {
            anyhow::bail!("unexpected deadline check after commit persistence: {phase}");
        }
        if phase == "commit persistence" {
            self.persistence_seen.set(true);
        }
        Ok(())
    }
}

#[test]
fn virtual_commit_rejects_invalid_timeout_before_file_work() {
    let mut vfs = VirtualFileSystem::new();
    let zero = vfs
        .commit_file_with_timeout(Path::new(""), Some(0))
        .expect_err("zero timeout should fail before path validation");
    assert!(
        zero.to_string()
            .contains("invalid virtual file commit timeout_ms: value must be greater than zero")
    );

    let excessive = vfs
        .commit_file_with_timeout(
            Path::new("buffer.py"),
            Some(MAX_VIRTUAL_FILE_COMMIT_TIMEOUT_MS + 1),
        )
        .expect_err("excessive timeout should fail");
    assert!(excessive.to_string().contains(&format!(
        "must not exceed {MAX_VIRTUAL_FILE_COMMIT_TIMEOUT_MS}"
    )));
}

#[test]
fn timeout_before_commit_persistence_preserves_dirty_buffer_and_disk() {
    let disk_source = "def value() -> int:\n    return 1\n";
    let file = temp_file(disk_source);
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();
    let digit = initial.source.rfind('1').unwrap();
    vfs.apply_edit(&file, digit, digit + 1, "2").unwrap();
    let dirty = vfs.read_file(&file).unwrap();
    let deadline = FailOnPhase {
        phase: "commit persistence",
    };

    let error = vfs
        .commit_file_with_deadline(&file, &deadline)
        .expect_err("pre-persistence timeout should fail");

    assert!(error.to_string().contains("commit persistence"));
    let snapshot = vfs.read_file(&file).unwrap();
    assert_eq!(snapshot.source, dirty.source);
    assert_eq!(snapshot.disk_source, dirty.disk_source);
    assert_eq!(snapshot.version, dirty.version);
    assert!(snapshot.dirty);
    assert_eq!(fs::read_to_string(&file).unwrap(), disk_source);
}

#[test]
fn timed_virtual_commit_writes_and_clears_dirty_state() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();
    let digit = initial.source.rfind('1').unwrap();
    vfs.apply_edit(&file, digit, digit + 1, "2").unwrap();

    let committed = vfs
        .commit_file_with_timeout(&file, Some(MAX_VIRTUAL_FILE_COMMIT_TIMEOUT_MS))
        .expect("timed commit should succeed");

    assert!(!committed.dirty);
    assert_eq!(committed.source, committed.disk_source);
    assert_eq!(fs::read_to_string(&file).unwrap(), committed.source);
}

#[test]
fn virtual_commit_does_not_check_deadline_after_persistence_starts() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();
    let digit = initial.source.rfind('1').unwrap();
    vfs.apply_edit(&file, digit, digit + 1, "2").unwrap();
    let deadline = RejectChecksAfterPersistence::default();

    let committed = vfs
        .commit_file_with_deadline(&file, &deadline)
        .expect("commit should not perform post-persistence deadline checks");

    assert!(deadline.persistence_seen.get());
    assert!(!committed.dirty);
    assert_eq!(fs::read_to_string(&file).unwrap(), committed.source);
}
