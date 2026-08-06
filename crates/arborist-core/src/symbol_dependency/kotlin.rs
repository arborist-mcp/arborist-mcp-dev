use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{
    detect_language, node_text, normalize_path, parse_document, parse_document_with_timeout,
    read_source,
};
use crate::model::LanguageId;
use crate::workspace_scan::WorkspaceScanDeadline;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct KotlinImportBinding {
    pub(crate) semantic_path: String,
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct KotlinImportContext {
    function_bindings: BTreeMap<String, KotlinImportBinding>,
}

fn kotlin_import_context_for_file_with_overrides_and_deadline(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<KotlinImportContext> {
    let path = Path::new(file_path);
    if detect_language(path).ok() != Some(LanguageId::Kotlin) {
        return Ok(KotlinImportContext::default());
    }

    if let Some(deadline) = deadline {
        deadline.check("reading Kotlin import context")?;
    }
    let source = file_overrides
        .and_then(|overrides| overrides.get(&normalize_path(path)))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))?;
    if let Some(deadline) = deadline {
        deadline.check("parsing Kotlin import context")?;
    }
    let document = if let Some(deadline) = deadline {
        parse_document_with_timeout(
            path,
            &source,
            deadline.remaining_timeout_micros("parsing Kotlin import context")?,
        )?
    } else {
        parse_document(path, &source)?
    };
    let root = document.tree.root_node();
    if root.has_error() {
        return Ok(KotlinImportContext::default());
    }

    let mut function_bindings = BTreeMap::new();
    let mut ambiguous_names = BTreeSet::new();
    let mut cursor = root.walk();
    for import in root
        .named_children(&mut cursor)
        .filter(|node| node.kind() == "import")
    {
        if let Some((local_name, binding)) = kotlin_explicit_import_binding(import, &source)? {
            insert_unique_kotlin_import_binding(
                &mut function_bindings,
                &mut ambiguous_names,
                local_name,
                binding,
            );
        }
    }
    Ok(KotlinImportContext { function_bindings })
}

fn kotlin_explicit_import_binding(
    import: Node<'_>,
    source: &str,
) -> Result<Option<(String, KotlinImportBinding)>> {
    let mut cursor = import.walk();
    let children = import.named_children(&mut cursor).collect::<Vec<_>>();
    let Some(qualified) = children
        .iter()
        .find(|child| child.kind() == "qualified_identifier")
    else {
        return Ok(None);
    };
    let qualified_text = node_text(*qualified, source)?.trim();
    if qualified_text.is_empty() || !is_safe_kotlin_qualified_name(qualified_text) {
        return Ok(None);
    }
    // Wildcard imports do not map to a unique local binding.
    if node_text(import, source)?.contains('*') {
        return Ok(None);
    }
    // An explicit `import pkg.name as alias` binds the alias; otherwise the
    // last dotted segment is the local name the caller uses.
    let local_name = children
        .iter()
        .find(|child| child.kind() == "identifier")
        .map(|alias| node_text(*alias, source).map(str::trim))
        .transpose()?
        .filter(|alias| !alias.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            qualified_text
                .rsplit_once('.')
                .map(|(_, last)| last.to_string())
                .unwrap_or_else(|| qualified_text.to_string())
        });
    Ok(Some((
        local_name,
        KotlinImportBinding {
            semantic_path: qualified_text.replace('.', "::"),
        },
    )))
}

fn insert_unique_kotlin_import_binding(
    bindings: &mut BTreeMap<String, KotlinImportBinding>,
    ambiguous_names: &mut BTreeSet<String>,
    local_name: String,
    binding: KotlinImportBinding,
) {
    if ambiguous_names.contains(&local_name) {
        return;
    }
    if bindings.insert(local_name.clone(), binding).is_some() {
        bindings.remove(&local_name);
        ambiguous_names.insert(local_name);
    }
}

fn is_safe_kotlin_qualified_name(name: &str) -> bool {
    name.split('.').all(|segment| {
        !segment.is_empty() && segment != "." && segment != ".." && !segment.contains(['/', '\\'])
    })
}

