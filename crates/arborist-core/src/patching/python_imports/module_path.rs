use std::path::{Path, PathBuf};

pub(crate) fn resolve_local_python_module_path(
    current_path: &Path,
    module_name: &str,
) -> Option<PathBuf> {
    crate::language::resolve_local_python_module_path(current_path, module_name)
}
