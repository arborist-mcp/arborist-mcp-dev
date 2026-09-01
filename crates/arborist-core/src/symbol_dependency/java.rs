use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;

use crate::language::{
    detect_language, java_local_explicit_static_member_imports, java_local_explicit_type_imports,
    node_text, normalize_path, parse_document, parse_document_with_timeout, read_source,
    validate_source_length,
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
    array_component_types: BTreeMap<String, String>,
    ambiguous_names: BTreeSet<String>,
    initializer_calls_by_name: BTreeMap<String, (String, usize)>,
    field_initializers_by_name: BTreeMap<String, String>,
    element_access_initializers_by_name: BTreeMap<String, (String, usize)>,
}

impl JavaReceiverTypeBindings {
    /// Returns whether `name` is bound locally, including as an ambiguous
    /// binding. Callers use this to distinguish "not bound" (a receiver may be
    /// a same-named type instead) from "bound but unusable" (fail closed).
    pub(in crate::symbol_dependency) fn contains(&self, name: &str) -> bool {
        self.types_by_name.contains_key(name)
            || self.array_component_types.contains_key(name)
            || self.element_access_initializers_by_name.contains_key(name)
            || self.ambiguous_names.contains(name)
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

    /// Returns the declared component type for a uniquely bound array-typed
    /// name such as Helper[] items, which resolves to the element type
    /// Helper when the chain accesses an element. Ambiguous bindings and
    /// names without a resolvable array component return None.
    pub(in crate::symbol_dependency) fn array_component_for(&self, name: &str) -> Option<String> {
        if self.ambiguous_names.contains(name) {
            return None;
        }
        self.array_component_types
            .get(name)
            .filter(|type_name| !type_name.is_empty())
            .cloned()
    }

    /// Returns the method-call initializer reference and call arity for a `var`
    /// local bound from a bare or qualified call such as `var value = makeFoo(...)`
    /// or `var value = group.makeFoo(...)`. Ambiguous bindings and names without a
    /// method-call initializer return `None`.
    pub(in crate::symbol_dependency) fn initializer_call_for(
        &self,
        name: &str,
    ) -> Option<(String, usize)> {
        if self.ambiguous_names.contains(name) {
            return None;
        }
        self.initializer_calls_by_name.get(name).cloned()
    }

    /// Returns the field-access initializer reference for a `var` local bound
    /// from a value reference such as `var value = this.helper;`,
    /// `var value = helper;`, `var value = Util.STATIC_HELPER;`, or a
    /// statically imported field name. Ambiguous bindings and names without a
    /// field-access initializer return `None`.
    pub(in crate::symbol_dependency) fn initializer_field_for(&self, name: &str) -> Option<String> {
        if self.ambiguous_names.contains(name) {
            return None;
        }
        self.field_initializers_by_name.get(name).cloned()
    }

    /// Returns the array-typed base spelling and call arity for a `var` local
    /// bound from an element-access initializer such as `var first = items[0]`
    /// or `var first = makeItems()[0]`, which resolves to the base array's
    /// element component type; factory-call bases carry the call's argument
    /// count. Ambiguous bindings and names without an element-access
    /// initializer return `None`.
    pub(in crate::symbol_dependency) fn element_access_base_for(
        &self,
        name: &str,
    ) -> Option<(String, usize)> {
        if self.ambiguous_names.contains(name) {
            return None;
        }
        self.element_access_initializers_by_name.get(name).cloned()
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
    validate_source_length(path, source.len())?;
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
                            } else if let Some(reference) =
                                java_initializer_field_access_from_declarator(declarator, source)?
                            {
                                insert_java_initializer_field(bindings, &name, reference);
                                String::new()
                            } else if let Some((base_spelling, call_arity)) =
                                java_initializer_element_access_from_declarator(declarator, source)?
                            {
                                insert_java_element_access_initializer(
                                    bindings,
                                    &name,
                                    base_spelling,
                                    call_arity,
                                );
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
/// Returns the inner expression of a parenthesized initializer such as
/// `(new Helper())`, `(makeFoo())`, or `(this.helper)`, so `var` locals with
/// parenthesized initializers bind the same receiver type as the
/// unparenthesized form. Malformed or empty parentheses return `None` and
/// fail closed.
fn java_parenthesized_initializer_expression(
    mut initializer: tree_sitter::Node<'_>,
) -> Option<tree_sitter::Node<'_>> {
    loop {
        if initializer.kind() != "parenthesized_expression" {
            return Some(initializer);
        }
        initializer = initializer.named_child(0)?;
    }
}

fn java_constructor_type_from_declarator(
    declarator: tree_sitter::Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    let Some(initializer) = declarator.child_by_field_name("value") else {
        return Ok(None);
    };
    let Some(initializer) = java_parenthesized_initializer_expression(initializer) else {
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

/// Records a `var` local's method-call initializer as a reference spelling:
/// bare calls such as `var value = makeFoo(...)` record `makeFoo`, while
/// qualified calls such as `var value = group.makeFoo(...)`,
/// `var value = new Group().makeFoo(...)`, or `var value = group.inner().makeFoo(...)`
/// record the receiver spelling plus the method name. The call arity is
/// recorded so trace-time resolution can require a non-varargs arity match.
fn java_initializer_call_from_declarator(
    declarator: tree_sitter::Node<'_>,
    source: &str,
) -> Result<Option<(String, usize)>> {
    let Some(initializer) = declarator.child_by_field_name("value") else {
        return Ok(None);
    };
    let Some(initializer) = java_parenthesized_initializer_expression(initializer) else {
        return Ok(None);
    };
    if initializer.kind() != "method_invocation" {
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
    let reference = match initializer.child_by_field_name("object") {
        None => name.to_string(),
        Some(object) => {
            let object_text = node_text(object, source)?.trim();
            if object_text.is_empty() {
                return Ok(None);
            }
            format!("{object_text}.{name}")
        }
    };
    Ok(Some((reference, arity)))
}

/// Records a `var` local whose initializer is a field-access value reference
/// such as `var value = this.helper;`, `var value = helper;`,
/// `var value = Util.STATIC_HELPER;`, a bare statically imported field name,
/// or an anonymous constructor-rooted chain such as
/// `var value = new Group() { }.entry` or
/// `var value = new Group() { }.inner2().entry`. Bare identifiers and
/// `field_access` expressions record the reference spelling for trace-time
/// field resolution; anonymous constructor roots canonicalize to
/// `new Group().entry` (or the equivalent chain) so the resolver dispatches on
/// the constructed class type, unless the anonymous body declares any accessed
/// field or method-call hop that would shadow the constructed type's member.
/// All other initializers record nothing and fail closed.
fn java_initializer_field_access_from_declarator(
    declarator: tree_sitter::Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    let Some(initializer) = declarator.child_by_field_name("value") else {
        return Ok(None);
    };
    let Some(initializer) = java_parenthesized_initializer_expression(initializer) else {
        return Ok(None);
    };
    let reference = match initializer.kind() {
        "field_access" => {
            let Some(object) = initializer.child_by_field_name("object") else {
                return Ok(None);
            };
            let Some(field) = initializer.child_by_field_name("field") else {
                return Ok(None);
            };
            let object_text = node_text(object, source)?.trim();
            let field_text = node_text(field, source)?.trim();
            if object_text.is_empty() || field_text.is_empty() {
                return Ok(None);
            }
            let root = java_expression_root_object(initializer);
            if root.kind() == "object_creation_expression" && java_has_anonymous_body(root) {
                // Anonymous constructor-rooted chains canonicalize so the
                // resolver dispatches on the constructed class type; a body
                // declaration of any accessed field or hop, a non-zero-argument
                // hop, or an unusable constructed type fails closed instead of
                // recording a non-resolving raw spelling.
                match java_anonymous_constructor_initializer_spelling(initializer, source)? {
                    Some(spelling) => spelling,
                    None => return Ok(None),
                }
            } else {
                format!("{object_text}.{field_text}")
            }
        }
        "identifier" => {
            let text = node_text(initializer, source)?.trim();
            if text.is_empty() {
                return Ok(None);
            }
            text.to_string()
        }
        _ => return Ok(None),
    };
    Ok(Some(reference))
}

/// Records a `var` local whose initializer is an element access such as
/// `var first = items[0]`, returning the array-typed base spelling and call
/// arity so the local resolves to the base array's element component type.
/// Plain-identifier bases such as `items`, `local`, or a bare enclosing-class
/// field name and field-access bases such as `this.fieldItems` or
/// `group.holder.fieldItems` record the spelling with arity zero; factory-call
/// bases such as `makeItems()` or `Util.makeItems()` record the reference with
/// a trailing `()` marker and the call's argument count. Multi-dimensional
/// element access and other initializer shapes record nothing and fail closed.
fn java_initializer_element_access_from_declarator(
    declarator: tree_sitter::Node<'_>,
    source: &str,
) -> Result<Option<(String, usize)>> {
    let Some(initializer) = declarator.child_by_field_name("value") else {
        return Ok(None);
    };
    let Some(initializer) = java_parenthesized_initializer_expression(initializer) else {
        return Ok(None);
    };
    if initializer.kind() != "array_access" {
        return Ok(None);
    }
    let Some(array) = initializer.child_by_field_name("array") else {
        return Ok(None);
    };
    let (base_spelling, call_arity) = match array.kind() {
        "identifier" | "field_access" => {
            let base_name = node_text(array, source)?.trim();
            if base_name.is_empty() {
                return Ok(None);
            }
            (base_name.to_string(), 0)
        }
        "method_invocation" => {
            let Some(name_node) = array.child_by_field_name("name") else {
                return Ok(None);
            };
            let name = node_text(name_node, source)?.trim();
            if name.is_empty() {
                return Ok(None);
            }
            let Some(arguments) = array.child_by_field_name("arguments") else {
                return Ok(None);
            };
            let mut cursor = arguments.walk();
            let arity = arguments.named_children(&mut cursor).count();
            let reference = match array.child_by_field_name("object") {
                None => name.to_string(),
                Some(object) => {
                    let object_text = node_text(object, source)?.trim();
                    if object_text.is_empty() {
                        return Ok(None);
                    }
                    format!("{object_text}.{name}")
                }
            };
            (format!("{reference}()"), arity)
        }
        _ => return Ok(None),
    };
    Ok(Some((base_spelling, call_arity)))
}

/// Walks a field-access or method-invocation chain down to its root object
/// expression, mirroring the receiver hops the resolver walks at trace time.
fn java_expression_root_object(mut node: tree_sitter::Node<'_>) -> tree_sitter::Node<'_> {
    loop {
        match node.kind() {
            "field_access" => {
                let Some(object) = node.child_by_field_name("object") else {
                    return node;
                };
                node = object;
            }
            "method_invocation" => {
                let Some(object) = node.child_by_field_name("object") else {
                    return node;
                };
                node = object;
            }
            _ => return node,
        }
    }
}

/// Canonicalizes an anonymous constructor-rooted field-access chain such as
/// `new Group() { }.entry`, `new Group() { }.holder.entry`, or
/// `new Group() { }.inner2().entry` or `new Group() { }.inner2(1).entry` to
/// `new Group().entry`, `new Group().holder.entry`, `new Group().inner2().entry`,
/// or `new Group().inner2(1).entry` so the resolver dispatches on the
/// constructed class type. Method-call hops encode their argument count so the
/// resolver can require an arity match. Chains rooted at an anonymous
/// constructor resolve only when the anonymous body declares none of the
/// accessed fields or method-call hops; malformed intermediate expressions and
/// unusable constructed type spellings fail closed.
fn java_anonymous_constructor_initializer_spelling(
    initializer: tree_sitter::Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    let mut segments = Vec::new();
    let mut current = initializer;
    loop {
        if current.kind() == "field_access" {
            let Some(field) = current.child_by_field_name("field") else {
                return Ok(None);
            };
            let field_name = node_text(field, source)?.trim();
            if field_name.is_empty() {
                return Ok(None);
            }
            segments.push(field_name.to_string());
            let Some(object) = current.child_by_field_name("object") else {
                return Ok(None);
            };
            current = object;
        } else if current.kind() == "method_invocation" {
            let Some(name_node) = current.child_by_field_name("name") else {
                return Ok(None);
            };
            let method_name = node_text(name_node, source)?.trim();
            let Some(arguments) = current.child_by_field_name("arguments") else {
                return Ok(None);
            };
            let mut cursor = arguments.walk();
            if method_name.is_empty() {
                return Ok(None);
            }
            let hop_arity = arguments.named_children(&mut cursor).count();
            if hop_arity == 0 {
                segments.push(format!("{method_name}()"));
            } else {
                segments.push(format!("{method_name}({hop_arity})"));
            }
            let Some(object) = current.child_by_field_name("object") else {
                return Ok(None);
            };
            current = object;
        } else if current.kind() == "object_creation_expression" {
            if !java_has_anonymous_body(current) {
                return Ok(None);
            }
            if java_anonymous_constructor_declared_members(current, source)?
                .iter()
                .any(|declared| {
                    segments
                        .iter()
                        .any(|segment| segment.split('(').next() == Some(declared.as_str()))
                })
            {
                return Ok(None);
            }
            let Some(type_node) = current.child_by_field_name("type") else {
                return Ok(None);
            };
            let type_name = node_text(type_node, source)?.trim();
            let Some(type_name) = java_dotted_type_name(type_name) else {
                return Ok(None);
            };
            segments.push(format!("new {type_name}()"));
            break;
        } else {
            return Ok(None);
        }
    }
    segments.reverse();
    Ok(Some(segments.join(".")))
}

/// Returns whether an `object_creation_expression` carries an anonymous-class
/// body.
fn java_has_anonymous_body(node: tree_sitter::Node<'_>) -> bool {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| child.kind() == "class_body")
}

/// Returns the names of fields and methods declared directly in an
/// anonymous-class body on `node`. Non-anonymous nodes return an empty set.
/// Anonymous `var` field-initializer chains resolve on the constructed class
/// type only when the body declares none of the accessed fields or method-call
/// hops; a same-name body declaration would shadow the constructed type's
/// member, so it fails closed conservatively.
fn java_anonymous_constructor_declared_members(
    node: tree_sitter::Node<'_>,
    source: &str,
) -> Result<BTreeSet<String>> {
    let mut declared = BTreeSet::new();
    if !java_has_anonymous_body(node) {
        return Ok(declared);
    }
    let mut cursor = node.walk();
    let Some(body) = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == "class_body")
    else {
        return Ok(declared);
    };
    let mut body_cursor = body.walk();
    for child in body.named_children(&mut body_cursor) {
        match child.kind() {
            "field_declaration" => {
                let mut declarator_cursor = child.walk();
                for declarator in child.children_by_field_name("declarator", &mut declarator_cursor)
                {
                    if let Some(name_node) = declarator.child_by_field_name("name") {
                        let name = node_text(name_node, source)?.trim();
                        if !name.is_empty() {
                            declared.insert(name.to_string());
                        }
                    }
                }
            }
            "method_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = node_text(name_node, source)?.trim();
                    if !name.is_empty() {
                        declared.insert(name.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    Ok(declared)
}

fn java_declared_name(node: tree_sitter::Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(name_node) = node.child_by_field_name("name") else {
        return Ok(None);
    };
    let name = node_text(name_node, source)?.trim();
    Ok((!name.is_empty()).then(|| name.to_string()))
}

/// Returns the declared type name for a binding node. Named and generic types
/// normalize through java_dotted_type_name; single-level array spellings
/// such as Helper[] or Box<String>[] are kept as their raw spelling so
/// receiver bindings can record the array component type. Primitives,
/// multi-dimensional arrays, and malformed spellings return None and fail
/// closed.
fn java_declared_type_name(node: tree_sitter::Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(type_node) = node.child_by_field_name("type") else {
        return Ok(None);
    };
    let type_name = node_text(type_node, source)?;
    let trimmed = type_name.trim();
    if let Some(normalized) = java_dotted_type_name(trimmed) {
        return Ok(Some(normalized));
    }
    Ok(java_array_type_component_name(trimmed).map(|_| trimmed.to_string()))
}

/// Extracts a named receiver type, allowing dotted qualified names such as
/// `Outer.Inner` and generic spellings such as `Box<String>` or
/// `java.util.List<? extends Number>` normalized to their raw base name
/// without type-argument selection, mirroring generic superclass
/// normalization. Arrays, varargs, primitives, malformed or empty generic
/// arguments, and otherwise complex spellings still fail closed; empty or
/// malformed dotted segments are rejected by the receiver path resolver.
/// Extracts the component type name from a single-level array spelling such
/// as Helper[], Helper [], or Box<String>[], normalizing the component
/// through java_dotted_type_name. Multi-dimensional arrays, primitives,
/// malformed brackets, and other complex spellings return None and fail
/// closed; the caller treats those bindings as unusable.
pub(crate) fn java_array_type_component_name(text: &str) -> Option<String> {
    let name = text.trim();
    let open = name.find('[')?;
    if open == 0 || !name[open..].trim_end().ends_with(']') {
        return None;
    }
    let bracket = name[open..].trim();
    if bracket != "[]" {
        return None;
    }
    java_dotted_type_name(name[..open].trim())
}
pub(crate) fn java_dotted_type_name(text: &str) -> Option<String> {
    let name = text.trim();
    if name.contains('<') {
        return java_generic_type_base_name(name);
    }
    java_dotted_type_name_base(name)
}

fn java_dotted_type_name_base(name: &str) -> Option<String> {
    if name.is_empty()
        || name.starts_with('.')
        || name.ends_with('.')
        || name.contains("..")
        || name.contains(['(', '[', ':', ',', ' ', '?', '|', '&'])
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

/// Strips a well-formed top-level type-argument list from a generic type
/// spelling, returning the raw dotted base name. The argument list must be
/// balanced and non-empty, and nothing may follow its closing bracket;
/// malformed, array, and otherwise complex spellings fail closed.
fn java_generic_type_base_name(name: &str) -> Option<String> {
    let open = name.find('<')?;
    let prefix = name[..open].trim();
    java_dotted_type_name_base(prefix)?;
    let suffix_len = name.len() - open;
    let mut depth = 0usize;
    let mut has_argument = false;
    for (offset, byte) in name[open..].bytes().enumerate() {
        match byte {
            b'<' => depth += 1,
            b'>' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                if depth == 0 && offset != suffix_len - 1 {
                    return None;
                }
            }
            _ => {
                if depth == 0 {
                    return None;
                }
                if !byte.is_ascii_whitespace() {
                    has_argument = true;
                }
            }
        }
    }
    (depth == 0 && has_argument).then(|| prefix.to_string())
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

fn insert_java_initializer_field(
    bindings: &mut JavaReceiverTypeBindings,
    name: &str,
    reference: String,
) {
    if name.is_empty() || bindings.ambiguous_names.contains(name) {
        return;
    }
    match bindings.field_initializers_by_name.get(name) {
        Some(existing) if *existing != reference => {
            bindings.field_initializers_by_name.remove(name);
            bindings.ambiguous_names.insert(name.to_string());
        }
        Some(_) => {}
        None => {
            bindings
                .field_initializers_by_name
                .insert(name.to_string(), reference);
        }
    }
}

/// Records the element-access initializer base and call arity for a `var`
/// local bound from an element access such as `var first = items[0]` or
/// `var first = makeItems()[0]`. Repeated declarations with different bases
/// make the name ambiguous so trace-time resolution fails closed.
fn insert_java_element_access_initializer(
    bindings: &mut JavaReceiverTypeBindings,
    name: &str,
    base_spelling: String,
    call_arity: usize,
) {
    if name.is_empty() || bindings.ambiguous_names.contains(name) {
        return;
    }
    match bindings.element_access_initializers_by_name.get(name) {
        Some(existing) if *existing != (base_spelling.clone(), call_arity) => {
            bindings.element_access_initializers_by_name.remove(name);
            bindings.ambiguous_names.insert(name.to_string());
        }
        Some(_) => {}
        None => {
            bindings
                .element_access_initializers_by_name
                .insert(name.to_string(), (base_spelling, call_arity));
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
    // Single-level array spellings bind the element component type so an
    // element-access receiver such as items[0] can dispatch on the element
    // type; primitive, multi-dimensional, and malformed array spellings have
    // no usable component and bind as an empty (unusable) type instead of
    // falling through to a same-named type call.
    if type_name.contains('[') {
        match java_array_type_component_name(&type_name) {
            Some(component) => match bindings.array_component_types.get(&name) {
                Some(existing) if *existing != component => {
                    bindings.array_component_types.remove(&name);
                    bindings.ambiguous_names.insert(name);
                }
                Some(_) => {}
                None => {
                    bindings.array_component_types.insert(name, component);
                }
            },
            None => {
                bindings.types_by_name.insert(name, String::new());
            }
        }
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
        JavaReceiverTypeBindings, java_dotted_type_name,
        java_import_context_for_file_with_overrides_and_deadline,
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
    fn var_locals_record_factory_initializer_calls_and_reject_array_or_conflicting_ones() {
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
        // Qualified method-call initializers record the receiver spelling
        // plus callee name; array initializers record no factory binding.
        assert!(run_bindings.contains("qualified"));
        assert_eq!(
            run_bindings.initializer_call_for("qualified"),
            Some(("Util.qualifiedFactory".to_string(), 0))
        );
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
    fn var_locals_unwrap_parenthesized_initializers() {
        let file = write_test_file(
            "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    private Helper fieldHelper = new Helper();
    int run() {
        var constructed = (new Helper());
        var factory = (makeHelper());
        var field = (this.fieldHelper);
        var bareField = (fieldHelper);
        var array = (new int[3]);
        return constructed.helper(1) + factory.helper(2) + field.helper(3);
    }
    Helper makeHelper() { return new Helper(); }
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
            .find(|bindings| bindings.type_for("constructed") == Some("Helper".to_string()))
            .unwrap();
        // Parenthesized constructor, factory, and field-access initializers
        // bind the same receiver type as their unparenthesized forms.
        assert_eq!(
            run_bindings.type_for("constructed"),
            Some("Helper".to_string())
        );
        assert!(run_bindings.contains("factory"));
        assert_eq!(run_bindings.type_for("factory"), None);
        assert_eq!(
            run_bindings.initializer_call_for("factory"),
            Some(("makeHelper".to_string(), 0))
        );
        assert_eq!(
            run_bindings.initializer_field_for("field"),
            Some("this.fieldHelper".to_string())
        );
        assert_eq!(
            run_bindings.initializer_field_for("bareField"),
            Some("fieldHelper".to_string())
        );
        // A parenthesized array creation still has no usable receiver type.
        assert!(run_bindings.contains("array"));
        assert_eq!(run_bindings.type_for("array"), None);
        assert_eq!(run_bindings.initializer_call_for("array"), None);
    }

    #[test]
    fn array_typed_receivers_bind_element_component_types() {
        let file = write_test_file(
            "package com.example;
class Helper { int helper(int value) { return value; } }
class Other { int helper(int value) { return value; } }
class Caller {
    private Helper[] fieldItems;
    int run(Helper[] param, Helper[][] matrix, int[] counts) {
        Helper[] local = new Helper[3];
        Helper[] created = new Helper[2];
        return param.helper(1) + local.helper(2) + fieldItems.helper(3);
    }
    int primitive(Helper[] plain, int[] numbers) { return numbers.length; }
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
            .find(|bindings| bindings.array_component_for("param") == Some("Helper".to_string()))
            .unwrap();
        // Parameters, locals, and enclosing fields bind the array element
        // component type instead of a plain receiver type.
        for name in ["param", "local", "created", "fieldItems"] {
            assert_eq!(
                run_bindings.array_component_for(name),
                Some("Helper".to_string()),
                "{name} must bind the array element component type"
            );
            assert!(run_bindings.contains(name), "{name} must be bound");
            assert_eq!(
                run_bindings.type_for(name),
                None,
                "{name} has no plain type"
            );
        }
        // Multi-dimensional arrays and primitive arrays have no usable
        // component type, so array parameters are not bound at all and fail
        // closed at trace time.
        assert_eq!(run_bindings.array_component_for("matrix"), None);
        assert!(!run_bindings.contains("matrix"));
        assert_eq!(run_bindings.type_for("matrix"), None);
        assert_eq!(run_bindings.array_component_for("counts"), None);
        assert!(!run_bindings.contains("counts"));
        assert_eq!(run_bindings.type_for("counts"), None);
    }

    #[test]
    fn var_element_access_initializers_bind_array_bases() {
        let file = write_test_file(
            "package com.example;
class Helper { int helper(int value) { return value; } }
class Group { Helper[] fieldItems; }
class Util { static Helper[] makeItems() { return new Helper[2]; } }
class Caller {
    private Helper[] fieldItems;
    Helper[] makeItems() { return new Helper[2]; }
    Helper[] makeItems(int value) { return new Helper[2]; }
    int run(Helper[] items, int[] counts, Helper[][] matrix, Group holder) {
        Helper[] local = new Helper[3];
        var first = items[0];
        var second = local[1];
        var third = fieldItems[0];
        var unbound = counts[0];
        var matrixAccess = matrix[0][0];
        var qualified = this.fieldItems[0];
        var fifth = holder.fieldItems[0];
        var factory = makeItems()[0];
        var factoryArity = makeItems(1)[0];
        var qualifiedFactory = Util.makeItems()[0];
        return first.helper(1) + second.helper(2) + third.helper(3);
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
                bindings.element_access_base_for("first") == Some(("items".to_string(), 0))
            })
            .unwrap();
        // Plain-identifier element-access initializers record the base name,
        // which resolves to the base array's element component type.
        for (name, base) in [
            ("first", "items"),
            ("second", "local"),
            ("third", "fieldItems"),
        ] {
            assert_eq!(
                run_bindings.element_access_base_for(name),
                Some((base.to_string(), 0)),
                "{name} must record its element-access base"
            );
            assert!(run_bindings.contains(name), "{name} must be bound");
            assert_eq!(
                run_bindings.type_for(name),
                None,
                "{name} has no plain type"
            );
        }
        // A primitive-array base records the spelling but has no resolvable
        // component, so resolution fails closed at trace time.
        assert_eq!(
            run_bindings.element_access_base_for("unbound"),
            Some(("counts".to_string(), 0))
        );
        assert!(run_bindings.contains("unbound"));
        assert_eq!(run_bindings.array_component_for("counts"), None);
        // `this`-rooted and bound-receiver field-access bases record the full
        // base spelling so trace-time resolution can walk the field chain.
        for (name, base) in [
            ("qualified", "this.fieldItems"),
            ("fifth", "holder.fieldItems"),
        ] {
            assert_eq!(
                run_bindings.element_access_base_for(name),
                Some((base.to_string(), 0)),
                "{name} must record its qualified element-access base"
            );
            assert!(run_bindings.contains(name), "{name} must be bound");
            assert_eq!(run_bindings.type_for(name), None);
        }
        // Factory-call bases record the reference with a trailing `()` marker
        // and the call's argument count so trace-time resolution can require
        // an arity match.
        for (name, base, arity) in [
            ("factory", "makeItems()", 0usize),
            ("factoryArity", "makeItems()", 1usize),
            ("qualifiedFactory", "Util.makeItems()", 0usize),
        ] {
            assert_eq!(
                run_bindings.element_access_base_for(name),
                Some((base.to_string(), arity)),
                "{name} must record its factory-call element-access base"
            );
            assert!(run_bindings.contains(name), "{name} must be bound");
            assert_eq!(run_bindings.type_for(name), None);
        }
        // Multi-dimensional element access has no element-access initializer
        // and stays bound as an unusable type.
        assert_eq!(run_bindings.element_access_base_for("matrixAccess"), None);
        assert!(run_bindings.contains("matrixAccess"));
        assert_eq!(run_bindings.type_for("matrixAccess"), None);
    }

    #[test]
    fn var_element_access_initializers_reject_conflicting_bases() {
        let file = write_test_file(
            "package com.example;
class Helper { int helper(int value) { return value; } }
class Other { int helper(int value) { return value; } }
class Caller {
    int run() {
        Helper[] first = new Helper[1];
        Other[] second = new Other[1];
        var shadowed = first[0];
        var shadowed = second[0];
        var plain = first[0];
        return shadowed.helper(1) + plain.helper(2);
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
                bindings.element_access_base_for("plain") == Some(("first".to_string(), 0))
            })
            .unwrap();
        assert_eq!(
            run_bindings.element_access_base_for("plain"),
            Some(("first".to_string(), 0))
        );
        // Conflicting element-access initializers make the name ambiguous.
        assert!(run_bindings.contains("shadowed"));
        assert_eq!(run_bindings.element_access_base_for("shadowed"), None);
        assert_eq!(run_bindings.type_for("shadowed"), None);
    }

    #[test]
    fn array_receiver_bindings_reject_conflicting_components() {
        let file = write_test_file(
            "package com.example;
class Helper { int helper(int value) { return value; } }
class Other { int helper(int value) { return value; } }
class Caller {
    int run() {
        Helper[] first = new Helper[1];
        Other[] second = new Other[1];
        Helper[] shadowed = new Helper[1];
        Other[] shadowed = new Other[1];
        return first.helper(1) + second.helper(2) + shadowed.helper(3);
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
            .find(|bindings| bindings.array_component_for("first") == Some("Helper".to_string()))
            .unwrap();
        assert_eq!(
            run_bindings.array_component_for("first"),
            Some("Helper".to_string())
        );
        assert_eq!(
            run_bindings.array_component_for("second"),
            Some("Other".to_string())
        );
        // Conflicting array declarations make the name ambiguous.
        assert!(run_bindings.contains("shadowed"));
        assert_eq!(run_bindings.array_component_for("shadowed"), None);
        assert_eq!(run_bindings.type_for("shadowed"), None);
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

    #[test]
    fn java_dotted_type_name_normalizes_generic_receiver_types() {
        assert_eq!(
            java_dotted_type_name("Box<String>"),
            Some("Box".to_string())
        );
        assert_eq!(
            java_dotted_type_name("java.util.Map<String, List<Integer>>"),
            Some("java.util.Map".to_string())
        );
        assert_eq!(
            java_dotted_type_name("Outer.Inner<? extends Number>"),
            Some("Outer.Inner".to_string())
        );
        assert_eq!(
            java_dotted_type_name("Box< String >"),
            Some("Box".to_string())
        );
        // Plain and dotted non-generic spellings stay unchanged.
        assert_eq!(java_dotted_type_name("Helper"), Some("Helper".to_string()));
        assert_eq!(
            java_dotted_type_name("com.example.Outer.Inner"),
            Some("com.example.Outer.Inner".to_string())
        );
    }

    #[test]
    fn java_dotted_type_name_rejects_malformed_generic_spellings() {
        for spelling in [
            "Box<>",
            "Box<",
            "Box<String",
            "Box<String>[]",
            "Box< >",
            "Box<String>>",
            "Box<<String>",
            "<String>",
            "int<String>",
            "Box<String> extra",
        ] {
            assert_eq!(
                java_dotted_type_name(spelling),
                None,
                "spelling {spelling:?} must fail closed"
            );
        }
    }
}
