use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;

use crate::language::{
    csharp_file_type_alias_imports, detect_language, normalize_path, parse_document,
    parse_document_with_timeout, read_source,
};
use crate::model::LanguageId;
use crate::workspace_scan::WorkspaceScanDeadline;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct CSharpTypeAliasBinding {
    pub(crate) semantic_type_path: String,
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct CSharpImportContext {
    type_alias_bindings: BTreeMap<String, CSharpTypeAliasBinding>,
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
    for import in csharp_file_type_alias_imports(root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting C# type alias bindings")?;
        }
        insert_unique_csharp_type_alias_binding(
            &mut type_alias_bindings,
            &mut ambiguous_alias_names,
            import.local_name,
            CSharpTypeAliasBinding {
                semantic_type_path: import.semantic_type_path,
            },
        );
    }
    Ok(CSharpImportContext {
        type_alias_bindings,
    })
}

fn insert_unique_csharp_type_alias_binding(
    bindings: &mut BTreeMap<String, CSharpTypeAliasBinding>,
    ambiguous_names: &mut BTreeSet<String>,
    local_name: String,
    binding: CSharpTypeAliasBinding,
) {
    if ambiguous_names.contains(&local_name) {
        return;
    }
    if bindings.insert(local_name.clone(), binding).is_some() {
        bindings.remove(&local_name);
        ambiguous_names.insert(local_name);
    }
}

pub(in crate::symbol_dependency) fn resolve_csharp_type_alias_binding_for_reference(
    source_file_path: &str,
    reference_name: &str,
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
    let Some(binding) = context.type_alias_bindings.get(local_type_name) else {
        return Ok(None);
    };
    Ok(Some((method_name.to_string(), binding.clone())))
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
