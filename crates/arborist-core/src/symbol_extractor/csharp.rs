use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::normalize_path;
use crate::semantic::csharp::{
    csharp_parameters, csharp_return_type, csharp_semantic_path, csharp_signature,
    csharp_symbol_name, is_csharp_symbol_node,
};
use crate::semantic::semantic_parent_path;
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::symbol_reference_compat::reference_facts_from_legacy;
use crate::workspace_scan::WorkspaceScanDeadline;

type ReferenceNames = BTreeSet<String>;
type CallAritiesByName = BTreeMap<String, BTreeSet<usize>>;
type DirectCalls = (ReferenceNames, CallAritiesByName);

pub(crate) fn index_csharp_symbols_with_deadline(
    path: &Path,
    source: &str,
    root: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<IndexedSymbol>> {
    let mut symbols = Vec::new();
    collect_symbols(path, source, root, root, deadline, &mut symbols)?;
    Ok(symbols)
}

fn collect_symbols(
    path: &Path,
    source: &str,
    root: Node<'_>,
    node: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
    symbols: &mut Vec<IndexedSymbol>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("extracting C# symbols")?;
    }
    if is_csharp_symbol_node(node)
        && let Some(symbol) = indexed_symbol(path, source, root, node)?
    {
        symbols.push(symbol);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_symbols(path, source, root, child, deadline, symbols)?;
    }
    Ok(())
}

