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
    /// Element component types for names bound from a single-level generic
    /// array spelling such as `Array<Helper>`; an element-access receiver such
    /// as `items[0]` dispatches on the recorded component type.
    array_component_types: BTreeMap<String, String>,
    /// Qualified element-access bases for names bound from an initializer such
    /// as `val x = group.holder.fieldItems[0]`, whose terminal array field's
    /// element component type is resolved at trace time because it can span
    /// type declarations. The name stays bound (shadowing objects and types)
    /// but has no usable type until the chain is walked.
    element_access_bases: BTreeMap<String, String>,
    ambiguous_names: BTreeSet<String>,
}

impl KotlinReceiverTypeBindings {
    /// Returns whether `name` is bound locally, including as an ambiguous
    /// binding. Callers use this to distinguish "not bound" (a receiver may be
    /// a named object or type instead) from "bound but ambiguous" (fail closed).
    pub(in crate::symbol_dependency) fn contains(&self, name: &str) -> bool {
        self.types_by_name.contains_key(name)
            || self.array_component_types.contains_key(name)
            || self.element_access_bases.contains_key(name)
            || self.ambiguous_names.contains(name)
    }

    pub(in crate::symbol_dependency) fn type_for(&self, name: &str) -> Option<String> {
        if self.ambiguous_names.contains(name) {
            return None;
        }
        self.types_by_name.get(name).cloned()
    }

    /// Returns the recorded element component type for a uniquely bound
    /// array-typed name such as `items` in `items: Array<Helper>`, which
    /// resolves to the element type `Helper` when the chain accesses an
    /// element. Ambiguous bindings and names without a usable single-level
    /// array component return `None`.
    pub(in crate::symbol_dependency) fn array_component_for(&self, name: &str) -> Option<String> {
        if self.ambiguous_names.contains(name) {
            return None;
        }
        self.array_component_types
            .get(name)
            .filter(|type_name| !type_name.is_empty())
            .cloned()
    }

