use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::normalize_path;
use crate::semantic::java::{
    is_java_symbol_node, java_parameters, java_return_type, java_semantic_path, java_signature,
    java_symbol_name,
};
use crate::semantic::semantic_parent_path;
use crate::symbol_dependency::java_dotted_type_name;
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::symbol_reference_compat::reference_facts_from_legacy;
use crate::workspace_scan::WorkspaceScanDeadline;

type ReferenceNames = BTreeSet<String>;
type CallAritiesByName = BTreeMap<String, BTreeSet<usize>>;
type DirectCalls = (ReferenceNames, CallAritiesByName);

pub(crate) fn index_java_symbols_with_deadline(
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
        deadline.check("extracting Java symbols")?;
    }
    if node.kind() == "field_declaration" {
        collect_field_symbols(path, source, root, node, deadline, symbols)?;
    } else if is_java_symbol_node(node)
        && let Some(symbol) = indexed_symbol(path, source, root, node, deadline)?
    {
        symbols.push(symbol);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_symbols(path, source, root, child, deadline, symbols)?;
    }
    Ok(())
}

/// Indexes each named field in a `field_declaration` (one symbol per
/// declarator) under the enclosing type path. Field symbols carry their
/// declared type as the return type and record no direct calls.
fn collect_field_symbols(
    path: &Path,
    source: &str,
    root: Node<'_>,
    node: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
    symbols: &mut Vec<IndexedSymbol>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("extracting Java field symbols")?;
    }
    let Some(field_type) = java_return_type(node, source) else {
        return Ok(());
    };
    let mut cursor = node.walk();
    for declarator in node.children_by_field_name("declarator", &mut cursor) {
        let Some(name) = java_symbol_name(declarator, source)? else {
            continue;
        };
        let Some(semantic_path) = java_semantic_path(root, node, source, &name)? else {
            continue;
        };
        symbols.push(IndexedSymbol {
            extension_receiver: None,
            symbol_id: String::new(),
            base_name: symbol_base_name(&semantic_path),
            scope_path: semantic_parent_path(&semantic_path),
            semantic_path,
            file_path: normalize_path(path),
            node_kind: "field_declaration".to_string(),
            byte_range: (declarator.start_byte(), declarator.end_byte()),
            signature: java_field_modifiers(node, source),
            is_overload: false,
            parameters: Vec::new(),
            return_type: Some(field_type.clone()),
            docstring: None,
            reference_facts: Vec::new(),
            references_by_name: BTreeSet::new(),
            call_arities_by_name: BTreeMap::new(),
        });
    }
    Ok(())
}

/// Returns the modifier text of a `field_declaration` such as `static`,
/// `public static final`, or an empty marker for fields without modifiers.
/// Trace-time static-field resolution uses the `static` token to require a
/// static field for `Type.field` references.
fn java_field_modifiers(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    let first_declarator = node
        .children_by_field_name("declarator", &mut cursor)
        .next()?;
    let modifiers = source
        .get(node.start_byte()..first_declarator.start_byte())?
        .trim();
    (!modifiers.is_empty()).then(|| modifiers.to_string())
}