fn indexed_symbol(
    path: &Path,
    source: &str,
    root: Node<'_>,
    node: Node<'_>,
) -> Result<Option<IndexedSymbol>> {
    let Some(name) = csharp_symbol_name(node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = csharp_semantic_path(root, node, source, &name)? else {
        return Ok(None);
    };

    let (references_by_name, call_arities_by_name) = collect_direct_same_type_calls(node, source)?;

    Ok(Some(IndexedSymbol {
        extension_receiver: None,
        symbol_id: String::new(),
        base_name: symbol_base_name(&semantic_path),
        scope_path: semantic_parent_path(&semantic_path),
        semantic_path,
        file_path: normalize_path(path),
        node_kind: node.kind().to_string(),
        byte_range: (node.start_byte(), node.end_byte()),
        signature: csharp_signature(node, source),
        is_overload: false,
        parameters: csharp_parameters(node, source),
        return_type: csharp_return_type(node, source),
        docstring: None,
        reference_facts: reference_facts_from_legacy(&references_by_name, &call_arities_by_name),
        references_by_name,
        call_arities_by_name,
    }))
}

fn collect_direct_same_type_calls(symbol_node: Node<'_>, source: &str) -> Result<DirectCalls> {
    if !matches!(
        symbol_node.kind(),
        "method_declaration" | "constructor_declaration"
    ) {
        return Ok((BTreeSet::new(), BTreeMap::new()));
    }
    let bindings = collect_typed_local_bindings(symbol_node, source)?;
    let mut references = BTreeSet::new();
    let mut call_arities_by_name = BTreeMap::new();
    collect_constructor_initializer_call(symbol_node, &mut references, &mut call_arities_by_name);
    if let Some(body) = symbol_node.child_by_field_name("body") {
        collect_direct_same_type_calls_from_node(
            body,
            source,
            &bindings,
            &mut references,
            &mut call_arities_by_name,
        )?;
    }
    Ok((references, call_arities_by_name))
}

fn collect_constructor_initializer_call(
    symbol_node: Node<'_>,
    references: &mut ReferenceNames,
    call_arities_by_name: &mut CallAritiesByName,
) {
    if symbol_node.kind() != "constructor_declaration" {
        return;
    }
    let mut cursor = symbol_node.walk();
    let Some(initializer) = symbol_node
        .named_children(&mut cursor)
        .find(|node| node.kind() == "constructor_initializer")
    else {
        return;
    };
    let mut initializer_cursor = initializer.walk();
    let Some(receiver) = initializer
        .children(&mut initializer_cursor)
        .find(|node| matches!(node.kind(), "this" | "base"))
    else {
        return;
    };
    let mut arguments_cursor = initializer.walk();
    let Some(arguments) = initializer
        .named_children(&mut arguments_cursor)
        .find(|node| node.kind() == "argument_list")
    else {
        return;
    };
    let mut argument_cursor = arguments.walk();
    let arity = arguments.named_children(&mut argument_cursor).count();
    let receiver = receiver.kind().to_string();
    references.insert(receiver.clone());
    call_arities_by_name
        .entry(receiver)
        .or_default()
        .insert(arity);
}

fn collect_direct_same_type_calls_from_node(
    node: Node<'_>,
    source: &str,
    bindings: &BTreeMap<String, String>,
    references: &mut ReferenceNames,
    call_arities_by_name: &mut CallAritiesByName,
) -> Result<()> {
    if is_csharp_symbol_node(node) {
        return Ok(());
    }
    if node.kind() == "invocation_expression"
        && let Some(function) = node.child_by_field_name("function")
        && let Some(arguments) = node.child_by_field_name("arguments")
        && let Some(name) = csharp_direct_invocation_name(function, source, bindings)?
    {
        let mut cursor = arguments.walk();
        let arity = arguments.named_children(&mut cursor).count();
        references.insert(name.clone());
        call_arities_by_name.entry(name).or_default().insert(arity);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_direct_same_type_calls_from_node(
            child,
            source,
            bindings,
            references,
            call_arities_by_name,
        )?;
    }
    Ok(())
}

/// Builds the spelling of a member chain whose leading receiver is a locally
/// bound value, `this`, or `base`, such as `group.inner().helper` or
/// `this.holder.inner().helper`. Field, property, and event hops keep their
/// plain names; method-call hops encode their argument count (`inner()` or
/// `inner(1)`) so the resolver can require an arity match. When
/// `keep_static_type_roots` is set, a chain whose leading receiver is an
/// unbound identifier or `global::`-qualified root is kept as a static
/// type-qualified candidate such as `Util.STATIC_HELPER` so the resolver can
/// look the member up on the resolved type; otherwise those roots fail closed
/// and the caller falls through to the static type-call handling. A leading
/// bare method call (`MakeHelper()` in `MakeHelper().entry`) records the call
/// as the first chain segment so the resolver can dispatch it as a factory
/// method on the enclosing type or a static-imported type; bare-call roots are
/// only kept when static type-qualified roots are allowed. Non-instance bases
/// (object creations, namespaces) and untyped bound receivers produce no
/// spelling and fail closed.
fn csharp_instance_member_chain_spelling(
    node: Node<'_>,
    source: &str,
    bindings: &BTreeMap<String, String>,
    keep_static_type_roots: bool,
) -> Result<Option<String>> {
    let mut segments = Vec::new();
    let mut current = node;
    loop {
        match current.kind() {
            "member_access_expression" => {
                let Some(name) = current.child_by_field_name("name") else {
                    return Ok(None);
                };
                let name = crate::language::node_text(name, source)?.trim();
                if name.is_empty() {
                    return Ok(None);
                }
                segments.push(name.to_string());
                let Some(expression) = current.child_by_field_name("expression") else {
                    return Ok(None);
                };
                current = expression;
            }
            "invocation_expression" => {
                let Some(function) = current.child_by_field_name("function") else {
                    return Ok(None);
                };
                let Some(arguments) = current.child_by_field_name("arguments") else {
                    return Ok(None);
                };
                let mut cursor = arguments.walk();
                let arity = arguments.named_children(&mut cursor).count();
                // A bare-call root such as `MakeHelper()` in
                // `MakeHelper().entry` records the call as the leading chain
                // segment; the resolver dispatches it as a factory method on
                // the enclosing type or a static-imported type. Bare-call
                // roots are only kept when static type-qualified roots are
                // allowed; otherwise the caller falls through to the direct
                // invocation handling.
                if function.kind() != "member_access_expression" {
                    if !keep_static_type_roots || function.kind() != "identifier" {
                        return Ok(None);
                    }
                    let name = crate::language::node_text(function, source)?.trim();
                    if name.is_empty() {
                        return Ok(None);
                    }
                    if arity == 0 {
                        segments.push(format!("{name}()"));
                    } else {
                        segments.push(format!("{name}({arity})"));
                    }
                    break;
                }
                let Some(name) = function.child_by_field_name("name") else {
                    return Ok(None);
                };
                let name = crate::language::node_text(name, source)?.trim();
                if name.is_empty() {
                    return Ok(None);
                }
                if arity == 0 {
                    segments.push(format!("{name}()"));
                } else {
                    segments.push(format!("{name}({arity})"));
                }
                let Some(expression) = function.child_by_field_name("expression") else {
                    return Ok(None);
                };
                current = expression;
            }
            "identifier" => {
                let base = crate::language::node_text(current, source)?.trim();
                if base.is_empty() {
                    return Ok(None);
                }
                // A locally bound receiver records an instance chain only
                // when its declared type is usable; untyped bindings (`var`
                // locals, lambda parameters) fail closed. When static
                // type-qualified roots are allowed, an unbound identifier is
                // kept as a candidate such as `Util.STATIC_HELPER` so the
                // resolver can look the member up on the resolved type;
                // otherwise it fails closed and the caller falls through to
                // the static type-call handling.
                if bindings
                    .get(base)
                    .is_some_and(|type_name| type_name.is_empty())
                    || (!keep_static_type_roots && !bindings.contains_key(base))
                {
                    return Ok(None);
                }
                segments.push(base.to_string());
                break;
            }
            "this" => {
                segments.push("this".to_string());
                break;
            }
            "base" => {
                // A base-rooted chain such as `base.inner().helper` records
                // the `base` keyword as the leading segment; the resolver
                // dispatches the remaining hops on the unique base type.
                segments.push("base".to_string());
                break;
            }
            "alias_qualified_name" => {
                // A `global::`-qualified root such as
                // `global::Demo.Util.STATIC_HELPER` records the alias-qualified
                // spelling (`global::Demo`) as the leading segment; the
                // resolver re-joins the dotted type path when resolving the
                // static member. Only kept when static type-qualified roots
                // are allowed; otherwise the caller falls through to the
                // static type-call handling.
                if !keep_static_type_roots {
                    return Ok(None);
                }
                let spelling = crate::language::node_text(current, source)?.trim();
                if spelling.is_empty() {
                    return Ok(None);
                }
                segments.push(spelling.to_string());
                break;
            }
            "object_creation_expression" => {
                // A constructor-rooted chain such as
                // `new Group().inner().helper` records the constructed type
                // spelling as the leading segment; the resolver dispatches the
                // remaining hops on the constructed type. Anonymous or
                // malformed creations produce no spelling and fail closed.
                let Some(spelling) = csharp_constructor_type_spelling(current, source)? else {
                    return Ok(None);
                };
                segments.push(spelling);
                break;
            }
            "element_access_expression" => {
                // An element-access hop such as `items[0]` or
                // `this.fieldItems[0]` records the bracket on the accessed
                // element's segment so the trailing member dispatches on the
                // element component type; malformed or nested element
                // accesses fail closed.
                let Some(subscript) = current.child_by_field_name("subscript") else {
                    return Ok(None);
                };
                let subscript_text = crate::language::node_text(subscript, source)?.trim();
                if subscript_text.is_empty()
                    || !subscript_text.starts_with('[')
                    || !subscript_text.ends_with(']')
                {
                    return Ok(None);
                }
                segments.push(subscript_text.to_string());
                let Some(expression) = current.child_by_field_name("expression") else {
                    return Ok(None);
                };
                current = expression;
            }
            "parenthesized_expression" => {
                // A parenthesized segment such as `(MakeFactory())` in
                // `(MakeFactory()).entry.Run` or `(group).inner().helper`
                // unwraps to its inner expression and keeps walking the
                // chain so parentheses never break member-chain tracing.
                let Some(inner) = csharp_parenthesized_inner_expression(current) else {
                    return Ok(None);
                };
                current = inner;
            }
            _ => return Ok(None),
        }
    }
    segments.reverse();
    // A bracket segment such as `[0]` from an element-access hop merges onto
    // the preceding member segment so `items` + `[0]` spells `items[0]`
    // rather than `items.[0]`; a leading bracket (a malformed chain) fails
    // closed.
    if segments.iter().any(|segment| segment.starts_with('[')) {
        let mut merged: Vec<String> = Vec::with_capacity(segments.len());
        for segment in segments {
            if segment.starts_with('[') {
                let Some(previous) = merged.last_mut() else {
                    return Ok(None);
                };
                previous.push_str(&segment);
            } else {
                merged.push(segment);
            }
        }
        segments = merged;
    }
    Ok(Some(segments.join(".")))
}

fn csharp_direct_invocation_name(
    node: Node<'_>,
    source: &str,
    bindings: &BTreeMap<String, String>,
) -> Result<Option<String>> {
    if node.kind() == "member_access_expression" {
        let Some(receiver) = node.child_by_field_name("expression") else {
            return Ok(None);
        };
        let Some(member) = node.child_by_field_name("name") else {
            return Ok(None);
        };
        // An instance member chain whose leading receiver is a locally bound
        // value or `this`, such as `group.inner().helper` or
        // `this.holder.inner().helper`, records one chain fact with
        // method-call hops encoded as `inner()` or `inner(1)`. Unbound
        // receivers fall through to the static type-qualified handling below.
        if matches!(
            receiver.kind(),
            "invocation_expression"
                | "member_access_expression"
                | "parenthesized_expression"
                | "element_access_expression"
        ) && let Some(spelling) =
            csharp_instance_member_chain_spelling(node, source, bindings, false)?
        {
            return Ok(Some(spelling));
        }
        // A bare factory-call root such as `MakeHelper().Run(1)` or
        // `MakeHelper().entry.Run(1)` spells the full chain with the call as
        // the leading segment so the resolver can dispatch the leading
        // arity-matched factory method on the enclosing type or a
        // static-imported type; a factory-call root with an element-access
        // suffix such as `makeItems()[0].Run(1)` keeps the same chain
        // spelling so the resolver can dispatch the trailing member on the
        // factory return array's element component type. Bare-call roots are
        // only kept when static type-qualified roots are allowed.
        if matches!(
            receiver.kind(),
            "invocation_expression" | "parenthesized_expression" | "element_access_expression"
        ) && let Some(spelling) =
            csharp_instance_member_chain_spelling(node, source, bindings, true)?
            && spelling.contains('(')
        {
            return Ok(Some(spelling));
        }
        if receiver.kind() == "this" {
            return csharp_invocation_member_name(member, source)
                .map(|name| name.map(|name| format!("this.{name}")));
        }
        if receiver.kind() == "base" {
            return csharp_invocation_member_name(member, source)
                .map(|name| name.map(|name| format!("base.{name}")));
        }
        if receiver.kind() == "object_creation_expression" {
            let Some(member_name) = csharp_invocation_member_name(member, source)? else {
                return Ok(None);
            };
            let Some(spelling) = csharp_constructor_type_spelling(receiver, source)? else {
                return Ok(None);
            };
            return Ok(Some(format!("{spelling}.{member_name}")));
        }
        if receiver.kind() == "identifier" {
            let receiver_name = crate::language::node_text(receiver, source)?.trim();
            if let Some(binding_type) = bindings.get(receiver_name) {
                // A locally bound receiver records an instance fact only when
                // its declared type is usable; untyped bindings (`var` locals,
                // lambda parameters, type parameters) fail closed instead of
                // guessing a static type call.
                if binding_type.is_empty() {
                    return Ok(None);
                }
                return csharp_invocation_member_name(member, source)
                    .map(|name| name.map(|name| format!("{receiver_name}.{name}")));
            }
            if let Some(name) = csharp_simple_type_static_invocation_name(receiver, member, source)?
                && !csharp_reference_is_shadowed(&name, bindings)
            {
                return Ok(Some(name));
            }
            return Ok(None);
        }
        if matches!(receiver.kind(), "identifier" | "generic_name") {
            if let Some(name) = csharp_simple_type_static_invocation_name(receiver, member, source)?
                && !csharp_reference_is_shadowed(&name, bindings)
            {
                return Ok(Some(name));
            }
            return Ok(None);
        }
        if receiver.kind() == "member_access_expression" {
            if let Some(name) =
                csharp_global_qualified_static_invocation_name(receiver, member, source)?
            {
                return Ok(Some(name));
            }
            if let Some(name) =
                csharp_qualified_type_static_invocation_name(receiver, member, source)?
            {
                return Ok(Some(name));
            }
            // A static type-qualified chain that includes a method-call hop
            // such as `Util.MakeHelper().entry.Run(1)` or
            // `STATIC_HELPER.inner().entry.Run(1)` cannot be spelled as a
            // plain dotted type path (the receiver text contains `()`), so
            // keep the full chain spelling with static type-qualified roots
            // allowed; the resolver dispatches the leading static factory
            // method or static-imported member root before walking the
            // remaining hops. Chains without a method-call hop are already
            // covered by the dotted type-path spellings above.
            if let Some(spelling) =
                csharp_instance_member_chain_spelling(node, source, bindings, true)?
                && spelling.contains('(')
            {
                return Ok(Some(spelling));
            }
            return Ok(None);
        }
        return csharp_global_qualified_static_invocation_name(receiver, member, source);
    }

    let name = csharp_invocation_member_name(node, source)?;
    Ok(name.filter(|name| !csharp_reference_is_shadowed(name, bindings)))
}

/// Records the constructed type spelling of an `object_creation_expression`
/// such as `new Helper()` or `new Outer.Inner()`, ignoring object-initializer
/// bodies and generic type arguments. Anonymous creations, `global::`-qualified
/// spellings, and malformed type text produce no fact and fail closed.
fn csharp_constructor_type_spelling(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let text = crate::language::node_text(node, source)?.trim();
    let Some(type_name) = text.strip_prefix("new") else {
        return Ok(None);
    };
    let type_name = type_name.trim_start();
    if type_name.is_empty() || type_name.contains("::") {
        return Ok(None);
    }
    let Some(type_name) = strip_csharp_constructor_suffix(type_name) else {
        return Ok(None);
    };
    let type_name = type_name.trim();
    if type_name.is_empty() || type_name == "var" {
        return Ok(None);
    }
    let Some(semantic_type_path) = crate::language::csharp_generic_type_semantic_path(type_name)
    else {
        return Ok(None);
    };
    Ok(Some(format!("{}()", semantic_type_path.replace("::", "."))))
}

/// Strips a trailing object-initializer body (`{ ... }`) and constructor
/// argument list (`(...)`) from a constructed type spelling, leaving the bare
/// type path such as `Helper` or `Outer.Inner`.
fn strip_csharp_constructor_suffix(type_name: &str) -> Option<&str> {
    let mut trimmed = type_name.trim_end();
    if trimmed.ends_with('}') {
        trimmed = strip_csharp_balanced_suffix(trimmed, '{', '}')?;
        trimmed = trimmed.trim_end();
    }
    if trimmed.ends_with(')') {
        trimmed = strip_csharp_balanced_suffix(trimmed, '(', ')')?;
        trimmed = trimmed.trim_end();
    }
    Some(trimmed)
}

fn strip_csharp_balanced_suffix(text: &str, open: char, close: char) -> Option<&str> {
    let mut depth = 0usize;
    for (index, character) in text.char_indices().rev() {
        if character == close {
            depth += 1;
        } else if character == open {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(&text[..index]);
            }
        }
    }
    None
}

