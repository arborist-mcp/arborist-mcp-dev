pub(super) use std::fs;
pub(super) use std::path::Path;
pub(super) use std::sync::atomic::{AtomicU64, Ordering};
pub(super) use std::time::{SystemTime, UNIX_EPOCH};

pub(super) use super::VirtualFileSystem;
pub(super) use crate::language::{point_for_offset, position_from};
pub(super) use crate::{Position, PositionEdit, TraceDirection, trace_symbol_graph_from_index};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

mod cpp_trace;
mod edits;
mod lifecycle;
mod misc;
mod patch;

pub(crate) fn temp_file(contents: &str) -> std::path::PathBuf {
    let suffix = format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let dir = std::env::temp_dir().join(format!("arborist-vfs-{suffix}"));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join(Path::new("buffer.py"));
    fs::write(&file, contents).unwrap();
    file
}

pub(crate) fn generated_edit_cases() -> [(&'static str, &'static str, &'static str, &'static str); 3]
{
    [
        ("alpha", "beta", "first", "second"),
        ("é", "茅", "ß", "文"),
        ("🙂", "尾", "星", "末"),
    ]
}

pub(crate) fn position_at(source: &str, byte_offset: usize) -> Position {
    position_from(point_for_offset(source, byte_offset).unwrap())
}

pub(crate) fn temp_workspace() -> std::path::PathBuf {
    let suffix = format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let dir = std::env::temp_dir().join(format!("arborist-vfs-workspace-{suffix}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[cfg(unix)]
pub(crate) fn try_symlink_file(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
pub(crate) fn try_symlink_file(target: &Path, link: &Path) -> bool {
    std::os::windows::fs::symlink_file(target, link).is_ok()
}