pub(in crate::symbol_dependency) fn resolve_kotlin_function_import_binding_for_reference(
    source_file_path: &str,
    reference_name: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<KotlinImportBinding>> {
    if reference_name.is_empty() || reference_name.contains('.') {
        return Ok(None);
    }
    let context = kotlin_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(context.function_bindings.get(reference_name).cloned())
}

fn kotlin_import_context_from_cache(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<KotlinImportContext> {
    let normalized_file_path = normalize_path(Path::new(file_path));
    if let Some(context) = contexts_by_file.get(&normalized_file_path) {
        return Ok(context.clone());
    }
    let context = kotlin_import_context_for_file_with_overrides_and_deadline(
        &normalized_file_path,
        file_overrides,
        deadline,
    )?;
    contexts_by_file.insert(normalized_file_path, context.clone());
    Ok(context)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{
        KotlinImportBinding, kotlin_import_context_for_file_with_overrides_and_deadline,
        resolve_kotlin_function_import_binding_for_reference,
    };
    use crate::language::normalize_path;

    static NEXT_KOTLIN_IMPORT_TEST_ID: AtomicUsize = AtomicUsize::new(0);

    fn context_for(source: &str) -> super::KotlinImportContext {
        let test_id = NEXT_KOTLIN_IMPORT_TEST_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "arborist-kotlin-import-{}-{}",
            std::process::id(),
            test_id
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("Caller.kt");
        std::fs::write(&file_path, source).unwrap();
        kotlin_import_context_for_file_with_overrides_and_deadline(
            &normalize_path(&file_path),
            None,
            None,
        )
        .unwrap()
    }

    #[test]
    fn binds_explicit_top_level_function_imports_to_semantic_paths() {
        let context = context_for(
            "package com.example\n\nimport org.util.helper\n\nfun caller(): Int = helper(1)\n",
        );
        assert_eq!(
            context.function_bindings.get("helper"),
            Some(&KotlinImportBinding {
                semantic_path: "org::util::helper".to_string()
            })
        );
    }

    #[test]
    fn binds_aliased_imports_to_the_alias_name() {
        let context = context_for(
            "package com.example\n\nimport org.util.helper as h\n\nfun caller(): Int = h(1)\n",
        );
        assert_eq!(
            context.function_bindings.get("h"),
            Some(&KotlinImportBinding {
                semantic_path: "org::util::helper".to_string()
            })
        );
        assert!(!context.function_bindings.contains_key("helper"));
    }

    #[test]
    fn ignores_wildcard_and_ambiguous_imports() {
        let context = context_for(
            "package com.example\n\nimport org.util.*\nimport org.a.helper\nimport org.b.helper\n\nfun caller(): Int = helper(1)\n",
        );
        assert!(context.function_bindings.is_empty());
    }

    #[test]
    fn keeps_unique_import_among_colliding_names_fail_closed() {
        let context = context_for(
            "package com.example\n\nimport org.a.helper\nimport org.b.helper\n\nfun caller(): Int = helper(1)\n",
        );
        assert!(context.function_bindings.is_empty());
    }

    #[test]
    fn resolves_import_binding_by_reference_name_without_parsing_again() {
        let test_id = NEXT_KOTLIN_IMPORT_TEST_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "arborist-kotlin-import-{}-{}",
            std::process::id(),
            test_id
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("Caller.kt");
        std::fs::write(
            &file_path,
            "package com.example\n\nimport org.util.helper\n\nfun caller(): Int = helper(1)\n",
        )
        .unwrap();
        let normalized = normalize_path(&file_path);
        let mut contexts = BTreeMap::new();
        let binding = resolve_kotlin_function_import_binding_for_reference(
            &normalized,
            "helper",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(binding.semantic_path, "org::util::helper");
        assert_eq!(contexts.len(), 1);
        assert!(
            resolve_kotlin_function_import_binding_for_reference(
                &normalized,
                "missing",
                None,
                &mut contexts,
                None,
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(contexts.len(), 1);
    }
}
