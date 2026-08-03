use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::language::{
    builtin_language_registry, detect_language, normalize_path, parse_document_with_timeout,
    path_is_inside_workspace, read_source,
};
use crate::workspace_scan::{
    WorkspaceScanDeadline, WorkspaceScanLimits, collect_source_files_with_deadline,
};

pub(super) fn reverse_local_c_include_index(
    workspace_root: &Path,
    limits: WorkspaceScanLimits,
    deadline: &WorkspaceScanDeadline,
) -> Result<BTreeMap<String, BTreeSet<PathBuf>>> {
    let mut reverse_index = BTreeMap::new();

    for path in collect_source_files_with_deadline(workspace_root, limits, deadline)? {
        deadline.check("building C include reverse index")?;
        let Ok(language_id) = detect_language(&path) else {
            continue;
        };
        let adapter = builtin_language_registry()
            .adapter(language_id)
            .expect("every LanguageId must have a builtin language adapter");
        if !adapter.supports_incremental_file_dependencies() {
            continue;
        }

        let source = read_source(&path)?;
        let document = parse_document_with_timeout(
            &path,
            &source,
            deadline.remaining_timeout_micros("parsing C include files")?,
        )?;
        deadline.check("extracting C include targets")?;
        for include_path in
            adapter.collect_local_file_dependencies(&path, document.tree.root_node(), &source)?
        {
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
