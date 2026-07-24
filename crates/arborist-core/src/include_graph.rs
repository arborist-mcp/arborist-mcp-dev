use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::language::{
    c_include_targets, c_local_include_targets, detect_language, normalize_absolute_path,
    normalize_path, parse_document, path_is_inside_workspace, read_source, resolve_local_c_include,
};
use crate::model::LanguageId;
use crate::workspace_scan::{
    WorkspaceScanDeadline, WorkspaceScanLimits, collect_source_files_with_deadline,
};

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
    let reverse_index = reverse_local_c_include_index(workspace_root, deadline)?;
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

fn reverse_local_c_include_index(
    workspace_root: &Path,
    deadline: &WorkspaceScanDeadline,
) -> Result<BTreeMap<String, BTreeSet<PathBuf>>> {
    let mut reverse_index = BTreeMap::new();

    for path in collect_source_files_with_deadline(
        workspace_root,
        WorkspaceScanLimits::default(),
        deadline,
    )? {
        deadline.check("building C include reverse index")?;
        if !matches!(detect_language(&path), Ok(LanguageId::C | LanguageId::Cpp)) {
            continue;
        }

        let source = read_source(&path)?;
        let document = parse_document(&path, &source)?;
        let local_include_targets = c_local_include_targets(document.tree.root_node(), &source)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        for include_target in c_include_targets(document.tree.root_node(), &source)? {
            let Some(include_path) =
                resolve_local_c_include(&path, &include_target).or_else(|| {
                    local_include_targets
                        .contains(&include_target)
                        .then(|| unresolved_local_c_include_path(&path, &include_target))
                        .flatten()
                })
            else {
                continue;
            };
            if !path_is_inside_workspace(workspace_root, &include_path)? {
                continue;
            }

            reverse_index
                .entry(normalize_path(&include_path))
                .or_insert_with(BTreeSet::new)
                .insert(path.clone());
        }
    }

    Ok(reverse_index)
}

fn unresolved_local_c_include_path(current_path: &Path, include_target: &str) -> Option<PathBuf> {
    let parent = current_path.parent()?;
    normalize_absolute_path(&parent.join(include_target)).ok()
}
