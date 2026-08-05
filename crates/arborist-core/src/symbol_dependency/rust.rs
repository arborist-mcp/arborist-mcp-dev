use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::language::{
    detect_language, normalize_path, parse_document, parse_document_with_timeout, read_source,
    rust_direct_module_candidate_paths,
};
use crate::model::LanguageId;
use crate::symbol_index_model::RustImportRoot;
use crate::workspace_scan::WorkspaceScanDeadline;

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct RustOutOfLineModuleContext {
    bindings_by_source_file: BTreeMap<String, BTreeMap<String, String>>,
    parents_by_child_file: BTreeMap<String, String>,
    ambiguous_parent_files: BTreeSet<String>,
    cyclic_parent_files: BTreeSet<String>,
}

pub(in crate::symbol_dependency) fn rust_out_of_line_module_context_for_files_with_overrides_and_deadline(
    source_file_paths: &[PathBuf],
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<RustOutOfLineModuleContext> {
    let known_rust_paths = source_file_paths
        .iter()
        .filter(|path| detect_language(path).ok() == Some(LanguageId::Rust))
        .map(|path| normalize_path(path))
        .collect::<BTreeSet<_>>();
    let mut bindings_by_source_file = BTreeMap::new();

    for path in source_file_paths {
        if detect_language(path).ok() != Some(LanguageId::Rust) {
            continue;
        }
        if let Some(deadline) = deadline {
            deadline.check("reading Rust module context")?;
        }
        let normalized_path = normalize_path(path);
        let source = file_overrides
            .and_then(|overrides| overrides.get(&normalized_path))
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| read_source(path))?;
        if let Some(deadline) = deadline {
            deadline.check("parsing Rust module context")?;
        }
        let document = if let Some(deadline) = deadline {
            parse_document_with_timeout(
                path,
                &source,
                deadline.remaining_timeout_micros("parsing Rust module context")?,
            )?
        } else {
            parse_document(path, &source)?
        };
        let root = document.tree.root_node();
        if root.has_error() {
            continue;
        }

        let bindings = rust_direct_module_candidate_paths(path, root, &source)?
            .into_iter()
            .filter_map(|(name, candidates)| {
                let matches = candidates
                    .into_iter()
                    .map(|candidate| normalize_path(&candidate))
                    .filter(|candidate| known_rust_paths.contains(candidate))
                    .collect::<Vec<_>>();
                (matches.len() == 1).then(|| (name, matches[0].clone()))
            })
            .collect::<BTreeMap<_, _>>();
        if !bindings.is_empty() {
            bindings_by_source_file.insert(normalized_path, bindings);
        }
    }

    let mut parent_candidates = BTreeMap::<String, BTreeSet<String>>::new();
    for (parent_path, bindings) in &bindings_by_source_file {
        for child_path in bindings.values() {
            parent_candidates
                .entry(child_path.clone())
                .or_default()
                .insert(parent_path.clone());
        }
    }
    let mut parents_by_child_file = BTreeMap::new();
    let mut ambiguous_parent_files = BTreeSet::new();
    for (child_path, parents) in parent_candidates {
        if parents.len() == 1 {
            parents_by_child_file.insert(child_path, parents.into_iter().next().unwrap());
        } else if parents.len() > 1 {
            ambiguous_parent_files.insert(child_path);
        }
    }

    let cyclic_parent_files = rust_cyclic_parent_files(&parents_by_child_file);

    Ok(RustOutOfLineModuleContext {
        bindings_by_source_file,
        parents_by_child_file,
        ambiguous_parent_files,
        cyclic_parent_files,
    })
}

