use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{
    detect_language, node_text, normalize_path, parse_document, parse_document_with_timeout,
    read_source, rust_direct_module_candidate_paths,
};
use crate::model::LanguageId;
use crate::symbol_index_model::RustImportRoot;
use crate::workspace_scan::WorkspaceScanDeadline;

#[derive(Debug, Clone)]
pub(in crate::symbol_dependency) struct RustReexportBinding {
    target_path: String,
    import_root: RustImportRoot,
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct RustOutOfLineModuleContext {
    bindings_by_source_file: BTreeMap<String, BTreeMap<String, String>>,
    parents_by_child_file: BTreeMap<String, String>,
    ambiguous_parent_files: BTreeSet<String>,
    cyclic_parent_files: BTreeSet<String>,
    reexport_bindings_by_file: BTreeMap<String, BTreeMap<String, RustReexportBinding>>,
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
    let mut reexport_bindings_by_file = BTreeMap::new();

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
            bindings_by_source_file.insert(normalized_path.clone(), bindings);
        }
        let reexport_bindings = rust_reexport_bindings(root, &source)?;
        if !reexport_bindings.is_empty() {
            reexport_bindings_by_file.insert(normalized_path.clone(), reexport_bindings);
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
        reexport_bindings_by_file,
    })
}

pub(in crate::symbol_dependency) fn resolve_rust_out_of_line_module_reference(
    context: &RustOutOfLineModuleContext,
    source_file_path: &str,
    reference_name: &str,
    import_root: Option<&RustImportRoot>,
) -> Option<(String, String)> {
    let mut visited = BTreeSet::new();
    resolve_rust_out_of_line_module_reference_following_reexports(
        context,
        source_file_path,
        reference_name,
        import_root,
        &mut visited,
    )
}

fn resolve_rust_out_of_line_module_reference_following_reexports(
    context: &RustOutOfLineModuleContext,
    source_file_path: &str,
    reference_name: &str,
    import_root: Option<&RustImportRoot>,
    visited: &mut BTreeSet<(String, String)>,
) -> Option<(String, String)> {
    let mut current_source_file = source_file_path.to_string();
    let mut current_reference_name = reference_name.to_string();
    let mut current_import_root = import_root.cloned();
    loop {
        if !visited.insert((current_source_file.clone(), current_reference_name.clone())) {
            return None;
        }
        let (target_file_path, target_name) = resolve_rust_out_of_line_module_reference_once(
            context,
            &current_source_file,
            &current_reference_name,
            current_import_root.as_ref(),
        )?;
        let Some(reexport) = context
            .reexport_bindings_by_file
            .get(&target_file_path)
            .and_then(|bindings| bindings.get(&target_name))
        else {
            return Some((target_file_path, target_name));
        };
        current_source_file = target_file_path;
        current_reference_name = reexport.target_path.clone();
        current_import_root = Some(reexport.import_root.clone());
    }
}

fn resolve_rust_out_of_line_module_reference_once(
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

fn rust_reexport_bindings(
    root: Node<'_>,
    source: &str,
) -> Result<BTreeMap<String, RustReexportBinding>> {
    let mut bindings_by_local_name = BTreeMap::<String, Vec<RustReexportBinding>>::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() != "use_declaration" || !is_rust_pub_use_declaration(child, source)? {
            continue;
        }
        let Some(argument) = child.child_by_field_name("argument") else {
            continue;
        };
        let mut bindings = Vec::new();
        collect_rust_reexport_bindings(argument, &[], source, &mut bindings)?;
        for (local_name, binding) in bindings {
            bindings_by_local_name
                .entry(local_name)
                .or_default()
                .push(binding);
        }
    }
    Ok(bindings_by_local_name
        .into_iter()
        .filter_map(|(local_name, bindings)| {
            (bindings.len() == 1).then(|| (local_name, bindings[0].clone()))
        })
        .collect())
}

fn is_rust_pub_use_declaration(node: Node<'_>, source: &str) -> Result<bool> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            return Ok(node_text(child, source)?.trim().starts_with("pub"));
        }
    }
    Ok(false)
}

