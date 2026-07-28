use std::collections::HashMap;
use std::path::PathBuf;

mod buffer;
mod indexes;
mod patch_context;
mod queries;
mod state;
mod status;

use self::state::VirtualFileEntry;

pub const MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS: u64 = 5 * 60 * 1_000;
pub const MAX_VIRTUAL_FILE_COMMIT_TIMEOUT_MS: u64 = MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS;
pub const MAX_VIRTUAL_FILE_EDIT_TIMEOUT_MS: u64 = MAX_VIRTUAL_FILE_LIFECYCLE_TIMEOUT_MS;

#[derive(Default)]
pub struct VirtualFileSystem {
    entries: HashMap<String, VirtualFileEntry>,
    symbol_indexes: HashMap<String, PathBuf>,
}

#[cfg(test)]
mod tests;
