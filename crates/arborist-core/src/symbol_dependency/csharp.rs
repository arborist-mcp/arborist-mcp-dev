use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::language::{
    csharp_file_base_types, csharp_file_namespace_imports, csharp_file_static_type_imports,
    csharp_file_type_alias_imports, csharp_global_namespace_imports,
    csharp_global_static_type_imports, csharp_global_type_alias_imports, detect_language,
    normalize_path, parse_document, parse_document_with_timeout, read_source,
};
use crate::model::LanguageId;
use crate::workspace_scan::WorkspaceScanDeadline;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct CSharpTypeAliasBinding {
    pub(crate) semantic_type_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct CSharpStaticTypeImportBinding {
    pub(crate) scope_path: Option<String>,
    pub(crate) semantic_type_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct CSharpNamespaceImportBinding {
    pub(crate) scope_path: Option<String>,
    pub(crate) semantic_namespace_path: String,
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct CSharpImportContext {
    type_alias_bindings: BTreeMap<(Option<String>, String), CSharpTypeAliasBinding>,
    ambiguous_type_alias_names: BTreeSet<(Option<String>, String)>,
    base_type_bindings_by_range: BTreeMap<(usize, usize), CSharpBaseTypeBinding>,
    static_type_import_bindings: Vec<CSharpStaticTypeImportBinding>,
    namespace_import_bindings: Vec<CSharpNamespaceImportBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct CSharpBaseTypeBinding {
    pub(crate) semantic_type_path: String,
    pub(crate) is_global_qualified: bool,
    pub(crate) alias_name: Option<String>,
    pub(crate) namespace_import_paths: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct CSharpGlobalImportContext {
    type_alias_bindings: BTreeMap<(Option<String>, String), CSharpTypeAliasBinding>,
    ambiguous_type_alias_names: BTreeSet<(Option<String>, String)>,
    static_type_import_bindings: Vec<CSharpStaticTypeImportBinding>,
    namespace_import_bindings: Vec<CSharpNamespaceImportBinding>,
}

pub(in crate::symbol_dependency) fn csharp_global_import_context_for_files_with_overrides_and_deadline(
    source_file_paths: &[PathBuf],
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<CSharpGlobalImportContext> {
    let mut type_alias_bindings = BTreeMap::new();
    let mut ambiguous_type_alias_names = BTreeSet::new();
    let mut static_type_import_bindings = Vec::new();
    let mut namespace_import_bindings = Vec::new();
    let mut visited_paths = BTreeSet::new();

    for source_file_path in source_file_paths {
        if let Some(deadline) = deadline {
            deadline.check("reading C# global import context")?;
        }
        let normalized_file_path = normalize_path(source_file_path);
        if !visited_paths.insert(normalized_file_path.clone()) {
            continue;
        }
        let path = Path::new(&normalized_file_path);
        if detect_language(path).ok() != Some(LanguageId::CSharp) {
            continue;
        }
        let source = file_overrides
            .and_then(|overrides| overrides.get(&normalized_file_path))
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| read_source(path))?;
        if let Some(deadline) = deadline {
            deadline.check("parsing C# global import context")?;
        }
        let document = if let Some(deadline) = deadline {
            parse_document_with_timeout(
                path,
                &source,
                deadline.remaining_timeout_micros("parsing C# global import context")?,
            )?
        } else {
            parse_document(path, &source)?
        };
        let root = document.tree.root_node();
        if root.has_error() {
            continue;
        }
        for (local_name, semantic_type_path) in csharp_global_type_alias_imports(root, &source)? {
            if let Some(deadline) = deadline {
                deadline.check("extracting C# global type alias bindings")?;
            }
            insert_unique_csharp_type_alias_binding(
                &mut type_alias_bindings,
                &mut ambiguous_type_alias_names,
                None,
                local_name,
                CSharpTypeAliasBinding { semantic_type_path },
            );
        }
        for semantic_type_path in csharp_global_static_type_imports(root, &source)? {
            if let Some(deadline) = deadline {
                deadline.check("extracting C# global static import bindings")?;
            }
            static_type_import_bindings.push(CSharpStaticTypeImportBinding {
                scope_path: None,
                semantic_type_path,
            });
        }
        for semantic_namespace_path in csharp_global_namespace_imports(root, &source)? {
            if let Some(deadline) = deadline {
                deadline.check("extracting C# global namespace import bindings")?;
            }
            namespace_import_bindings.push(CSharpNamespaceImportBinding {
                scope_path: None,
                semantic_namespace_path,
            });
        }
    }

    Ok(CSharpGlobalImportContext {
        type_alias_bindings,
        ambiguous_type_alias_names,
        static_type_import_bindings,
        namespace_import_bindings,
    })
}

pub(in crate::symbol_dependency) fn resolve_csharp_global_type_alias_binding_for_reference(
    reference_name: &str,
    context: &CSharpGlobalImportContext,
) -> Option<(String, CSharpTypeAliasBinding)> {
    let (local_type_name, method_name) = reference_name.split_once('.')?;
    if local_type_name.is_empty() || method_name.is_empty() || method_name.contains('.') {
        return None;
    }
    let binding = context
        .type_alias_bindings
        .get(&(None, local_type_name.to_string()))?
        .clone();
    Some((method_name.to_string(), binding))
}

pub(in crate::symbol_dependency) fn resolve_csharp_global_base_type_alias(
    local_type_name: &str,
    context: &CSharpGlobalImportContext,
) -> Option<CSharpTypeAliasBinding> {
    if local_type_name.is_empty() {
        return None;
    }
    context
        .type_alias_bindings
        .get(&(None, local_type_name.to_string()))
        .cloned()
}

pub(in crate::symbol_dependency) fn csharp_global_base_type_alias_is_ambiguous(
    local_type_name: &str,
    context: &CSharpGlobalImportContext,
) -> bool {
    !local_type_name.is_empty()
        && context
            .ambiguous_type_alias_names
            .contains(&(None, local_type_name.to_string()))
}

pub(in crate::symbol_dependency) fn csharp_global_base_namespace_import_paths(
    context: &CSharpGlobalImportContext,
) -> Vec<String> {
    context
        .namespace_import_bindings
        .iter()
        .map(|binding| binding.semantic_namespace_path.clone())
        .collect()
}

pub(in crate::symbol_dependency) fn csharp_global_type_alias_name_is_ambiguous(
    reference_name: &str,
    context: &CSharpGlobalImportContext,
) -> bool {
    let Some((local_type_name, method_name)) = reference_name.split_once('.') else {
        return false;
    };
    if local_type_name.is_empty() || method_name.is_empty() || method_name.contains('.') {
        return false;
    }
    context
        .ambiguous_type_alias_names
        .contains(&(None, local_type_name.to_string()))
}

pub(in crate::symbol_dependency) fn resolve_csharp_global_static_type_imports_for_reference(
    reference_name: &str,
    context: &CSharpGlobalImportContext,
) -> Vec<CSharpStaticTypeImportBinding> {
    if reference_name.is_empty() || reference_name.contains('.') {
        return Vec::new();
    }
    context.static_type_import_bindings.clone()
}

fn csharp_import_context_for_file_with_overrides_and_deadline(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<CSharpImportContext> {
    let path = Path::new(file_path);
    if detect_language(path).ok() != Some(LanguageId::CSharp) {
        return Ok(CSharpImportContext::default());
    }

    if let Some(deadline) = deadline {
        deadline.check("reading C# import context")?;
    }
    let source = file_overrides
        .and_then(|overrides| overrides.get(&normalize_path(path)))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))?;
    if let Some(deadline) = deadline {
        deadline.check("parsing C# import context")?;
    }
    let document = if let Some(deadline) = deadline {
        parse_document_with_timeout(
            path,
            &source,
            deadline.remaining_timeout_micros("parsing C# import context")?,
        )?
    } else {
        parse_document(path, &source)?
    };
    let root = document.tree.root_node();
    if root.has_error() {
        return Ok(CSharpImportContext::default());
    }

    let mut type_alias_bindings = BTreeMap::new();
    let mut ambiguous_alias_names = BTreeSet::new();
    let mut base_type_bindings_by_range = BTreeMap::new();
    for import in csharp_file_type_alias_imports(root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting C# type alias bindings")?;
        }
        insert_unique_csharp_type_alias_binding(
            &mut type_alias_bindings,
            &mut ambiguous_alias_names,
            import.scope_path,
            import.local_name,
            CSharpTypeAliasBinding {
                semantic_type_path: import.semantic_type_path,
            },
        );
    }
    for base_type in csharp_file_base_types(root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting C# base type bindings")?;
        }
        if base_type_bindings_by_range
            .insert(
                base_type.type_range,
                CSharpBaseTypeBinding {
                    semantic_type_path: base_type.semantic_base_type_path,
                    is_global_qualified: base_type.is_global_qualified,
                    alias_name: None,
                    namespace_import_paths: Vec::new(),
                },
            )
            .is_some()
        {
            return Ok(CSharpImportContext::default());
        }
    }
    let mut static_type_import_bindings = Vec::new();
    for import in csharp_file_static_type_imports(root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting C# static type import bindings")?;
        }
        static_type_import_bindings.push(CSharpStaticTypeImportBinding {
            scope_path: import.scope_path,
            semantic_type_path: import.semantic_type_path,
        });
    }
    let mut namespace_import_bindings = Vec::new();
    for import in csharp_file_namespace_imports(root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting C# namespace import bindings")?;
        }
        namespace_import_bindings.push(CSharpNamespaceImportBinding {
            scope_path: import.scope_path,
            semantic_namespace_path: import.semantic_namespace_path,
        });
    }
    Ok(CSharpImportContext {
        type_alias_bindings,
        ambiguous_type_alias_names: ambiguous_alias_names,
        base_type_bindings_by_range,
        static_type_import_bindings,
        namespace_import_bindings,
    })
}

