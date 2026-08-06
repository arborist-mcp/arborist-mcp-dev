use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{
    detect_language, node_text, normalize_path, parse_document, parse_document_with_timeout,
    read_source,
};
use crate::model::LanguageId;
use crate::semantic::kotlin::is_kotlin_semantic_symbol_node;
use crate::workspace_scan::WorkspaceScanDeadline;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct KotlinImportBinding {
    pub(crate) semantic_path: String,
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct KotlinImportContext {
    import_bindings: BTreeMap<String, KotlinImportBinding>,
    receiver_type_bindings_by_range: BTreeMap<(usize, usize), KotlinReceiverTypeBindings>,
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct KotlinReceiverTypeBindings {
    types_by_name: BTreeMap<String, String>,
    ambiguous_names: BTreeSet<String>,
}

impl KotlinReceiverTypeBindings {
    pub(in crate::symbol_dependency) fn type_for(&self, name: &str) -> Option<String> {
        if self.ambiguous_names.contains(name) {
            return None;
        }
        self.types_by_name.get(name).cloned()
    }
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

    let mut import_bindings = BTreeMap::new();
    let mut ambiguous_import_names = BTreeSet::new();
    let mut cursor = root.walk();
    for import in root
        .named_children(&mut cursor)
        .filter(|node| node.kind() == "import")
    {
        if let Some((local_name, binding)) = kotlin_explicit_import_binding(import, &source)? {
            insert_unique_kotlin_import_binding(
                &mut import_bindings,
                &mut ambiguous_import_names,
                local_name,
                binding,
            );
        }
    }

    let mut receiver_type_bindings_by_range = BTreeMap::new();
    collect_kotlin_receiver_type_bindings(root, &source, &mut receiver_type_bindings_by_range)?;

    Ok(KotlinImportContext {
        import_bindings,
        receiver_type_bindings_by_range,
    })
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

fn collect_kotlin_receiver_type_bindings(
    node: Node<'_>,
    source: &str,
    bindings_by_range: &mut BTreeMap<(usize, usize), KotlinReceiverTypeBindings>,
) -> Result<()> {
    if node.kind() == "function_declaration" && is_kotlin_semantic_symbol_node(node) {
        bindings_by_range.insert(
            (node.start_byte(), node.end_byte()),
            kotlin_receiver_type_bindings_for_node(node, source)?,
        );
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_kotlin_receiver_type_bindings(child, source, bindings_by_range)?;
    }
    Ok(())
}

fn kotlin_receiver_type_bindings_for_node(
    function: Node<'_>,
    source: &str,
) -> Result<KotlinReceiverTypeBindings> {
    let mut bindings = KotlinReceiverTypeBindings::default();

    // Enclosing-type properties are visible to member functions.
    if let Some(type_node) = kotlin_enclosing_type_declaration(function)
        && let Some(class_body) = type_node
            .named_children(&mut type_node.walk())
            .find(|child| child.kind() == "class_body")
    {
        let mut cursor = class_body.walk();
        for child in class_body.named_children(&mut cursor) {
            if child.kind() == "property_declaration"
                && let Some((name, type_name)) = kotlin_property_binding(child, source)?
            {
                insert_kotlin_receiver_binding(&mut bindings, name, type_name);
            }
        }
    }

    // Parameters carry explicit types.
    if let Some(parameters) = function
        .named_children(&mut function.walk())
        .find(|child| child.kind() == "function_value_parameters")
    {
        let mut cursor = parameters.walk();
        for parameter in parameters.named_children(&mut cursor) {
            if parameter.kind() == "parameter"
                && let Some((name, type_name)) = kotlin_parameter_binding(parameter, source)?
            {
                insert_kotlin_receiver_binding(&mut bindings, name, type_name);
            }
        }
    }

    // Body locals, stopping at nested declarations that have their own scope.
    if let Some(body) = function
        .named_children(&mut function.walk())
        .find(|child| child.kind() == "function_body")
    {
        collect_kotlin_body_property_bindings(body, source, &mut bindings)?;
    }
    Ok(bindings)
}

fn kotlin_enclosing_type_declaration<'a>(node: Node<'a>) -> Option<Node<'a>> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if matches!(candidate.kind(), "class_declaration" | "object_declaration") {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

fn collect_kotlin_body_property_bindings(
    node: Node<'_>,
    source: &str,
    bindings: &mut KotlinReceiverTypeBindings,
) -> Result<()> {
    if matches!(
        node.kind(),
        "function_declaration" | "class_declaration" | "object_declaration"
    ) {
        return Ok(());
    }
    if node.kind() == "property_declaration"
        && let Some((name, type_name)) = kotlin_property_binding(node, source)?
    {
        insert_kotlin_receiver_binding(bindings, name, type_name);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_kotlin_body_property_bindings(child, source, bindings)?;
    }
    Ok(())
}

fn kotlin_property_binding(property: Node<'_>, source: &str) -> Result<Option<(String, String)>> {
    let mut cursor = property.walk();
    let children = property.named_children(&mut cursor).collect::<Vec<_>>();
    let Some(variable) = children
        .iter()
        .find(|child| child.kind() == "variable_declaration")
    else {
        return Ok(None);
    };
    let mut variable_cursor = variable.walk();
    let variable_children = variable
        .named_children(&mut variable_cursor)
        .collect::<Vec<_>>();
    let Some(name_node) = variable_children
        .iter()
        .find(|child| child.kind() == "identifier")
    else {
        return Ok(None);
    };
    let name = node_text(*name_node, source)?.trim().to_string();
    if name.is_empty() {
        return Ok(None);
    }
    if let Some(type_node) = variable_children
        .iter()
        .find(|child| kotlin_is_type_node_kind(child.kind()))
        && let Some(type_name) = kotlin_simple_type_name(node_text(*type_node, source)?)
    {
        return Ok(Some((name, type_name)));
    }
    // Fall back to a constructor-call initializer such as `val x = Other()`.
    let initializer = children
        .iter()
        .find(|child| child.kind() == "call_expression")
        .copied();
    if let Some(expression) = initializer
        && let Some(callee) = expression.named_child(0)
        && callee.kind() == "identifier"
    {
        let type_name = node_text(callee, source)?.trim().to_string();
        if !type_name.is_empty() {
            return Ok(Some((name, type_name)));
        }
    }
    Ok(None)
}

fn kotlin_parameter_binding(parameter: Node<'_>, source: &str) -> Result<Option<(String, String)>> {
    let mut cursor = parameter.walk();
    let children = parameter.named_children(&mut cursor).collect::<Vec<_>>();
    let Some(name_node) = children.iter().find(|child| child.kind() == "identifier") else {
        return Ok(None);
    };
    let Some(type_node) = children
        .iter()
        .find(|child| kotlin_is_type_node_kind(child.kind()))
    else {
        return Ok(None);
    };
    let name = node_text(*name_node, source)?.trim().to_string();
    let Some(type_name) = kotlin_simple_type_name(node_text(*type_node, source)?) else {
        return Ok(None);
    };
    if name.is_empty() {
        return Ok(None);
    }
    Ok(Some((name, type_name)))
}

fn kotlin_is_type_node_kind(kind: &str) -> bool {
    matches!(kind, "type" | "user_type" | "nullable_type")
}

pub(in crate::symbol_dependency) fn kotlin_simple_type_name(text: &str) -> Option<String> {
    let mut name = text.trim();
    if let Some(stripped) = name.strip_suffix('?') {
        name = stripped.trim();
    }
    if name.is_empty() || name.contains(['.', '<', '(', '[', ':', ',', ' ']) {
        return None;
    }
    Some(name.to_string())
}

fn insert_kotlin_receiver_binding(
    bindings: &mut KotlinReceiverTypeBindings,
    name: String,
    type_name: String,
) {
    if bindings.ambiguous_names.contains(&name) {
        return;
    }
    if bindings
        .types_by_name
        .insert(name.clone(), type_name)
        .is_some()
    {
        bindings.types_by_name.remove(&name);
        bindings.ambiguous_names.insert(name);
    }
}

pub(in crate::symbol_dependency) fn resolve_kotlin_import_binding_for_reference(
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
    Ok(context.import_bindings.get(reference_name).cloned())
}

pub(in crate::symbol_dependency) fn kotlin_receiver_type_bindings_for_function(
    source_file_path: &str,
    function_range: (usize, usize),
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, KotlinImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<KotlinReceiverTypeBindings>> {
    let context = kotlin_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(context
        .receiver_type_bindings_by_range
        .get(&function_range)
        .cloned())
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
        KotlinImportBinding, KotlinReceiverTypeBindings,
        kotlin_import_context_for_file_with_overrides_and_deadline,
        kotlin_receiver_type_bindings_for_function, resolve_kotlin_import_binding_for_reference,
    };
    use crate::language::normalize_path;

    static NEXT_KOTLIN_TEST_ID: AtomicUsize = AtomicUsize::new(0);

    struct TestFile {
        normalized_path: String,
    }

    fn write_test_file(source: &str) -> TestFile {
        let test_id = NEXT_KOTLIN_TEST_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "arborist-kotlin-{}-{}",
            std::process::id(),
            test_id
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("Caller.kt");
        std::fs::write(&file_path, source).unwrap();
        TestFile {
            normalized_path: normalize_path(&file_path),
        }
    }

    #[test]
    fn binds_explicit_top_level_function_imports_to_semantic_paths() {
        let file = write_test_file(
            "package com.example\n\nimport org.util.helper\n\nfun caller(): Int = helper(1)\n",
        );
        let context = kotlin_import_context_for_file_with_overrides_and_deadline(
            &file.normalized_path,
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            context.import_bindings.get("helper"),
            Some(&KotlinImportBinding {
                semantic_path: "org::util::helper".to_string()
            })
        );
    }

    #[test]
    fn binds_aliased_imports_to_the_alias_name() {
        let file = write_test_file(
            "package com.example\n\nimport org.util.helper as h\n\nfun caller(): Int = h(1)\n",
        );
        let context = kotlin_import_context_for_file_with_overrides_and_deadline(
            &file.normalized_path,
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            context.import_bindings.get("h"),
            Some(&KotlinImportBinding {
                semantic_path: "org::util::helper".to_string()
            })
        );
        assert!(!context.import_bindings.contains_key("helper"));
    }

    #[test]
    fn ignores_wildcard_and_ambiguous_imports() {
        let file = write_test_file(
            "package com.example\n\nimport org.util.*\nimport org.a.helper\nimport org.b.helper\n\nfun caller(): Int = helper(1)\n",
        );
        let context = kotlin_import_context_for_file_with_overrides_and_deadline(
            &file.normalized_path,
            None,
            None,
        )
        .unwrap();
        assert!(context.import_bindings.is_empty());
    }

    #[test]
    fn resolves_import_binding_by_reference_name_without_parsing_again() {
        let file = write_test_file(
            "package com.example\n\nimport org.util.helper\n\nfun caller(): Int = helper(1)\n",
        );
        let mut contexts = BTreeMap::new();
        let binding = resolve_kotlin_import_binding_for_reference(
            &file.normalized_path,
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
            resolve_kotlin_import_binding_for_reference(
                &file.normalized_path,
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

    #[test]
    fn binds_local_constructor_receivers_and_parameter_types() {
        let file = write_test_file(
            "package com.example\n\nclass Counter {\n    fun run() {\n        val other = Other()\n        other.helper(1)\n    }\n}\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\nfun process(counter: Counter): Int = counter.increment()\n",
        );
        let context = kotlin_import_context_for_file_with_overrides_and_deadline(
            &file.normalized_path,
            None,
            None,
        )
        .unwrap();

        let run_bindings = context
            .receiver_type_bindings_by_range
            .values()
            .find(|bindings| bindings.type_for("other") == Some("Other".to_string()))
            .unwrap();
        assert_eq!(run_bindings.type_for("other"), Some("Other".to_string()));

        let process_bindings = context
            .receiver_type_bindings_by_range
            .values()
            .find(|bindings| bindings.type_for("counter") == Some("Counter".to_string()))
            .unwrap();
        assert_eq!(
            process_bindings.type_for("counter"),
            Some("Counter".to_string())
        );
    }

    #[test]
    fn binds_class_property_receivers_with_explicit_and_constructor_types() {
        let file = write_test_file(
            "package com.example\n\nclass Holder {\n    val explicit: Other = Other()\n    val constructed = Other()\n    fun run() {\n        explicit.touch()\n        constructed.touch()\n    }\n}\n\nclass Other {\n    fun touch(): Int = 1\n}\n",
        );
        let context = kotlin_import_context_for_file_with_overrides_and_deadline(
            &file.normalized_path,
            None,
            None,
        )
        .unwrap();
        let run_bindings = context
            .receiver_type_bindings_by_range
            .values()
            .find(|bindings| bindings.type_for("explicit").is_some())
            .unwrap();
        assert_eq!(run_bindings.type_for("explicit"), Some("Other".to_string()));
        assert_eq!(
            run_bindings.type_for("constructed"),
            Some("Other".to_string())
        );
    }

    #[test]
    fn rejects_ambiguous_or_uninferrable_receiver_bindings() {
        let file = write_test_file(
            "package com.example\n\nfun caller(flag: Boolean): Int {\n    val other = Other()\n    val other = Third()\n    val unknown = makeOther()\n    return other.helper(1)\n}\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\nclass Third {\n    fun helper(value: Int): Int = value\n}\n\nfun makeOther(): Other = Other()\n",
        );
        let context = kotlin_import_context_for_file_with_overrides_and_deadline(
            &file.normalized_path,
            None,
            None,
        )
        .unwrap();
        let caller_bindings = context
            .receiver_type_bindings_by_range
            .values()
            .find(|bindings| bindings.type_for("unknown").is_none())
            .unwrap();
        assert_eq!(caller_bindings.type_for("other"), None);
        assert_eq!(caller_bindings.type_for("unknown"), None);
    }

    #[test]
    fn receiver_bindings_are_keyed_by_function_byte_range() {
        let file = write_test_file(
            "package com.example\n\nfun first(): Int {\n    val other = Other()\n    return other.helper(1)\n}\n\nfun second(): Int = 0\n",
        );
        let context = kotlin_import_context_for_file_with_overrides_and_deadline(
            &file.normalized_path,
            None,
            None,
        )
        .unwrap();
        assert_eq!(context.receiver_type_bindings_by_range.len(), 2);
        let first_range = *context
            .receiver_type_bindings_by_range
            .keys()
            .next()
            .unwrap();
        let first_bindings = context
            .receiver_type_bindings_by_range
            .get(&first_range)
            .unwrap();
        assert_eq!(first_bindings.type_for("other"), Some("Other".to_string()));
        let mut contexts = BTreeMap::new();
        let fetched = kotlin_receiver_type_bindings_for_function(
            &file.normalized_path,
            first_range,
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(fetched.type_for("other"), Some("Other".to_string()));
    }

    #[test]
    fn receiver_binding_type_for_returns_none_for_unknown_names() {
        let bindings = KotlinReceiverTypeBindings::default();
        assert_eq!(bindings.type_for("missing"), None);
    }
}