    /// Returns the recorded qualified element-access base spelling for a name
    /// bound from an initializer such as `val x = group.holder.fieldItems[0]`.
    /// Ambiguous bindings and names without a qualified base return `None`.
    pub(in crate::symbol_dependency) fn element_access_base_for(
        &self,
        name: &str,
    ) -> Option<String> {
        if self.ambiguous_names.contains(name) {
            return None;
        }
        self.element_access_bases.get(name).cloned()
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
                && let Some((name, type_name, element_access_base)) =
                    kotlin_property_binding(child, source, &bindings)?
            {
                if let Some(base) = element_access_base {
                    insert_kotlin_element_access_base_binding(&mut bindings, name, base);
                } else {
                    insert_kotlin_receiver_binding(&mut bindings, name, type_name);
                }
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
        && let Some((name, type_name, element_access_base)) =
            kotlin_property_binding(node, source, bindings)?
    {
        if let Some(base) = element_access_base {
            insert_kotlin_element_access_base_binding(bindings, name, base);
        } else {
            insert_kotlin_receiver_binding(bindings, name, type_name);
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_kotlin_body_property_bindings(child, source, bindings)?;
    }
    Ok(())
}

/// Returns the inner expression of a parenthesized initializer such as
/// `(Other())`, `(makeItems())`, or `(items[0])`, so `val` locals with
/// parenthesized initializers bind the same receiver type as the
/// unparenthesized form. Malformed or empty parentheses return `None` and
/// fail closed.
fn kotlin_parenthesized_initializer_expression(mut initializer: Node<'_>) -> Option<Node<'_>> {
    loop {
        if initializer.kind() != "parenthesized_expression" {
            return Some(initializer);
        }
        initializer = initializer.named_child(0)?;
    }
}

fn kotlin_property_binding(
    property: Node<'_>,
    source: &str,
    bindings: &KotlinReceiverTypeBindings,
) -> Result<Option<(String, String, Option<String>)>> {
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
        && let Some(type_name) = kotlin_declared_type_name(node_text(*type_node, source)?)
    {
        return Ok(Some((name, type_name, None)));
    }
    // Fall back to a constructor-call initializer such as `val x = Other()` or
    // `val x = Outer.Inner()`, or an element-access initializer such as
    // `val x = items[0]`; a parenthesized initializer such as `(Other())`,
    // `(makeItems())`, or `(items[0])` unwraps to the same inner expression so
    // `val` locals bind the same receiver type as the unparenthesized form.
    // Qualified callees must be pure identifier chains, and element-access
    // bases follow the plain/qualified/factory rules below.
    let initializer = children
        .iter()
        .find(|child| {
            matches!(
                child.kind(),
                "call_expression" | "index_expression" | "parenthesized_expression"
            )
        })
        .copied();
    let Some(initializer) = initializer else {
        return Ok(None);
    };
    let Some(initializer) = kotlin_parenthesized_initializer_expression(initializer) else {
        return Ok(None);
    };
    if initializer.kind() == "call_expression"
        && let Some(callee) = initializer.named_child(0)
        && let Some(type_name) = kotlin_call_initializer_callee_name(callee, source)?
        && !type_name.is_empty()
    {
        return Ok(Some((name, type_name, None)));
    }
    // An element-access initializer: a plain-identifier base already bound to
    // a single-level generic array binds the property to the base array's
    // element component type, a qualified base such as
    // `group.holder.fieldItems` records the base spelling so trace-time
    // resolution can walk the property chain to the terminal array field's
    // component type, a `this`-rooted base such as `this.groups` records the
    // spelling so trace-time resolution can walk the enclosing type's
    // property chain, a `super`-rooted base such as `super.inheritedItems`
    // records the spelling so trace-time resolution can walk the direct
    // superclass's property chain, and a factory-call base such as
    // `makeItems()` records the callee with a trailing `()` marker so
    // trace-time resolution can walk the factory's declared return array.
    // Multi-dimensional element access, function-call subscripts, qualified
    // call callees, and bases without a usable array component fail closed.
    if initializer.kind() == "index_expression"
        && let Some(base_name) = kotlin_element_access_base(initializer, source)?
    {
        if let Some(component_type) = bindings.array_component_for(&base_name) {
            return Ok(Some((name, component_type, None)));
        }
        if base_name.contains('.') || base_name.ends_with("()") {
            return Ok(Some((name, String::new(), Some(base_name))));
        }
    }
    Ok(None)
}

/// Returns the callee spelling of a call-initializer such as `Other` in
/// `val x = Other()`, `Util.makeItems` in `val x = Util.makeItems()`,
/// `this.ownMake` in `val x = this.ownMake()`, or
/// `super.inheritedMake` in `val x = super.inheritedMake()`. Plain-identifier
/// and safe dotted navigation callees keep their spelling, and
/// `this`/`super`-rooted dotted callees keep the root so trace-time
/// resolution can dispatch the member function on the enclosing type or the
/// direct superclass. Parenthesized roots and other non-name callees return
/// `None` so call-initializer bindings fail closed for genuinely unsupported
/// shapes.
fn kotlin_call_initializer_callee_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    if node.kind() == "identifier" {
        let name = node_text(node, source)?.trim().to_string();
        return Ok((!name.is_empty()).then_some(name));
    }
    if matches!(node.kind(), "this_expression" | "super_expression") {
        let name = node_text(node, source)?.trim().to_string();
        return Ok((!name.is_empty()).then_some(name));
    }
    if node.kind() != "navigation_expression" {
        return Ok(None);
    }
    let mut cursor = node.walk();
    let children = node.named_children(&mut cursor).collect::<Vec<_>>();
    if children.len() != 2 || children[1].kind() != "identifier" {
        return Ok(None);
    }
    let text = node_text(node, source)?.trim();
    if text.contains('?') || text.contains("::") {
        return Ok(None);
    }
    let Some(prefix) = kotlin_call_initializer_callee_name(children[0], source)? else {
        return Ok(None);
    };
    let member = node_text(children[1], source)?.trim().to_string();
    if member.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!("{prefix}.{member}")))
}

/// Returns the callee spelling of a factory-call element-access base such as
/// `makeItems` in `makeItems()[0]`, `Util.makeItems` in
/// `Util.makeItems()[0]`, `this.ownMake` in `this.ownMake()[0]`, or
/// `super.inheritedMake` in `super.inheritedMake()[0]`. Plain-identifier,
/// safe dotted navigation, and `this`/`super`-rooted callees are accepted
/// (the root is kept so trace-time resolution can dispatch the member
/// function on the enclosing type or the direct superclass); parenthesized
/// roots and other non-name callees return `None` so factory element-access
/// bases fail closed only for genuinely unsupported shapes.
fn kotlin_factory_call_callee_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    if node.kind() == "identifier" {
        let name = node_text(node, source)?.trim().to_string();
        return Ok((!name.is_empty()).then_some(name));
    }
    if matches!(node.kind(), "this_expression" | "super_expression") {
        let name = node_text(node, source)?.trim().to_string();
        return Ok((!name.is_empty()).then_some(name));
    }
    if node.kind() != "navigation_expression" {
        return Ok(None);
    }
    let mut cursor = node.walk();
    let children = node.named_children(&mut cursor).collect::<Vec<_>>();
    if children.len() != 2 || children[1].kind() != "identifier" {
        return Ok(None);
    }
    let text = node_text(node, source)?.trim();
    if text.contains('?') || text.contains("::") {
        return Ok(None);
    }
    let Some(prefix) = kotlin_factory_call_callee_name(children[0], source)? else {
        return Ok(None);
    };
    let member = node_text(children[1], source)?.trim().to_string();
    if member.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!("{prefix}.{member}")))
}