fn insert_unique_csharp_type_alias_binding(
    bindings: &mut BTreeMap<(Option<String>, String), CSharpTypeAliasBinding>,
    ambiguous_names: &mut BTreeSet<(Option<String>, String)>,
    scope_path: Option<String>,
    local_name: String,
    binding: CSharpTypeAliasBinding,
) {
    let key = (scope_path, local_name);
    if ambiguous_names.contains(&key) {
        return;
    }
    if bindings.insert(key.clone(), binding).is_some() {
        bindings.remove(&key);
        ambiguous_names.insert(key);
    }
}

pub(in crate::symbol_dependency) fn resolve_csharp_base_type_binding_for_reference(
    source_file_path: &str,
    source_type_range: (usize, usize),
    source_namespace_path: Option<&str>,
    global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    let Some(mut binding) = context
        .base_type_bindings_by_range
        .get(&source_type_range)
        .cloned()
    else {
        return Ok(None);
    };
    if !binding.is_global_qualified && binding.semantic_type_path.contains("::") {
        let Some(first_segment) = binding.semantic_type_path.split("::").next() else {
            return Ok(None);
        };
        for scope_path in csharp_import_scope_paths(source_namespace_path) {
            let key = (scope_path, first_segment.to_string());
            if context.ambiguous_type_alias_names.contains(&key)
                || context.type_alias_bindings.contains_key(&key)
            {
                return Ok(None);
            }
        }
        if let Some(global_import_context) = global_import_context
            && (csharp_global_base_type_alias_is_ambiguous(first_segment, global_import_context)
                || resolve_csharp_global_base_type_alias(first_segment, global_import_context)
                    .is_some())
        {
            return Ok(None);
        }
    } else if !binding.is_global_qualified {
        let local_name = binding.semantic_type_path.clone();
        let scope_paths = csharp_import_scope_paths(source_namespace_path);
        for scope_path in &scope_paths {
            let key = (scope_path.clone(), local_name.clone());
            if context.ambiguous_type_alias_names.contains(&key) {
                return Ok(None);
            }
            if let Some(alias) = context.type_alias_bindings.get(&key) {
                binding.semantic_type_path = alias.semantic_type_path.clone();
                binding.is_global_qualified = true;
                binding.alias_name = Some(local_name.clone());
                break;
            }
        }
        if binding.alias_name.is_none() {
            binding.namespace_import_paths = scope_paths
                .into_iter()
                .flat_map(|scope_path| {
                    context
                        .namespace_import_bindings
                        .iter()
                        .filter(move |candidate| candidate.scope_path == scope_path)
                        .map(|candidate| candidate.semantic_namespace_path.clone())
                })
                .collect();
            if let Some(global_import_context) = global_import_context {
                if csharp_global_base_type_alias_is_ambiguous(&local_name, global_import_context) {
                    return Ok(None);
                }
                if let Some(alias) =
                    resolve_csharp_global_base_type_alias(&local_name, global_import_context)
                {
                    binding.semantic_type_path = alias.semantic_type_path;
                    binding.is_global_qualified = true;
                    binding.alias_name = Some(local_name.clone());
                    binding.namespace_import_paths.clear();
                } else {
                    binding.namespace_import_paths.extend(
                        csharp_global_base_namespace_import_paths(global_import_context),
                    );
                }
            }
        }
    }
    Ok(Some(binding))
}