pub(in crate::symbol_dependency) fn resolve_rust_out_of_line_module_reference(
    context: &RustOutOfLineModuleContext,
    source_file_path: &str,
    reference_name: &str,
    import_root: Option<&RustImportRoot>,
) -> Option<(String, String)> {
    let components = reference_name.split("::").collect::<Vec<_>>();
    let (target_name, module_components) = components.split_last()?;
    if target_name.is_empty()
        || module_components
            .iter()
            .any(|component| component.is_empty())
    {
        return None;
    }

    let mut target_file_path = normalize_path(Path::new(source_file_path));
    match import_root {
        Some(RustImportRoot::Crate) => {
            target_file_path = rust_crate_root(context, &target_file_path)?;
        }
        Some(RustImportRoot::SelfModule) | None => {}
        Some(RustImportRoot::Super { levels }) => {
            for _ in 0..*levels {
                target_file_path = rust_parent_file(context, &target_file_path)?;
            }
        }
    }
    for module_name in module_components {
        target_file_path = context
            .bindings_by_source_file
            .get(&target_file_path)?
            .get(*module_name)?
            .clone();
    }
    Some((target_file_path, (*target_name).to_string()))
}

fn rust_cyclic_parent_files(parents_by_child_file: &BTreeMap<String, String>) -> BTreeSet<String> {
    let mut cyclic_files = BTreeSet::new();
    for start in parents_by_child_file.keys() {
        let mut path = start.clone();
        let mut seen = BTreeSet::new();
        loop {
            if !seen.insert(path.clone()) {
                cyclic_files.extend(seen);
                break;
            }
            let Some(parent) = parents_by_child_file.get(&path) else {
                break;
            };
            path = parent.clone();
        }
    }
    cyclic_files
}

fn rust_crate_root(context: &RustOutOfLineModuleContext, source_file_path: &str) -> Option<String> {
    let mut current = source_file_path.to_string();
    let mut visited = BTreeSet::new();
    while visited.insert(current.clone()) {
        if context.ambiguous_parent_files.contains(&current)
            || context.cyclic_parent_files.contains(&current)
        {
            return None;
        }
        let Some(parent) = rust_parent_file(context, &current) else {
            return Some(current);
        };
        current = parent;
    }
    None
}