/// Extracts the base of a single-level element-access initializer such as
/// `items[0]`, `this.groups[0]`, `super.inheritedItems[0]`, `makeItems()[0]`,
/// `Util.makeItems()[0]`, `this.ownMake()[0]`, or
/// `super.inheritedMake()[0]`.
/// Plain-identifier and dotted field-chain bases (including `this`- and
/// `super`-rooted chains) return their spelling, and a factory-call base with
/// a plain, safe dotted, or `this`/`super`-rooted callee returns the callee
/// with a trailing `()` marker so trace-time resolution can walk the
/// factory's declared return array. Parenthesized roots, nested element
/// access, and function-call, multi-index, or nullable subscripts return
/// `None` so element-access-inferred bindings fail closed.
fn kotlin_element_access_base(initializer: Node<'_>, source: &str) -> Result<Option<String>> {
    if initializer.kind() != "index_expression" {
        return Ok(None);
    }
    let mut cursor = initializer.walk();
    let children = initializer.named_children(&mut cursor).collect::<Vec<_>>();
    if children.len() != 2 {
        return Ok(None);
    }
    let subscript = node_text(children[1], source)?.trim();
    if subscript.is_empty() || subscript.contains(['[', '(', ')', ',', '?', '.']) {
        return Ok(None);
    }
    // A factory-call base such as `makeItems()`, `Util.makeItems()`,
    // `this.ownMake()`, or `super.inheritedMake()` records the callee with a
    // trailing `()` marker so trace-time resolution can walk the factory's
    // declared return array. Plain-identifier, safe dotted, and
    // `this`/`super`-rooted callees are accepted; parenthesized roots and
    // other non-name callees fail closed.
    if children[0].kind() == "call_expression" {
        let Some(callee) = children[0].named_child(0) else {
            return Ok(None);
        };
        let Some(callee_name) = kotlin_factory_call_callee_name(callee, source)? else {
            return Ok(None);
        };
        return Ok(Some(format!("{callee_name}()")));
    }
    let base = node_text(children[0], source)?.trim();
    if base.is_empty()
        || base.contains(['(', '[', ' ', '?'])
        || base.contains("::")
        || base.split('.').any(|segment| {
            segment.is_empty()
                || !segment
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
        })
    {
        return Ok(None);
    }
    Ok(Some(base.to_string()))
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
    let Some(type_name) = kotlin_declared_type_name(node_text(*type_node, source)?) else {
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

/// Extracts a named receiver type, allowing dotted qualified names such as
/// `Outer.Inner` and well-formed generic spellings such as `Box<String>` (which
/// normalize to the raw dotted base name `Box`). Generic array spellings such
/// as `Array<Helper>` remain capability-gated because array receiver handling
/// is a dedicated slice that records the raw spelling, and nullable, otherwise
/// complex, and malformed spellings still fail closed; empty or malformed
/// dotted segments are rejected by the receiver path resolver.
pub(in crate::symbol_dependency) fn kotlin_dotted_type_name(text: &str) -> Option<String> {
    let mut name = text.trim();
    if let Some(stripped) = name.strip_suffix('?') {
        name = stripped.trim();
    }
    if name.is_empty() || name.contains(['(', '[', ':']) {
        return None;
    }
    if name.contains('<') {
        if name.starts_with("Array<") {
            return None;
        }
        return kotlin_generic_type_base_name(name);
    }
    if name.contains('>') || name.contains([',', ' ']) {
        return None;
    }
    Some(name.to_string())
}

/// Strips a well-formed top-level type-argument list from a generic type
/// spelling, returning the raw dotted base name. The argument list must be
/// balanced and non-empty, and nothing may follow its closing bracket;
/// malformed and otherwise complex spellings fail closed.
fn kotlin_generic_type_base_name(name: &str) -> Option<String> {
    let open = name.find('<')?;
    let prefix = name[..open].trim();
    if prefix.is_empty()
        || prefix.starts_with('.')
        || prefix.ends_with('.')
        || prefix.contains("..")
        || prefix.contains(['(', '[', ':', ',', ' '])
    {
        return None;
    }
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

/// Returns a receiver type spelling for a declared-type node: plain dotted
/// names normalize through `kotlin_dotted_type_name`, and generic array
/// spellings such as `Array<Helper>` or `Array<Array<Helper>>` are kept as
/// their raw spelling so the binding can record the element component type or
/// mark the receiver unusable. Other complex and malformed spellings return
/// `None` and fail closed.
fn kotlin_declared_type_name(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if let Some(normalized) = kotlin_dotted_type_name(trimmed) {
        return Some(normalized);
    }
    if trimmed.starts_with("Array<") && trimmed.ends_with('>') {
        return Some(trimmed.to_string());
    }
    None
}

/// Extracts the element component type name from a single-level Kotlin generic
/// array spelling such as `Array<Helper>` or `Array<Outer.Inner>`, normalizing
/// the component through `kotlin_dotted_type_name`. Nested generic arrays such
/// as `Array<Array<Helper>>`, primitive arrays such as `IntArray`, malformed
/// spellings, and non-`Array<...>` spellings return `None` and fail closed.
pub(in crate::symbol_dependency) fn kotlin_array_type_component_name(text: &str) -> Option<String> {
    let name = text.trim();
    let rest = name.strip_prefix("Array<")?;
    let close = rest.rfind('>')?;
    if !rest[close + 1..].trim().is_empty() {
        return None;
    }
    let component = rest[..close].trim();
    if component.is_empty() {
        return None;
    }
    kotlin_dotted_type_name(component)
}

fn insert_kotlin_receiver_binding(
    bindings: &mut KotlinReceiverTypeBindings,
    name: String,
    type_name: String,
) {
    if bindings.ambiguous_names.contains(&name) {
        return;
    }
    // A generic array spelling such as `Array<Helper>` binds the element
    // component type so an element-access receiver such as `items[0]` can
    // dispatch on the element type; nested generic arrays such as
    // `Array<Array<Helper>>` have no usable component and bind as an empty
    // (unusable) type instead of falling through to a same-named object or
    // type.
    if type_name.starts_with("Array<") {
        match kotlin_array_type_component_name(&type_name) {
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
    if bindings
        .types_by_name
        .insert(name.clone(), type_name)
        .is_some()
    {
        bindings.types_by_name.remove(&name);
        bindings.ambiguous_names.insert(name);
    }
}

/// Records a name bound from a qualified element-access initializer such as
/// `val x = group.holder.fieldItems[0]` under its base spelling. The name
/// shadows same-named objects and types; a duplicate declaration of the same
/// name fails closed as ambiguous.
fn insert_kotlin_element_access_base_binding(
    bindings: &mut KotlinReceiverTypeBindings,
    name: String,
    base: String,
) {
    if bindings.ambiguous_names.contains(&name) {
        return;
    }
    if bindings.types_by_name.contains_key(&name)
        || bindings.array_component_types.contains_key(&name)
        || bindings
            .element_access_bases
            .insert(name.clone(), base)
            .is_some()
    {
        bindings.types_by_name.remove(&name);
        bindings.array_component_types.remove(&name);
        bindings.element_access_bases.remove(&name);
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
        KotlinImportBinding, KotlinReceiverTypeBindings, kotlin_dotted_type_name,
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
    fn kotlin_dotted_type_name_normalizes_generic_receiver_types() {
        assert_eq!(
            kotlin_dotted_type_name("Box<String>").as_deref(),
            Some("Box")
        );
        assert_eq!(
            kotlin_dotted_type_name("Outer.Inner<String>").as_deref(),
            Some("Outer.Inner")
        );
        assert_eq!(
            kotlin_dotted_type_name("Box<String>?").as_deref(),
            Some("Box")
        );
        assert_eq!(
            kotlin_dotted_type_name("Box<Outer.Inner>").as_deref(),
            Some("Box")
        );
        assert_eq!(
            kotlin_dotted_type_name("Box<String, Int>").as_deref(),
            Some("Box")
        );
        assert_eq!(kotlin_dotted_type_name("Box"), Some("Box".to_string()));
        assert_eq!(
            kotlin_dotted_type_name("Outer.Inner").as_deref(),
            Some("Outer.Inner")
        );
    }

    #[test]
    fn kotlin_dotted_type_name_rejects_malformed_generic_spellings() {
        // Generic arrays stay capability-gated for the array slice, and
        // malformed, unbalanced, empty, or trailing generic spellings fail
        // closed.
        assert_eq!(kotlin_dotted_type_name("Array<Helper>"), None);
        assert_eq!(kotlin_dotted_type_name("Box<"), None);
        assert_eq!(kotlin_dotted_type_name("Box>"), None);
        assert_eq!(kotlin_dotted_type_name("Box<>"), None);
        assert_eq!(kotlin_dotted_type_name("Box<String"), None);
        assert_eq!(kotlin_dotted_type_name("Box<String>Extra"), None);
        assert_eq!(kotlin_dotted_type_name(""), None);
        assert_eq!(kotlin_dotted_type_name("Box (String)"), None);
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
    fn array_typed_receivers_bind_element_component_types() {
        let file = write_test_file(
            "package com.example\n\nclass Helper {\n    fun helper(value: Int): Int = value\n}\n\nclass Holder {\n    val fieldItems: Array<Helper> = arrayOf()\n    fun run() {\n        fieldItems[0].helper(1)\n    }\n}\n\nfun process(items: Array<Helper>, matrix: Array<Array<Helper>>, counts: IntArray) {\n    items[0].helper(1)\n    matrix[0][0]\n    counts[0]\n}\n",
        );
        let context = kotlin_import_context_for_file_with_overrides_and_deadline(
            &file.normalized_path,
            None,
            None,
        )
        .unwrap();
        let process_bindings = context
            .receiver_type_bindings_by_range
            .values()
            .find(|bindings| bindings.array_component_for("items") == Some("Helper".to_string()))
            .unwrap();
        assert_eq!(
            process_bindings.array_component_for("items"),
            Some("Helper".to_string())
        );
        assert_eq!(process_bindings.type_for("items"), None);
        // A nested generic array has no usable component but still shadows
        // same-named objects and types; primitive arrays bind as an unusable
        // type with no component.
        assert!(process_bindings.contains("matrix"));
        assert_eq!(process_bindings.array_component_for("matrix"), None);
        assert!(process_bindings.contains("counts"));
        assert_eq!(process_bindings.array_component_for("counts"), None);
        // Enclosing-class array-typed properties bind the component too.
        let run_bindings = context
            .receiver_type_bindings_by_range
            .values()
            .find(|bindings| {
                bindings.array_component_for("fieldItems") == Some("Helper".to_string())
            })
            .unwrap();
        assert_eq!(
            run_bindings.array_component_for("fieldItems"),
            Some("Helper".to_string())
        );
    }

    #[test]
    fn element_access_initializer_properties_bind_component_types() {
        let file = write_test_file(
            "package com.example\n\nclass Helper {\n    fun helper(value: Int): Int = value\n}\n\nclass Holder {\n    val fieldItems: Array<Helper> = arrayOf()\n    fun run(items: Array<Helper>, counts: IntArray) {\n        val first = items[0]\n        val fromField = fieldItems[0]\n        val fromCounts = counts[0]\n        first.helper(1)\n    }\n}\n",
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
            .find(|bindings| bindings.type_for("first") == Some("Helper".to_string()))
            .unwrap();
        // A `val` bound from a single-level element access inherits the base
        // array's element component type, whether the base is a parameter or an
        // enclosing-class array-typed property. A primitive-component base has
        // no usable component, so its `val` is not bound.
        assert_eq!(run_bindings.type_for("first"), Some("Helper".to_string()));
        assert_eq!(
            run_bindings.type_for("fromField"),
            Some("Helper".to_string())
        );
        assert!(!run_bindings.contains("fromCounts"));
    }

    #[test]
    fn qualified_element_access_initializers_record_base_spellings() {
        let file = write_test_file(
            "package com.example\n\nclass Helper {\n    fun helper(value: Int): Int = value\n}\n\nclass Holder {\n    val fieldItems: Array<Helper> = arrayOf()\n}\n\nclass Group {\n    val holder: Holder = Holder()\n    fun run(group: Group, items: Array<Helper>) {\n        val first = group.fieldItems[0]\n        val multi = group.holder.fieldItems[0]\n        val plain = items[0]\n        val fromThis = this.fieldItems[0]\n        first.helper(1)\n    }\n}\n",
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
            .find(|bindings| {
                bindings.element_access_base_for("first") == Some("group.fieldItems".to_string())
            })
            .unwrap();
        // A qualified element-access base records the base spelling with no
        // usable type so trace-time resolution can walk the property chain; a
        // plain base still binds the element component type directly. A
        // `this`-rooted base records its spelling the same way.
        assert_eq!(
            run_bindings.element_access_base_for("first"),
            Some("group.fieldItems".to_string())
        );
        assert_eq!(run_bindings.type_for("first"), None);
        assert_eq!(
            run_bindings.element_access_base_for("multi"),
            Some("group.holder.fieldItems".to_string())
        );
        assert_eq!(run_bindings.type_for("multi"), None);
        assert_eq!(run_bindings.type_for("plain"), Some("Helper".to_string()));
        assert_eq!(run_bindings.element_access_base_for("plain"), None);
        assert_eq!(
            run_bindings.element_access_base_for("fromThis"),
            Some("this.fieldItems".to_string())
        );
        assert_eq!(run_bindings.type_for("fromThis"), None);
    }

    #[test]
    fn factory_call_element_access_initializers_record_base_spellings() {
        let file = write_test_file(
            "package com.example\n\nclass Helper {\n    fun helper(value: Int): Int = value\n}\n\nclass Util {\n    fun makeItems(): Array<Helper> = arrayOf()\n}\n\nfun makeItems(): Array<Helper> = arrayOf()\n\nfun caller(items: Array<Helper>, group: Util): Int {\n    val factory = makeItems()[0]\n    val plain = items[0]\n    val qualified = Util.makeItems()[0]\n    val member = group.makeItems()[0]\n    factory.helper(1)\n}\n",
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
            .find(|bindings| {
                bindings.element_access_base_for("factory") == Some("makeItems()".to_string())
            })
            .unwrap();
        // A factory-call base records the callee with a trailing `()` marker
        // and no usable type, for both plain and safe dotted callees; a plain
        // base still binds the element component type directly.
        assert_eq!(
            run_bindings.element_access_base_for("factory"),
            Some("makeItems()".to_string())
        );
        assert_eq!(run_bindings.type_for("factory"), None);
        assert_eq!(run_bindings.type_for("plain"), Some("Helper".to_string()));
        assert_eq!(run_bindings.element_access_base_for("plain"), None);
        assert_eq!(
            run_bindings.element_access_base_for("qualified"),
            Some("Util.makeItems()".to_string())
        );
        assert_eq!(
            run_bindings.element_access_base_for("member"),
            Some("group.makeItems()".to_string())
        );
        assert_eq!(run_bindings.type_for("qualified"), None);
        assert_eq!(run_bindings.type_for("member"), None);
    }

    #[test]
    fn super_rooted_element_access_initializers_record_base_spellings() {
        let file = write_test_file(
            "package com.example\n\nclass Helper {\n    fun helper(value: Int): Int = value\n}\n\nopen class Base {\n    val inheritedItems: Array<Helper> = arrayOf()\n}\n\nclass Caller : Base() {\n    fun run(items: Array<Helper>): Int {\n        val fromSuper = super.inheritedItems[0]\n        val fromThis = this.inheritedItems[0]\n        val plain = items[0]\n        fromSuper.helper(1)\n    }\n}\n",
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
            .find(|bindings| {
                bindings.element_access_base_for("fromSuper")
                    == Some("super.inheritedItems".to_string())
            })
            .unwrap();
        // A `super`-rooted base records the spelling with no usable type so
        // trace-time resolution can walk the direct superclass's property
        // chain; a plain base still binds the element component type directly.
        // A `this`-rooted base records its spelling the same way.
        assert_eq!(
            run_bindings.element_access_base_for("fromSuper"),
            Some("super.inheritedItems".to_string())
        );
        assert_eq!(run_bindings.type_for("fromSuper"), None);
        assert_eq!(run_bindings.type_for("plain"), Some("Helper".to_string()));
        assert_eq!(
            run_bindings.element_access_base_for("fromThis"),
            Some("this.inheritedItems".to_string())
        );
        assert_eq!(run_bindings.type_for("fromThis"), None);
    }

    #[test]
    fn var_locals_unwrap_parenthesized_initializers() {
        let file = write_test_file(
            "package com.example\n\nclass Helper {\n    fun helper(value: Int): Int = value\n}\n\nclass Holder {\n    val fieldItems: Array<Helper> = arrayOf()\n}\n\nclass Group {\n    val holder: Holder = Holder()\n}\n\nclass Caller {\n    fun run(items: Array<Helper>, group: Group): Int {\n        val constructed = (Helper())\n        val factory = (makeItems())\n        val element = (items[0])\n        val qualified = (group.holder.fieldItems[0])\n        val nested = (((Helper())))\n        return constructed.helper(1)\n    }\n}\n\nfun makeItems(): Array<Helper> = arrayOf()\n",
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
            .find(|bindings| bindings.type_for("constructed") == Some("Helper".to_string()))
            .unwrap();
        // Parenthesized constructor, factory, element-access, and qualified
        // element-access initializers bind the same receiver type or base
        // spelling as their unparenthesized forms; nested parentheses unwrap
        // fully.
        assert_eq!(
            run_bindings.type_for("constructed"),
            Some("Helper".to_string())
        );
        assert_eq!(
            run_bindings.type_for("factory"),
            Some("makeItems".to_string())
        );
        assert_eq!(run_bindings.type_for("element"), Some("Helper".to_string()));
        assert_eq!(
            run_bindings.element_access_base_for("qualified"),
            Some("group.holder.fieldItems".to_string())
        );
        assert_eq!(run_bindings.type_for("nested"), Some("Helper".to_string()));
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