pub(in crate::symbol_dependency) fn resolve_csharp_type_alias_binding_for_reference(
    source_file_path: &str,
    reference_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<(String, CSharpTypeAliasBinding)>> {
    let Some((local_type_name, method_name)) = reference_name.split_once('.') else {
        return Ok(None);
    };
    if local_type_name.is_empty() || method_name.is_empty() || method_name.contains('.') {
        return Ok(None);
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    for scope_path in csharp_import_scope_paths(source_namespace_path) {
        let key = (scope_path, local_type_name.to_string());
        if context.ambiguous_type_alias_names.contains(&key) {
            return Ok(None);
        }
        if let Some(binding) = context.type_alias_bindings.get(&key) {
            return Ok(Some((method_name.to_string(), binding.clone())));
        }
    }
    Ok(None)
}

pub(in crate::symbol_dependency) fn csharp_type_alias_name_is_ambiguous_for_reference(
    source_file_path: &str,
    reference_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<bool> {
    let Some((local_type_name, method_name)) = reference_name.split_once('.') else {
        return Ok(false);
    };
    if local_type_name.is_empty() || method_name.is_empty() || method_name.contains('.') {
        return Ok(false);
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(csharp_import_scope_paths(source_namespace_path)
        .into_iter()
        .map(|scope_path| (scope_path, local_type_name.to_string()))
        .any(|key| context.ambiguous_type_alias_names.contains(&key)))
}

fn csharp_import_scope_paths(source_namespace_path: Option<&str>) -> Vec<Option<String>> {
    let mut scope_paths = Vec::new();
    let mut current_scope_path = source_namespace_path;
    while let Some(scope_path) = current_scope_path {
        scope_paths.push(Some(scope_path.to_string()));
        current_scope_path = scope_path
            .rsplit_once("::")
            .map(|(parent_path, _)| parent_path);
    }
    scope_paths.push(None);
    scope_paths
}

pub(in crate::symbol_dependency) fn resolve_csharp_static_type_imports_for_reference(
    source_file_path: &str,
    reference_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<CSharpStaticTypeImportBinding>> {
    if reference_name.is_empty() || reference_name.contains('.') {
        return Ok(Vec::new());
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(csharp_import_scope_paths(source_namespace_path)
        .into_iter()
        .flat_map(|scope_path| {
            context
                .static_type_import_bindings
                .iter()
                .filter(move |binding| binding.scope_path == scope_path)
                .cloned()
        })
        .collect())
}

pub(in crate::symbol_dependency) fn resolve_csharp_namespace_imports_for_reference(
    source_file_path: &str,
    reference_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<CSharpNamespaceImportBinding>> {
    if reference_name.is_empty() || reference_name.contains('.') {
        return Ok(Vec::new());
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(csharp_import_scope_paths(source_namespace_path)
        .into_iter()
        .flat_map(|scope_path| {
            context
                .namespace_import_bindings
                .iter()
                .filter(move |binding| binding.scope_path == scope_path)
                .cloned()
        })
        .collect())
}

pub(in crate::symbol_dependency) fn resolve_csharp_global_namespace_imports_for_reference(
    reference_name: &str,
    context: &CSharpGlobalImportContext,
) -> Vec<CSharpNamespaceImportBinding> {
    if reference_name.is_empty() || reference_name.contains('.') {
        return Vec::new();
    }
    context.namespace_import_bindings.clone()
}

fn csharp_import_context_from_cache(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<CSharpImportContext> {
    let normalized_file_path = normalize_path(Path::new(file_path));
    if let Some(context) = contexts_by_file.get(&normalized_file_path) {
        return Ok(context.clone());
    }

    let context = csharp_import_context_for_file_with_overrides_and_deadline(
        &normalized_file_path,
        file_overrides,
        deadline,
    )?;
    contexts_by_file.insert(normalized_file_path, context.clone());
    Ok(context)
}