fn csharp_simple_type_static_invocation_name(
    receiver: Node<'_>,
    member: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    let receiver_name = crate::language::node_text(receiver, source)?.trim();
    let Some(semantic_type_path) =
        crate::language::csharp_generic_type_semantic_path(receiver_name)
    else {
        return Ok(None);
    };
    let Some(member_name) = csharp_invocation_member_name(member, source)? else {
        return Ok(None);
    };
    Ok(Some(format!(
        "{}.{}",
        semantic_type_path.replace("::", "."),
        member_name
    )))
}

fn csharp_qualified_type_static_invocation_name(
    receiver: Node<'_>,
    member: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    let type_path = crate::language::node_text(receiver, source)?.trim();
    let Some(semantic_type_path) = crate::language::csharp_generic_type_semantic_path(type_path)
    else {
        return Ok(None);
    };
    let Some(member_name) = csharp_invocation_member_name(member, source)? else {
        return Ok(None);
    };
    Ok(Some(format!(
        "{}.{}",
        semantic_type_path.replace("::", "."),
        member_name
    )))
}

fn csharp_global_qualified_static_invocation_name(
    receiver: Node<'_>,
    member: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    let receiver = crate::language::node_text(receiver, source)?.trim();
    let Some(type_path) = receiver.strip_prefix("global::") else {
        return Ok(None);
    };
    let Some(semantic_type_path) = crate::language::csharp_generic_type_semantic_path(type_path)
    else {
        return Ok(None);
    };
    let Some(member_name) = csharp_invocation_member_name(member, source)? else {
        return Ok(None);
    };
    Ok(Some(format!(
        "global::{}.{}",
        semantic_type_path.replace("::", "."),
        member_name
    )))
}