fn rust_parent_file(
    context: &RustOutOfLineModuleContext,
    source_file_path: &str,
) -> Option<String> {
    if context.ambiguous_parent_files.contains(source_file_path)
        || context.cyclic_parent_files.contains(source_file_path)
    {
        return None;
    }
    context.parents_by_child_file.get(source_file_path).cloned()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        resolve_rust_out_of_line_module_reference,
        rust_out_of_line_module_context_for_files_with_overrides_and_deadline,
    };
    use crate::language::normalize_path;
    use crate::symbol_index_model::RustImportRoot;

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temporary_dir() -> std::path::PathBuf {
        let suffix = format!(
            "{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(format!("arborist-rust-module-context-{suffix}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn resolves_unambiguous_direct_out_of_line_module_references() {
        let dir = temporary_dir();
        let root = dir.join("lib.rs");
        let api = dir.join("api.rs");
        fs::write(&root, "mod api;\nfn caller() { api::helper(); }\n").unwrap();
        fs::write(&api, "pub fn helper() {}\n").unwrap();

        let context = rust_out_of_line_module_context_for_files_with_overrides_and_deadline(
            &[root.clone(), api.clone()],
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            resolve_rust_out_of_line_module_reference(
                &context,
                &normalize_path(&root),
                "api::helper",
                None,
            ),
            Some((normalize_path(&api), "helper".to_string()))
        );
    }

    #[test]
    fn resolves_unambiguous_nested_out_of_line_module_references() {
        let dir = temporary_dir();
        let root = dir.join("lib.rs");
        let api_directory = dir.join("api");
        let api = api_directory.join("mod.rs");
        let helper = api_directory.join("helper.rs");
        fs::create_dir_all(&api_directory).unwrap();
        fs::write(&root, "mod api;\n").unwrap();
        fs::write(&api, "mod helper;\n").unwrap();
        fs::write(&helper, "pub fn value() {}\n").unwrap();

        let context = rust_out_of_line_module_context_for_files_with_overrides_and_deadline(
            &[root.clone(), api, helper.clone()],
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            resolve_rust_out_of_line_module_reference(
                &context,
                &normalize_path(&root),
                "api::helper::value",
                None,
            ),
            Some((normalize_path(&helper), "value".to_string()))
        );
    }

    #[test]
    fn ignores_path_semantic_and_ambiguous_module_layouts() {
        let dir = temporary_dir();
        let root = dir.join("lib.rs");
        let api_file = dir.join("api.rs");
        let api_dir = dir.join("api");
        fs::create_dir_all(&api_dir).unwrap();
        fs::write(&root, "#[path = \"custom.rs\"]\nmod custom;\nmod api;\n").unwrap();
        fs::write(&api_file, "pub fn helper() {}\n").unwrap();
        fs::write(api_dir.join("mod.rs"), "pub fn helper() {}\n").unwrap();

        let context = rust_out_of_line_module_context_for_files_with_overrides_and_deadline(
            &[root.clone(), api_file, api_dir.join("mod.rs")],
            None,
            None,
        )
        .unwrap();
        assert!(
            resolve_rust_out_of_line_module_reference(
                &context,
                &normalize_path(&root),
                "custom::helper",
                None,
            )
            .is_none()
        );
        assert!(
            resolve_rust_out_of_line_module_reference(
                &context,
                &normalize_path(&root),
                "api::helper",
                None,
            )
            .is_none()
        );
    }

    #[test]
    fn resolves_crate_and_super_imports_from_out_of_line_children() {
        let dir = temporary_dir();
        let root = dir.join("lib.rs");
        let api = dir.join("api.rs");
        let sibling = dir.join("sibling.rs");
        fs::write(&root, "mod api;\nmod sibling;\nfn root_helper() {}\n").unwrap();
        fs::write(
            &api,
            "use crate::sibling::helper;\nuse super::root_helper;\n",
        )
        .unwrap();
        fs::write(&sibling, "pub fn helper() {}\n").unwrap();

        let context = rust_out_of_line_module_context_for_files_with_overrides_and_deadline(
            &[root.clone(), api.clone(), sibling.clone()],
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            resolve_rust_out_of_line_module_reference(
                &context,
                &normalize_path(&api),
                "sibling::helper",
                Some(&RustImportRoot::Crate),
            ),
            Some((normalize_path(&sibling), "helper".to_string()))
        );
        assert_eq!(
            resolve_rust_out_of_line_module_reference(
                &context,
                &normalize_path(&api),
                "root_helper",
                Some(&RustImportRoot::Super { levels: 1 }),
            ),
            Some((normalize_path(&root), "root_helper".to_string()))
        );
    }

    #[test]
    fn rejects_ancestor_imports_when_a_child_has_ambiguous_parents() {
        let dir = temporary_dir();
        let lib = dir.join("lib.rs");
        let main = dir.join("main.rs");
        let api = dir.join("api.rs");
        let sibling = dir.join("sibling.rs");
        fs::write(&lib, "mod api;\nmod sibling;\nfn root_helper() {}\n").unwrap();
        fs::write(&main, "mod api;\nmod sibling;\nfn root_helper() {}\n").unwrap();
        fs::write(&api, "use crate::root_helper;\n").unwrap();
        fs::write(&sibling, "pub fn helper() {}\n").unwrap();

        let context = rust_out_of_line_module_context_for_files_with_overrides_and_deadline(
            &[lib, main, api.clone(), sibling],
            None,
            None,
        )
        .unwrap();
        assert!(
            resolve_rust_out_of_line_module_reference(
                &context,
                &normalize_path(&api),
                "root_helper",
                Some(&RustImportRoot::Crate),
            )
            .is_none()
        );
    }

    #[test]
    fn reads_direct_module_declarations_from_vfs_overrides() {
        let dir = temporary_dir();
        let root = dir.join("lib.rs");
        let api = dir.join("api.rs");
        fs::write(&root, "mod stale;\n").unwrap();
        fs::write(&api, "pub fn helper() {}\n").unwrap();
        let overrides = BTreeMap::from([(normalize_path(&root), "mod api;\n".to_string())]);

        let context = rust_out_of_line_module_context_for_files_with_overrides_and_deadline(
            &[root.clone(), api.clone()],
            Some(&overrides),
            None,
        )
        .unwrap();
        assert_eq!(
            resolve_rust_out_of_line_module_reference(
                &context,
                &normalize_path(&root),
                "api::helper",
                None,
            ),
            Some((normalize_path(&api), "helper".to_string()))
        );
    }
}
