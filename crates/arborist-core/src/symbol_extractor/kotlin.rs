use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, normalize_path};
use crate::semantic::kotlin::{
    is_kotlin_semantic_symbol_node, kotlin_parameters, kotlin_return_type, kotlin_semantic_path,
    kotlin_signature, kotlin_symbol_name,
};
use crate::semantic::semantic_parent_path;
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::symbol_reference_compat::reference_facts_from_legacy;
use crate::workspace_scan::WorkspaceScanDeadline;

type ReferenceNames = BTreeSet<String>;
type CallAritiesByName = BTreeMap<String, BTreeSet<usize>>;
type DirectCalls = (ReferenceNames, CallAritiesByName);

pub(crate) fn index_kotlin_symbols_with_deadline(
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
        deadline.check("extracting Kotlin symbols")?;
    }
    if is_kotlin_semantic_symbol_node(node)
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

fn indexed_symbol(
    path: &Path,
    source: &str,
    root: Node<'_>,
    node: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<IndexedSymbol>> {
    let Some(name) = kotlin_symbol_name(node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = kotlin_semantic_path(root, node, source, &name)? else {
        return Ok(None);
    };
    let (references_by_name, call_arities_by_name) =
        collect_direct_local_calls(node, source, deadline)?;

    Ok(Some(IndexedSymbol {
        extension_receiver: kotlin_extension_receiver(node, source)?,
        symbol_id: String::new(),
        base_name: symbol_base_name(&semantic_path),
        scope_path: semantic_parent_path(&semantic_path),
        semantic_path,
        file_path: normalize_path(path),
        node_kind: node.kind().to_string(),
        byte_range: (node.start_byte(), node.end_byte()),
        signature: kotlin_signature(node, source),
        is_overload: false,
        parameters: kotlin_parameters(node, source),
        return_type: kotlin_return_type(node, source),
        docstring: None,
        reference_facts: reference_facts_from_legacy(&references_by_name, &call_arities_by_name),
        references_by_name,
        call_arities_by_name,
    }))
}

/// Records the receiver type of a top-level extension function such as
/// `fun Other.helper(...)`. Only simple named non-nullable receivers are
/// recorded; generic, nullable, parenthesized, and modifier-laden receivers
/// fail closed so resolution never guesses a target.
fn kotlin_extension_receiver(node: Node<'_>, source: &str) -> Result<Option<String>> {
    if node.kind() != "function_declaration" {
        return Ok(None);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "function_value_parameters" {
            return Ok(None);
        }
        if child.kind() == "user_type" {
            return Ok(kotlin_simple_type_name(node_text(child, source)?));
        }
    }
    Ok(None)
}

fn kotlin_simple_type_name(text: &str) -> Option<String> {
    let mut name = text.trim();
    if let Some(stripped) = name.strip_suffix('?') {
        name = stripped.trim();
    }
    if name.is_empty() || name.contains(['.', '<', '(', '[', ':', ',', ' ']) {
        return None;
    }
    Some(name.to_string())
}

fn collect_direct_local_calls(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<DirectCalls> {
    if symbol_node.kind() != "function_declaration" {
        return Ok((BTreeSet::new(), BTreeMap::new()));
    }
    let Some(body) = symbol_node
        .named_children(&mut symbol_node.walk())
        .find(|child| child.kind() == "function_body")
    else {
        return Ok((BTreeSet::new(), BTreeMap::new()));
    };

    let mut references = BTreeSet::new();
    let mut call_arities_by_name = BTreeMap::new();
    collect_direct_local_calls_from_node(
        body,
        source,
        deadline,
        &mut references,
        &mut call_arities_by_name,
    )?;
    Ok((references, call_arities_by_name))
}

fn kotlin_call_spelling(callee: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(segments) = kotlin_navigation_segments(callee, source)? else {
        return Ok(None);
    };
    Ok((!segments.is_empty()).then(|| segments.join(".")))
}

/// Collects the dotted segments of a navigation chain such as `other.helper`,
/// `group.member.helper`, `Outer.Inner().helper`, `Group().member.helper`, or
/// `items[0].helper`. The base may be a plain identifier, a pure identifier
/// navigation chain, a call expression over either, or an element access over
/// either; a call base marks its last segment with `()` so resolution can
/// distinguish a constructor-call receiver from a class-name receiver, and an
/// element-access base merges its bracket onto the accessed element's segment
/// so resolution dispatches on the element component type. A parenthesized
/// receiver such as `(group)` in `(group).entry.helper(...)` or
/// `(makeGroup())` in `(makeGroup()).entry.helper(...)` unwraps to the same
/// chain spelling as the unparenthesized form so the trailing member
/// dispatches on the same resolved receiver. Nullable (`?.`),
/// callable-reference (`::`), complex-index, and multi-dimensional receivers
/// still fail closed and produce no direct-call fact so resolution never
/// guesses a target.
fn kotlin_navigation_segments(node: Node<'_>, source: &str) -> Result<Option<Vec<String>>> {
    if node.kind() == "identifier" {
        let segment = node_text(node, source)?.trim().to_string();
        return Ok((!segment.is_empty()).then(|| vec![segment]));
    }
    // A call base such as `Group()` in `Group().member.helper(...)` or
    // `Outer.Inner()` in `Outer.Inner().helper(...)` records the constructed
    // type path with a `()` marker on its last segment. A function-call base
    // such as `makeOther()` is recorded the same way and fails closed later in
    // resolution because the callee is not a constructible type.
    if node.kind() == "call_expression" {
        let Some(callee) = node.named_child(0) else {
            return Ok(None);
        };
        let Some(mut segments) = kotlin_navigation_segments(callee, source)? else {
            return Ok(None);
        };
        let Some(last) = segments.last_mut() else {
            return Ok(None);
        };
        last.push_str("()");
        return Ok(Some(segments));
    }
    // An element-access base such as `items[0]` in `items[0].helper(...)`
    // merges its bracket onto the accessed element's segment so resolution can
    // dispatch on the base array's element component type. Only a simple
    // single subscript (an identifier or literal) is recorded; nested,
    // multi-index, function-call, and nullable indices fail closed.
    if node.kind() == "index_expression" {
        let mut cursor = node.walk();
        let children = node.named_children(&mut cursor).collect::<Vec<_>>();
        if children.len() != 2 {
            return Ok(None);
        }
        let Some(mut segments) = kotlin_navigation_segments(children[0], source)? else {
            return Ok(None);
        };
        let subscript = node_text(children[1], source)?.trim();
        if subscript.is_empty() || subscript.contains(['[', '(', ')', ',', '?', '.']) {
            return Ok(None);
        }
        let Some(last) = segments.last_mut() else {
            return Ok(None);
        };
        last.push('[');
        last.push_str(subscript);
        last.push(']');
        return Ok(Some(segments));
    }
    // A parenthesized receiver such as `(group)` in `(group).entry.helper(...)`,
    // `(makeGroup())` in `(makeGroup()).entry.helper(...)`, or `((group))` in
    // `((group)).entry.helper(...)` unwraps to the same chain spelling as the
    // unparenthesized form so the trailing member dispatches on the same
    // resolved receiver; malformed or empty parentheses fail closed.
    if node.kind() == "parenthesized_expression" {
        let Some(inner) = node.named_child(0) else {
            return Ok(None);
        };
        return kotlin_navigation_segments(inner, source);
    }
    if node.kind() != "navigation_expression" {
        return Ok(None);
    }
    let text = node_text(node, source)?.trim();
    if text.contains('?') || text.contains("::") {
        return Ok(None);
    }
    let mut cursor = node.walk();
    let children = node.named_children(&mut cursor).collect::<Vec<_>>();
    if children.len() != 2 || children[1].kind() != "identifier" {
        return Ok(None);
    }
    let Some(mut segments) = kotlin_navigation_segments(children[0], source)? else {
        return Ok(None);
    };
    let member = node_text(children[1], source)?.trim().to_string();
    if member.is_empty() {
        return Ok(None);
    }
    segments.push(member);
    Ok(Some(segments))
}

fn collect_direct_local_calls_from_node(
    node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
    references: &mut ReferenceNames,
    call_arities_by_name: &mut CallAritiesByName,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("collecting Kotlin direct calls")?;
    }
    if matches!(
        node.kind(),
        "function_declaration" | "class_declaration" | "object_declaration"
    ) {
        return Ok(());
    }
    if node.kind() == "call_expression"
        && let Some(callee) = node.named_child(0)
        && let Some(arguments) = node
            .named_children(&mut node.walk())
            .find(|child| child.kind() == "value_arguments")
        && let Some(reference) = kotlin_call_spelling(callee, source)?
    {
        let mut cursor = arguments.walk();
        let arity = arguments.named_children(&mut cursor).count();
        references.insert(reference.clone());
        call_arities_by_name
            .entry(reference)
            .or_default()
            .insert(arity);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_direct_local_calls_from_node(
            child,
            source,
            deadline,
            references,
            call_arities_by_name,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::index_kotlin_symbols_with_deadline;
    use crate::language::parse_document;

    #[test]
    fn indexes_package_qualified_kotlin_declarations_and_direct_calls() {
        let source = r#"
package com.example

typealias UserId = String

fun helper(amount: Int): Int = amount

class Counter {
    val label: String = "counter"
    fun increment(amount: Int): Int = amount
    fun increment(amount: Long): Long = amount
    fun outer() {
        class Local
        fun nested() = 1
        helper(1)
    }
}

object Config {
    val answer = 42
}
"#;
        let path = Path::new("Counter.kt");
        let document = parse_document(path, source).unwrap();
        assert!(!document.tree.root_node().has_error());
        let symbols =
            index_kotlin_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.semantic_path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "com::example::UserId",
                "com::example::helper",
                "com::example::Counter",
                "com::example::Counter::label",
                "com::example::Counter::increment",
                "com::example::Counter::increment",
                "com::example::Counter::outer",
                "com::example::Config",
                "com::example::Config::answer",
            ]
        );
        let outer = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::Counter::outer")
            .unwrap();
        assert_eq!(
            outer.references_by_name,
            ["helper".to_string()].into_iter().collect()
        );
        assert_eq!(
            outer.call_arities_by_name,
            [("helper".to_string(), [1usize].into_iter().collect())]
                .into_iter()
                .collect()
        );
        assert!(
            symbols
                .iter()
                .filter(|symbol| symbol.semantic_path != "com::example::Counter::outer")
                .all(|symbol| symbol.reference_facts.is_empty()
                    && symbol.references_by_name.is_empty()
                    && symbol.call_arities_by_name.is_empty())
        );
        let increment = symbols
            .iter()
            .find(|symbol| {
                symbol.semantic_path == "com::example::Counter::increment"
                    && symbol.parameters == ["amount: Int"]
            })
            .unwrap();
        assert_eq!(increment.return_type.as_deref(), Some("Int"));
    }

    #[test]
    fn records_kotlin_qualified_receiver_calls_and_skips_complex_or_nullable_receivers() {
        let source = r#"
package com.example

class Other {
    fun helper(value: Int): Int = value
}

fun caller(other: Other): Int {
    other.helper(1)
    other?.helper(2)
    val group = Group()
    group.member.helper(3)
    factory().helper(4)
    return 0
}

fun factory(): Other = Other()
"#;
        let path = Path::new("Caller.kt");
        let document = parse_document(path, source).unwrap();
        assert!(!document.tree.root_node().has_error());
        let symbols =
            index_kotlin_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();
        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::caller")
            .unwrap();
        assert!(caller.references_by_name.contains("other.helper"));
        assert!(caller.references_by_name.contains("group.member.helper"));
        // Call-rooted navigation bases are recorded with a `()` marker; the
        // extractor cannot tell a constructor call from a function call, so
        // `factory().helper` is recorded here and resolution fails closed
        // because `factory` does not resolve to a constructible type.
        assert!(caller.references_by_name.contains("factory().helper"));
        assert!(!caller.references_by_name.contains("other?.helper"));
        assert_eq!(
            caller.call_arities_by_name.get("other.helper"),
            Some(&[1usize].into_iter().collect())
        );
        assert_eq!(
            caller.call_arities_by_name.get("group.member.helper"),
            Some(&[1usize].into_iter().collect())
        );
    }

    #[test]
    fn records_kotlin_constructor_chain_receiver_calls() {
        let source = r#"
package com.example

class Outer {
    class Inner {
        fun helper(value: Int): Int = value
    }
}

class Group {
    val member: Inner = Inner()
    fun helper(value: Int): Int = value
}

fun caller(): Int {
    Outer.Inner().helper(1)
    Group().member.helper(2)
    Group().helper(3)
    return 0
}
"#;
        let path = Path::new("Caller.kt");
        let document = parse_document(path, source).unwrap();
        assert!(!document.tree.root_node().has_error());
        let symbols =
            index_kotlin_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();
        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::caller")
            .unwrap();
        // A call-rooted navigation base records the constructed type path with
        // a `()` marker, and the inner constructor call keeps its own fact.
        assert!(caller.references_by_name.contains("Outer.Inner().helper"));
        assert!(caller.references_by_name.contains("Group().member.helper"));
        assert!(caller.references_by_name.contains("Group().helper"));
        assert!(caller.references_by_name.contains("Outer.Inner"));
        assert!(caller.references_by_name.contains("Group"));
        assert_eq!(
            caller.call_arities_by_name.get("Outer.Inner().helper"),
            Some(&[1usize].into_iter().collect())
        );
        assert_eq!(
            caller.call_arities_by_name.get("Outer.Inner"),
            Some(&[0usize].into_iter().collect())
        );
    }

    #[test]
    fn records_kotlin_element_access_receiver_calls_and_skips_complex_indices() {
        let source = r#"
package com.example

class Helper {
    fun helper(value: Int): Int = value
}

fun caller(items: Array<Helper>, index: Int): Int {
    items[0].helper(1)
    items[index].helper(2)
    items[0][0].helper(3)
    items[getIndex()].helper(4)
    items?.helper(5)
    return 0
}

fun getIndex(): Int = 0
"#;
        let path = Path::new("Caller.kt");
        let document = parse_document(path, source).unwrap();
        assert!(!document.tree.root_node().has_error());
        let symbols =
            index_kotlin_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();
        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::caller")
            .unwrap();
        // An element-access base merges its bracket onto the accessed element's
        // segment so resolution dispatches on the array's element component
        // type; a simple identifier index records the same way. A
        // multi-dimensional element access records its full spelling and fails
        // closed in resolution, while function-call and nullable indices
        // produce no direct-call fact.
        assert!(caller.references_by_name.contains("items[0].helper"));
        assert!(caller.references_by_name.contains("items[index].helper"));
        assert!(caller.references_by_name.contains("items[0][0].helper"));
        assert!(
            !caller
                .references_by_name
                .contains("items[getIndex()].helper")
        );
        assert!(!caller.references_by_name.contains("items?.helper"));
        assert_eq!(
            caller.call_arities_by_name.get("items[0].helper"),
            Some(&[1usize].into_iter().collect())
        );
        assert_eq!(
            caller.call_arities_by_name.get("items[index].helper"),
            Some(&[1usize].into_iter().collect())
        );
    }

    #[test]
    fn records_extension_receivers_for_simple_top_level_extension_functions() {
        let source = r#"
package com.example

fun Other.helper(value: Int): Int = value
fun String.describe(): String = this
class Other {
    fun member(value: Int): Int = value
}
fun regular(value: Int): Int = value
"#;
        let path = Path::new("Caller.kt");
        let document = parse_document(path, source).unwrap();
        assert!(!document.tree.root_node().has_error());
        let symbols =
            index_kotlin_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();
        let helper = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::helper")
            .unwrap();
        assert_eq!(helper.extension_receiver.as_deref(), Some("Other"));
        let describe = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::describe")
            .unwrap();
        assert_eq!(describe.extension_receiver.as_deref(), Some("String"));
        let member = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::Other::member")
            .unwrap();
        assert_eq!(member.extension_receiver, None);
        let regular = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::regular")
            .unwrap();
        assert_eq!(regular.extension_receiver, None);
    }

    #[test]
    fn does_not_record_extension_receivers_for_complex_receiver_shapes() {
        let source = r#"
package com.example

fun Other?.nullableHelper(value: Int): Int = value
fun List<Int>.summed(): Int = 0
fun Outer.Inner.nested(): Int = 0
fun regular(value: Int): Int = value
"#;
        let path = Path::new("Caller.kt");
        let document = parse_document(path, source).unwrap();
        assert!(!document.tree.root_node().has_error());
        let symbols =
            index_kotlin_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();
        for symbol in symbols {
            assert_eq!(
                symbol.extension_receiver, None,
                "complex receiver on {} must fail closed",
                symbol.semantic_path
            );
        }
    }
}