fn csharp_reference_is_shadowed(name: &str, bindings: &BTreeMap<String, String>) -> bool {
    let binding_name = name.split_once('.').map_or(name, |(receiver, _)| receiver);
    bindings.contains_key(binding_name)
}

fn csharp_invocation_member_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let identifier = match node.kind() {
        "identifier" => Some(node),
        "generic_name" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|child| child.kind() == "identifier")
        }
        _ => None,
    };
    identifier
        .map(|identifier| crate::language::node_text(identifier, source).map(str::trim))
        .transpose()
        .map(|name| name.filter(|name| !name.is_empty()).map(str::to_string))
}

fn csharp_insert_binding(
    bindings: &mut BTreeMap<String, String>,
    name: &str,
    type_name: Option<String>,
) {
    if !name.is_empty() {
        bindings.insert(name.to_string(), type_name.unwrap_or_default());
    }
}

fn csharp_declared_type_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(type_node) = node.child_by_field_name("type") else {
        return Ok(None);
    };
    let type_name = crate::language::node_text(type_node, source)?.trim();
    if type_name.is_empty() || type_name == "var" {
        return Ok(None);
    }
    Ok(Some(type_name.to_string()))
}

/// Collects locally bound receiver names with their declared type spellings
/// for a function. Parameters, typed locals, and enclosing-type fields and
/// properties carry their declared type; `var` locals, lambda parameters,
/// `foreach` variables, local functions, and type parameters are bound with an
/// empty type so they still suppress static type interpretation while failing
/// closed for instance dispatch.
fn collect_typed_local_bindings(
    symbol_node: Node<'_>,
    source: &str,
) -> Result<BTreeMap<String, String>> {
    fn declarator_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
        let name = if let Some(name) = node.child_by_field_name("name") {
            name
        } else {
            let mut cursor = node.walk();
            let Some(declarator) = node
                .named_children(&mut cursor)
                .find(|child| child.kind() == "variable_declarator")
            else {
                return Ok(None);
            };
            let Some(name) = declarator.child_by_field_name("name") else {
                return Ok(None);
            };
            name
        };
        let name = crate::language::node_text(name, source)?.trim();
        Ok((!name.is_empty()).then(|| name.to_string()))
    }

    fn collect(
        node: Node<'_>,
        source: &str,
        bindings: &mut BTreeMap<String, String>,
    ) -> Result<()> {
        if is_csharp_symbol_node(node) && node.parent().is_some() {
            return Ok(());
        }
        if node.kind() == "implicit_parameter" {
            let name = crate::language::node_text(node, source)?.trim();
            csharp_insert_binding(bindings, name, None);
        }
        if matches!(
            node.kind(),
            "parameter" | "catch_declaration" | "declaration_expression" | "declaration_pattern"
        ) && let Some(name) = declarator_name(node, source)?
        {
            csharp_insert_binding(bindings, &name, csharp_declared_type_name(node, source)?);
        }
        if node.kind() == "local_function_statement"
            && let Some(name) = declarator_name(node, source)?
        {
            csharp_insert_binding(bindings, &name, None);
        }
        if node.kind() == "variable_declarator"
            && let Some(name) = node.child_by_field_name("name")
        {
            let name = crate::language::node_text(name, source)?.trim();
            let type_name = match node.parent() {
                Some(parent) if parent.kind() == "variable_declaration" => {
                    match csharp_declared_type_name(parent, source)? {
                        Some(type_name) => Some(type_name),
                        // A `var` local infers its receiver type from a
                        // constructor initializer such as
                        // `var helper = new Helper()`, from a factory call
                        // initializer such as `var helper = MakeHelper()`,
                        // from a field/property-access initializer such as
                        // `var helper = this.holder.helper`, or from an
                        // element-access initializer such as
                        // `var first = items[0]` whose base array's element
                        // component type pins the receiver; other
                        // initializers bind an empty type and fail closed.
                        None => {
                            if let Some(type_name) =
                                csharp_var_initializer_type_binding(node, source, bindings)?
                            {
                                Some(type_name)
                            } else {
                                csharp_initializer_element_access_from_declarator(node, source)?
                                    .map(|(base_spelling, _)| format!("@element:{base_spelling}"))
                            }
                        }
                    }
                }
                _ => None,
            };
            csharp_insert_binding(bindings, name, type_name);
        }
        if node.kind() == "foreach_statement"
            && let Some(left) = node.child_by_field_name("left")
            && left.kind() == "identifier"
        {
            let name = crate::language::node_text(left, source)?.trim();
            csharp_insert_binding(bindings, name, None);
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            collect(child, source, bindings)?;
        }
        Ok(())
    }

    let mut bindings = BTreeMap::new();
    collect_enclosing_type_typed_bindings(symbol_node, source, &mut bindings)?;
    let mut cursor = symbol_node.walk();
    for child in symbol_node.named_children(&mut cursor) {
        collect(child, source, &mut bindings)?;
    }
    Ok(bindings)
}

