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
    /// Property-chain initializer spellings for names bound from an
    /// initializer such as `val first = holder.item` or
    /// `val first = this.holder.item`, whose terminal property type is
    /// resolved at trace time because it can span type declarations. The name
    /// stays bound (shadowing objects and types) but has no usable type until
    /// the chain is walked.
    property_chain_bases: BTreeMap<String, String>,
    /// Branch initializer spellings for names bound from an `if`/`when`
    /// expression initializer such as `val group = if (flag) h.make() else
    /// Holder().make()`, whose common declared type is resolved at trace time
    /// by resolving every branch spelling and requiring them all to agree. The
    /// name stays bound (shadowing objects and types) but has no usable type
    /// until the branches are walked.
    branch_initializers_by_name: BTreeMap<String, Vec<String>>,
    ambiguous_names: BTreeSet<String>,
    /// Names bound from companion-object members, which are shadowed by any
    /// higher-priority local, parameter, or instance-property binding of the
    /// same name. Tracked separately so a higher-priority binding replaces the
    /// companion binding cleanly instead of creating a false ambiguity.
    shadowable_names: BTreeSet<String>,
}

impl KotlinReceiverTypeBindings {
    /// Returns whether `name` is bound locally, including as an ambiguous
    /// binding. Callers use this to distinguish "not bound" (a receiver may be
    /// a named object or type instead) from "bound but ambiguous" (fail closed).
    pub(in crate::symbol_dependency) fn contains(&self, name: &str) -> bool {
        self.types_by_name.contains_key(name)
            || self.array_component_types.contains_key(name)
            || self.element_access_bases.contains_key(name)
            || self.property_chain_bases.contains_key(name)
            || self.branch_initializers_by_name.contains_key(name)
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

    /// Returns the recorded property-chain initializer spelling for a name
    /// bound from an initializer such as `val first = holder.item`. Ambiguous
    /// bindings and names without a recorded chain return `None`.
    pub(in crate::symbol_dependency) fn property_chain_base_for(
        &self,
        name: &str,
    ) -> Option<String> {
        if self.ambiguous_names.contains(name) {
            return None;
        }
        self.property_chain_bases.get(name).cloned()
    }

    /// Returns the recorded branch initializer spellings for a name bound
    /// from an `if`/`when` expression initializer such as
    /// `val group = if (flag) h.make() else Holder().make()`. Ambiguous
    /// bindings and names without recorded branches return `None`.
    pub(in crate::symbol_dependency) fn branch_initializers_for(
        &self,
        name: &str,
    ) -> Option<Vec<String>> {
        if self.ambiguous_names.contains(name) {
            return None;
        }
        self.branch_initializers_by_name.get(name).cloned()
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
    let enclosing_body = kotlin_enclosing_type_declaration(function).and_then(|type_node| {
        type_node
            .named_children(&mut type_node.walk())
            .find(|child| child.kind() == "class_body")
    });

    // Enclosing-type properties are visible to member functions.
    if let Some(class_body) = enclosing_body {
        let mut cursor = class_body.walk();
        for child in class_body.named_children(&mut cursor) {
            if child.kind() == "property_declaration"
                && let Some((name, branches)) = kotlin_branch_initializer_binding(child, source)?
            {
                insert_kotlin_branch_initializer_binding(&mut bindings, name, branches);
            } else if child.kind() == "property_declaration"
                && let Some((name, type_name, element_access_base, property_chain_base)) =
                    kotlin_property_binding(child, source, &bindings)?
            {
                if let Some(base) = element_access_base {
                    insert_kotlin_element_access_base_binding(&mut bindings, name, base);
                } else if let Some(chain) = property_chain_base {
                    insert_kotlin_property_chain_base_binding(&mut bindings, name, chain);
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

    // Companion-object properties are visible unqualified to members of the
    // enclosing type, shadowed by any higher-priority local, parameter, or
    // instance-property binding of the same name. Companion bindings are
    // collected before body locals so a local whose initializer references a
    // companion member (such as `val first = items[0]` for a companion
    // `val items: Array<Item>`) can bind its element type; the shadowable
    // insert discipline still lets locals, parameters, and instance
    // properties replace companion bindings cleanly.
    if let Some(class_body) = enclosing_body {
        let mut cursor = class_body.walk();
        for child in class_body.named_children(&mut cursor) {
            if child.kind() != "companion_object" {
                continue;
            }
            let Some(companion_body) = child
                .named_children(&mut child.walk())
                .find(|member| member.kind() == "class_body")
            else {
                continue;
            };
            let mut member_cursor = companion_body.walk();
            for member in companion_body.named_children(&mut member_cursor) {
                if member.kind() == "property_declaration"
                    && let Some((name, branches)) =
                        kotlin_scope_branch_initializer_binding(member, source)?
                {
                    insert_kotlin_shadowable_branch_initializer_binding(
                        &mut bindings,
                        name,
                        branches,
                    );
                } else if member.kind() == "property_declaration"
                    && let Some((name, branches)) =
                        kotlin_branch_initializer_binding(member, source)?
                {
                    insert_kotlin_shadowable_branch_initializer_binding(
                        &mut bindings,
                        name,
                        branches,
                    );
                } else if member.kind() == "property_declaration"
                    && let Some((name, type_name, element_access_base, property_chain_base)) =
                        kotlin_property_binding(member, source, &bindings)?
                {
                    if let Some(base) = element_access_base {
                        insert_kotlin_shadowable_element_access_base_binding(
                            &mut bindings,
                            name,
                            base,
                        );
                    } else if let Some(chain) = property_chain_base {
                        insert_kotlin_shadowable_property_chain_base_binding(
                            &mut bindings,
                            name,
                            chain,
                        );
                    } else {
                        insert_kotlin_shadowable_receiver_binding(&mut bindings, name, type_name);
                    }
                }
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
        && let Some((name, branches)) = kotlin_scope_branch_initializer_binding(node, source)?
    {
        insert_kotlin_branch_initializer_binding(bindings, name, branches);
    } else if node.kind() == "property_declaration"
        && let Some((name, branches)) = kotlin_branch_initializer_binding(node, source)?
    {
        insert_kotlin_branch_initializer_binding(bindings, name, branches);
    } else if node.kind() == "property_declaration"
        && let Some((name, type_name, element_access_base, property_chain_base)) =
            kotlin_property_binding(node, source, bindings)?
    {
        if let Some(base) = element_access_base {
            insert_kotlin_element_access_base_binding(bindings, name, base);
        } else if let Some(chain) = property_chain_base {
            insert_kotlin_property_chain_base_binding(bindings, name, chain);
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

/// Returns the operand of a postfix force-unwrap expression such as
/// `makeNullable()!!` or `items!!` when `node` is a `unary_expression` whose
/// operator is the `!!` force-unwrap token. Other unary operators such as
/// boolean negation or arithmetic signs return `None` so initializer and
/// element-access bindings only strip nullability without guessing about value
/// transformations.
fn kotlin_force_unwrap_operand(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() != "unary_expression" {
        return None;
    }
    let mut cursor = node.walk();
    let is_force_unwrap = node.children(&mut cursor).any(|child| child.kind() == "!!");
    if !is_force_unwrap {
        return None;
    }
    node.named_child(0)
}

/// Strips parentheses and postfix `!!` force-unwrap operators from an
/// initializer expression such as `(makeItems()!!)` so `val` locals bind the
/// same receiver shape as the fully unwrapped expression. Only the `!!`
/// operator unwraps; other unary operators (negation, arithmetic signs) fail
/// closed because they transform the value rather than only nullability.
fn kotlin_initializer_expression(mut initializer: Node<'_>) -> Option<Node<'_>> {
    loop {
        let unwrapped = kotlin_parenthesized_initializer_expression(initializer)?;
        if unwrapped != initializer {
            initializer = unwrapped;
            continue;
        }
        let Some(inner) = kotlin_force_unwrap_operand(initializer) else {
            return Some(initializer);
        };
        initializer = inner;
    }
}

/// Extracts the dotted spelling of a property-chain initializer such as
/// `holder.item` in `val first = holder.item`, including `this`- and
/// `super`-rooted chains such as `this.holder.item` or `super.baseItem`,
/// chains with single-level element-access hops such as `h.items[0].item` in
/// `val first = h.items[0].item`, chains ending in a zero-argument
/// method-call hop such as `h.items[0].make()` in
/// `val x = h.items[0].make()`, and element-access initializers whose base
/// chain contains a method-call hop such as `h.make().items[0]` in
/// `val group = h.make().items[0]`. Identifier, method-call, and single-level
/// element-access hops (with a plain-identifier base) return their spelling
/// so trace-time resolution can walk each hop; parenthesized roots, nullable,
/// multi-dimensional element access, and otherwise non-name shapes return
/// `None` so property-chain bindings fail closed.
fn kotlin_property_chain_initializer(
    initializer: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    if !matches!(
        initializer.kind(),
        "navigation_expression" | "call_expression" | "index_expression"
    ) {
        return Ok(None);
    }
    let text = node_text(initializer, source)?.trim();
    if text.is_empty() || text.contains([' ', '?']) || text.contains("::") {
        return Ok(None);
    }
    // A zero-argument call such as `h.items[0].make()` in
    // `val x = h.items[0].make()` parses as a `call_expression` whose callee
    // is a navigation chain containing an element-access hop. Only calls
    // whose spelling contains an element-access hop record as chains (plain
    // and dotted callee calls keep the constructor/factory callee-name
    // binding); non-zero-argument calls and other call shapes fail closed
    // because their hop text does not validate below.
    if initializer.kind() == "call_expression" && !text.contains('[') {
        return Ok(None);
    }
    let hops = text.split('.').collect::<Vec<_>>();
    let mut valid = true;
    for (index, hop) in hops.iter().enumerate() {
        let is_root = index == 0 && matches!(*hop, "this" | "super");
        if hop.is_empty() || (!is_root && !kotlin_property_chain_hop_valid(hop)) {
            valid = false;
            break;
        }
    }
    if !valid || hops.len() < 2 {
        return Ok(None);
    }
    Ok(Some(text.to_string()))
}

/// Returns whether a property-chain hop is a plain identifier property name,
/// a zero-argument method-call spelling such as `make()`, a generic
/// constructor spelling such as `Box<Holder>()`, or a single-level
/// element-access hop such as `items[0]` (plain-identifier base) or
/// `makeGroups()[0]` (plain-identifier zero-argument call base).
/// Method-call hops let a property-chain initializer such as
/// `val first = make().item` dispatch through the enclosing type's member
/// function declared return type before walking the remaining property hops,
/// generic constructor hops let a chain such as
/// `val first = Box<Holder>().item` start on the raw constructed type,
/// property element-access hops let a chain such as
/// `val first = h.items[0].item` dispatch through the array property's
/// element component type, and factory element-access hops let a chain such
/// as `val first = h.makeGroups()[0].item` dispatch through the factory's
/// declared return array element component type before walking the remaining
/// hops; non-zero-argument call spellings, multi-dimensional element access,
/// and other shapes fail closed at capture time.
fn kotlin_property_chain_hop_valid(hop: &str) -> bool {
    if let Some(name) = hop.strip_suffix("()") {
        if name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return !name.is_empty();
        }
        return kotlin_dotted_type_name(name).is_some();
    }
    // A single-level element-access hop whose base is a plain identifier or a
    // plain-identifier zero-argument call lets a property-chain initializer
    // such as `val first = h.items[0].item` dispatch through the array
    // property's element component type or `val first = h.makeGroups()[0].item`
    // dispatch through the factory's declared return array element component
    // type before walking the remaining hops.
    if let Some(open) = hop.find('[') {
        if !hop.ends_with(']') {
            return false;
        }
        let base = &hop[..open];
        let subscript = &hop[open + 1..hop.len() - 1];
        let base_name = base.strip_suffix("()").unwrap_or(base);
        let base_valid = !base_name.is_empty()
            && base_name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_');
        let subscript_valid =
            !subscript.is_empty() && !subscript.contains(['[', ']', '(', ')', ',', '?', '.', ' ']);
        return base_valid && subscript_valid;
    }
    hop.chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// A branch initializer binding extracted from a `val`/`var` declaration
/// whose initializer is an `if`/`when` expression: the bound name and the
/// deduplicated branch initializer spellings whose common declared type is
/// resolved at trace time.
type KotlinBranchInitializerBinding = (String, Vec<String>);

/// Returns the initializer spelling of an `if`/`when` expression branch: a
/// call callee such as `h.make` or `Holder().make`, a property-chain spelling
/// such as `h.make().items[0].item`, a bare property name such as `item`, or
/// the receiver-qualified lambda result of a scope-function call such as the
/// `h.make` of `h.let { it.make() }`, the `h.make().items[0].item` of
/// `h.let { it.make().items[0].item }`, or the `h` receiver of `h.apply { }`.
/// Parenthesized and force-unwrapped branches unwrap to the same inner
/// expression; other branch shapes return `None` so branch bindings fail
/// closed.
fn kotlin_initializer_branch_spelling(branch: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(branch) = kotlin_initializer_expression(branch) else {
        return Ok(None);
    };
    // A scope-function call branch such as `h.let { it.make() }` or
    // `with(h) { make() }` binds the receiver-qualified lambda result as a
    // branch spelling through the same rules as a direct initializer: a
    // dotted factory callee (`h.make`), a property chain
    // (`h.make().items[0].item`), or the receiver type (`h` for
    // `apply`/`also`); unknown scope names, malformed lambda bodies, and
    // non-plain receivers fail closed and fall through to the generic callee
    // binding below.
    if branch.kind() == "call_expression"
        && let Some((type_name, _, property_chain_base)) =
            kotlin_scope_function_binding(branch, source)?
    {
        if !type_name.is_empty() {
            return Ok(Some(type_name));
        }
        if let Some(chain) = property_chain_base {
            return Ok(Some(chain));
        }
        return Ok(None);
    }
    if branch.kind() == "call_expression"
        && let Some(callee) = branch.named_child(0)
        && let Some(name) = kotlin_call_initializer_callee_name(callee, source)?
        && !name.is_empty()
    {
        return Ok(Some(name));
    }
    if matches!(
        branch.kind(),
        "navigation_expression" | "index_expression" | "call_expression"
    ) && let Some(chain) = kotlin_property_chain_initializer(branch, source)?
    {
        return Ok(Some(chain));
    }
    if branch.kind() == "identifier" {
        let name = node_text(branch, source)?.trim().to_string();
        return Ok((!name.is_empty()).then_some(name));
    }
    Ok(None)
}

/// Collects the deduplicated branch initializer spellings of an `if`/`when`
/// expression initializer. `if` expression branches come from the then arm,
/// any `else if` arms (recursively), and the else arm; an `if` without an
/// `else` arm has no value type (`Unit`), so the expression must yield at
/// least two branches or the whole binding fails closed. `when` expression
/// branches come from each `when_entry` body (its last named child), and the
/// expression must include an `else` arm (a `when` without `else` has no
/// value type to bind). A branch that is not a call callee, property chain,
/// or bare identifier makes the whole binding fail closed.
fn kotlin_if_when_branch_spellings(
    initializer: Node<'_>,
    source: &str,
) -> Result<Option<Vec<String>>> {
    let mut branches = Vec::new();
    let mut seen = BTreeSet::new();
    if initializer.kind() == "if_expression" {
        let mut cursor = initializer.walk();
        let children = initializer.named_children(&mut cursor).collect::<Vec<_>>();
        // The first named child is the condition; the rest are then/else arms
        // (an `else if` arm is itself a nested `if_expression`).
        for branch in children.iter().skip(1) {
            if matches!(branch.kind(), "if_expression" | "when_expression") {
                let Some(nested) = kotlin_if_when_branch_spellings(*branch, source)? else {
                    return Ok(None);
                };
                for spelling in nested {
                    if seen.insert(spelling.clone()) {
                        branches.push(spelling);
                    }
                }
            } else if let Some(spelling) = kotlin_initializer_branch_spelling(*branch, source)? {
                if seen.insert(spelling.clone()) {
                    branches.push(spelling);
                }
            } else {
                return Ok(None);
            }
        }
        // An `if` without an `else` arm has no value type to bind.
        if branches.len() < 2 {
            return Ok(None);
        }
        return Ok(Some(branches));
    }
    if initializer.kind() == "when_expression" {
        let mut cursor = initializer.walk();
        let mut has_else = false;
        for entry in initializer.named_children(&mut cursor) {
            if entry.kind() != "when_entry" {
                continue;
            }
            let mut entry_cursor = entry.walk();
            let entry_children = entry.named_children(&mut entry_cursor).collect::<Vec<_>>();
            // An `else` arm has no condition child, only the body expression.
            has_else |= entry_children.len() == 1;
            let Some(body) = entry_children.last().copied() else {
                return Ok(None);
            };
            let Some(spelling) = kotlin_initializer_branch_spelling(body, source)? else {
                return Ok(None);
            };
            if seen.insert(spelling.clone()) {
                branches.push(spelling);
            }
        }
        // A `when` used as an expression without an `else` arm has no value
        // type to bind (its type includes `Unit`), so it fails closed.
        if !has_else || branches.is_empty() {
            return Ok(None);
        }
        return Ok(Some(branches));
    }
    Ok(None)
}

/// Collects the two operand spellings of an elvis (`?:`) binary-expression
/// initializer such as `val group = nullableH?.let { it.make() } ?:
/// Holder().make()`. Both operands must yield a call-callee, property-chain,
/// scope-function, or bare-property spelling through the same rules as an
/// `if`/`when` branch, otherwise the whole binding fails closed; other binary
/// operators (arithmetic, comparison, logic) fail closed because they
/// transform the value rather than selecting between two receiver spellings.
fn kotlin_elvis_branch_spellings(binary: Node<'_>, source: &str) -> Result<Option<Vec<String>>> {
    if binary.kind() != "binary_expression" {
        return Ok(None);
    }
    let mut cursor = binary.walk();
    let children = binary.named_children(&mut cursor).collect::<Vec<_>>();
    if children.len() != 2 {
        return Ok(None);
    }
    // The `?:` operator sits between the two operand nodes; other binary
    // operators fail closed.
    let operator = source
        .get(children[0].end_byte()..children[1].start_byte())
        .unwrap_or_default()
        .trim();
    if operator != "?:" {
        return Ok(None);
    }
    let mut branches = Vec::new();
    for operand in [children[0], children[1]] {
        let Some(spelling) = kotlin_initializer_branch_spelling(operand, source)? else {
            return Ok(None);
        };
        branches.push(spelling);
    }
    Ok(Some(branches))
}

/// Extracts a branch initializer binding from a `val`/`var` declaration whose
/// initializer is an `if`/`when` expression such as
/// `val group = if (flag) h.make() else Holder().make()` or
/// `val first = when (flag) { true -> h.make().items[0].item; else ->
/// Holder().make().items[0].item }`, or an elvis (`?:`) expression such as
/// `val group = nullableH?.let { it.make() } ?: Holder().make()`. Every
/// branch must yield a call-callee, property-chain, scope-function, or
/// bare-property spelling, an `if` expression must have an `else` arm, and an
/// elvis expression must use the `?:` operator, otherwise the whole binding
/// fails closed; an explicitly typed declaration binds through the declared
/// type instead. The bound name shadows same-named objects and types at trace
/// time.
fn kotlin_branch_initializer_binding(
    property: Node<'_>,
    source: &str,
) -> Result<Option<KotlinBranchInitializerBinding>> {
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
    // An explicitly typed declaration binds through the declared type, not
    // the initializer branches.
    if variable_children
        .iter()
        .any(|child| kotlin_is_type_node_kind(child.kind()))
    {
        return Ok(None);
    }
    let Some(initializer) = children
        .iter()
        .find(|child| {
            matches!(
                child.kind(),
                "if_expression" | "when_expression" | "binary_expression"
            )
        })
        .copied()
    else {
        return Ok(None);
    };
    let Some(branches) = (if initializer.kind() == "binary_expression" {
        kotlin_elvis_branch_spellings(initializer, source)?
    } else {
        kotlin_if_when_branch_spellings(initializer, source)?
    }) else {
        return Ok(None);
    };
    if branches.is_empty() {
        return Ok(None);
    }
    Ok(Some((name, branches)))
}

/// A property binding extracted from a `val`/`var` declaration: the bound
/// name, its declared type name (empty when inferred), an optional
/// element-access base spelling, and an optional property-chain base
/// spelling. Exactly one of the declared type, element-access base, or
/// property-chain base is set for a bound name.
type KotlinPropertyBinding = (String, String, Option<String>, Option<String>);

/// A scope-function initializer binding: the type name (empty when the
/// binding is an element-access or property-chain base), the optional
/// element-access base spelling, and the optional property-chain base
/// spelling. Exactly one of the three is set for a bound scope-function
/// initializer, matching the `KotlinPropertyBinding` shape without the bound
/// name.
type KotlinScopeFunctionBinding = (String, Option<String>, Option<String>);

/// One receiver-qualified branch arm of a scope-function lambda branch local:
/// a dotted factory-call callee (resolved as a call, so a chain base keeps the
/// call marker) or a property chain / bare receiver.
#[derive(Clone, Debug)]
enum KotlinScopeLocalBranch {
    Callee(String),
    Chain(String),
}

impl KotlinScopeLocalBranch {
    fn from_binding(binding: &KotlinScopeFunctionBinding) -> Option<Self> {
        let (type_name, _, chain) = binding;
        if !type_name.is_empty() {
            Some(Self::Callee(type_name.clone()))
        } else {
            chain.clone().map(Self::Chain)
        }
    }

    /// The bare branch-initializer spelling (a callee without its call
    /// marker), matching the branch spellings of a direct branch result.
    fn branch_spelling(&self) -> String {
        match self {
            Self::Callee(callee) => callee.clone(),
            Self::Chain(chain) => chain.clone(),
        }
    }

    /// The call-marker local spelling with a chain suffix applied: a callee
    /// keeps its `()` marker so the base resolves the hop as a call
    /// (`h.make()` becomes `h.make().items[0].item`), while a chain appends
    /// the suffix directly.
    fn with_suffix(&self, suffix: &str) -> String {
        match self {
            Self::Callee(callee) => format!("{callee}(){suffix}"),
            Self::Chain(chain) => format!("{chain}{suffix}"),
        }
    }
}

/// The receiver-qualified spelling of a scope-function lambda `val` local:
/// either a single spelling (the same call-callee, property-chain, or
/// bare-receiver forms as a single-expression body) or the branch spellings
/// of an `if`/`when`/elvis initializer (each arm expanded through the same
/// receiver rules as a direct branch initializer). A branch local has no
/// single value type, so it is only consumed where branch spellings are
/// accepted (as the result of a branch initializer binding); any other
/// consumer fails closed.
#[derive(Clone, Debug)]
enum KotlinScopeLocalSpelling {
    Single(String),
    Branches(Vec<KotlinScopeLocalBranch>),
}

type KotlinScopeLocals = BTreeMap<String, KotlinScopeLocalSpelling>;

/// Returns whether a scope-function receiver spelling is a plain identifier
/// chain such as `h` or `this.h`, or a receiver-chain spelling such as
/// `h.make()`, `Holder()`, or `h.make().items[0]` (each hop a valid
/// identifier, zero-argument call, or single-level element access), without
/// spaces, nullability, or qualified-name operators.
fn kotlin_scope_receiver_spelling(receiver: &str) -> bool {
    !receiver.is_empty()
        && !receiver.contains([' ', '?'])
        && !receiver.contains("::")
        && receiver.split('.').all(kotlin_property_chain_hop_valid)
}

/// Returns the receiver spelling and scope-function name of a scope-function
/// call callee such as `h.let` in `h.let { ... }` (a navigation expression
/// whose receiver is the leading expression) or `with(h)` in
/// `with(h) { ... }` (a call whose first argument is the receiver). Receivers
/// must be plain identifier chains; a nested scope-function call receiver
/// such as `h.let { it.make() }` in `h.let { it.make() }.let { ... }`
/// resolves recursively to its receiver-qualified lambda-result spelling;
/// other callee shapes return `None` so scope-function initializers fail
/// closed.
fn kotlin_scope_function_callee(
    callee: Node<'_>,
    source: &str,
) -> Result<Option<(String, String)>> {
    if callee.kind() == "navigation_expression" {
        let mut cursor = callee.walk();
        let children = callee.named_children(&mut cursor).collect::<Vec<_>>();
        if children.len() != 2 || children[1].kind() != "identifier" {
            return Ok(None);
        }
        let scope_name = node_text(children[1], source)?.trim().to_string();
        // A nested scope-function chain such as `h.let { it.make() }.let { ... }`
        // or `h.apply { }.let { ... }` has a scope-function call as the
        // navigation receiver; resolve it recursively and reuse its
        // receiver-qualified lambda-result spelling as the receiver chain so
        // the outer scope function binds through the inner call's result
        // type. Other receiver shapes keep their plain spelling.
        let receiver = if children[0].kind() == "call_expression" {
            match kotlin_scope_function_binding(children[0], source)? {
                Some((type_name, _, property_chain_base)) => {
                    if !type_name.is_empty() {
                        // The inner lambda result is a factory call such as
                        // `h.make`; keep its method-call marker so the outer
                        // chain walks the hop as a call (`h.make().make`)
                        // rather than a property.
                        format!("{type_name}()")
                    } else if let Some(chain) = property_chain_base {
                        chain
                    } else {
                        return Ok(None);
                    }
                }
                // A non-scope call receiver such as `Holder()` in
                // `Holder().let { ... }` keeps its plain spelling.
                None => node_text(children[0], source)?.trim().to_string(),
            }
        } else {
            node_text(children[0], source)?.trim().to_string()
        };
        if receiver.is_empty()
            || scope_name.is_empty()
            || !kotlin_scope_receiver_spelling(&receiver)
        {
            return Ok(None);
        }
        return Ok(Some((receiver, scope_name)));
    }
    if callee.kind() == "call_expression" {
        let mut cursor = callee.walk();
        let children = callee.named_children(&mut cursor).collect::<Vec<_>>();
        if children.len() != 2 || children[0].kind() != "identifier" {
            return Ok(None);
        }
        let scope_name = node_text(children[0], source)?.trim().to_string();
        if scope_name != "with" {
            return Ok(None);
        }
        let Some(receiver) = kotlin_scope_function_with_receiver(children[1], source)? else {
            return Ok(None);
        };
        return Ok(Some((receiver, scope_name)));
    }
    Ok(None)
}

/// Returns the receiver spelling of a `with(receiver) { ... }` call's value
/// arguments: exactly one plain identifier-chain argument such as `h` in
/// `with(h) { make() }`, or a nested scope-function call argument such as
/// `h.let { it.make() }` in `with(h.let { it.make() }) { make() }` resolved
/// recursively to its receiver-qualified lambda-result spelling. Zero,
/// multiple, or non-plain arguments return `None` so `with` initializers fail
/// closed.
fn kotlin_scope_function_with_receiver(
    value_arguments: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    if value_arguments.kind() != "value_arguments" {
        return Ok(None);
    }
    let mut cursor = value_arguments.walk();
    let children = value_arguments
        .named_children(&mut cursor)
        .collect::<Vec<_>>();
    if children.len() != 1 {
        return Ok(None);
    }
    // A nested scope-function chain such as `with(h.let { it.make() }) { ... }`
    // has a scope-function call as the single value argument (wrapped in a
    // `value_argument` node); resolve it recursively to its
    // receiver-qualified lambda-result spelling, and keep the plain spelling
    // of other receiver shapes.
    let Some(argument) = children[0].named_children(&mut children[0].walk()).next() else {
        return Ok(None);
    };
    let receiver = if argument.kind() == "call_expression" {
        match kotlin_scope_function_binding(argument, source)? {
            Some((type_name, _, property_chain_base)) => {
                if !type_name.is_empty() {
                    // The inner lambda result is a factory call such as
                    // `h.make`; keep its method-call marker so the outer
                    // chain walks the hop as a call (`h.make().make`) rather
                    // than a property.
                    format!("{type_name}()")
                } else if let Some(chain) = property_chain_base {
                    chain
                } else {
                    return Ok(None);
                }
            }
            // A non-scope call argument such as `Holder()` in
            // `with(Holder()) { ... }` keeps its plain spelling.
            None => node_text(argument, source)?.trim().to_string(),
        }
    } else {
        node_text(argument, source)?.trim().to_string()
    };
    if !kotlin_scope_receiver_spelling(&receiver) {
        return Ok(None);
    }
    Ok(Some(receiver))
}

/// A scope-function lambda body's optional explicit parameter name, its
/// leading `val` statements, and its final result expression.
type KotlinScopeLambdaBody<'a> = (Option<String>, Vec<Node<'a>>, Node<'a>);

/// Returns the statements and result expression of a scope-function lambda
/// such as the `it.make()` in `h.let { it.make() }` or the `val g = it.make()`
/// statement plus `g` result in `h.let { val g = it.make(); g }`, together
/// with the lambda's explicit parameter name when one is declared (such as
/// `holder` in `h.let { holder -> holder.make() }`). Every node before the
/// final result expression is a statement (typically a `val` local that the
/// caller substitutes into the result); an empty lambda and a lambda with
/// multiple parameters return `None` so scope-function initializers fail
/// closed (empty lambdas still bind for `apply`/`also`, which the caller
/// handles without a body).
fn kotlin_scope_lambda_body<'a>(
    lambda: Node<'a>,
    source: &str,
) -> Result<Option<KotlinScopeLambdaBody<'a>>> {
    let Some(lambda_literal) = lambda
        .named_children(&mut lambda.walk())
        .find(|child| child.kind() == "lambda_literal")
    else {
        return Ok(None);
    };
    let mut cursor = lambda_literal.walk();
    let children = lambda_literal
        .named_children(&mut cursor)
        .collect::<Vec<_>>();
    if children.is_empty() {
        return Ok(None);
    }
    // An explicit parameter list such as `holder` in
    // `{ holder -> holder.make() }` must declare exactly one parameter so the
    // body's receiver reference is unambiguous; multiple parameters fail
    // closed.
    let param_name = if children[0].kind() == "lambda_parameters" {
        if children.len() < 2 {
            return Ok(None);
        }
        let mut param_cursor = children[0].walk();
        let params = children[0]
            .named_children(&mut param_cursor)
            .filter(|child| child.kind() == "variable_declaration")
            .collect::<Vec<_>>();
        if params.len() != 1 {
            return Ok(None);
        }
        let Some(name) = params[0]
            .named_children(&mut params[0].walk())
            .find(|child| child.kind() == "identifier")
            .map(|name| node_text(name, source))
            .transpose()?
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
        else {
            return Ok(None);
        };
        Some(name)
    } else {
        None
    };
    // The final named child is the lambda result expression; any nodes before
    // it (after an optional parameter list) are body statements.
    let statements = if children[0].kind() == "lambda_parameters" {
        children[1..children.len() - 1].to_vec()
    } else {
        children[..children.len() - 1].to_vec()
    };
    Ok(Some((param_name, statements, children[children.len() - 1])))
}

/// Builds the local-name to initializer-spelling map for the `val` statements
/// of a multi-statement scope-function lambda body such as the `g` to
/// `it.make()` entry of `val g = it.make()` in
/// `h.let { val g = it.make(); g }`. Every statement must be a
/// `property_declaration` with exactly one named initializer expression
/// (typed declarations such as `val g: Group = it.make()` are allowed as long
/// as an initializer is present), and an initializer must not reference
/// another declared local; any other statement shape returns `None` so
/// multi-statement bodies fail closed unless every hop is
/// receiver-qualified through the same rules as a single-expression body. An
/// initializer that is itself a scope-function call such as
/// `val g1 = nullableH?.let { it.make() }` or
/// `val g1 = it.make().let { g -> g }` stores the call's
/// receiver-qualified lambda-result spelling instead of its raw source text,
/// and an `if`/`when`/elvis initializer stores the receiver-qualified branch
/// spellings of its arms (with factory-call callees keeping their call
/// marker) so a result that is exactly the local, or a chain rooted on the
/// local, binds through the branch rules.
fn kotlin_scope_lambda_locals(
    statements: &[Node<'_>],
    source: &str,
    scope_name: &str,
    param_name: Option<&str>,
    receiver: &str,
) -> Result<Option<KotlinScopeLocals>> {
    let mut locals = BTreeMap::new();
    for statement in statements {
        if statement.kind() != "property_declaration" {
            return Ok(None);
        }
        let mut cursor = statement.walk();
        let children = statement.named_children(&mut cursor).collect::<Vec<_>>();
        if children.len() != 2 || children[0].kind() != "variable_declaration" {
            return Ok(None);
        }
        let mut variable_cursor = children[0].walk();
        let Some(name_node) = children[0]
            .named_children(&mut variable_cursor)
            .find(|child| child.kind() == "identifier")
        else {
            return Ok(None);
        };
        let name = node_text(name_node, source)?.trim().to_string();
        let initializer = node_text(children[1], source)?.trim().to_string();
        if name.is_empty() || initializer.is_empty() {
            return Ok(None);
        }
        // A local whose initializer roots on another declared local such as
        // `val b = a` cannot be expanded without dependency ordering, so
        // multi-statement bodies with chained locals fail closed, except a
        // scope-function call on a branch-local receiver such as
        // `val g2 = g1.let { g -> g.items[0].item }`, which expands per arm
        // without leaving the branch-local root in the stored spelling.
        let root = initializer
            .split(['.', '[', '('])
            .next()
            .unwrap_or_default();
        if locals.contains_key(root)
            && !(children[1].kind() == "call_expression"
                && kotlin_scope_branch_local_scope_call_shapes(children[1], &locals, source)?
                    .is_some())
        {
            return Ok(None);
        }
        // A scope-function call initializer stores the call's
        // receiver-qualified lambda-result spelling (a factory-call callee
        // keeps its method-call marker such as `nullableH.make()`, and a chain
        // or bare receiver keeps its chain spelling), so expanding the local
        // into a result expression or a nested chain base resolves through the
        // same receiver rules as a single-expression body. An `if`/`when`/elvis
        // initializer stores the branch spellings of its arms through the same
        // receiver rules as a direct branch initializer, so a result that is
        // exactly the local binds through the branch rules. Non-scope
        // initializers and unknown scope-function or branch shapes keep their
        // raw spelling and fail closed through the caller's rewrite rules.
        let spelling = if children[1].kind() == "call_expression" {
            // A scope-function call on a branch-local receiver stores the
            // per-arm binding shapes as the local's branches, so a result
            // that is exactly the local (`g2`) binds through the same branch
            // rules as a direct branch result; other scope-function calls
            // store their receiver-qualified lambda-result spelling.
            match kotlin_scope_branch_local_scope_call_shapes(children[1], &locals, source)? {
                Some(shapes) => KotlinScopeLocalSpelling::Branches(
                    shapes
                        .into_iter()
                        .filter_map(|shape| KotlinScopeLocalBranch::from_binding(&shape))
                        .collect(),
                ),
                None => match kotlin_scope_function_binding(children[1], source)? {
                    Some((type_name, _, _)) if !type_name.is_empty() => {
                        KotlinScopeLocalSpelling::Single(format!("{type_name}()"))
                    }
                    Some((_, _, Some(chain))) => KotlinScopeLocalSpelling::Single(chain),
                    Some((_, _, None)) => return Ok(None),
                    None => KotlinScopeLocalSpelling::Single(initializer.clone()),
                },
            }
        } else if matches!(
            children[1].kind(),
            "if_expression" | "when_expression" | "binary_expression"
        ) {
            match kotlin_scope_lambda_branch_shapes(
                children[1],
                &locals,
                scope_name,
                param_name,
                receiver,
                source,
            )? {
                Some(shapes) if !shapes.is_empty() => KotlinScopeLocalSpelling::Branches(
                    shapes
                        .into_iter()
                        .filter_map(|shape| KotlinScopeLocalBranch::from_binding(&shape))
                        .collect(),
                ),
                _ => KotlinScopeLocalSpelling::Single(initializer.clone()),
            }
        } else {
            KotlinScopeLocalSpelling::Single(initializer.clone())
        };
        locals.insert(name, spelling);
    }
    Ok(Some(locals))
}

/// Expands the local-name roots of a multi-statement scope-function lambda
/// result expression to their `val` initializer spellings, so a body such as
/// `val g = it.make(); g` reduces to the single expression `it.make()` before
/// the caller's receiver rewriting. Only a leading identifier root that names
/// a declared local is substituted (`g` in `g.items[0].item`); any other
/// result text is returned unchanged and the caller's receiver rewrite fails
/// closed on unknown roots. A root that names a branch local has no single
/// spelling, so the result returns `None` and the caller fails closed unless
/// it resolves through the branch-result rules.
fn kotlin_scope_lambda_result_spelling(
    result: Node<'_>,
    locals: &KotlinScopeLocals,
    source: &str,
) -> Result<Option<String>> {
    let text = node_text(result, source)?.trim().to_string();
    Ok(kotlin_scope_lambda_result_spelling_text(&text, locals))
}

fn kotlin_scope_lambda_result_spelling_text(
    text: &str,
    locals: &KotlinScopeLocals,
) -> Option<String> {
    if text.is_empty() {
        return Some(text.to_string());
    }
    let root = text.split(['.', '[', '(']).next().unwrap_or_default();
    match locals.get(root) {
        Some(KotlinScopeLocalSpelling::Single(initializer)) => {
            Some(format!("{initializer}{}", &text[root.len()..]))
        }
        Some(KotlinScopeLocalSpelling::Branches(_)) => None,
        None => Some(text.to_string()),
    }
}

/// Resolves a single expression of a scope-function lambda to its
/// receiver-qualified binding shape: one branch of an `if`/`when`/elvis
/// result, or the single result expression of a `let`/`run`/`with` body. An
/// expression that is itself a scope-function call (such as
/// `it.make().let { g -> g }` or `g1.run { g -> g }` with a local `g1`)
/// first resolves through the same nested receiver rules as a
/// scope-function initializer to its lambda-result spelling, then expands its
/// `val` local roots; other expressions keep their text. The resulting text
/// is rewritten through the outer lambda's receiver rules
/// (`it.`/`{param}.`/`this`/unqualified), and classifies into a dotted
/// factory callee (`h.make`), a property chain (`h.make().items[0].item`), or
/// a bare receiver (`h`). A nested factory-call callee keeps its type-name
/// form through the outer rewrite so trace time resolves it as a factory call
/// rather than a property chain. Unsupported expressions return `None` so
/// scope-function bindings fail closed.
fn kotlin_scope_lambda_expression_binding(
    expression: Node<'_>,
    locals: &KotlinScopeLocals,
    scope_name: &str,
    param_name: Option<&str>,
    receiver: &str,
    source: &str,
) -> Result<Option<KotlinScopeFunctionBinding>> {
    // An expression that is itself a scope-function call binds its
    // receiver-qualified lambda result first (a dotted factory callee such as
    // `it.make`, a property chain, or the nested receiver itself), so the
    // outer lambda can continue through the nested call's result type.
    if expression.kind() == "call_expression"
        && let Some((nested_type, _, nested_chain)) =
            kotlin_scope_function_binding(expression, source)?
    {
        let nested_is_type = !nested_type.is_empty();
        let spelling = if nested_is_type {
            nested_type
        } else if let Some(chain) = nested_chain {
            chain
        } else {
            return Ok(None);
        };
        let Some(expanded) = kotlin_scope_lambda_result_spelling_text(&spelling, locals) else {
            return Ok(None);
        };
        // A `let` lambda only references its receiver through `it`/the
        // explicit parameter, so a nested spelling that roots on another name
        // already refers to the enclosing scope (such as the `nullableH.make`
        // of `nullableH?.let { it.make() }` inside `h.let { ... }`) and is
        // kept as a qualified spelling; `run`/`with` spellings always route
        // through the receiver hop and keep failing closed on unknown roots.
        if scope_name == "let"
            && kotlin_scope_function_body_rewrite(&expanded, scope_name, param_name, receiver)
                .is_none()
        {
            if nested_is_type {
                return Ok(Some((expanded, None, None)));
            }
            return Ok(kotlin_scope_body_binding(&expanded));
        }
        let Some(rewritten) =
            kotlin_scope_function_body_rewrite(&expanded, scope_name, param_name, receiver)
        else {
            return Ok(None);
        };
        if nested_is_type {
            return Ok(Some((rewritten, None, None)));
        }
        return Ok(kotlin_scope_body_binding(&rewritten));
    }
    let Some(text) = kotlin_scope_lambda_result_spelling(expression, locals, source)? else {
        return Ok(None);
    };
    if text.is_empty() {
        return Ok(None);
    }
    let Some(rewritten) =
        kotlin_scope_function_body_rewrite(&text, scope_name, param_name, receiver)
    else {
        // A `let` lambda whose result roots on an enclosing-scope reference
        // (such as the `nullableH.make()` of `h.let { nullableH.make() }` or
        // the `nullableH.make()` reached through the local of
        // `h.let { val g1 = nullableH; g1.make() }`) keeps the
        // already-qualified spelling because a `let` lambda only references
        // its receiver through `it`/the explicit parameter; trace time
        // resolves the root against the enclosing scope and fails closed on
        // unknown roots. `run`/`with` outers keep failing closed on unknown
        // roots, matching the documented unqualified-body rule.
        if scope_name == "let" {
            return Ok(kotlin_scope_body_binding(&text));
        }
        return Ok(None);
    };
    Ok(kotlin_scope_body_binding(&rewritten))
}

/// Returns the bare receiver-qualified spelling of a scope-function lambda
/// binding shape: the dotted factory-callee name when the shape is a callee,
/// or the property-chain / bare-receiver spelling otherwise.
fn kotlin_scope_function_binding_spelling(binding: &KotlinScopeFunctionBinding) -> Option<String> {
    let (type_name, _, chain) = binding;
    if !type_name.is_empty() {
        Some(type_name.clone())
    } else {
        chain.clone()
    }
}

/// Collects the receiver-qualified binding shapes of a scope-function lambda
/// result that is an `if`/`when` or elvis (`?:`) expression, such as the
/// `it.make()` and `it.makeAlt()` shapes of
/// `h.let { if (flag) it.make() else it.makeAlt() }`. Each arm expands its
/// `val` local roots and rewrites through the same receiver rules as a
/// single-expression body, keeping the call-callee / property-chain
/// classification so a branch local can expand a chain base with the correct
/// call marker; a branch arm that roots on a branch local flattens to the
/// local's arm shapes, so `if (flag()) g1 else it.makeAlt()` over a branch
/// local `g1` binds through the same branch rules as a direct branch result.
/// An `if` must have an `else` arm (or else-if arms) with at least two
/// distinct spellings, a `when` must include an `else` arm, and an elvis
/// expression must use the `?:` operator with both operands, otherwise the
/// whole binding fails closed.
fn kotlin_scope_lambda_branch_shapes(
    result: Node<'_>,
    locals: &KotlinScopeLocals,
    scope_name: &str,
    param_name: Option<&str>,
    receiver: &str,
    source: &str,
) -> Result<Option<Vec<KotlinScopeFunctionBinding>>> {
    let mut shapes = Vec::new();
    let mut seen = BTreeSet::new();
    if result.kind() == "if_expression" {
        let mut cursor = result.walk();
        let children = result.named_children(&mut cursor).collect::<Vec<_>>();
        // The first named child is the condition; the rest are then/else arms
        // (an `else if` arm is itself a nested `if_expression`).
        for branch in children.iter().skip(1) {
            if matches!(branch.kind(), "if_expression" | "when_expression") {
                let Some(nested) = kotlin_scope_lambda_branch_shapes(
                    *branch, locals, scope_name, param_name, receiver, source,
                )?
                else {
                    return Ok(None);
                };
                for shape in nested {
                    if let Some(spelling) = kotlin_scope_function_binding_spelling(&shape)
                        && seen.insert(spelling)
                    {
                        shapes.push(shape);
                    }
                }
            } else {
                let Some(arm_shapes) = kotlin_scope_lambda_branch_arm_shapes(
                    *branch, locals, scope_name, param_name, receiver, source,
                )?
                else {
                    return Ok(None);
                };
                for shape in arm_shapes {
                    if let Some(spelling) = kotlin_scope_function_binding_spelling(&shape)
                        && seen.insert(spelling)
                    {
                        shapes.push(shape);
                    }
                }
            }
        }
        // An `if` without an `else` arm has no value type to bind, and an
        // `if` whose distinct branches collapse to a single spelling binds
        // nothing either; a single arm that flattens a branch local still
        // needs a real `else` arm before any binding is recorded.
        let arm_count = children.len().saturating_sub(1);
        if arm_count < 2 || shapes.len() < 2 {
            return Ok(None);
        }
        return Ok(Some(shapes));
    }
    if result.kind() == "when_expression" {
        let mut cursor = result.walk();
        let mut has_else = false;
        for entry in result.named_children(&mut cursor) {
            if entry.kind() != "when_entry" {
                continue;
            }
            let mut entry_cursor = entry.walk();
            let entry_children = entry.named_children(&mut entry_cursor).collect::<Vec<_>>();
            // An `else` arm has no condition child, only the body expression.
            has_else |= entry_children.len() == 1;
            let Some(body) = entry_children.last().copied() else {
                return Ok(None);
            };
            let Some(arm_shapes) = kotlin_scope_lambda_branch_arm_shapes(
                body, locals, scope_name, param_name, receiver, source,
            )?
            else {
                return Ok(None);
            };
            for shape in arm_shapes {
                if let Some(spelling) = kotlin_scope_function_binding_spelling(&shape)
                    && seen.insert(spelling)
                {
                    shapes.push(shape);
                }
            }
        }
        // A `when` used as an expression without an `else` arm has no value
        // type to bind (its type includes `Unit`), so it fails closed.
        if !has_else || shapes.is_empty() {
            return Ok(None);
        }
        return Ok(Some(shapes));
    }
    if result.kind() == "binary_expression" {
        let mut cursor = result.walk();
        let children = result.named_children(&mut cursor).collect::<Vec<_>>();
        if children.len() != 2 {
            return Ok(None);
        }
        // The `?:` operator sits between the two operand nodes; other binary
        // operators fail closed.
        let operator = source
            .get(children[0].end_byte()..children[1].start_byte())
            .unwrap_or_default()
            .trim();
        if operator != "?:" {
            return Ok(None);
        }
        for operand in [children[0], children[1]] {
            let Some(arm_shapes) = kotlin_scope_lambda_branch_arm_shapes(
                operand, locals, scope_name, param_name, receiver, source,
            )?
            else {
                return Ok(None);
            };
            shapes.extend(arm_shapes);
        }
        return Ok(Some(shapes));
    }
    Ok(None)
}

/// Returns the deduplicated receiver-qualified branch spellings of a
/// scope-function lambda result that is an `if`/`when` or elvis (`?:`)
/// expression, such as the `it.make()` and `it.makeAlt()` branches of
/// `h.let { if (flag) it.make() else it.makeAlt() }`, by mapping each binding
/// shape of [`kotlin_scope_lambda_branch_shapes`] to its bare spelling.
fn kotlin_scope_lambda_branch_spellings(
    result: Node<'_>,
    locals: &KotlinScopeLocals,
    scope_name: &str,
    param_name: Option<&str>,
    receiver: &str,
    source: &str,
) -> Result<Option<Vec<String>>> {
    let Some(shapes) = kotlin_scope_lambda_branch_shapes(
        result, locals, scope_name, param_name, receiver, source,
    )?
    else {
        return Ok(None);
    };
    let mut spellings = Vec::new();
    let mut seen = BTreeSet::new();
    for shape in shapes {
        let Some(spelling) = kotlin_scope_function_binding_spelling(&shape) else {
            return Ok(None);
        };
        if seen.insert(spelling.clone()) {
            spellings.push(spelling);
        }
    }
    if spellings.is_empty() {
        return Ok(None);
    }
    Ok(Some(spellings))
}

/// Expands a scope-function lambda result whose root is a branch local into
/// the local's branch spellings with the chain suffix applied, so
/// `g1.items[0].item` over a branch local whose arms are `h.make()` and
/// `h.makeAlt()` expands to `h.make().items[0].item` and
/// `h.makeAlt().items[0].item`. A bare result (`g1`) converts each stored
/// callee back to its bare branch-initializer spelling; a result that does
/// not root on a branch local returns `None`.
fn kotlin_scope_lambda_local_branch_spellings(
    text: &str,
    locals: &KotlinScopeLocals,
) -> Option<Vec<String>> {
    if text.is_empty() {
        return None;
    }
    let root = text.split(['.', '[', '(']).next().unwrap_or_default();
    let Some(KotlinScopeLocalSpelling::Branches(branches)) = locals.get(root) else {
        return None;
    };
    if branches.is_empty() {
        return None;
    }
    let suffix = &text[root.len()..];
    Some(
        branches
            .iter()
            .map(|branch| {
                if suffix.is_empty() {
                    branch.branch_spelling()
                } else {
                    branch.with_suffix(suffix)
                }
            })
            .collect(),
    )
}

/// Expands a scope-function lambda result whose root is a branch local into
/// the local's binding shapes with the chain suffix applied, so a branch arm
/// that is exactly the local (`g1`) or a chain rooted on it (`g1.make()`)
/// flattens into the same call-callee / property-chain shapes as the local's
/// arms. A result that does not root on a branch local, or a chain expansion
/// that cannot classify into a call-callee or property-chain shape, returns
/// `None` so the consuming branch fails closed.
fn kotlin_scope_lambda_local_branch_shapes(
    text: &str,
    locals: &KotlinScopeLocals,
) -> Option<Vec<KotlinScopeFunctionBinding>> {
    if text.is_empty() {
        return None;
    }
    let root = text.split(['.', '[', '(']).next().unwrap_or_default();
    let Some(KotlinScopeLocalSpelling::Branches(branches)) = locals.get(root) else {
        return None;
    };
    if branches.is_empty() {
        return None;
    }
    let suffix = &text[root.len()..];
    let mut shapes = Vec::new();
    for branch in branches {
        let shape = if suffix.is_empty() {
            match branch {
                KotlinScopeLocalBranch::Callee(callee) => (callee.clone(), None, None),
                KotlinScopeLocalBranch::Chain(chain) => (String::new(), None, Some(chain.clone())),
            }
        } else {
            kotlin_scope_body_binding(&branch.with_suffix(suffix))?
        };
        shapes.push(shape);
    }
    Some(shapes)
}

/// Resolves one scope-function lambda branch arm to its binding shapes: an
/// arm that roots on a branch local flattens to the local's shapes (with any
/// chain suffix), otherwise the arm resolves through the single-expression
/// rules of [`kotlin_scope_lambda_expression_binding`].
fn kotlin_scope_lambda_branch_arm_shapes(
    arm: Node<'_>,
    locals: &KotlinScopeLocals,
    scope_name: &str,
    param_name: Option<&str>,
    receiver: &str,
    source: &str,
) -> Result<Option<Vec<KotlinScopeFunctionBinding>>> {
    // An arm that is itself a scope-function call on a branch-local receiver
    // (such as `g1.let { g -> g.items[0].item }` over a branch local `g1`)
    // binds the nested lambda once per branch arm, flattening into the same
    // shapes as the local's arms.
    if arm.kind() == "call_expression"
        && let Some(shapes) = kotlin_scope_branch_local_scope_call_shapes(arm, locals, source)?
    {
        return Ok(Some(shapes));
    }
    let arm_text = node_text(arm, source)?.trim().to_string();
    if let Some(shapes) = kotlin_scope_lambda_local_branch_shapes(&arm_text, locals) {
        return Ok(Some(shapes));
    }
    let Some(shape) = kotlin_scope_lambda_expression_binding(
        arm, locals, scope_name, param_name, receiver, source,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(vec![shape]))
}

/// Binds a scope-function lambda result that is itself a scope-function call
/// on a branch-local receiver, such as the `g1.let { g -> g.items[0].item }`
/// of `h.let { val g1 = if (flag()) it.make() else it.makeAlt();
/// g1.let { g -> g.items[0].item } }`, by binding the nested lambda once per
/// branch arm with the arm's receiver spelling (a factory-callee arm keeps
/// its call marker so the nested chain walks the hop as a call), so the outer
/// initializer or branch arm binds through the local's branch spellings.
/// `apply`/`also` outers, receivers that are not branch locals, malformed
/// calls or lambdas, and arms that cannot bind fail closed.
fn kotlin_scope_branch_local_scope_call_shapes(
    call: Node<'_>,
    locals: &KotlinScopeLocals,
    source: &str,
) -> Result<Option<Vec<KotlinScopeFunctionBinding>>> {
    if call.kind() != "call_expression" {
        return Ok(None);
    }
    let mut cursor = call.walk();
    let children = call.named_children(&mut cursor).collect::<Vec<_>>();
    if children.len() != 2 {
        return Ok(None);
    }
    let Some(lambda) = children
        .iter()
        .find(|child| child.kind() == "annotated_lambda")
        .copied()
    else {
        return Ok(None);
    };
    let Some((receiver_text, scope_name)) = kotlin_scope_function_callee(children[0], source)?
    else {
        return Ok(None);
    };
    if !matches!(scope_name.as_str(), "let" | "run" | "with") {
        return Ok(None);
    }
    // The nested receiver must name a branch local so the lambda binds once
    // per arm; any other receiver falls through to the caller's rules.
    let Some(KotlinScopeLocalSpelling::Branches(branches)) = locals.get(&receiver_text) else {
        return Ok(None);
    };
    if branches.is_empty() {
        return Ok(None);
    }
    let Some((param_name, statements, result)) = kotlin_scope_lambda_body(lambda, source)? else {
        return Ok(None);
    };
    let mut shapes = Vec::new();
    let mut seen = BTreeSet::new();
    for branch in branches {
        let receiver = branch.with_suffix("");
        let Some(nested_locals) = kotlin_scope_lambda_locals(
            &statements,
            source,
            &scope_name,
            param_name.as_deref(),
            &receiver,
        )?
        else {
            return Ok(None);
        };
        let Some(shape) = kotlin_scope_lambda_expression_binding(
            result,
            &nested_locals,
            &scope_name,
            param_name.as_deref(),
            &receiver,
            source,
        )?
        else {
            return Ok(None);
        };
        let Some(spelling) = kotlin_scope_function_binding_spelling(&shape) else {
            return Ok(None);
        };
        if seen.insert(spelling) {
            shapes.push(shape);
        }
    }
    if shapes.len() < 2 {
        return Ok(None);
    }
    Ok(Some(shapes))
}

/// Returns the deduplicated branch spellings of a scope-function call on a
/// branch-local receiver, mapped from the binding shapes of
/// [`kotlin_scope_branch_local_scope_call_shapes`].
fn kotlin_scope_branch_local_scope_call_spellings(
    call: Node<'_>,
    locals: &KotlinScopeLocals,
    source: &str,
) -> Result<Option<Vec<String>>> {
    let Some(shapes) = kotlin_scope_branch_local_scope_call_shapes(call, locals, source)? else {
        return Ok(None);
    };
    let mut spellings = Vec::new();
    for shape in shapes {
        let Some(spelling) = kotlin_scope_function_binding_spelling(&shape) else {
            return Ok(None);
        };
        spellings.push(spelling);
    }
    if spellings.is_empty() {
        return Ok(None);
    }
    Ok(Some(spellings))
}

/// Extracts a branch initializer binding from a `val`/`var` declaration whose
/// initializer is a scope-function call whose lambda result is an `if`/`when`
/// or elvis (`?:`) expression, such as
/// `val group = h.let { if (flag) it.make() else it.makeAlt() }` or
/// `val first = h.let { when (flag) { true -> it.make().items[0].item; else ->
/// it.makeAlt().items[0].item } }`, or whose lambda result is exactly a `val`
/// local with an `if`/`when`/elvis initializer, such as
/// `val group = h.let { val g1 = if (flag) it.make() else it.makeAlt(); g1 }`,
/// or a chain rooted on such a local, such as
/// `val first = h.let { val g1 = if (flag) it.make() else it.makeAlt();
/// g1.items[0].item }`. Each branch is expanded through the lambda's `val`
/// locals and rewritten to
/// the receiver-qualified spelling of a direct branch initializer, and the
/// common type is resolved at trace time through the same branch rules; an
/// explicitly typed declaration binds through the declared type instead.
/// Unknown terminal methods, divergent branch types, an `if` without an
/// `else` arm, and malformed lambda bodies fail closed.
fn kotlin_scope_branch_initializer_binding(
    property: Node<'_>,
    source: &str,
) -> Result<Option<KotlinBranchInitializerBinding>> {
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
    // An explicitly typed declaration binds through the declared type, not
    // the initializer branches.
    if variable_children
        .iter()
        .any(|child| kotlin_is_type_node_kind(child.kind()))
    {
        return Ok(None);
    }
    let Some(initializer) = children
        .iter()
        .find(|child| child.kind() == "call_expression")
        .copied()
    else {
        return Ok(None);
    };
    let mut cursor = initializer.walk();
    let call_children = initializer.named_children(&mut cursor).collect::<Vec<_>>();
    if call_children.len() != 2 {
        return Ok(None);
    }
    let Some(lambda) = call_children
        .iter()
        .find(|child| child.kind() == "annotated_lambda")
        .copied()
    else {
        return Ok(None);
    };
    let callee = call_children[0];
    let Some((receiver, scope_name)) = kotlin_scope_function_callee(callee, source)? else {
        return Ok(None);
    };
    // `apply`/`also` return the receiver regardless of the lambda body, so
    // only result-bearing scope functions can bind through a branch result.
    if !matches!(scope_name.as_str(), "let" | "run" | "with") {
        return Ok(None);
    }
    let Some((param_name, statements, result)) = kotlin_scope_lambda_body(lambda, source)? else {
        return Ok(None);
    };
    let Some(locals) = kotlin_scope_lambda_locals(
        &statements,
        source,
        &scope_name,
        param_name.as_deref(),
        &receiver,
    )?
    else {
        return Ok(None);
    };
    let Some(branches) = (if matches!(
        result.kind(),
        "if_expression" | "when_expression" | "binary_expression"
    ) {
        kotlin_scope_lambda_branch_spellings(
            result,
            &locals,
            &scope_name,
            param_name.as_deref(),
            &receiver,
            source,
        )?
    } else if let Some(scope_call_branches) =
        kotlin_scope_branch_local_scope_call_spellings(result, &locals, source)?
    {
        // A result that is itself a scope-function call on a branch-local
        // receiver, such as `g1.let { g -> g.items[0].item }` over
        // `val g1 = if (flag()) it.make() else it.makeAlt()`, binds the
        // nested lambda once per branch arm so the outer initializer binds
        // through the local's branch spellings.
        Some(scope_call_branches)
    } else {
        // A result that is exactly a `val` local whose initializer is an
        // `if`/`when`/elvis branch, or a chain rooted on such a local, expands
        // to the local's branch spellings (each arm converted back to its
        // branch-initializer spelling, or with the chain suffix applied), so
        // `h.let { val g1 = if (flag()) it.make() else it.makeAlt(); g1 }` and
        // `h.let { val g1 = if (flag()) it.make() else it.makeAlt();
        // g1.items[0].item }` bind through the same branch rules as a direct
        // branch result.
        let result_text = node_text(result, source)?.trim().to_string();
        kotlin_scope_lambda_local_branch_spellings(&result_text, &locals)
    }) else {
        return Ok(None);
    };
    if branches.is_empty() {
        return Ok(None);
    }
    Ok(Some((name, branches)))
}

/// Classifies a receiver-qualified scope-function lambda body spelling into a
/// property binding shape: a dotted factory-call body such as `h.make()`
/// records the callee `h.make` as a type name, a chain body such as
/// `h.make().items[0].item` records the dotted chain as a property-chain
/// base, and a plain identifier body such as `h` records the receiver as a
/// type name. Unsupported spellings return `None` so scope-function
/// initializers fail closed.
fn kotlin_scope_body_binding(rewritten: &str) -> Option<(String, Option<String>, Option<String>)> {
    if let Some(callee) = rewritten.strip_suffix("()")
        && !callee.is_empty()
        && !callee.contains('[')
        && callee.split('.').all(kotlin_property_chain_hop_valid)
    {
        return Some((callee.to_string(), None, None));
    }
    if rewritten.contains(['[', '.']) && rewritten.split('.').all(kotlin_property_chain_hop_valid) {
        return Some((String::new(), None, Some(rewritten.to_string())));
    }
    if !rewritten.is_empty()
        && rewritten
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Some((String::new(), None, Some(rewritten.to_string())));
    }
    None
}

/// Rewrites a scope-function lambda body expression spelling to the
/// receiver-qualified spelling used by direct initializers: `let` lambdas
/// receive `it` (or an explicit parameter name), so the body must reference
/// the receiver through that name (`it.`/`{param}.`) or as the bare name
/// itself; `run`/`with` lambdas reference the receiver as `this` (or through
/// an explicit parameter name when one is declared), so unqualified bodies,
/// `this`-rooted bodies such as `this.make()`, and parameter-rooted bodies
/// such as `holder.make()` are rewritten with a leading receiver hop (a bare
/// `this` or bare parameter body rewrites to the receiver itself, since
/// `run`/`with` return the lambda result). Returns `None` when the body roots
/// on an unknown name (`it.`/`super.` inside `run`/`with`, or a name other
/// than the `let` receiver reference) so scope-function initializers fail
/// closed.
fn kotlin_scope_function_body_rewrite(
    body_text: &str,
    scope_name: &str,
    param_name: Option<&str>,
    receiver: &str,
) -> Option<String> {
    if scope_name == "let" {
        let reference = param_name.unwrap_or("it");
        if let Some(rest) = body_text.strip_prefix(&format!("{reference}.")) {
            return Some(format!("{receiver}.{rest}"));
        }
        if body_text == reference {
            return Some(receiver.to_string());
        }
        return None;
    }
    // `run` and `with` lambdas reference the receiver as `this` (or through
    // an explicit parameter name when one is declared), so unqualified
    // bodies, `this`-rooted bodies such as `this.make()`, and
    // parameter-rooted bodies such as `holder.make()` are rewritten with a
    // leading receiver hop (a bare `this` or bare parameter body rewrites to
    // the receiver itself, since `run`/`with` return the lambda result);
    // bodies that use another explicit root fail closed.
    if body_text.starts_with("it.") || body_text == "it" || body_text.starts_with("super.") {
        return None;
    }
    if let Some(rest) = body_text.strip_prefix("this.") {
        return Some(format!("{receiver}.{rest}"));
    }
    if body_text == "this" {
        return Some(receiver.to_string());
    }
    if let Some(param) = param_name {
        if let Some(rest) = body_text.strip_prefix(&format!("{param}.")) {
            return Some(format!("{receiver}.{rest}"));
        }
        if body_text == param {
            return Some(receiver.to_string());
        }
        return Some(format!("{receiver}.{body_text}"));
    }
    Some(format!("{receiver}.{body_text}"))
}

/// Recognizes a scope-function call initializer such as `h.let { it.make() }`,
/// `h.run { make() }`, `h.apply { }`, `with(h) { make() }`, or
/// `h.also { it.consume() }` and returns a property binding shape in the same
/// form as other initializers: a dotted factory-call type name when the
/// lambda result is a receiver-qualified factory call (`h.let { it.make() }`
/// pins the type to `h.make`), a property-chain base when the result is a
/// receiver-qualified chain (`h.let { it.make().items[0].item }`), or a
/// receiver type name when the scope function returns its receiver
/// (`h.apply { }`, `h.also { ... }`, or `h.let { it }`). `run`/`with` lambdas
/// reference the receiver unqualified, so their bodies are rewritten with a
/// leading receiver hop. A lambda body may declare `val` locals before its
/// result expression (such as `val g = it.make(); g`), which expand to the
/// result's receiver-qualified spelling through the same rules as a
/// single-expression body. Unknown scope names, malformed bodies, chained
/// locals, and non-plain receivers return `None` so scope-function
/// initializers fail closed.
fn kotlin_scope_function_binding(
    initializer: Node<'_>,
    source: &str,
) -> Result<Option<KotlinScopeFunctionBinding>> {
    if initializer.kind() != "call_expression" {
        return Ok(None);
    }
    let mut cursor = initializer.walk();
    let children = initializer.named_children(&mut cursor).collect::<Vec<_>>();
    if children.len() != 2 {
        return Ok(None);
    }
    let Some(lambda) = children
        .iter()
        .find(|child| child.kind() == "annotated_lambda")
        .copied()
    else {
        return Ok(None);
    };
    let callee = children[0];
    let Some((receiver, scope_name)) = kotlin_scope_function_callee(callee, source)? else {
        return Ok(None);
    };
    match scope_name.as_str() {
        // `apply` and `also` return the receiver regardless of the lambda
        // body, so the binding records the receiver spelling as a
        // property-chain base whose terminal type resolves through the same
        // bound-receiver rules as a bare `val holder = h` initializer.
        "apply" | "also" => Ok(Some((String::new(), None, Some(receiver)))),
        "let" | "run" | "with" => {
            let Some((param_name, statements, result)) = kotlin_scope_lambda_body(lambda, source)?
            else {
                return Ok(None);
            };
            let Some(locals) = kotlin_scope_lambda_locals(
                &statements,
                source,
                &scope_name,
                param_name.as_deref(),
                &receiver,
            )?
            else {
                return Ok(None);
            };
            // The result expression resolves through the same rules as a
            // branch: a result that is itself a scope-function call (such as
            // `it.make().let { g -> g }` or the enclosing-member-rooted
            // `nullableH?.let { it.make() }`) binds its nested
            // receiver-qualified binding first, then expands local roots and
            // applies the outer receiver rewrite (a `let` keeps an
            // already-qualified enclosing-scope spelling).
            Ok(kotlin_scope_lambda_expression_binding(
                result,
                &locals,
                &scope_name,
                param_name.as_deref(),
                &receiver,
                source,
            )?)
        }
        _ => Ok(None),
    }
}

fn kotlin_property_binding(
    property: Node<'_>,
    source: &str,
    bindings: &KotlinReceiverTypeBindings,
) -> Result<Option<KotlinPropertyBinding>> {
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
        return Ok(Some((name, type_name, None, None)));
    }
    // Fall back to a constructor-call initializer such as `val x = Other()` or
    // `val x = Outer.Inner()`, an element-access initializer such as
    // `val x = items[0]`, or a zero-argument call whose callee is a
    // navigation chain with an element-access hop such as
    // `val x = h.items[0].make()` (recorded as a property chain); a
    // parenthesized initializer such as `(Other())`, `(makeItems())`, or
    // `(items[0])` and a postfix force-unwrap initializer such as
    // `makeNullable()!!` unwrap to the same inner expression so `val` locals
    // bind the same receiver type as the unwrapped form. Qualified callees
    // must be pure identifier chains, and element-access bases follow the
    // plain/qualified/factory rules below.
    let initializer = children
        .iter()
        .find(|child| {
            matches!(
                child.kind(),
                "call_expression"
                    | "index_expression"
                    | "navigation_expression"
                    | "parenthesized_expression"
                    | "unary_expression"
                    | "identifier"
            )
        })
        .copied();
    let Some(initializer) = initializer else {
        return Ok(None);
    };
    let Some(initializer) = kotlin_initializer_expression(initializer) else {
        return Ok(None);
    };
    // A scope-function call initializer such as `val group = h.let { it.make() }`,
    // `val group = h.run { make() }`, `val holder = h.apply { }`, or
    // `val group = with(h) { make() }` binds the receiver-qualified lambda
    // result through the same rules as a direct initializer: a dotted factory
    // call (`h.make`), a property chain (`h.make().items[0].item`), or the
    // receiver type itself (`h` for `apply`/`also`); unknown scope names,
    // malformed lambda bodies, and non-plain receivers fail closed and fall
    // through to the generic callee binding below.
    if initializer.kind() == "call_expression"
        && let Some((type_name, element_access_base, property_chain_base)) =
            kotlin_scope_function_binding(initializer, source)?
    {
        return Ok(Some((
            name,
            type_name,
            element_access_base,
            property_chain_base,
        )));
    }
    if initializer.kind() == "call_expression"
        && let Some(callee) = initializer.named_child(0)
        && let Some(type_name) = kotlin_call_initializer_callee_name(callee, source)?
        && !type_name.is_empty()
    {
        return Ok(Some((name, type_name, None, None)));
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
            return Ok(Some((name, component_type, None, None)));
        }
        if base_name.contains('.') || base_name.ends_with("()") {
            return Ok(Some((name, String::new(), Some(base_name), None)));
        }
        // A plain bare base that is not bound locally, as an enclosing-class
        // property, or as a companion member may be a same-package or
        // explicitly imported top-level array property such as `itemGroup` in
        // `val first = itemGroup[0]` with `val itemGroup: Array<Holder>` at
        // package scope, resolved at trace time; a bound non-array base fails
        // closed because element access on a non-array is invalid.
        if !bindings.contains(&base_name) {
            return Ok(Some((name, String::new(), Some(base_name), None)));
        }
    }
    // A plain property-chain initializer such as `val first = holder.item`,
    // `val first = this.holder.item`, or `val first = super.baseItem`, and an
    // element-access initializer whose base chain contains a method-call hop
    // such as `val group = h.make().items[0]` records the dotted chain
    // spelling so trace-time resolution can walk each hop to the terminal
    // property type (including inherited properties). Unknown or
    // unresolvable chains fail closed there; parenthesized and force-unwrapped
    // spellings unwrap through `kotlin_initializer_expression` above.
    if matches!(
        initializer.kind(),
        "navigation_expression" | "call_expression" | "index_expression"
    ) && let Some(chain) = kotlin_property_chain_initializer(initializer, source)?
    {
        return Ok(Some((name, String::new(), None, Some(chain))));
    }
    // A bare property initializer such as `val first = item` or
    // `val first = items` records the property name as a single-hop property
    // chain so trace-time resolution can walk the enclosing type's own or
    // inherited property (including terminal array element access) to the
    // declared type; parenthesized and force-unwrapped spellings unwrap
    // through `kotlin_initializer_expression` above, and unknown or
    // unresolvable properties fail closed there.
    if initializer.kind() == "identifier" {
        let chain = node_text(initializer, source)?.trim().to_string();
        if !chain.is_empty() {
            return Ok(Some((name, String::new(), None, Some(chain))));
        }
    }
    Ok(None)
}

/// Returns the callee spelling of a call-initializer such as `Other` in
/// `val x = Other()`, `Util.makeItems` in `val x = Util.makeItems()`,
/// `this.ownMake` in `val x = this.ownMake()`,
/// `super.inheritedMake` in `val x = super.inheritedMake()`, or a chained
/// call such as `h.make().make` in `val x = h.make().make()`. Plain-identifier
/// and safe dotted navigation callees keep their spelling, and
/// `this`/`super`-rooted dotted callees keep the root so trace-time
/// resolution can dispatch the member function on the enclosing type or the
/// direct superclass; a call-expression receiver keeps its `()` marker so
/// trace-time resolution can walk the chained method-call hops as a receiver
/// chain. Parenthesized non-call roots and other non-name callees return
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
    if node.kind() == "call_expression"
        && let Some(callee) = node.named_child(0)
        && let Some(prefix) = kotlin_call_initializer_callee_name(callee, source)?
        && !prefix.is_empty()
    {
        return Ok(Some(format!("{prefix}()")));
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
/// `Util.makeItems()[0]`, `this.ownMake()[0]`,
/// `super.inheritedMake()[0]`, or the force-unwrapped spellings
/// `items!![0]`, `this.makeNullable()!![0]`, or `super.makeNullable()!![0]`.
/// Plain-identifier and dotted field-chain bases (including `this`- and
/// `super`-rooted chains) return their spelling, a postfix `!!` force-unwrap
/// on the base unwraps to the operand because it only strips nullability, and
/// a factory-call base with a plain, safe dotted, or `this`/`super`-rooted
/// callee returns the callee with a trailing `()` marker so trace-time
/// resolution can walk the factory's declared return array. Parenthesized
/// roots, nested element access, and function-call, multi-index, or nullable
/// subscripts return `None` so element-access-inferred bindings fail closed.
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
    // A postfix `!!` force-unwrap base such as `items!!` in `items!![0]` or
    // `this.makeNullable()!!` in `this.makeNullable()!![0]` unwraps to its
    // operand because `!!` only strips nullability without changing the
    // element component or receiver type; other unary operators fail closed.
    let mut base_node = children[0];
    while let Some(inner) = kotlin_force_unwrap_operand(base_node) {
        base_node = inner;
    }
    // A factory-call base such as `makeItems()`, `Util.makeItems()`,
    // `this.ownMake()`, or `super.inheritedMake()` records the callee with a
    // trailing `()` marker so trace-time resolution can walk the factory's
    // declared return array. Plain-identifier, safe dotted, and
    // `this`/`super`-rooted callees are accepted; parenthesized roots and
    // other non-name callees fail closed.
    if base_node.kind() == "call_expression" {
        let Some(callee) = base_node.named_child(0) else {
            return Ok(None);
        };
        let Some(callee_name) = kotlin_factory_call_callee_name(callee, source)? else {
            return Ok(None);
        };
        return Ok(Some(format!("{callee_name}()")));
    }
    let base = node_text(base_node, source)?.trim();
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
/// spellings such as `Array<Helper>`, `Array<Array<Helper>>`, or the nullable
/// spelling `Array<Helper>?` are kept as their raw spelling so the binding can
/// record the element component type or mark the receiver unusable. Other
/// complex and malformed spellings return `None` and fail closed.
fn kotlin_declared_type_name(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if let Some(normalized) = kotlin_dotted_type_name(trimmed) {
        return Some(normalized);
    }
    // A nullable array spelling such as `Array<Helper>?` keeps its raw
    // spelling (with the trailing `?`) so the binding records the element
    // component type just like the non-null spelling; nullability does not
    // change the component type.
    let without_nullable = trimmed.strip_suffix('?').map(str::trim).unwrap_or(trimmed);
    if without_nullable.starts_with("Array<") && without_nullable.ends_with('>') {
        return Some(trimmed.to_string());
    }
    None
}

/// Extracts the element component type name from a single-level Kotlin generic
/// array spelling such as `Array<Helper>`, `Array<Outer.Inner>`, or the
/// nullable spelling `Array<Helper>?`, normalizing the component through
/// `kotlin_dotted_type_name`. Nullability does not change the element component
/// type, so nullable single-level arrays bind the same component as non-null
/// arrays. Nested generic arrays such as `Array<Array<Helper>>`, primitive
/// arrays such as `IntArray`, malformed spellings, and non-`Array<...>`
/// spellings return `None` and fail closed.
pub(in crate::symbol_dependency) fn kotlin_array_type_component_name(text: &str) -> Option<String> {
    let name = text.trim();
    let rest = name.strip_prefix("Array<")?;
    let close = rest.rfind('>')?;
    // A trailing `?` marks a nullable array such as `Array<Helper>?`; the
    // element component type is unchanged by nullability. Double-nullable and
    // other trailing spellings still fail closed.
    match rest[close + 1..].trim() {
        "" | "?" => {}
        _ => return None,
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
    // A higher-priority local, parameter, or instance-property binding
    // replaces a same-named companion-member binding cleanly instead of
    // creating a false ambiguity.
    if bindings.shadowable_names.remove(&name) {
        bindings.types_by_name.remove(&name);
        bindings.array_component_types.remove(&name);
        bindings.element_access_bases.remove(&name);
        bindings.property_chain_bases.remove(&name);
        bindings.branch_initializers_by_name.remove(&name);
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
    // A higher-priority local, parameter, or instance-property binding
    // replaces a same-named companion-member binding cleanly instead of
    // creating a false ambiguity.
    if bindings.shadowable_names.remove(&name) {
        bindings.types_by_name.remove(&name);
        bindings.array_component_types.remove(&name);
        bindings.element_access_bases.remove(&name);
        bindings.property_chain_bases.remove(&name);
        bindings.branch_initializers_by_name.remove(&name);
    }
    if bindings.types_by_name.contains_key(&name)
        || bindings.array_component_types.contains_key(&name)
        || bindings.property_chain_bases.contains_key(&name)
        || bindings.branch_initializers_by_name.contains_key(&name)
        || bindings
            .element_access_bases
            .insert(name.clone(), base)
            .is_some()
    {
        bindings.types_by_name.remove(&name);
        bindings.array_component_types.remove(&name);
        bindings.element_access_bases.remove(&name);
        bindings.property_chain_bases.remove(&name);
        bindings.branch_initializers_by_name.remove(&name);
        bindings.ambiguous_names.insert(name);
    }
}

/// Records a name bound from a property-chain initializer such as
/// `val first = holder.item` or `val first = this.holder.item` under its
/// dotted chain spelling. The name shadows same-named objects and types; a
/// duplicate declaration of the same name fails closed as ambiguous.
fn insert_kotlin_property_chain_base_binding(
    bindings: &mut KotlinReceiverTypeBindings,
    name: String,
    chain: String,
) {
    if bindings.ambiguous_names.contains(&name) {
        return;
    }
    // A higher-priority local, parameter, or instance-property binding
    // replaces a same-named companion-member binding cleanly instead of
    // creating a false ambiguity.
    if bindings.shadowable_names.remove(&name) {
        bindings.types_by_name.remove(&name);
        bindings.array_component_types.remove(&name);
        bindings.element_access_bases.remove(&name);
        bindings.property_chain_bases.remove(&name);
        bindings.branch_initializers_by_name.remove(&name);
    }
    if bindings.types_by_name.contains_key(&name)
        || bindings.array_component_types.contains_key(&name)
        || bindings.element_access_bases.contains_key(&name)
        || bindings.branch_initializers_by_name.contains_key(&name)
        || bindings
            .property_chain_bases
            .insert(name.clone(), chain)
            .is_some()
    {
        bindings.types_by_name.remove(&name);
        bindings.array_component_types.remove(&name);
        bindings.element_access_bases.remove(&name);
        bindings.property_chain_bases.remove(&name);
        bindings.branch_initializers_by_name.remove(&name);
        bindings.ambiguous_names.insert(name);
    }
}

/// Records a name bound from an `if`/`when` expression initializer such as
/// `val group = if (flag) h.make() else Holder().make()` under its branch
/// initializer spellings. The name shadows same-named objects and types; a
/// duplicate declaration of the same name fails closed as ambiguous.
fn insert_kotlin_branch_initializer_binding(
    bindings: &mut KotlinReceiverTypeBindings,
    name: String,
    branches: Vec<String>,
) {
    if bindings.ambiguous_names.contains(&name) {
        return;
    }
    // A higher-priority local, parameter, or instance-property binding
    // replaces a same-named companion-member binding cleanly instead of
    // creating a false ambiguity.
    if bindings.shadowable_names.remove(&name) {
        bindings.types_by_name.remove(&name);
        bindings.array_component_types.remove(&name);
        bindings.element_access_bases.remove(&name);
        bindings.property_chain_bases.remove(&name);
        bindings.branch_initializers_by_name.remove(&name);
    }
    if bindings.types_by_name.contains_key(&name)
        || bindings.array_component_types.contains_key(&name)
        || bindings.element_access_bases.contains_key(&name)
        || bindings.property_chain_bases.contains_key(&name)
        || bindings
            .branch_initializers_by_name
            .insert(name.clone(), branches)
            .is_some()
    {
        bindings.types_by_name.remove(&name);
        bindings.array_component_types.remove(&name);
        bindings.element_access_bases.remove(&name);
        bindings.property_chain_bases.remove(&name);
        bindings.branch_initializers_by_name.remove(&name);
        bindings.ambiguous_names.insert(name);
    }
}

/// Inserts a companion-member branch-initializer binding with the same
/// shadowing discipline as `insert_kotlin_shadowable_receiver_binding`.
fn insert_kotlin_shadowable_branch_initializer_binding(
    bindings: &mut KotlinReceiverTypeBindings,
    name: String,
    branches: Vec<String>,
) {
    if bindings.ambiguous_names.contains(&name)
        || bindings.types_by_name.contains_key(&name)
        || bindings.array_component_types.contains_key(&name)
        || bindings.element_access_bases.contains_key(&name)
        || bindings.property_chain_bases.contains_key(&name)
        || bindings.branch_initializers_by_name.contains_key(&name)
    {
        return;
    }
    insert_kotlin_branch_initializer_binding(bindings, name.clone(), branches);
    bindings.shadowable_names.insert(name);
}

/// Inserts a companion-member receiver binding that is shadowed by any
/// higher-priority local, parameter, instance-property, or enclosing-class
/// binding of the same name, matching Kotlin's scope rules where locals
/// shadow members and instance members shadow companion members. Names
/// already bound (including ambiguous names) skip the companion binding
/// instead of creating a false ambiguity; a companion member that does not
/// collide binds like any other receiver.
fn insert_kotlin_shadowable_receiver_binding(
    bindings: &mut KotlinReceiverTypeBindings,
    name: String,
    type_name: String,
) {
    if bindings.ambiguous_names.contains(&name)
        || bindings.types_by_name.contains_key(&name)
        || bindings.array_component_types.contains_key(&name)
        || bindings.element_access_bases.contains_key(&name)
        || bindings.property_chain_bases.contains_key(&name)
    {
        return;
    }
    insert_kotlin_receiver_binding(bindings, name.clone(), type_name);
    bindings.shadowable_names.insert(name);
}

/// Inserts a companion-member element-access base binding with the same
/// shadowing discipline as `insert_kotlin_shadowable_receiver_binding`.
fn insert_kotlin_shadowable_element_access_base_binding(
    bindings: &mut KotlinReceiverTypeBindings,
    name: String,
    base: String,
) {
    if bindings.ambiguous_names.contains(&name)
        || bindings.types_by_name.contains_key(&name)
        || bindings.array_component_types.contains_key(&name)
        || bindings.element_access_bases.contains_key(&name)
        || bindings.property_chain_bases.contains_key(&name)
    {
        return;
    }
    insert_kotlin_element_access_base_binding(bindings, name.clone(), base);
    bindings.shadowable_names.insert(name);
}

/// Inserts a companion-member property-chain base binding with the same
/// shadowing discipline as `insert_kotlin_shadowable_receiver_binding`.
fn insert_kotlin_shadowable_property_chain_base_binding(
    bindings: &mut KotlinReceiverTypeBindings,
    name: String,
    chain: String,
) {
    if bindings.ambiguous_names.contains(&name)
        || bindings.types_by_name.contains_key(&name)
        || bindings.array_component_types.contains_key(&name)
        || bindings.element_access_bases.contains_key(&name)
        || bindings.property_chain_bases.contains_key(&name)
    {
        return;
    }
    insert_kotlin_property_chain_base_binding(bindings, name.clone(), chain);
    bindings.shadowable_names.insert(name);
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
        KotlinImportBinding, KotlinReceiverTypeBindings, kotlin_array_type_component_name,
        kotlin_declared_type_name, kotlin_dotted_type_name,
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
    fn kotlin_array_type_component_name_accepts_nullable_and_plain_arrays() {
        assert_eq!(
            kotlin_array_type_component_name("Array<Helper>").as_deref(),
            Some("Helper")
        );
        assert_eq!(
            kotlin_array_type_component_name("Array<Outer.Inner>").as_deref(),
            Some("Outer.Inner")
        );
        // Nullability does not change the element component type, so a
        // nullable single-level array binds the same component as the plain
        // spelling.
        assert_eq!(
            kotlin_array_type_component_name("Array<Helper>?").as_deref(),
            Some("Helper")
        );
        assert_eq!(
            kotlin_array_type_component_name("Array<Outer.Inner>?").as_deref(),
            Some("Outer.Inner")
        );
        // Double-nullable, nested generic, primitive-array, and malformed
        // spellings still fail closed.
        assert_eq!(kotlin_array_type_component_name("Array<Helper>??"), None);
        assert_eq!(
            kotlin_array_type_component_name("Array<Array<Helper>>?"),
            None
        );
        assert_eq!(kotlin_array_type_component_name("IntArray?"), None);
        assert_eq!(kotlin_array_type_component_name("Helper?"), None);
        assert_eq!(kotlin_array_type_component_name("Array<>?"), None);
        assert_eq!(kotlin_array_type_component_name(""), None);
    }

    #[test]
    fn kotlin_declared_type_name_accepts_nullable_array_spellings() {
        assert_eq!(
            kotlin_declared_type_name("Array<Helper>").as_deref(),
            Some("Array<Helper>")
        );
        // Nullable array spellings keep their raw spelling with the trailing
        // `?` so the binding records the same element component type as the
        // non-null spelling.
        assert_eq!(
            kotlin_declared_type_name("Array<Helper>?").as_deref(),
            Some("Array<Helper>?")
        );
        assert_eq!(
            kotlin_declared_type_name("Array<Outer.Inner>?").as_deref(),
            Some("Array<Outer.Inner>?")
        );
        // Primitive arrays already normalize through the dotted-name path, and
        // malformed or double-nullable spellings fail closed.
        assert_eq!(
            kotlin_declared_type_name("IntArray?").as_deref(),
            Some("IntArray")
        );
        assert_eq!(kotlin_declared_type_name("Array<Helper>??"), None);
        assert_eq!(kotlin_declared_type_name("Array<Helper"), None);
        assert_eq!(kotlin_declared_type_name(""), None);
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
    fn companion_property_receivers_bind_shadowably_and_feed_local_element_access() {
        let file = write_test_file(
            "package com.example\n\nclass Item {\n    fun helper(value: Int): Int = value\n}\n\nclass Helper {\n    fun inner(): Item = Item()\n}\n\nclass Util {\n    companion object {\n        val items: Array<Item> = arrayOf()\n        val nullableItems: Array<Item>? = arrayOf()\n        val groups: Array<Helper> = arrayOf()\n    }\n    fun run(): Int {\n        val first = items[0]\n        return first.helper(1)\n    }\n}\n",
        );
        let context = kotlin_import_context_for_file_with_overrides_and_deadline(
            &file.normalized_path,
            None,
            None,
        )
        .unwrap();
        // A companion array property binds its element component type, so a
        // body local such as `val first = items[0]` binds the element type and
        // direct element-access receivers can dispatch on it; nullability does
        // not change the component type.
        let run_bindings = context
            .receiver_type_bindings_by_range
            .values()
            .find(|bindings| bindings.type_for("first") == Some("Item".to_string()))
            .unwrap();
        assert_eq!(
            run_bindings.array_component_for("items"),
            Some("Item".to_string())
        );
        assert_eq!(run_bindings.type_for("first"), Some("Item".to_string()));
        assert_eq!(
            run_bindings.array_component_for("nullableItems"),
            Some("Item".to_string())
        );
        assert_eq!(
            run_bindings.array_component_for("groups"),
            Some("Helper".to_string())
        );
    }

    #[test]
    fn companion_property_receivers_are_shadowed_by_locals_parameters_and_instance_properties() {
        let file = write_test_file(
            "package com.example\n\nclass Item {\n    fun helper(value: Int): Int = value\n}\n\nclass Helper {\n    fun inner(): Item = Item()\n}\n\nclass Util {\n    companion object {\n        val items: Array<Item> = arrayOf()\n    }\n    fun run(): Int {\n        val items: Array<Helper> = arrayOf()\n        return items[0].inner().helper(1)\n    }\n}\n\nclass Shadowed {\n    val items: Array<Item> = arrayOf()\n    companion object {\n        val items: Array<Helper> = arrayOf()\n    }\n    fun run(): Int {\n        return items[0].helper(2)\n    }\n}\n\nfun topLevel(): Int {\n    return items[0].helper(3)\n}\n",
        );
        let context = kotlin_import_context_for_file_with_overrides_and_deadline(
            &file.normalized_path,
            None,
            None,
        )
        .unwrap();
        // A local with the same name replaces the companion binding cleanly
        // instead of creating a false ambiguity.
        let local_wins = context
            .receiver_type_bindings_by_range
            .values()
            .find(|bindings| bindings.array_component_for("items") == Some("Helper".to_string()))
            .unwrap();
        assert_eq!(
            local_wins.array_component_for("items"),
            Some("Helper".to_string())
        );
        // An instance property with the same name shadows the companion
        // binding (the companion insert is skipped), so the instance
        // component wins.
        let instance_wins = context
            .receiver_type_bindings_by_range
            .values()
            .find(|bindings| bindings.array_component_for("items") == Some("Item".to_string()))
            .unwrap();
        assert_eq!(
            instance_wins.array_component_for("items"),
            Some("Item".to_string())
        );
        // A top-level function has no enclosing companion scope: the name
        // stays unbound and fails closed.
        let top_level = context
            .receiver_type_bindings_by_range
            .values()
            .find(|bindings| !bindings.contains("items"))
            .unwrap();
        assert_eq!(top_level.type_for("items"), None);
        assert_eq!(top_level.array_component_for("items"), None);
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
