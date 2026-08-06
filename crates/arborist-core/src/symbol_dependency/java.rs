use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;

use crate::language::{
    detect_language, java_local_explicit_static_member_imports, java_local_explicit_type_imports,
    node_text, normalize_path, parse_document, parse_document_with_timeout, read_source,
};
use crate::model::LanguageId;
use crate::semantic::java::is_java_symbol_node;
use crate::workspace_scan::WorkspaceScanDeadline;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct JavaImportBinding {
    pub(crate) semantic_path: String,
    pub(crate) source_path: String,
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct JavaImportContext {
    type_bindings: BTreeMap<String, JavaImportBinding>,
    static_method_bindings: BTreeMap<String, JavaImportBinding>,
    receiver_type_bindings_by_range: BTreeMap<(usize, usize), JavaReceiverTypeBindings>,
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct JavaReceiverTypeBindings {
    types_by_name: BTreeMap<String, String>,
    ambiguous_names: BTreeSet<String>,
    initializer_calls_by_name: BTreeMap<String, (String, usize)>,
}

impl JavaReceiverTypeBindings {
    /// Returns whether `name` is bound locally, including as an ambiguous
    /// binding. Callers use this to distinguish "not bound" (a receiver may be
    /// a same-named type instead) from "bound but unusable" (fail closed).
    pub(in crate::symbol_dependency) fn contains(&self, name: &str) -> bool {
        self.types_by_name.contains_key(name) || self.ambiguous_names.contains(name)
    }

    /// Returns the declared type for a uniquely bound name. Names bound without
    /// a resolvable type (for example inferred lambda parameters) and ambiguous
    /// bindings return `None`.
    pub(in crate::symbol_dependency) fn type_for(&self, name: &str) -> Option<String> {
        if self.ambiguous_names.contains(name) {
            return None;
        }
        self.types_by_name
            .get(name)
            .filter(|type_name| !type_name.is_empty())
            .cloned()
    }

    /// Returns the factory callee name and call arity for a `var` local bound
    /// from a bare method-call initializer such as `var value = makeFoo(...)`.
    /// Ambiguous bindings and names without a factory initializer return `None`.
    pub(in crate::symbol_dependency) fn initializer_call_for(
        &self,
        name: &str,
    ) -> Option<(String, usize)> {
        if self.ambiguous_names.contains(name) {
            return None;
        }
        self.initializer_calls_by_name.get(name).cloned()
    }
}

fn java_import_context_for_file_with_overrides_and_deadline(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<JavaImportContext> {
    let path = Path::new(file_path);
    if detect_language(path).ok() != Some(LanguageId::Java) {
        return Ok(JavaImportContext::default());
    }

    if let Some(deadline) = deadline {
        deadline.check("reading Java import context")?;
    }
    let source = file_overrides
        .and_then(|overrides| overrides.get(&normalize_path(path)))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))?;
    if let Some(deadline) = deadline {
        deadline.check("parsing Java import context")?;
    }
    let document = if let Some(deadline) = deadline {
        parse_document_with_timeout(
            path,
            &source,
            deadline.remaining_timeout_micros("parsing Java import context")?,
        )?
    } else {
        parse_document(path, &source)?
    };
    let root = document.tree.root_node();
    if root.has_error() {
        return Ok(JavaImportContext::default());
    }

    let mut type_bindings = BTreeMap::new();
    let mut ambiguous_type_names = BTreeSet::new();
    for import in java_local_explicit_type_imports(path, root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting Java type import bindings")?;
        }
        insert_unique_java_import_binding(
            &mut type_bindings,
            &mut ambiguous_type_names,
            import.local_name,
            JavaImportBinding {
                semantic_path: import.semantic_path,
                source_path: normalize_path(&import.source_path),
            },
        );
    }

    let mut static_method_bindings = BTreeMap::new();
    let mut ambiguous_static_method_names = BTreeSet::new();
    for import in java_local_explicit_static_member_imports(path, root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting Java static import bindings")?;
        }
        insert_unique_java_import_binding(
            &mut static_method_bindings,
            &mut ambiguous_static_method_names,
            import.local_name,
            JavaImportBinding {
                semantic_path: import.semantic_type_path,
                source_path: normalize_path(&import.source_path),
            },
        );
    }
    let mut receiver_type_bindings_by_range = BTreeMap::new();
    collect_java_receiver_type_bindings(root, &source, &mut receiver_type_bindings_by_range)?;

    Ok(JavaImportContext {
        type_bindings,
        static_method_bindings,
        receiver_type_bindings_by_range,
    })
}

