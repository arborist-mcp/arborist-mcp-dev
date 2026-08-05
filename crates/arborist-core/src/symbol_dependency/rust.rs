use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::language::{
    detect_language, normalize_path, parse_document, parse_document_with_timeout, read_source,
    rust_direct_module_candidate_paths,
};
use crate::model::LanguageId;
use crate::workspace_scan::WorkspaceScanDeadline;

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct RustOutOfLineModuleContext {
    bindings_by_source_file: BTreeMap<String, BTreeMap<String, String>>,
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

    Ok(RustOutOfLineModuleContext {
        bindings_by_source_file,
    })
}

pub(in crate::symbol_dependency) fn resolve_rust_out_of_line_module_reference(
    context: &RustOutOfLineModuleContext,
    source_file_path: &str,
    reference_name: &str,
) -> Option<(String, String)> {
    let components = reference_name.split("::").collect::<Vec<_>>();
    let (target_name, module_components) = components.split_last()?;
    if target_name.is_empty()
        || module_components.is_empty()
        || module_components
            .iter()
            .any(|component| component.is_empty())
    {
        return None;
    }

    let mut target_file_path = normalize_path(Path::new(source_file_path));
    for module_name in module_components {
        target_file_path = context
            .bindings_by_source_file
            .get(&target_file_path)?
            .get(*module_name)?
            .clone();
    }
    Some((target_file_path, (*target_name).to_string()))
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
            )
            .is_none()
        );
        assert!(
            resolve_rust_out_of_line_module_reference(
                &context,
                &normalize_path(&root),
                "api::helper",
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
            ),
            Some((normalize_path(&api), "helper".to_string()))
        );
    }
}
