mod limits;
mod walker;

pub use limits::{
    DEFAULT_WORKSPACE_MAX_FILES, MAX_WORKSPACE_SCAN_FILE_BYTES, MAX_WORKSPACE_SCAN_FILES,
    MAX_WORKSPACE_SCAN_TIMEOUT_MS, WorkspaceScanLimits,
};
pub(crate) use limits::{
    WorkspaceScanDeadline, validate_source_file_size, validate_source_text_size,
    validate_workspace_scan_limits,
};
#[cfg(test)]
pub(crate) use walker::{SKIPPED_WORKSPACE_DIR_NAMES, should_skip_dir_name};
pub(crate) use walker::{
    collect_source_files, collect_source_files_with_deadline, collect_source_files_with_limits,
    should_skip_index_path,
};

#[cfg(test)]
mod tests;
