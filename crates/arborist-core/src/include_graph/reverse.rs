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

pub(super) fn reverse_local_c_include_index(
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