fn indexed_symbol(
    path: &Path,
    source: &str,
    root: Node<'_>,
    node: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<IndexedSymbol>> {
    let Some(name) = java_symbol_name(node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = java_semantic_path(root, node, source, &name)? else {
        return Ok(None);
    };
    let (references_by_name, call_arities_by_name) =
        collect_direct_local_calls(node, source, deadline)?;

    Ok(Some(IndexedSymbol {
        extension_receiver: None,
        symbol_id: String::new(),
        base_name: symbol_base_name(&semantic_path),
        scope_path: semantic_parent_path(&semantic_path),
        semantic_path,
        file_path: normalize_path(path),
        node_kind: node.kind().to_string(),
        byte_range: (node.start_byte(), node.end_byte()),
        signature: java_signature(node, source),
        is_overload: false,
        parameters: java_parameters(node, source),
        return_type: java_return_type(node, source),
        docstring: None,
        reference_facts: reference_facts_from_legacy(&references_by_name, &call_arities_by_name),
        references_by_name,
        call_arities_by_name,
    }))
}

/// Records the constructed type spelling of an `object_creation_expression`
/// such as `Helper()` or `Outer.Inner()`, ignoring any anonymous-class body.
/// Malformed and non-dotted type spellings produce no fact and fail closed.
fn java_constructor_type_spelling(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(type_node) = node.child_by_field_name("type") else {
        return Ok(None);
    };
    let type_name = crate::language::node_text(type_node, source)?.trim();
    Ok(java_dotted_type_name(type_name).map(|name| format!("{name}()")))
}

/// Returns whether `node` is an `object_creation_expression` carrying an
/// anonymous-class body.
fn java_has_anonymous_body(node: Node<'_>) -> bool {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| child.kind() == "class_body")
}

/// Returns the names of methods and fields declared directly in an
/// anonymous-class body on `node`. Non-anonymous nodes return an empty set.
/// Anonymous receiver slices resolve on the constructed class type only when
/// the body does not declare a member that the call would dispatch through; a
/// same-name body declaration (method or field) would change the actual
/// dispatch target, so it fails closed conservatively.
fn java_anonymous_constructor_declared_members(
    node: Node<'_>,
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
            "method_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = crate::language::node_text(name_node, source)?.trim();
                    if !name.is_empty() {
                        declared.insert(name.to_string());
                    }
                }
            }
            "field_declaration" => {
                let mut declarator_cursor = child.walk();
                for declarator in child.children_by_field_name("declarator", &mut declarator_cursor)
                {
                    if let Some(name_node) = declarator.child_by_field_name("name") {
                        let name = crate::language::node_text(name_node, source)?.trim();
                        if !name.is_empty() {
                            declared.insert(name.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(declared)
}

/// Canonicalizes a receiver chain whose leading expression is a constructor
/// call, such as `new Group().inner.helper` or `new Group().inner().helper`,
/// to `Group().inner.helper` or `Group().inner().helper` so the resolver can
/// dispatch the constructed type through field and method-call hops.
/// Method-call hops encode their argument count, such as `inner()` or
/// `inner(1)`, so the resolver can require an arity match. Chains rooted at an
/// anonymous constructor resolve on the constructed class type only when the
/// anonymous body declares neither the final member nor any method-call hop in
/// the chain. Chains rooted at any other expression keep their raw spelling
/// and fail closed.
fn java_constructor_receiver_chain_spelling(
    node: Node<'_>,
    source: &str,
    final_member: &str,
) -> Result<Option<String>> {
    let mut segments = Vec::new();
    let mut current = node;
    loop {
        if current.kind() == "field_access" {
            let Some(field) = current.child_by_field_name("field") else {
                return Ok(None);
            };
            let field_name = crate::language::node_text(field, source)?.trim();
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
            let method_name = crate::language::node_text(name_node, source)?.trim();
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
            if java_has_anonymous_body(current)
                && java_anonymous_constructor_declared_members(current, source)?
                    .iter()
                    .any(|declared| {
                        declared == final_member
                            || segments
                                .iter()
                                .any(|segment| segment.split('(').next() == Some(declared.as_str()))
                    })
            {
                return Ok(None);
            }
            let Some(spelling) = java_constructor_type_spelling(current, source)? else {
                return Ok(None);
            };
            segments.push(spelling);
            break;
        } else {
            return Ok(None);
        }
    }
    segments.reverse();
    Ok(Some(segments.join(".")))
}

fn collect_direct_local_calls(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<DirectCalls> {
    if !matches!(
        symbol_node.kind(),
        "method_declaration" | "constructor_declaration"
    ) {
        return Ok((BTreeSet::new(), BTreeMap::new()));
    }
    let Some(body) = symbol_node.child_by_field_name("body") else {
        return Ok((BTreeSet::new(), BTreeMap::new()));
    };

    let type_name_exclusions = collect_local_type_name_exclusions(symbol_node, source)?;
    let mut references = BTreeSet::new();
    let mut call_arities_by_name = BTreeMap::new();
    collect_direct_local_calls_from_node(
        body,
        source,
        deadline,
        &type_name_exclusions,
        &mut references,
        &mut call_arities_by_name,
    )?;
    Ok((references, call_arities_by_name))
}

fn collect_direct_local_calls_from_node(
    node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
    qualified_call_exclusions: &BTreeSet<String>,
    references: &mut ReferenceNames,
    call_arities_by_name: &mut CallAritiesByName,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("collecting Java direct calls")?;
    }
    if node.kind() == "class_body" {
        return Ok(());
    }
    if node.kind() == "explicit_constructor_invocation"
        && let Some(constructor) = node.child_by_field_name("constructor")
        && matches!(constructor.kind(), "this" | "super")
        && let Some(arguments) = node.child_by_field_name("arguments")
    {
        let mut cursor = arguments.walk();
        let arity = arguments.named_children(&mut cursor).count();
        let reference = constructor.kind().to_string();
        references.insert(reference.clone());
        call_arities_by_name
            .entry(reference)
            .or_default()
            .insert(arity);
    }
    if node.kind() == "method_invocation"
        && let Some(name_node) = node.child_by_field_name("name")
        && let Some(arguments) = node.child_by_field_name("arguments")
    {
        let name = crate::language::node_text(name_node, source)?.trim();
        let reference = match node.child_by_field_name("object") {
            None => (!name.is_empty()).then(|| name.to_string()),
            Some(object) if object.kind() == "identifier" && !name.is_empty() => {
                let object_name = crate::language::node_text(object, source)?.trim();
                (!object_name.is_empty() && !qualified_call_exclusions.contains(object_name))
                    .then(|| format!("{object_name}.{name}"))
            }
            Some(object)
                if matches!(object.kind(), "field_access" | "method_invocation")
                    && !name.is_empty() =>
            {
                if let Some(spelling) =
                    java_constructor_receiver_chain_spelling(object, source, name)?
                {
                    Some(format!("{spelling}.{name}"))
                } else {
                    let object_name = crate::language::node_text(object, source)?.trim();
                    let receiver_name = object_name.split(['.', '(']).next().unwrap_or_default();
                    (!object_name.is_empty()
                        && !receiver_name.is_empty()
                        && !qualified_call_exclusions.contains(receiver_name))
                    .then(|| format!("{object_name}.{name}"))
                }
            }
            Some(object) if matches!(object.kind(), "this" | "super") && !name.is_empty() => {
                Some(format!("{}.{name}", object.kind()))
            }
            Some(object) if object.kind() == "object_creation_expression" && !name.is_empty() => {
                // Direct anonymous constructor receivers such as
                // `new Helper() { }.helper(...)` resolve on the constructed
                // class type only when the anonymous body does not declare the
                // invoked member; a same-name body declaration would change
                // the actual dispatch target, so it produces no fact.
                if java_anonymous_constructor_declared_members(object, source)?.contains(name) {
                    None
                } else {
                    java_constructor_type_spelling(object, source)?
                        .map(|spelling| format!("{spelling}.{name}"))
                }
            }
            Some(_) => None,
        };
        if let Some(reference) = reference {
            let mut cursor = arguments.walk();
            let arity = arguments.named_children(&mut cursor).count();
            references.insert(reference.clone());
            call_arities_by_name
                .entry(reference)
                .or_default()
                .insert(arity);
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_direct_local_calls_from_node(
            child,
            source,
            deadline,
            qualified_call_exclusions,
            references,
            call_arities_by_name,
        )?;
    }
    Ok(())
}

fn collect_local_type_name_exclusions(
    symbol_node: Node<'_>,
    source: &str,
) -> Result<BTreeSet<String>> {
    fn insert_name(node: Node<'_>, source: &str, exclusions: &mut BTreeSet<String>) -> Result<()> {
        let name = crate::language::node_text(node, source)?.trim();
        if !name.is_empty() {
            exclusions.insert(name.to_string());
        }
        Ok(())
    }

    fn collect_type_names(
        node: Node<'_>,
        source: &str,
        exclusions: &mut BTreeSet<String>,
    ) -> Result<()> {
        if matches!(
            node.kind(),
            "annotation_type_declaration"
                | "class_declaration"
                | "enum_declaration"
                | "interface_declaration"
                | "record_declaration"
        ) && let Some(name) = node.child_by_field_name("name")
        {
            insert_name(name, source, exclusions)?;
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            collect_type_names(child, source, exclusions)?;
        }
        Ok(())
    }

    let mut exclusions = BTreeSet::new();
    collect_type_names(symbol_node, source, &mut exclusions)?;
    Ok(exclusions)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::index_java_symbols_with_deadline;
    use crate::language::parse_document;

    #[test]
    fn indexes_package_qualified_java_declarations_and_unqualified_direct_calls() {
        let source = r#"
package com.example;

public class Counter {
    public Counter(int initial) {}
    public Counter(String label) {}
    public int increment(int amount) { return amount; }
    public int increment(long amount) { return (int) amount; }
    public int callIncrement() { return increment(1); }
    public int qualified() { return Helper.run(1); }
    public int shadowed(Helper Helper) { return Helper.run(1); }
    public int lambdaShadowed() {
        return ((java.util.function.IntUnaryOperator) (Helper -> Helper.run(1))).applyAsInt(0);
    }
    public int resourceShadowed() {
        try (java.io.InputStream Helper = null) { return Helper.run(1); }
        catch (Exception ignored) { return 0; }
    }
    public int patternShadowed(Object value) {
        if (value instanceof Object Helper) { return Helper.run(1); }
        return 0;
    }
    public int ambiguousIncrement() { return increment(1L); }
}
class Outer {
    private Helper Helper;
    class Nested {
        public int outerFieldShadowed() { return Helper.run(1); }
    }
}
interface Renderer { String render(); }
enum Kind { BASIC }
"#;
        let path = Path::new("Counter.java");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_java_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.semantic_path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "com::example::Counter",
                "com::example::Counter::Counter",
                "com::example::Counter::Counter",
                "com::example::Counter::increment",
                "com::example::Counter::increment",
                "com::example::Counter::callIncrement",
                "com::example::Counter::qualified",
                "com::example::Counter::shadowed",
                "com::example::Counter::lambdaShadowed",
                "com::example::Counter::resourceShadowed",
                "com::example::Counter::patternShadowed",
                "com::example::Counter::ambiguousIncrement",
                "com::example::Outer",
                "com::example::Outer::Helper",
                "com::example::Outer::Nested",
                "com::example::Outer::Nested::outerFieldShadowed",
                "com::example::Renderer",
                "com::example::Renderer::render",
                "com::example::Kind",
            ]
        );
        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::Counter::callIncrement")
            .unwrap();
        assert_eq!(caller.references_by_name, ["increment".to_string()].into());
        assert_eq!(
            caller.call_arities_by_name,
            [("increment".to_string(), [1].into())].into()
        );
        assert_eq!(caller.reference_facts.len(), 1);
        let qualified = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::Counter::qualified")
            .unwrap();
        assert_eq!(
            qualified.references_by_name,
            ["Helper.run".to_string()].into()
        );
        for method_name in [
            "com::example::Counter::shadowed",
            "com::example::Counter::lambdaShadowed",
            "com::example::Counter::resourceShadowed",
            "com::example::Counter::patternShadowed",
            "com::example::Outer::Nested::outerFieldShadowed",
        ] {
            let bound_receiver = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == method_name)
                .unwrap();
            // Receivers that resolve to a local value binding (parameter,
            // lambda/resource/pattern binding, or enclosing field) are recorded
            // as instance calls and dispatched by the resolver on the declared
            // receiver type.
            assert_eq!(
                bound_receiver.references_by_name,
                ["Helper.run".to_string()].into()
            );
        }

        let method = symbols
            .iter()
            .find(|symbol| {
                symbol.semantic_path == "com::example::Counter::increment"
                    && symbol.parameters == ["int amount"]
            })
            .unwrap();
        assert_eq!(method.return_type.as_deref(), Some("int"));
    }

    #[test]
    fn records_constructor_call_receivers_for_qualified_direct_calls() {
        let source = r#"
package com.example;
class Helper { int helper(int value) { return value; } }
class Other { int helper(int value) { return value + 10; } }
class Outer { static class Inner { int helper(int value) { return value; } } }
class Group {
    Helper inner = new Helper();
    Group inner2(int value) { return this; }
}
class Caller {
    int first() { return new Helper().helper(1); }
    int nested() { return new Outer.Inner().helper(2); }
    int anonymous() { return new Helper() { }.helper(3); }
    int overridden() { return new Helper() { int helper(int value) { return value + 1; } }.helper(4); }
    int chained() { return new Group().inner.helper(4); }
    int anonymousChain() { return new Group() { }.inner.helper(5); }
    int anonymousChainHop() { return new Group() { }.inner2(1).inner.helper(8); }
    int anonymousOverrideFinal() {
        return new Group() { int helper(int value) { return value + 1; } }.inner.helper(6);
    }
    int anonymousOverrideHop() {
        return new Group() { int inner2() { return new Group(); } }.inner2().inner.helper(7);
    }
    int anonymousOverrideArgHop() {
        return new Group() { Group inner2(int value) { return this; } }.inner2(1).inner.helper(9);
    }
    int anonymousFieldShadow() {
        return new Group() { Other inner = new Other(); }.inner.helper(10);
    }
}
"#;
        let path = Path::new("Caller.java");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_java_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();
        let first = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::Caller::first")
            .unwrap();
        assert_eq!(
            first.references_by_name,
            ["Helper().helper".to_string()].into()
        );
        let nested = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::Caller::nested")
            .unwrap();
        assert_eq!(
            nested.references_by_name,
            ["Outer.Inner().helper".to_string()].into()
        );
        // Constructor-call chains canonicalize to `Group().inner.helper`.
        let chained = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::Caller::chained")
            .unwrap();
        assert_eq!(
            chained.references_by_name,
            ["Group().inner.helper".to_string()].into()
        );
        // Anonymous-rooted chains canonicalize to the constructed type when
        // the body declares neither the final member nor a method-call hop.
        let anonymous_chain = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::Caller::anonymousChain")
            .unwrap();
        assert_eq!(
            anonymous_chain.references_by_name,
            ["Group().inner.helper".to_string()].into()
        );
        // Arity-matched method-call hops encode their argument count in the
        // canonical spelling so the resolver can require a matching arity.
        let anonymous_chain_hop = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::Caller::anonymousChainHop")
            .unwrap();
        assert_eq!(
            anonymous_chain_hop.references_by_name,
            [
                "Group().inner2".to_string(),
                "Group().inner2(1).inner.helper".to_string()
            ]
            .into()
        );
        // Direct anonymous-class receivers resolve on the constructed class
        // type when the body does not declare the invoked member.
        let anonymous = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::Caller::anonymous")
            .unwrap();
        assert_eq!(
            anonymous.references_by_name,
            ["Helper().helper".to_string()].into()
        );
        // A same-name body declaration changes the dispatch target, so the
        // anonymous receiver produces no direct-call fact.
        let overridden = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::Caller::overridden")
            .unwrap();
        assert!(overridden.references_by_name.is_empty());
        // A body declaration of the final member or of a method-call hop in an
        // anonymous-rooted chain prevents canonicalization; the raw fallback
        // spelling never resolves at trace time.
        for method in ["anonymousOverrideFinal", "anonymousOverrideHop"] {
            let symbol = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == format!("com::example::Caller::{method}"))
                .unwrap();
            assert!(
                !symbol
                    .references_by_name
                    .iter()
                    .any(|reference| reference == "Group().inner.helper"),
                "anonymous-rooted chains whose body declares a dispatched member must not canonicalize"
            );
        }
        // A body declaration of an arity-matched method-call hop prevents
        // canonicalization of the anonymous-rooted chain.
        let override_arg_hop = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::Caller::anonymousOverrideArgHop")
            .unwrap();
        assert!(
            !override_arg_hop
                .references_by_name
                .iter()
                .any(|reference| reference == "Group().inner2(1).inner.helper"),
            "anonymous-rooted chains whose body declares an arity-matched hop must not canonicalize"
        );
        // A body field declaration shadows a same-named field hop in an
        // anonymous-rooted chain, so the chain must not canonicalize.
        let field_shadow = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::Caller::anonymousFieldShadow")
            .unwrap();
        assert!(
            !field_shadow
                .references_by_name
                .iter()
                .any(|reference| reference == "Group().inner.helper"),
            "anonymous-rooted chains whose body declares a field hop must not canonicalize"
        );
    }
}
