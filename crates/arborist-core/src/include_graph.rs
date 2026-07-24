use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::language::{detect_language, normalize_path};
use crate::model::LanguageId;
use crate::workspace_scan::WorkspaceScanDeadline;
#[cfg(test)]
use crate::workspace_scan::WorkspaceScanLimits;

mod reverse;

pub(crate) fn expanded_refresh_file_paths(
    workspace_root: &Path,
    file_path: &Path,
    deadline: &WorkspaceScanDeadline,
) -> Result<Vec<PathBuf>> {
    let mut refresh_paths = BTreeSet::new();
    refresh_paths.insert(file_path.to_path_buf());

    if matches!(detect_language(file_path)?, LanguageId::C | LanguageId::Cpp) {
        refresh_paths.extend(transitive_c_include_dependents_with_deadline(
            workspace_root,
            file_path,
            deadline,
        )?);
    }

    Ok(refresh_paths.into_iter().collect())
}

#[cfg(test)]
pub(crate) fn transitive_c_include_dependents(
    workspace_root: &Path,
    target_path: &Path,
) -> Result<BTreeSet<PathBuf>> {
    let deadline = WorkspaceScanDeadline::new(WorkspaceScanLimits::default())?;
    transitive_c_include_dependents_with_deadline(workspace_root, target_path, &deadline)
}

fn transitive_c_include_dependents_with_deadline(
    workspace_root: &Path,
    target_path: &Path,
    deadline: &WorkspaceScanDeadline,
) -> Result<BTreeSet<PathBuf>> {
    let reverse_index = reverse::reverse_local_c_include_index(workspace_root, deadline)?;
    let normalized_target = normalize_path(target_path);
    let mut queue = vec![normalized_target.clone()];
    let mut visited = BTreeSet::from([normalized_target]);
    let mut dependents = BTreeSet::new();

    while let Some(current_path) = queue.pop() {
        deadline.check("expanding C include dependents")?;
        let Some(children) = reverse_index.get(&current_path) else {
            continue;
        };

        for dependent_path in children {
            let normalized_dependent = normalize_path(dependent_path);
            if visited.insert(normalized_dependent.clone()) {
                dependents.insert(dependent_path.clone());
                queue.push(normalized_dependent);
            }
        }
    }

    Ok(dependents)
}
