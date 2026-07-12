use std::path::{Path, PathBuf};

pub(crate) fn resolve_local_python_module_path(
    current_path: &Path,
    module_name: &str,
) -> Option<PathBuf> {
    let parent = current_path.parent()?;
    let (relative_levels, module_parts) = split_python_module_reference(module_name);
    if relative_levels > 0 {
        let mut candidate = parent.to_path_buf();
        for _ in 0..relative_levels.saturating_sub(1) {
            candidate = candidate.parent()?.to_path_buf();
        }
        return resolve_python_module_candidate(candidate, &module_parts);
    }

    let mut search_root = Some(parent);
    while let Some(root) = search_root {
        if let Some(candidate) = resolve_python_module_candidate(root.to_path_buf(), &module_parts)
        {
            return Some(candidate);
        }
        search_root = root.parent();
    }

    None
}

fn split_python_module_reference(module_name: &str) -> (usize, Vec<&str>) {
    let relative_levels = module_name.chars().take_while(|ch| *ch == '.').count();
    let trimmed = module_name.trim_start_matches('.');
    let parts = if trimmed.is_empty() {
        Vec::new()
    } else {
        trimmed
            .split('.')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
    };
    (relative_levels, parts)
}

fn resolve_python_module_candidate(
    mut base_dir: PathBuf,
    module_parts: &[&str],
) -> Option<PathBuf> {
    for part in module_parts {
        base_dir.push(part);
    }

    let file_candidate = base_dir.with_extension("py");
    if file_candidate.exists() {
        return Some(file_candidate);
    }

    let package_candidate = base_dir.join("__init__.py");
    package_candidate.exists().then_some(package_candidate)
}