/// Returns the inner expression of a `parenthesized_expression` such as
/// `(MakeFactory())` or `(group)`, recovering the anonymous keyword token for
/// `(this)`; malformed or empty parentheses return `None` and fail closed.
fn csharp_parenthesized_inner_expression(node: Node<'_>) -> Option<Node<'_>> {
    let inner = {
        let mut cursor = node.walk();
        node.named_children(&mut cursor).next()
    };
    match inner {
        Some(inner) => Some(inner),
        None => {
            let mut cursor = node.walk();
            node.children(&mut cursor)
                .find(|child| !matches!(child.kind(), "(" | ")"))
        }
    }
}

/// Infers a receiver type binding for `var` locals. Constructor initializers
/// such as `var helper = new Helper()` or `var helper = new Outer.Inner()`
/// bind the constructed type; invocation initializers such as
/// `var helper = MakeHelper()` bind a factory marker the resolver expands to
/// the factory method's declared return type; identifier and
/// member-access initializers such as `var helper = this.holder.helper` bind
/// a chain marker the resolver walks to the final member's declared type.
/// Other initializers, target-typed creations, array creations, and malformed
/// spellings return `None` and fail closed.
fn csharp_var_initializer_type_binding(
    declarator: Node<'_>,
    source: &str,
    bindings: &BTreeMap<String, String>,
) -> Result<Option<String>> {
    let Some(initializer) = csharp_declarator_initializer(declarator) else {
        return Ok(None);
    };
    csharp_initializer_type_binding(initializer, source, bindings)
}

/// Records a `var` local whose initializer is an element access such as
/// `var first = items[0]`, returning the array-typed base spelling and call
/// arity so the local resolves to the base array's element component type.
/// Plain-identifier bases such as `items`, `local`, or a bare enclosing-class
/// field name and member-access bases such as `this.fieldItems` or
/// `group.holder.fieldItems` record the spelling with arity zero; factory-call
/// bases such as `makeItems()` or `Util.makeItems()` record the reference with
/// a trailing `()` marker and the call's argument count. Multi-dimensional
/// element access and other initializer shapes record nothing and fail
/// closed.
fn csharp_initializer_element_access_from_declarator(
    declarator: Node<'_>,
    source: &str,
) -> Result<Option<(String, usize)>> {
    let Some(initializer) = csharp_declarator_initializer(declarator) else {
        return Ok(None);
    };
    let initializer = match initializer.kind() {
        "parenthesized_expression" => {
            let Some(inner) = csharp_parenthesized_inner_expression(initializer) else {
                return Ok(None);
            };
            inner
        }
        _ => initializer,
    };
    if initializer.kind() != "element_access_expression" {
        return Ok(None);
    }
    let Some(mut array) = initializer.child_by_field_name("expression") else {
        return Ok(None);
    };
    // A parenthesized base array such as `(Util.makeItems())` in
    // `var first = (Util.makeItems())[0]` unwraps to the same base shape as
    // the unparenthesized form before dispatch.
    while array.kind() == "parenthesized_expression" {
        let Some(inner) = csharp_parenthesized_inner_expression(array) else {
            return Ok(None);
        };
        array = inner;
    }
    let (base_spelling, call_arity) = match array.kind() {
        "identifier" | "member_access_expression" => {
            let base_name = crate::language::node_text(array, source)?.trim();
            if base_name.is_empty() {
                return Ok(None);
            }
            (base_name.to_string(), 0)
        }
        "invocation_expression" => {
            let Some(function) = array.child_by_field_name("function") else {
                return Ok(None);
            };
            let spelling = match function.kind() {
                "identifier" | "member_access_expression" => {
                    crate::language::node_text(function, source)?
                        .trim()
                        .to_string()
                }
                _ => return Ok(None),
            };
            if spelling.is_empty() {
                return Ok(None);
            }
            let Some(arguments) = array.child_by_field_name("arguments") else {
                return Ok(None);
            };
            let mut cursor = arguments.walk();
            let arity = arguments.named_children(&mut cursor).count();
            (format!("{spelling}()"), arity)
        }
        _ => return Ok(None),
    };
    if base_spelling.is_empty() {
        return Ok(None);
    }
    Ok(Some((base_spelling, call_arity)))
}

/// Infers a receiver type binding from an initializer expression, unwrapping
/// parenthesized initializers such as `(MakeFactory())`, `(new Helper())`, or
/// `(MakeHelper()).entry` to the same shape as the unparenthesized form.
fn csharp_initializer_type_binding(
    initializer: Node<'_>,
    source: &str,
    bindings: &BTreeMap<String, String>,
) -> Result<Option<String>> {
    match initializer.kind() {
        "parenthesized_expression" => {
            let Some(inner) = csharp_parenthesized_inner_expression(initializer) else {
                return Ok(None);
            };
            csharp_initializer_type_binding(inner, source, bindings)
        }
        "object_creation_expression" => {
            let Some(type_node) = initializer.child_by_field_name("type") else {
                return Ok(None);
            };
            let type_name = crate::language::node_text(type_node, source)?.trim();
            if type_name.is_empty() || type_name == "var" {
                return Ok(None);
            }
            Ok(Some(type_name.to_string()))
        }
        "invocation_expression" => csharp_factory_marker_from_initializer(initializer, source),
        "member_access_expression" | "identifier" => {
            let Some(chain) =
                csharp_instance_member_chain_spelling(initializer, source, bindings, true)?
            else {
                return Ok(None);
            };
            Ok(Some(format!("@init:{chain}")))
        }
        _ => Ok(None),
    }
}

/// Builds a factory marker such as `@factory:MakeHelper(0)`,
/// `@factory:this.MakeHelper(0)`, or `@factory:Factories.MakeHelper(1)` for a
/// `var` local initialized from an invocation. The spelling mirrors the
/// extractor's reference spellings so the resolver can expand the marker to
/// the factory method's declared return type. Unsupported initializer shapes
/// return `None` and fail closed.
fn csharp_factory_marker_from_initializer(
    initializer: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    let Some(function) = initializer.child_by_field_name("function") else {
        return Ok(None);
    };
    let spelling = match function.kind() {
        "identifier" => crate::language::node_text(function, source)?
            .trim()
            .to_string(),
        "member_access_expression" => {
            let Some(expression) = function.child_by_field_name("expression") else {
                return Ok(None);
            };
            let Some(name) = function.child_by_field_name("name") else {
                return Ok(None);
            };
            let expression_text = crate::language::node_text(expression, source)?.trim();
            let name_text = crate::language::node_text(name, source)?.trim();
            if expression_text.is_empty() || name_text.is_empty() {
                return Ok(None);
            }
            format!("{expression_text}.{name_text}")
        }
        _ => return Ok(None),
    };
    if spelling.is_empty() {
        return Ok(None);
    }
    let Some(arguments) = initializer.child_by_field_name("arguments") else {
        return Ok(None);
    };
    let mut cursor = arguments.walk();
    let arity = arguments.named_children(&mut cursor).count();
    Ok(Some(format!("@factory:{spelling}({arity})")))
}

/// Returns the initializer expression of a `variable_declarator` such as
/// `helper = new Helper()`, `helper = MakeHelper()`, or `helper = helper`.
/// The grammar does not name the `= expression` child, so the last named
/// child that is not the declared name or a tuple/indexer suffix is the
/// initializer; a bare identifier initializer (`var helper = helper`) is kept
/// because only the declared-name child is skipped.
fn csharp_declarator_initializer<'a>(declarator: Node<'a>) -> Option<Node<'a>> {
    let declared_name = declarator.child_by_field_name("name");
    let mut cursor = declarator.walk();
    let mut initializer = None;
    for child in declarator.named_children(&mut cursor) {
        if matches!(child.kind(), "tuple_pattern" | "bracketed_argument_list")
            || declared_name.is_some_and(|name| name == child)
        {
            continue;
        }
        initializer = Some(child);
    }
    initializer
}

