use anyhow::Result;

use super::VirtualFileSystem;
use crate::deadline::DeadlineCheck;

mod edits;
mod lifecycle;
mod loading;

pub(super) fn check_optional_deadline(
    deadline: Option<&dyn DeadlineCheck>,
    phase: &str,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check(phase)?;
    }
    Ok(())
}

impl VirtualFileSystem {
    pub fn new() -> Self {
        Self::default()
    }
}