fn insert_unique_java_import_binding(
    bindings: &mut BTreeMap<String, JavaImportBinding>,
    ambiguous_names: &mut BTreeSet<String>,
    local_name: String,
    binding: JavaImportBinding,
) {
    if ambiguous_names.contains(&local_name) {
        return;
    }
    if bindings.insert(local_name.clone(), binding).is_some() {
        bindings.remove(&local_name);
        ambiguous_names.insert(local_name);
    }
}

pub(in crate::symbol_dependency) fn resolve_java_type_import_binding_for_name(
    source_file_path: &str,
    type_name: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<JavaImportBinding>> {
    if type_name.is_empty() || type_name.contains('.') {
        return Ok(None);
    }
    let context = java_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(context.type_bindings.get(type_name).cloned())
}

pub(in crate::symbol_dependency) fn resolve_java_import_binding_for_reference(
    source_file_path: &str,
    reference_name: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<(String, JavaImportBinding)>> {
    let Some((local_type_name, method_name)) = reference_name.split_once('.') else {
        return Ok(None);
    };
    if local_type_name.is_empty() || method_name.is_empty() || method_name.contains('.') {
        return Ok(None);
    }

    let context = java_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    let Some(binding) = context.type_bindings.get(local_type_name) else {
        return Ok(None);
    };
    Ok(Some((method_name.to_string(), binding.clone())))
}

pub(in crate::symbol_dependency) fn resolve_java_static_method_import_binding_for_reference(
    source_file_path: &str,
    reference_name: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<JavaImportBinding>> {
    if reference_name.is_empty() || reference_name.contains('.') {
        return Ok(None);
    }

    let context = java_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(context.static_method_bindings.get(reference_name).cloned())
}

fn java_import_context_from_cache(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<JavaImportContext> {
    let normalized_file_path = normalize_path(Path::new(file_path));
    if let Some(context) = contexts_by_file.get(&normalized_file_path) {
        return Ok(context.clone());
    }

    let context = java_import_context_for_file_with_overrides_and_deadline(
        &normalized_file_path,
        file_overrides,
        deadline,
    )?;
    contexts_by_file.insert(normalized_file_path, context.clone());
    Ok(context)
}

pub(in crate::symbol_dependency) fn java_receiver_type_bindings_for_function(
    source_file_path: &str,
    function_range: (usize, usize),
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, JavaImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<JavaReceiverTypeBindings>> {
    let context = java_import_context_from_cache(
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

fn collect_java_receiver_type_bindings(
    node: tree_sitter::Node<'_>,
    source: &str,
    bindings_by_range: &mut BTreeMap<(usize, usize), JavaReceiverTypeBindings>,
) -> Result<()> {
    if matches!(
        node.kind(),
        "method_declaration" | "constructor_declaration"
    ) && is_java_symbol_node(node)
    {
        bindings_by_range.insert(
            (node.start_byte(), node.end_byte()),
            java_receiver_type_bindings_for_node(node, source)?,
        );
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_java_receiver_type_bindings(child, source, bindings_by_range)?;
    }
    Ok(())
}

fn java_receiver_type_bindings_for_node(
    function: tree_sitter::Node<'_>,
    source: &str,
) -> Result<JavaReceiverTypeBindings> {
    let mut bindings = JavaReceiverTypeBindings::default();

    // Enclosing-type fields are visible to member functions.
    if let Some(type_node) = java_enclosing_type_declaration(function) {
        collect_java_type_field_bindings(type_node, source, &mut bindings)?;
    }

    // Parameters carry explicit types; varargs and generics fail closed.
    if let Some(parameters) = function.child_by_field_name("parameters") {
        let mut cursor = parameters.walk();
        for parameter in parameters.named_children(&mut cursor) {
            if matches!(parameter.kind(), "formal_parameter" | "spread_parameter")
                && let Some((name, type_name)) = java_parameter_binding(parameter, source)?
            {
                insert_java_receiver_binding(&mut bindings, name, type_name);
            }
        }
    }

    // Body locals, stopping at nested declarations that have their own scope.
    if let Some(body) = function.child_by_field_name("body") {
        collect_java_body_bindings(body, source, &mut bindings)?;
    }
    Ok(bindings)
}

fn java_enclosing_type_declaration<'a>(
    node: tree_sitter::Node<'a>,
) -> Option<tree_sitter::Node<'a>> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if matches!(
            candidate.kind(),
            "annotation_type_declaration"
                | "class_declaration"
                | "enum_declaration"
                | "interface_declaration"
                | "record_declaration"
        ) {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

fn collect_java_type_field_bindings(
    type_node: tree_sitter::Node<'_>,
    source: &str,
    bindings: &mut JavaReceiverTypeBindings,
) -> Result<()> {
    let Some(body) = type_node.child_by_field_name("body") else {
        return Ok(());
    };
    let mut cursor = body.walk();
    for field in body
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "field_declaration")
    {
        let Some(type_name) = java_declared_type_name(field, source)? else {
            continue;
        };
        let mut declarator_cursor = field.walk();
        for declarator in field.children_by_field_name("declarator", &mut declarator_cursor) {
            if let Some(name) = java_declared_name(declarator, source)? {
                insert_java_receiver_binding(bindings, name, type_name.clone());
            }
        }
    }
    Ok(())
}

fn collect_java_body_bindings(
    node: tree_sitter::Node<'_>,
    source: &str,
    bindings: &mut JavaReceiverTypeBindings,
) -> Result<()> {
    if matches!(
        node.kind(),
        "method_declaration"
            | "constructor_declaration"
            | "annotation_type_declaration"
            | "class_declaration"
            | "enum_declaration"
            | "interface_declaration"
            | "record_declaration"
    ) {
        return Ok(());
    }
    match node.kind() {
        "local_variable_declaration" => {
            let declared_type = java_declared_type_name(node, source)?;
            let mut cursor = node.walk();
            for declarator in node.children_by_field_name("declarator", &mut cursor) {
                if let Some(name) = java_declared_name(declarator, source)? {
                    // `var` locals have no usable declared type; infer the
                    // receiver type from a constructor initializer such as
                    // `var value = new Helper(...)`, or record a bare factory
                    // initializer such as `var value = makeFoo(...)` for
                    // trace-time resolution.
                    let type_name = match &declared_type {
                        Some(type_name) => type_name.clone(),
                        None => {
                            if let Some(type_name) =
                                java_constructor_type_from_declarator(declarator, source)?
                            {
                                type_name
                            } else if let Some((function_name, arity)) =
                                java_initializer_call_from_declarator(declarator, source)?
                            {
                                insert_java_initializer_call(bindings, &name, function_name, arity);
                                String::new()
                            } else {
                                String::new()
                            }
                        }
                    };
                    insert_java_receiver_binding(bindings, name, type_name);
                }
            }
        }
        "catch_formal_parameter"
        | "enhanced_for_statement"
        | "instanceof_expression"
        | "resource"
        | "type_pattern"
        | "record_pattern_component" => {
            if let Some(name) = java_declared_name(node, source)? {
                let type_name = java_declared_type_name(node, source)?.unwrap_or_default();
                insert_java_receiver_binding(bindings, name, type_name);
            }
        }
        "lambda_expression" => {
            collect_java_lambda_parameter_bindings(node, source, bindings)?;
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_java_body_bindings(child, source, bindings)?;
    }
    Ok(())
}

fn collect_java_lambda_parameter_bindings(
    lambda: tree_sitter::Node<'_>,
    source: &str,
    bindings: &mut JavaReceiverTypeBindings,
) -> Result<()> {
    let Some(parameters) = lambda.child_by_field_name("parameters") else {
        return Ok(());
    };
    match parameters.kind() {
        "identifier" | "_reserved_identifier" => {
            let name = node_text(parameters, source)?.trim().to_string();
            if !name.is_empty() {
                insert_java_receiver_binding(bindings, name, String::new());
            }
        }
        "inferred_parameters" => {
            let mut cursor = parameters.walk();
            for parameter in parameters.named_children(&mut cursor) {
                let name = node_text(parameter, source)?.trim().to_string();
                if !name.is_empty() {
                    insert_java_receiver_binding(bindings, name, String::new());
                }
            }
        }
        "formal_parameters" => {
            let mut cursor = parameters.walk();
            for parameter in parameters.named_children(&mut cursor) {
                if parameter.kind() == "formal_parameter"
                    && let Some((name, type_name)) = java_parameter_binding(parameter, source)?
                {
                    insert_java_receiver_binding(bindings, name, type_name);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn java_parameter_binding(
    parameter: tree_sitter::Node<'_>,
    source: &str,
) -> Result<Option<(String, String)>> {
    let Some(name) = java_declared_name(parameter, source)? else {
        return Ok(None);
    };
    let Some(type_name) = java_declared_type_name(parameter, source)? else {
        return Ok(None);
    };
    Ok(Some((name, type_name)))
}

/// Infers a receiver type for `var` locals whose initializer is a constructor
/// call such as `var value = new Helper(...)`. Non-constructor initializers,
/// array creations, and malformed type spellings return `None` and fail closed.
fn java_constructor_type_from_declarator(
    declarator: tree_sitter::Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    let Some(initializer) = declarator.child_by_field_name("value") else {
        return Ok(None);
    };
    if initializer.kind() != "object_creation_expression" {
        return Ok(None);
    }
    let Some(type_node) = initializer.child_by_field_name("type") else {
        return Ok(None);
    };
    let type_name = node_text(type_node, source)?;
    Ok(java_dotted_type_name(type_name))
}

/// Records a `var` local whose initializer is a bare method call such as
/// `var value = makeFoo(...)`. The callee name and call arity are stored for
/// trace-time factory resolution; qualified initializers such as
/// `Util.makeFoo(...)` fail closed like other non-constructor initializers.
fn java_initializer_call_from_declarator(
    declarator: tree_sitter::Node<'_>,
    source: &str,
) -> Result<Option<(String, usize)>> {
    let Some(initializer) = declarator.child_by_field_name("value") else {
        return Ok(None);
    };
    if initializer.kind() != "method_invocation"
        || initializer.child_by_field_name("object").is_some()
    {
        return Ok(None);
    }
    let Some(name_node) = initializer.child_by_field_name("name") else {
        return Ok(None);
    };
    let name = node_text(name_node, source)?.trim();
    if name.is_empty() {
        return Ok(None);
    }
    let Some(arguments) = initializer.child_by_field_name("arguments") else {
        return Ok(None);
    };
    let mut cursor = arguments.walk();
    let arity = arguments.named_children(&mut cursor).count();
    Ok(Some((name.to_string(), arity)))
}

fn java_declared_name(node: tree_sitter::Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(name_node) = node.child_by_field_name("name") else {
        return Ok(None);
    };
    let name = node_text(name_node, source)?.trim();
    Ok((!name.is_empty()).then(|| name.to_string()))
}

fn java_declared_type_name(node: tree_sitter::Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(type_node) = node.child_by_field_name("type") else {
        return Ok(None);
    };
    let type_name = node_text(type_node, source)?;
    Ok(java_dotted_type_name(type_name))
}

/// Extracts a named receiver type, allowing dotted qualified names such as
/// `Outer.Inner`. Generic, array, varargs, and otherwise complex spellings
/// still fail closed; empty or malformed dotted segments are rejected by the
/// receiver path resolver.
pub(crate) fn java_dotted_type_name(text: &str) -> Option<String> {
    let name = text.trim();
    if name.is_empty()
        || name.starts_with('.')
        || name.ends_with('.')
        || name.contains("..")
        || name.contains(['<', '(', '[', ':', ',', ' ', '?', '|', '&'])
        || matches!(
            name,
            "boolean"
                | "byte"
                | "char"
                | "short"
                | "int"
                | "long"
                | "float"
                | "double"
                | "void"
                | "var"
        )
    {
        return None;
    }
    Some(name.to_string())
}

/// Records the factory initializer for a bound `var` local. Repeated
/// declarations with different factories make the name ambiguous so
/// trace-time resolution fails closed.
fn insert_java_initializer_call(
    bindings: &mut JavaReceiverTypeBindings,
    name: &str,
    function_name: String,
    arity: usize,
) {
    if name.is_empty() || bindings.ambiguous_names.contains(name) {
        return;
    }
    match bindings.initializer_calls_by_name.get(name) {
        Some(existing) if *existing != (function_name.clone(), arity) => {
            bindings.initializer_calls_by_name.remove(name);
            bindings.ambiguous_names.insert(name.to_string());
        }
        Some(_) => {}
        None => {
            bindings
                .initializer_calls_by_name
                .insert(name.to_string(), (function_name, arity));
        }
    }
}

fn insert_java_receiver_binding(
    bindings: &mut JavaReceiverTypeBindings,
    name: String,
    type_name: String,
) {
    if name.is_empty() {
        return;
    }
    if bindings.ambiguous_names.contains(&name) {
        return;
    }
    match bindings.types_by_name.get(&name) {
        Some(existing) if *existing != type_name => {
            bindings.types_by_name.remove(&name);
            bindings.ambiguous_names.insert(name);
        }
        Some(_) => {}
        None => {
            bindings.types_by_name.insert(name, type_name);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{
        JavaReceiverTypeBindings, java_import_context_for_file_with_overrides_and_deadline,
        java_receiver_type_bindings_for_function,
    };
    use crate::language::normalize_path;

    static NEXT_JAVA_TEST_ID: AtomicUsize = AtomicUsize::new(0);

    struct TestFile {
        normalized_path: String,
    }

    fn write_test_file(source: &str) -> TestFile {
        let id = NEXT_JAVA_TEST_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("arborist-java-binding-test-{id}"));
        std::fs::create_dir_all(&dir).unwrap();
        let path: PathBuf = dir.join("Types.java");
        std::fs::write(&path, source).unwrap();
        TestFile {
            normalized_path: normalize_path(&path),
        }
    }

    #[test]
    fn binds_parameter_local_and_field_receiver_types() {
        let file = write_test_file(
            "package com.example;
class Helper { int helper(int value) { return value; } }
class Holder {
    private Helper fieldHelper;
    int run(Helper param, int primitive) {
        Helper local = new Helper();
        var inferred = other();
        return param.helper(1) + local.helper(2) + fieldHelper.helper(3);
    }
}
",
        );
        let context = java_import_context_for_file_with_overrides_and_deadline(
            &file.normalized_path,
            None,
            None,
        )
        .unwrap();
        let run_bindings = context
            .receiver_type_bindings_by_range
            .values()
            .find(|bindings| bindings.type_for("param") == Some("Helper".to_string()))
            .unwrap();
        assert_eq!(run_bindings.type_for("param"), Some("Helper".to_string()));
        assert_eq!(run_bindings.type_for("local"), Some("Helper".to_string()));
        assert_eq!(
            run_bindings.type_for("fieldHelper"),
            Some("Helper".to_string())
        );
        // `var` locals and primitive parameters have no usable class type.
        assert_eq!(run_bindings.type_for("inferred"), None);
        assert_eq!(run_bindings.type_for("primitive"), None);
        assert!(run_bindings.contains("inferred"));
    }

    #[test]
    fn var_locals_infer_receiver_types_from_constructor_initializers() {
        let file = write_test_file(
            "package com.example;
class Helper { int helper(int value) { return value; } }
class Outer { static class Inner { int helper(int value) { return value; } } }
class Caller {
    int run() {
        var first = new Helper();
        var nested = new Outer.Inner();
        var factory = makeHelper();
        var array = new int[3];
        return first.helper(1) + nested.helper(2) + factory.helper(3);
    }
}
",
        );
        let context = java_import_context_for_file_with_overrides_and_deadline(
            &file.normalized_path,
            None,
            None,
        )
        .unwrap();
        let run_bindings = context
            .receiver_type_bindings_by_range
            .values()
            .find(|bindings| bindings.type_for("first") == Some("Helper".to_string()))
            .unwrap();
        assert_eq!(run_bindings.type_for("first"), Some("Helper".to_string()));
        assert_eq!(
            run_bindings.type_for("nested"),
            Some("Outer.Inner".to_string())
        );
        // Bare method-call initializers record a factory binding for
        // trace-time resolution; array initializers have no inferred type.
        assert!(run_bindings.contains("factory"));
        assert_eq!(run_bindings.type_for("factory"), None);
        assert_eq!(
            run_bindings.initializer_call_for("factory"),
            Some(("makeHelper".to_string(), 0))
        );
        assert!(run_bindings.contains("array"));
        assert_eq!(run_bindings.type_for("array"), None);
        assert_eq!(run_bindings.initializer_call_for("array"), None);
    }

    #[test]
    fn var_locals_record_factory_initializer_calls_and_reject_qualified_or_conflicting_ones() {
        let file = write_test_file(
            "package com.example;
class Helper { int helper(int value) { return value; } }
class Util {
    static Helper qualifiedFactory() { return new Helper(); }
}
class Caller {
    int run(int value) {
        var bare = makeHelper(value);
        var qualified = Util.qualifiedFactory();
        var array = new int[3];
        return bare.helper(1) + qualified.helper(2) + array.helper(3);
    }
    int shadowed() {
        var factory = makeHelper(1);
        var factory = makeOther();
        return factory.helper(1);
    }
}
",
        );
        let context = java_import_context_for_file_with_overrides_and_deadline(
            &file.normalized_path,
            None,
            None,
        )
        .unwrap();
        let run_bindings = context
            .receiver_type_bindings_by_range
            .values()
            .find(|bindings| {
                bindings.initializer_call_for("bare") == Some(("makeHelper".to_string(), 1))
            })
            .unwrap();
        assert_eq!(
            run_bindings.initializer_call_for("bare"),
            Some(("makeHelper".to_string(), 1))
        );
        // Qualified and array initializers record no factory binding.
        assert!(run_bindings.contains("qualified"));
        assert_eq!(run_bindings.initializer_call_for("qualified"), None);
        assert!(run_bindings.contains("array"));
        assert_eq!(run_bindings.initializer_call_for("array"), None);
        // Conflicting factory declarations make the name ambiguous.
        let shadowed_bindings = context
            .receiver_type_bindings_by_range
            .values()
            .find(|bindings| bindings.contains("factory"))
            .unwrap();
        assert!(shadowed_bindings.ambiguous_names.contains("factory"));
        assert_eq!(shadowed_bindings.type_for("factory"), None);
        assert_eq!(shadowed_bindings.initializer_call_for("factory"), None);
    }

    #[test]
    fn rejects_ambiguous_and_untyped_receiver_bindings() {
        let file = write_test_file(
            "package com.example;
class Helper { static int run(int value) { return value; } }
class Other { int run(int value) { return value; } }
class Caller {
    int run(boolean flag) {
        Helper local = new Helper();
        Other local = new Other();
        java.util.function.IntFunction<Integer> function = value -> value.run(1);
        return local.run(1) + function.apply(0);
    }
}
",
        );
        let context = java_import_context_for_file_with_overrides_and_deadline(
            &file.normalized_path,
            None,
            None,
        )
        .unwrap();
        let run_bindings = context
            .receiver_type_bindings_by_range
            .values()
            .find(|bindings| bindings.contains("local"))
            .unwrap();
        // Duplicate locals are ambiguous, and lambda parameters without an
        // explicit type are bound without a usable type; both fail closed.
        assert!(run_bindings.contains("local"));
        assert_eq!(run_bindings.type_for("local"), None);
        assert!(run_bindings.contains("value"));
        assert_eq!(run_bindings.type_for("value"), None);
    }

    #[test]
    fn receiver_bindings_are_keyed_by_function_byte_range() {
        let file = write_test_file(
            "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int first(Helper helper) { return helper.helper(1); }
    int second() { return 0; }
}
",
        );
        let context = java_import_context_for_file_with_overrides_and_deadline(
            &file.normalized_path,
            None,
            None,
        )
        .unwrap();
        assert_eq!(context.receiver_type_bindings_by_range.len(), 3);
        let first_range = *context
            .receiver_type_bindings_by_range
            .iter()
            .find(|(_, bindings)| bindings.type_for("helper") == Some("Helper".to_string()))
            .map(|(range, _)| range)
            .unwrap();
        let mut contexts = BTreeMap::new();
        let fetched = java_receiver_type_bindings_for_function(
            &file.normalized_path,
            first_range,
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(fetched.type_for("helper"), Some("Helper".to_string()));
    }

    #[test]
    fn receiver_binding_type_for_returns_none_for_unknown_names() {
        let bindings = JavaReceiverTypeBindings::default();
        assert_eq!(bindings.type_for("missing"), None);
        assert!(!bindings.contains("missing"));
    }
}
