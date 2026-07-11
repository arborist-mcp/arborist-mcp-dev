use std::collections::HashMap;
use std::path::PathBuf;

mod buffer;
mod patch_context;
mod queries;
mod state;

use self::state::VirtualFileEntry;

#[derive(Default)]
pub struct VirtualFileSystem {
    entries: HashMap<String, VirtualFileEntry>,
    symbol_indexes: HashMap<String, PathBuf>,
}

#[cfg(test)]
mod tests;