fn collect_enclosing_type_typed_bindings(
    symbol_node: Node<'_>,
    source: &str,
    bindings: &mut BTreeMap<String, String>,
) -> Result<()> {
    fn collect(
        node: Node<'_>,
        root: Node<'_>,
        source: &str,
        bindings: &mut BTreeMap<String, String>,
    ) -> Result<()> {
        if node != root && is_csharp_symbol_node(node) {
            return Ok(());
        }
        if node.kind() == "field_declaration" {
            let mut declaration_cursor = node.walk();
            for declaration in node
                .named_children(&mut declaration_cursor)
                .filter(|child| child.kind() == "variable_declaration")
            {
                let type_name = csharp_declared_type_name(declaration, source)?;
                let mut declarator_cursor = declaration.walk();
                for declarator in declaration
                    .named_children(&mut declarator_cursor)
                    .filter(|child| child.kind() == "variable_declarator")
                {
                    let Some(name) = declarator.child_by_field_name("name") else {
                        continue;
                    };
                    let name = crate::language::node_text(name, source)?.trim();
                    csharp_insert_binding(bindings, name, type_name.clone());
                }
            }
        }
        if matches!(node.kind(), "property_declaration" | "event_declaration")
            && let Some(name) = node.child_by_field_name("name")
        {
            let name = crate::language::node_text(name, source)?.trim();
            csharp_insert_binding(bindings, name, csharp_declared_type_name(node, source)?);
        }
        if node.kind() == "type_parameter"
            && let Some(name) = node.child_by_field_name("name")
        {
            let name = crate::language::node_text(name, source)?.trim();
            csharp_insert_binding(bindings, name, None);
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            collect(child, root, source, bindings)?;
        }
        Ok(())
    }

    let mut current = symbol_node.parent();
    while let Some(node) = current {
        if is_csharp_type_declaration(node) {
            return collect(node, node, source, bindings);
        }
        current = node.parent();
    }
    Ok(())
}

