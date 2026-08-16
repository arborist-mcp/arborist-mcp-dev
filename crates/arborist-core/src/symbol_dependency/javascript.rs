use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;

use crate::language::{
    detect_language, javascript_named_export_names,
    javascript_named_import_module_paths_with_overrides_and_check,
    javascript_named_reexport_module_paths_with_overrides_and_check,
    javascript_star_reexport_module_paths_with_overrides_and_check, normalize_path, parse_document,
    parse_document_with_timeout, read_source,
};
use crate::model::LanguageId;
use crate::workspace_scan::WorkspaceScanDeadline;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct JavaScriptImportBinding {
    pub(crate) imported_name: String,
    pub(crate) module_paths: BTreeSet<String>,
    pub(crate) unresolved: bool,
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct JavaScriptImportContext {
    pub(crate) named_import_bindings: BTreeMap<String, JavaScriptImportBinding>,
    pub(crate) named_reexport_bindings: BTreeMap<String, JavaScriptImportBinding>,
    pub(crate) star_reexport_module_paths: BTreeSet<String>,
    pub(crate) named_export_names: BTreeSet<String>,
}

fn javascript_import_context_for_file_with_overrides_and_deadline(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<JavaScriptImportContext> {
    let path = Path::new(file_path);
    if !matches!(
        detect_language(path).ok(),
        Some(LanguageId::JavaScript | LanguageId::TypeScript | LanguageId::Tsx)
    ) {
        return Ok(JavaScriptImportContext::default());
    }

    if let Some(deadline) = deadline {
        deadline.check("reading JavaScript/TypeScript import context")?;
    }
    let source = file_overrides
        .and_then(|overrides| overrides.get(&normalize_path(path)))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))?;
    if let Some(deadline) = deadline {
        deadline.check("parsing JavaScript/TypeScript import context")?;
    }
    let document = if let Some(deadline) = deadline {
        parse_document_with_timeout(
            path,
            &source,
            deadline.remaining_timeout_micros("parsing JavaScript/TypeScript import context")?,
        )?
    } else {
        parse_document(path, &source)?
    };
    if let Some(deadline) = deadline {
        deadline.check("extracting JavaScript/TypeScript import bindings")?;
    }

    let check_traversal_deadline = || -> Result<()> {
        if let Some(deadline) = deadline {
            deadline.check("extracting JavaScript/TypeScript import bindings")?;
        }
        Ok(())
    };

    Ok(JavaScriptImportContext {
        named_import_bindings: javascript_named_import_module_paths_with_overrides_and_check(
            path,
            document.tree.root_node(),
            &source,
            file_overrides,
            Some(&check_traversal_deadline),
        )?
        .into_iter()
        .map(|(local_name, binding)| {
            (
                local_name,
                JavaScriptImportBinding {
                    imported_name: binding.imported_name,
                    module_paths: binding
                        .module_paths
                        .into_iter()
                        .map(|path| normalize_path(&path))
                        .collect(),
                    unresolved: binding.unresolved,
                },
            )
        })
        .collect(),
        named_reexport_bindings: javascript_named_reexport_module_paths_with_overrides_and_check(
            path,
            document.tree.root_node(),
            &source,
            file_overrides,
            Some(&check_traversal_deadline),
        )?
        .into_iter()
        .map(|(exported_name, binding)| {
            (
                exported_name,
                JavaScriptImportBinding {
                    imported_name: binding.imported_name,
                    module_paths: binding
                        .module_paths
                        .into_iter()
                        .map(|path| normalize_path(&path))
                        .collect(),
                    unresolved: binding.unresolved,
                },
            )
        })
        .collect(),
        star_reexport_module_paths: javascript_star_reexport_module_paths_with_overrides_and_check(
            path,
            document.tree.root_node(),
            &source,
            file_overrides,
            Some(&check_traversal_deadline),
        )?
        .into_iter()
        .map(|path| normalize_path(&path))
        .collect(),
        named_export_names: javascript_named_export_names(
            document.tree.root_node(),
            &source,
            Some(&check_traversal_deadline),
        )?,
    })
}