fn collect_rust_reexport_bindings(
    node: Node<'_>,
    prefix: &[String],
    source: &str,
    bindings: &mut Vec<(String, RustReexportBinding)>,
) -> Result<()> {
    match node.kind() {
        "scoped_use_list" => {
            let Some(path) = node.child_by_field_name("path") else {
                return Ok(());
            };
            let Some(path_components) = rust_import_path_components(path, source)? else {
                return Ok(());
            };
            let Some(prefix) = rust_join_import_path_components(prefix, &path_components) else {
                return Ok(());
            };
            let Some(list) = node.child_by_field_name("list") else {
                return Ok(());
            };
            collect_rust_reexport_bindings(list, &prefix, source, bindings)?;
        }
        "use_list" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                collect_rust_reexport_bindings(child, prefix, source, bindings)?;
            }
        }
        "use_as_clause" => {
            let Some(path) = node.child_by_field_name("path") else {
                return Ok(());
            };
            let Some(alias) = node.child_by_field_name("alias") else {
                return Ok(());
            };
            let Some(path_components) = rust_import_path_components(path, source)? else {
                return Ok(());
            };
            let Some(target_components) =
                rust_join_import_path_components(prefix, &path_components)
            else {
                return Ok(());
            };
            let alias = node_text(alias, source)?.trim();
            if let Some(binding) = rust_reexport_binding(&target_components, alias) {
                bindings.push(binding);
            }
        }
        "scoped_identifier" | "identifier" => {
            let Some(path_components) = rust_import_path_components(node, source)? else {
                return Ok(());
            };
            let Some(target_components) =
                rust_join_import_path_components(prefix, &path_components)
            else {
                return Ok(());
            };
            let Some(local_name) = target_components.last() else {
                return Ok(());
            };
            if let Some(binding) = rust_reexport_binding(&target_components, local_name) {
                bindings.push(binding);
            }
        }
        _ => {}
    }
    Ok(())
}