fn is_csharp_type_declaration(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "class_declaration"
            | "struct_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::index_csharp_symbols_with_deadline;
    use crate::language::parse_document;

    #[test]
    fn indexes_namespace_qualified_csharp_declarations_and_direct_calls() {
        let source = r#"
namespace Demo.Core;

public class Counter {
    public Counter(int initial) {}
    public int Increment(int amount) => amount;
    public int Increment(long amount) => (int)amount;
    public int Helper() => 1;
    public int Caller() => Helper();
}
public struct Point { public int X; }
public interface IRenderer { string Render(); }
public enum Kind { Basic }
public record Entry(string Name);
"#;
        let path = Path::new("Counter.cs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_csharp_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.semantic_path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Demo::Core::Counter",
                "Demo::Core::Counter::Counter",
                "Demo::Core::Counter::Increment",
                "Demo::Core::Counter::Increment",
                "Demo::Core::Counter::Helper",
                "Demo::Core::Counter::Caller",
                "Demo::Core::Point",
                "Demo::Core::IRenderer",
                "Demo::Core::IRenderer::Render",
                "Demo::Core::Kind",
                "Demo::Core::Entry",
            ]
        );
        let increment = symbols
            .iter()
            .find(|symbol| {
                symbol.semantic_path == "Demo::Core::Counter::Increment"
                    && symbol.parameters == ["int amount"]
            })
            .unwrap();
        assert_eq!(increment.return_type.as_deref(), Some("int"));
        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "Demo::Core::Counter::Caller")
            .expect("fixture should include a direct caller");
        assert_eq!(caller.references_by_name, ["Helper".to_string()].into());
        assert_eq!(
            caller.call_arities_by_name,
            [("Helper".to_string(), [0].into())].into()
        );
    }

    #[test]
    fn collects_conservative_csharp_direct_call_facts() {
        let source = r#"
using System;

class GlobalHelper {
    public static int Utility() => 1;
    public static T GenericUtility<T>() => default;
    public int Instance() => 1;
}

class Base {
    public int BaseHelper() => 1;
}

class Counter<T> : Base {
    GlobalHelper GlobalHelper { get; } = new GlobalHelper();
    Counter field = new Counter();
    Counter() {}
    Counter(int value) : this() {}
    Counter(string value) : base() {}
    int Helper() => 1;
    T Generic<T>() => default;
    int Caller() => Helper();
    int GenericCaller() => Generic<int>();
    int ExplicitThis() => this.Helper();
    int ExplicitThisParameterShadow(Func<int> Helper) => this.Helper();
    int BaseCaller() => base.BaseHelper();
    int Other(Counter counter) => counter.Helper();
    int Inherited(Counter counter) => counter.BaseHelper();
    int LocalTyped() { Counter local = new Counter(); return local.Helper(); }
    int NewReceiver() => new Counter().Helper();
    int NewGenericReceiver() => new Counter<int>().Helper();
    int NewStaticReceiver() => new GlobalHelper().Utility();
    int AnonymousToString() => new { Value = 1 }.ToString();
    int FieldReceiver() => field.Helper();
    int GlobalStaticCaller() => global::GlobalHelper.Utility();
    int GlobalGenericStaticCaller() => global::GlobalHelper.GenericUtility<int>();
    int GlobalInstanceCaller() => global::GlobalHelper.Instance();
    int TypeParameterShadow() => T.Equals(default);
    int MemberShadow() => GlobalHelper.Instance();
    int LocalShadow() { var GlobalHelper = new GlobalHelper(); return GlobalHelper.Instance(); }
    int ParameterShadow(Func<int> Helper) => Helper();
    int LocalFunctionShadow() { int Helper() => 2; return Helper(); }
    int LambdaShadow() => ((Func<int, int>)(Helper => Helper())).Invoke(1);
}

class SimpleCaller {
    int SimpleStaticCaller() => GlobalHelper.Utility();
}
"#;
        let path = Path::new("Counter.cs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_csharp_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        let references = |path: &str| {
            symbols
                .iter()
                .find(|symbol| symbol.semantic_path == path)
                .unwrap()
                .references_by_name
                .clone()
        };
        let delegated_constructor = symbols
            .iter()
            .find(|symbol| {
                symbol.semantic_path == "Counter::Counter" && symbol.parameters == ["int value"]
            })
            .unwrap();
        assert_eq!(
            delegated_constructor.references_by_name,
            ["this".to_string()].into()
        );
        assert_eq!(
            delegated_constructor.call_arities_by_name,
            [("this".to_string(), [0].into())].into()
        );
        let base_constructor = symbols
            .iter()
            .find(|symbol| {
                symbol.semantic_path == "Counter::Counter" && symbol.parameters == ["string value"]
            })
            .unwrap();
        assert_eq!(
            base_constructor.references_by_name,
            ["base".to_string()].into()
        );
        assert_eq!(
            base_constructor.call_arities_by_name,
            [("base".to_string(), [0].into())].into()
        );
        assert_eq!(references("Counter::Caller"), ["Helper".to_string()].into());
        assert_eq!(
            references("Counter::GenericCaller"),
            ["Generic".to_string()].into()
        );
        assert_eq!(
            references("Counter::ExplicitThis"),
            ["this.Helper".to_string()].into()
        );
        assert_eq!(
            references("Counter::ExplicitThisParameterShadow"),
            ["this.Helper".to_string()].into()
        );
        assert_eq!(
            references("Counter::BaseCaller"),
            ["base.BaseHelper".to_string()].into()
        );
        assert_eq!(
            references("Counter::Other"),
            ["counter.Helper".to_string()].into()
        );
        assert_eq!(
            references("Counter::Inherited"),
            ["counter.BaseHelper".to_string()].into()
        );
        assert_eq!(
            references("Counter::LocalTyped"),
            ["local.Helper".to_string()].into()
        );
        assert_eq!(
            references("Counter::NewReceiver"),
            ["Counter().Helper".to_string()].into()
        );
        assert_eq!(
            references("Counter::NewGenericReceiver"),
            ["Counter().Helper".to_string()].into()
        );
        assert_eq!(
            references("Counter::NewStaticReceiver"),
            ["GlobalHelper().Utility".to_string()].into()
        );
        assert!(references("Counter::AnonymousToString").is_empty());
        assert_eq!(
            references("Counter::FieldReceiver"),
            ["field.Helper".to_string()].into()
        );
        assert_eq!(
            references("Counter::GlobalStaticCaller"),
            ["global::GlobalHelper.Utility".to_string()].into()
        );
        assert_eq!(
            references("Counter::GlobalGenericStaticCaller"),
            ["global::GlobalHelper.GenericUtility".to_string()].into()
        );
        assert_eq!(
            references("Counter::GlobalInstanceCaller"),
            ["global::GlobalHelper.Instance".to_string()].into()
        );
        assert_eq!(
            references("SimpleCaller::SimpleStaticCaller"),
            ["GlobalHelper.Utility".to_string()].into()
        );
        assert!(references("Counter::TypeParameterShadow").is_empty());
        assert_eq!(
            references("Counter::MemberShadow"),
            ["GlobalHelper.Instance".to_string()].into()
        );
        assert_eq!(
            references("Counter::LocalShadow"),
            ["GlobalHelper.Instance".to_string()].into()
        );
        assert!(references("Counter::ParameterShadow").is_empty());
        assert!(references("Counter::LocalFunctionShadow").is_empty());
        assert!(references("Counter::LambdaShadow").is_empty());
    }

    #[test]
    fn element_access_receivers_canonicalize_to_array_spellings() {
        let source = r#"
namespace Demo;

class Helper {
    public int helper(int value) => value;
}
class Group {
    public Helper item = new Helper();
    public int helper(int value) => value;
    public Group inner() => this;
}
class Caller {
    private Helper[] fieldItems = new Helper[2];
    private Group[] groups = new Group[1];
    public int plainBound(Helper[] items) => items[0].helper(1);
    public int indexedField(Helper[] items) => items[1].helper(2);
    public int boundChain(Group[] groups) => groups[0].inner().helper(1);
    public int thisField() => this.fieldItems[0].helper(1);
    public int bareField() => fieldItems[0].helper(1);
    public int factoryRoot() => makeItems()[0].helper(3);
    private Helper[] makeItems() => new Helper[2];
}
"#;
        let path = Path::new("Caller.cs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_csharp_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        let symbol = |name: &str| {
            symbols
                .iter()
                .find(|symbol| symbol.semantic_path == format!("Demo::Caller::{name}"))
                .unwrap()
        };
        // `items[0]` and `items[1]` keep their element-access spelling so the
        // resolver can dispatch the trailing member on the element type while
        // still distinguishing it from a direct call on the array itself.
        assert!(
            symbol("plainBound")
                .references_by_name
                .contains("items[0].helper")
        );
        assert!(
            symbol("indexedField")
                .references_by_name
                .contains("items[1].helper")
        );
        assert!(
            symbol("boundChain")
                .references_by_name
                .contains("groups[0].inner().helper")
        );
        assert!(
            symbol("thisField")
                .references_by_name
                .contains("this.fieldItems[0].helper")
        );
        assert!(
            symbol("bareField")
                .references_by_name
                .contains("fieldItems[0].helper")
        );
        // A factory-rooted element access records the full element-access
        // chain so the resolver can dispatch the trailing member on the
        // factory return array's element component type; the inner bare call
        // is recorded too.
        assert!(
            symbol("factoryRoot")
                .references_by_name
                .contains("makeItems()[0].helper")
        );
        assert!(
            symbol("factoryRoot")
                .references_by_name
                .contains("makeItems")
        );
    }
}