pub(in crate::symbol_dependency) fn resolve_javascript_named_import_binding_for_reference(
    source_file_path: &str,
    reference_name: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, JavaScriptImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<JavaScriptImportBinding>> {
    let source_context = javascript_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    let Some(binding) = source_context.named_import_bindings.get(reference_name) else {
        return Ok(None);
    };

    let mut resolution_stack = BTreeSet::new();
    Ok(Some(resolve_named_module_binding(
        binding.clone(),
        file_overrides,
        contexts_by_file,
        deadline,
        &mut resolution_stack,
    )?))
}

fn resolve_named_module_binding(
    binding: JavaScriptImportBinding,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, JavaScriptImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
    resolution_stack: &mut BTreeSet<(String, String)>,
) -> Result<JavaScriptImportBinding> {
    if let Some(deadline) = deadline {
        deadline.check("resolving JavaScript/TypeScript named import")?;
    }
    if binding.unresolved || binding.module_paths.is_empty() {
        return Ok(binding);
    }

    let mut resolved: Option<JavaScriptImportBinding> = None;
    for module_path in &binding.module_paths {
        let resolution_key = (module_path.clone(), binding.imported_name.clone());
        if !resolution_stack.insert(resolution_key.clone()) {
            return Ok(unresolved_named_module_binding(&binding.imported_name));
        }

        let module_context = javascript_import_context_from_cache(
            module_path,
            file_overrides,
            contexts_by_file,
            deadline,
        )?;
        let candidate = if let Some(reexport_binding) = module_context
            .named_reexport_bindings
            .get(&binding.imported_name)
        {
            resolve_named_module_binding(
                reexport_binding.clone(),
                file_overrides,
                contexts_by_file,
                deadline,
                resolution_stack,
            )?
        } else if binding.imported_name != "default" {
            // `export * from "./module"` forwards the target's named exports,
            // but never a module's default export.
            match resolve_star_reexported_module_paths(
                module_path,
                &binding.imported_name,
                file_overrides,
                contexts_by_file,
                deadline,
                resolution_stack,
            )? {
                StarReexportLookup::Unresolved => {
                    return Ok(unresolved_named_module_binding(&binding.imported_name));
                }
                StarReexportLookup::Found(paths) if paths.len() == 1 => JavaScriptImportBinding {
                    imported_name: binding.imported_name.clone(),
                    module_paths: paths,
                    unresolved: false,
                },
                // Multiple defining modules make the star re-export ambiguous;
                // fail closed instead of guessing.
                StarReexportLookup::Found(_) => {
                    return Ok(unresolved_named_module_binding(&binding.imported_name));
                }
                StarReexportLookup::Absent => JavaScriptImportBinding {
                    imported_name: binding.imported_name.clone(),
                    module_paths: BTreeSet::from([module_path.clone()]),
                    unresolved: false,
                },
            }
        } else {
            JavaScriptImportBinding {
                imported_name: binding.imported_name.clone(),
                module_paths: BTreeSet::from([module_path.clone()]),
                unresolved: false,
            }
        };
        resolution_stack.remove(&resolution_key);

        if candidate.unresolved || candidate.module_paths.is_empty() {
            return Ok(unresolved_named_module_binding(&binding.imported_name));
        }
        match &mut resolved {
            Some(resolved_binding) if resolved_binding.imported_name == candidate.imported_name => {
                resolved_binding.module_paths.extend(candidate.module_paths);
            }
            Some(_) => return Ok(unresolved_named_module_binding(&binding.imported_name)),
            None => resolved = Some(candidate),
        }
    }

    Ok(resolved.unwrap_or(binding))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StarReexportLookup {
    /// The name is not reachable through the module's re-export machinery.
    Absent,
    /// The name is exported; the set holds the defining module paths (never
    /// empty, and more than one entry means the export is ambiguous).
    Found(BTreeSet<String>),
    /// The re-export chain is broken, ambiguous, or cyclic and must fail
    /// closed.
    Unresolved,
}

/// Resolves the module paths that define `name` when it is exported by
/// `module_path` through star re-exports, following named re-export chains
/// and nested star re-exports transitively with cycle detection. Direct named
/// exports shadow star re-exports of the same name, matching ESM semantics.
fn resolve_star_reexported_module_paths(
    module_path: &str,
    name: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, JavaScriptImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
    resolution_stack: &mut BTreeSet<(String, String)>,
) -> Result<StarReexportLookup> {
    if let Some(deadline) = deadline {
        deadline.check("resolving JavaScript/TypeScript star re-export")?;
    }
    // `export *` never re-exports a module's default export.
    if name == "default" {
        return Ok(StarReexportLookup::Absent);
    }
    let module_context = javascript_import_context_from_cache(
        module_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    // A named re-export of the name is an explicit export and takes
    // precedence over any star re-export of the same name.
    if let Some(reexport_binding) = module_context.named_reexport_bindings.get(name) {
        let resolved = resolve_named_module_binding(
            reexport_binding.clone(),
            file_overrides,
            contexts_by_file,
            deadline,
            resolution_stack,
        )?;
        if resolved.unresolved || resolved.module_paths.is_empty() {
            return Ok(StarReexportLookup::Unresolved);
        }
        return Ok(StarReexportLookup::Found(resolved.module_paths));
    }
    // A direct named export shadows star re-exports of the same name.
    if module_context.named_export_names.contains(name) {
        return Ok(StarReexportLookup::Found(BTreeSet::from([
            module_path.to_owned()
        ])));
    }
    if module_context.star_reexport_module_paths.is_empty() {
        return Ok(StarReexportLookup::Absent);
    }
    let mut defining_paths = BTreeSet::new();
    for star_target in &module_context.star_reexport_module_paths {
        let resolution_key = (star_target.clone(), name.to_owned());
        if !resolution_stack.insert(resolution_key.clone()) {
            // A star re-export cycle must fail closed.
            return Ok(StarReexportLookup::Unresolved);
        }
        match resolve_star_reexported_module_paths(
            star_target,
            name,
            file_overrides,
            contexts_by_file,
            deadline,
            resolution_stack,
        )? {
            StarReexportLookup::Unresolved => {
                resolution_stack.remove(&resolution_key);
                return Ok(StarReexportLookup::Unresolved);
            }
            StarReexportLookup::Found(paths) => defining_paths.extend(paths),
            StarReexportLookup::Absent => {}
        }
        resolution_stack.remove(&resolution_key);
    }
    Ok(if defining_paths.is_empty() {
        StarReexportLookup::Absent
    } else {
        StarReexportLookup::Found(defining_paths)
    })
}

fn unresolved_named_module_binding(imported_name: &str) -> JavaScriptImportBinding {
    JavaScriptImportBinding {
        imported_name: imported_name.to_owned(),
        module_paths: BTreeSet::new(),
        unresolved: true,
    }
}

fn javascript_import_context_from_cache(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, JavaScriptImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<JavaScriptImportContext> {
    let normalized_file_path = normalize_path(Path::new(file_path));
    if let Some(context) = contexts_by_file.get(&normalized_file_path) {
        return Ok(context.clone());
    }

    let context = javascript_import_context_for_file_with_overrides_and_deadline(
        &normalized_file_path,
        file_overrides,
        deadline,
    )?;
    contexts_by_file.insert(normalized_file_path, context.clone());
    Ok(context)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;

    use super::{
        javascript_import_context_for_file_with_overrides_and_deadline,
        resolve_javascript_named_import_binding_for_reference,
    };
    use crate::language::normalize_path;

    #[test]
    fn import_context_reads_source_overrides() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-import-context-overrides-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let helper = root.join("helper.ts");
        fs::write(&caller, "export function caller() {}\n").unwrap();
        fs::write(&helper, "export function helper() {}\n").unwrap();
        let mut overrides = BTreeMap::new();
        overrides.insert(
            normalize_path(&caller),
            "import { helper } from \"./helper\";\nexport function caller() { return helper(); }\n"
                .to_owned(),
        );

        let context = javascript_import_context_for_file_with_overrides_and_deadline(
            &normalize_path(&caller),
            Some(&overrides),
            None,
        )
        .unwrap();
        assert!(
            context
                .named_import_bindings
                .get("helper")
                .is_some_and(|binding| binding.module_paths.contains(&normalize_path(&helper)))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_named_reexport_chains_from_source_overrides() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-reexport-context-overrides-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let helper = root.join("helper.ts");
        for path in [&caller, &bridge, &helper] {
            fs::write(path, "export function placeholder() {}\n").unwrap();
        }
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { forwarded as selected } from \"./bridge\";\nselected();\n".to_owned(),
            ),
            (
                normalize_path(&bridge),
                "export { helper as forwarded } from \"./helper\";\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "selected",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert_eq!(binding.imported_name, "helper");
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_named_reexport_chains_through_overlay_only_modules() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-overlay-only-reexports-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let helper = root.join("helper.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { forwarded as selected } from \"./bridge\";\nselected();\n".to_owned(),
            ),
            (
                normalize_path(&bridge),
                "export { helper as forwarded } from \"./helper\";\n".to_owned(),
            ),
            (
                normalize_path(&helper),
                "export function helper() {}\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "selected",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert_eq!(binding.imported_name, "helper");
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        assert!(!binding.unresolved);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn treats_named_reexport_cycles_as_unresolved() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-reexport-cycle-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let first = root.join("first.ts");
        let second = root.join("second.ts");
        for path in [&caller, &first, &second] {
            fs::write(path, "export function placeholder() {}\n").unwrap();
        }
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { helper } from \"./first\";\nhelper();\n".to_owned(),
            ),
            (
                normalize_path(&first),
                "export { helper } from \"./second\";\n".to_owned(),
            ),
            (
                normalize_path(&second),
                "export { helper } from \"./first\";\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert!(binding.module_paths.is_empty());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn resolves_named_import_through_star_reexport_from_source_overrides() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-star-reexport-context-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let helper = root.join("helper.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { helper } from \"./bridge\";\nhelper();\n".to_owned(),
            ),
            (
                normalize_path(&bridge),
                "export * from \"./helper\";\n".to_owned(),
            ),
            (
                normalize_path(&helper),
                "export function helper() {}\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert_eq!(binding.imported_name, "helper");
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        assert!(!binding.unresolved);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_star_reexport_through_named_reexport_chain() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-star-named-chain-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let mid = root.join("mid.ts");
        let helper = root.join("helper.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { helper } from \"./bridge\";\nhelper();\n".to_owned(),
            ),
            (
                normalize_path(&bridge),
                "export * from \"./mid\";\n".to_owned(),
            ),
            (
                normalize_path(&mid),
                "export { helper } from \"./helper\";\n".to_owned(),
            ),
            (
                normalize_path(&helper),
                "export function helper() {}\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert_eq!(binding.imported_name, "helper");
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        assert!(!binding.unresolved);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_star_reexport_to_direct_export_when_both_present() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-star-direct-shadow-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let helper = root.join("helper.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { helper } from \"./bridge\";\nhelper();\n".to_owned(),
            ),
            (
                normalize_path(&bridge),
                "export * from \"./helper\";\nexport function helper() {}\n".to_owned(),
            ),
            (
                normalize_path(&helper),
                "export function helper() {}\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert_eq!(binding.imported_name, "helper");
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&bridge)])
        );
        assert!(!binding.unresolved);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn treats_star_reexport_cycles_as_unresolved() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-star-cycle-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let first = root.join("first.ts");
        let second = root.join("second.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { helper } from \"./first\";\nhelper();\n".to_owned(),
            ),
            (
                normalize_path(&first),
                "export * from \"./second\";\n".to_owned(),
            ),
            (
                normalize_path(&second),
                "export * from \"./first\";\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert!(binding.module_paths.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn treats_ambiguous_star_reexports_as_unresolved() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-star-ambiguous-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let first = root.join("first.ts");
        let second = root.join("second.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { helper } from \"./bridge\";\nhelper();\n".to_owned(),
            ),
            (
                normalize_path(&bridge),
                "export * from \"./first\";\nexport * from \"./second\";\n".to_owned(),
            ),
            (
                normalize_path(&first),
                "export function helper() {}\n".to_owned(),
            ),
            (
                normalize_path(&second),
                "export function helper() {}\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert!(binding.module_paths.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_star_reexport_missing_exports_absent() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-star-missing-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let helper = root.join("helper.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { helper } from \"./bridge\";\nhelper();\n".to_owned(),
            ),
            (
                normalize_path(&bridge),
                "export * from \"./helper\";\n".to_owned(),
            ),
            (normalize_path(&helper), "function helper() {}\n".to_owned()),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert_eq!(binding.imported_name, "helper");
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&bridge)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_namespace_reexports_to_target_module() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-namespace-reexport-binding-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let helper = root.join("helper.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { ns } from \"./bridge\";\nns.helper();\n".to_owned(),
            ),
            (
                normalize_path(&bridge),
                "export * as ns from \"./helper\";\n".to_owned(),
            ),
            (
                normalize_path(&helper),
                "export function helper() {}\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert_eq!(binding.imported_name, "<namespace>");
        assert!(!binding.unresolved);
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_namespace_reexports_with_missing_targets_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-namespace-reexport-missing-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { ns } from \"./bridge\";\nns.helper();\n".to_owned(),
            ),
            (
                normalize_path(&bridge),
                "export * as ns from \"./missing\";\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert!(binding.unresolved);
        assert!(binding.module_paths.is_empty());
        let _ = fs::remove_dir_all(root);
    }
}