fn rust_import_path_components(node: Node<'_>, source: &str) -> Result<Option<Vec<String>>> {
    if !matches!(
        node.kind(),
        "crate" | "self" | "super" | "identifier" | "scoped_identifier"
    ) {
        return Ok(None);
    }
    let spelling = node_text(node, source)?.trim();
    let components = spelling
        .split("::")
        .filter(|component| !component.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if components.is_empty() || spelling.split("::").any(str::is_empty) {
        return Ok(None);
    }
    Ok(Some(components))
}

fn rust_join_import_path_components(prefix: &[String], path: &[String]) -> Option<Vec<String>> {
    if prefix.is_empty() {
        return Some(path.to_vec());
    }
    (!matches!(
        path.first().map(String::as_str),
        Some("crate" | "self" | "super")
    ))
    .then(|| prefix.iter().chain(path).cloned().collect::<Vec<String>>())
}

fn rust_reexport_binding(
    target_components: &[String],
    local_name: &str,
) -> Option<(String, RustReexportBinding)> {
    if local_name.is_empty()
        || target_components.len() < 2
        || target_components
            .iter()
            .any(|component| component.is_empty())
    {
        return None;
    }
    let (import_root, root_len) = match target_components.first()?.as_str() {
        "crate" => (RustImportRoot::Crate, 1),
        "self" => (RustImportRoot::SelfModule, 1),
        "super" => {
            let levels = target_components
                .iter()
                .take_while(|component| component.as_str() == "super")
                .count();
            (RustImportRoot::Super { levels }, levels)
        }
        _ => (RustImportRoot::SelfModule, 0),
    };
    let target_components = target_components.get(root_len..)?;
    (!target_components.is_empty()).then(|| {
        (
            local_name.to_string(),
            RustReexportBinding {
                target_path: target_components.join("::"),
                import_root,
            },
        )
    })
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

    #[test]
    fn follows_pub_use_reexports_to_the_defining_out_of_line_module() {
        let dir = temporary_dir();
        let root = dir.join("lib.rs");
        let bridge = dir.join("bridge.rs");
        let impl_mod = dir.join("impl_mod.rs");
        fs::write(&root, "mod bridge;\nmod impl_mod;\n").unwrap();
        fs::write(&bridge, "pub use crate::impl_mod::function;\n").unwrap();
        fs::write(&impl_mod, "pub fn function() {}\n").unwrap();

        let context = rust_out_of_line_module_context_for_files_with_overrides_and_deadline(
            &[root.clone(), bridge.clone(), impl_mod.clone()],
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            resolve_rust_out_of_line_module_reference(
                &context,
                &normalize_path(&root),
                "bridge::function",
                None,
            ),
            Some((normalize_path(&impl_mod), "function".to_string()))
        );
        assert_eq!(
            resolve_rust_out_of_line_module_reference(
                &context,
                &normalize_path(&root),
                "bridge::function",
                Some(&RustImportRoot::Crate),
            ),
            Some((normalize_path(&impl_mod), "function".to_string()))
        );
    }

    #[test]
    fn follows_relative_pub_use_reexports_at_the_crate_root() {
        let dir = temporary_dir();
        let root = dir.join("lib.rs");
        let api = dir.join("api.rs");
        fs::write(&root, "mod api;\npub use api::helper;\n").unwrap();
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
                "helper",
                Some(&RustImportRoot::Crate),
            ),
            Some((normalize_path(&api), "helper".to_string()))
        );
    }

    #[test]
    fn follows_nested_pub_use_reexport_chains() {
        let dir = temporary_dir();
        let root = dir.join("lib.rs");
        let bridge = dir.join("bridge.rs");
        let impl_mod = dir.join("impl_mod.rs");
        let deeper = dir.join("deeper.rs");
        fs::write(&root, "mod bridge;\nmod impl_mod;\nmod deeper;\n").unwrap();
        fs::write(&bridge, "pub use crate::impl_mod::function;\n").unwrap();
        fs::write(&impl_mod, "pub use crate::deeper::function;\n").unwrap();
        fs::write(&deeper, "pub fn function() {}\n").unwrap();

        let context = rust_out_of_line_module_context_for_files_with_overrides_and_deadline(
            &[
                root.clone(),
                bridge.clone(),
                impl_mod.clone(),
                deeper.clone(),
            ],
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            resolve_rust_out_of_line_module_reference(
                &context,
                &normalize_path(&root),
                "bridge::function",
                None,
            ),
            Some((normalize_path(&deeper), "function".to_string()))
        );
    }

    #[test]
    fn resolves_aliased_and_grouped_pub_use_reexports() {
        let dir = temporary_dir();
        let root = dir.join("lib.rs");
        let bridge = dir.join("bridge.rs");
        let impl_mod = dir.join("impl_mod.rs");
        fs::write(&root, "mod bridge;\nmod impl_mod;\n").unwrap();
        fs::write(
            &bridge,
            "pub use crate::impl_mod::function as renamed;\npub use crate::impl_mod::{other};\n",
        )
        .unwrap();
        fs::write(&impl_mod, "pub fn function() {}\npub fn other() {}\n").unwrap();

        let context = rust_out_of_line_module_context_for_files_with_overrides_and_deadline(
            &[root.clone(), bridge.clone(), impl_mod.clone()],
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            resolve_rust_out_of_line_module_reference(
                &context,
                &normalize_path(&root),
                "bridge::renamed",
                None,
            ),
            Some((normalize_path(&impl_mod), "function".to_string()))
        );
        assert_eq!(
            resolve_rust_out_of_line_module_reference(
                &context,
                &normalize_path(&root),
                "bridge::other",
                None,
            ),
            Some((normalize_path(&impl_mod), "other".to_string()))
        );
    }

    #[test]
    fn ignores_private_use_and_ambiguous_or_cyclic_pub_use_reexports() {
        let dir = temporary_dir();
        let root = dir.join("lib.rs");
        let bridge = dir.join("bridge.rs");
        let impl_mod = dir.join("impl_mod.rs");
        let other_mod = dir.join("other_mod.rs");
        let api = dir.join("api.rs");
        fs::write(
            &root,
            "mod bridge;\nmod impl_mod;\nmod other_mod;\nmod api;\n",
        )
        .unwrap();
        fs::write(&bridge, "use crate::impl_mod::function;\n").unwrap();
        fs::write(
            &impl_mod,
            "pub use crate::other_mod::ambiguous;\npub use crate::api::ambiguous;\npub use crate::other_mod::function;\n",
        )
        .unwrap();
        fs::write(&other_mod, "pub fn ambiguous() {}\npub fn function() {}\n").unwrap();
        fs::write(&api, "pub fn ambiguous() {}\npub fn helper() {}\n").unwrap();

        let context = rust_out_of_line_module_context_for_files_with_overrides_and_deadline(
            &[
                root.clone(),
                bridge.clone(),
                impl_mod.clone(),
                other_mod.clone(),
                api.clone(),
            ],
            None,
            None,
        )
        .unwrap();
        // Private use does not re-export a name for sibling modules: the
        // reference stays in the re-exporting file instead of being followed.
        assert_eq!(
            resolve_rust_out_of_line_module_reference(
                &context,
                &normalize_path(&root),
                "bridge::function",
                None,
            ),
            Some((normalize_path(&bridge), "function".to_string()))
        );
        // Two pub-use re-exports of the same local name are ambiguous, so the
        // name is not followed to either defining module.
        assert_eq!(
            resolve_rust_out_of_line_module_reference(
                &context,
                &normalize_path(&root),
                "impl_mod::ambiguous",
                None,
            ),
            Some((normalize_path(&impl_mod), "ambiguous".to_string()))
        );
        // A unique pub-use re-export still resolves next to the ambiguous one.
        assert_eq!(
            resolve_rust_out_of_line_module_reference(
                &context,
                &normalize_path(&root),
                "impl_mod::function",
                None,
            ),
            Some((normalize_path(&other_mod), "function".to_string()))
        );
    }

    #[test]
    fn fails_closed_on_cyclic_pub_use_reexports() {
        let dir = temporary_dir();
        let root = dir.join("lib.rs");
        let api = dir.join("api.rs");
        fs::write(&root, "mod api;\npub use api::helper;\n").unwrap();
        fs::write(&api, "pub use crate::helper;\n").unwrap();

        let context = rust_out_of_line_module_context_for_files_with_overrides_and_deadline(
            &[root.clone(), api.clone()],
            None,
            None,
        )
        .unwrap();
        assert!(
            resolve_rust_out_of_line_module_reference(
                &context,
                &normalize_path(&root),
                "helper",
                Some(&RustImportRoot::Crate),
            )
            .is_none()
        );
    }
}
