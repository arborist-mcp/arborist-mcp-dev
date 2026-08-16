use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{direct_require_specifier, node_text, normalize_path};
use crate::semantic::javascript::{
    is_javascript_symbol_node, javascript_parameters, javascript_return_type,
    javascript_semantic_path, javascript_signature, javascript_symbol_name,
};
use crate::symbol_index_model::{
    IndexedSymbol, JavaScriptReferenceDetails, ReferenceFact, ReferenceLanguageDetails,
    symbol_base_name,
};
use crate::symbol_reference_compat::reference_facts_from_legacy;
use crate::workspace_scan::WorkspaceScanDeadline;

type ReferenceNames = BTreeSet<String>;
type CallAritiesByName = BTreeMap<String, BTreeSet<usize>>;
/// Namespace-import member calls keyed by `(receiver, member)` with the arities
/// observed for that spelling, so `ns.helper(value)` records the member name
/// `helper` plus its namespace receiver `ns`.
type NamespaceMemberCalls = BTreeMap<(String, String), BTreeSet<usize>>;
/// Inline `require("./module").member(...)` member calls keyed by
/// `(specifier, member)` with the arities observed for that spelling.
type RequireMemberCalls = BTreeMap<(String, String), BTreeSet<usize>>;
/// Inline bare `require("./module")(...)` namespace-object calls keyed by
/// specifier with the arities observed for that spelling.
type RequireObjectCalls = BTreeMap<String, BTreeSet<usize>>;
type DirectCalls = (
    ReferenceNames,
    CallAritiesByName,
    NamespaceMemberCalls,
    RequireMemberCalls,
    RequireObjectCalls,
);

pub(crate) fn index_javascript_symbols_with_deadline(
    path: &Path,
    source: &str,
    root: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<IndexedSymbol>> {
    let mut symbols = Vec::new();
    collect_symbols(path, source, root, deadline, &mut symbols)?;
    Ok(symbols)
}

fn collect_symbols(
    path: &Path,
    source: &str,
    node: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
    symbols: &mut Vec<IndexedSymbol>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("extracting JavaScript/TypeScript symbols")?;
    }
    if is_javascript_symbol_node(node)
        && let Some(symbol) = indexed_symbol(path, source, node, deadline)?
    {
        symbols.push(symbol);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_symbols(path, source, child, deadline, symbols)?;
    }
    Ok(())
}

fn indexed_symbol(
    path: &Path,
    source: &str,
    node: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<IndexedSymbol>> {
    let Some(name) = javascript_symbol_name(node, source)? else {
        return Ok(None);
    };
    let semantic_path = javascript_semantic_path(node, source, &name)?;
    let scope_path = semantic_path
        .rsplit_once("::")
        .map(|(scope_path, _)| scope_path.to_string());
    let (
        references_by_name,
        call_arities_by_name,
        namespace_member_calls,
        require_member_calls,
        require_object_calls,
    ) = collect_direct_calls(node, source, deadline)?;
    let mut reference_facts =
        reference_facts_from_legacy(&references_by_name, &call_arities_by_name);
    reference_facts.extend(namespace_member_calls.into_iter().map(
        |((receiver, member), arities)| ReferenceFact {
            spelling: member,
            call_arities: Some(arities),
            language_details: ReferenceLanguageDetails::JavaScript(JavaScriptReferenceDetails {
                namespace_receiver: Some(receiver),
                require_member_call: None,
                require_object_call: None,
            }),
        },
    ));
    reference_facts.extend(require_member_calls.into_iter().map(
        |((specifier, member), arities)| ReferenceFact {
            spelling: member.clone(),
            call_arities: Some(arities),
            language_details: ReferenceLanguageDetails::JavaScript(JavaScriptReferenceDetails {
                namespace_receiver: None,
                require_member_call: Some((specifier, member)),
                require_object_call: None,
            }),
        },
    ));
    reference_facts.extend(
        require_object_calls
            .into_iter()
            .map(|(specifier, arities)| ReferenceFact {
                spelling: specifier.clone(),
                call_arities: Some(arities),
                language_details: ReferenceLanguageDetails::JavaScript(
                    JavaScriptReferenceDetails {
                        namespace_receiver: None,
                        require_member_call: None,
                        require_object_call: Some(specifier),
                    },
                ),
            }),
    );

    Ok(Some(IndexedSymbol {
        extension_receiver: None,
        symbol_id: semantic_path.clone(),
        base_name: symbol_base_name(&semantic_path),
        semantic_path,
        scope_path,
        file_path: normalize_path(path),
        node_kind: node.kind().to_string(),
        byte_range: (node.start_byte(), node.end_byte()),
        signature: javascript_signature(node, source),
        is_overload: false,
        parameters: javascript_parameters(node, source),
        return_type: javascript_return_type(node, source),
        docstring: None,
        reference_facts,
        references_by_name,
        call_arities_by_name,
    }))
}

fn collect_direct_calls(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<DirectCalls> {
    let mut references = BTreeSet::new();
    let mut call_arities_by_name = BTreeMap::new();
    let mut namespace_member_calls = BTreeMap::new();
    let mut require_member_calls = BTreeMap::new();
    let mut require_object_calls = BTreeMap::new();
    let root = symbol_node
        .child_by_field_name("body")
        .or_else(|| symbol_node.child_by_field_name("value"));
    if let Some(root) = root {
        collect_direct_calls_from_node(
            root,
            source,
            deadline,
            &mut references,
            &mut call_arities_by_name,
            &mut namespace_member_calls,
            &mut require_member_calls,
            &mut require_object_calls,
        )?;
    }
    Ok((
        references,
        call_arities_by_name,
        namespace_member_calls,
        require_member_calls,
        require_object_calls,
    ))
}

#[allow(clippy::too_many_arguments)]
fn collect_direct_calls_from_node(
    node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
    references: &mut ReferenceNames,
    call_arities_by_name: &mut CallAritiesByName,
    namespace_member_calls: &mut NamespaceMemberCalls,
    require_member_calls: &mut RequireMemberCalls,
    require_object_calls: &mut RequireObjectCalls,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("collecting JavaScript/TypeScript direct calls")?;
    }
    if node.kind() == "call_expression"
        && let Some(function) = node.child_by_field_name("function")
        && function.kind() == "identifier"
        && let Ok(name) = node_text(function, source)
    {
        let name = name.trim();
        if !name.is_empty() {
            references.insert(name.to_string());
            let arity = node
                .child_by_field_name("arguments")
                .map(|arguments| {
                    let mut cursor = arguments.walk();
                    arguments.named_children(&mut cursor).count()
                })
                .unwrap_or(0);
            call_arities_by_name
                .entry(name.to_string())
                .or_default()
                .insert(arity);
        }
    } else if node.kind() == "call_expression"
        && let Some(function) = node.child_by_field_name("function")
        && function.kind() == "member_expression"
        && let Some(object) = function.child_by_field_name("object")
        && object.kind() == "identifier"
        && let Some(property) = function.child_by_field_name("property")
        && property.kind() == "property_identifier"
        && let Ok(receiver) = node_text(object, source)
        && let Ok(member) = node_text(property, source)
    {
        let receiver = receiver.trim();
        let member = member.trim();
        if !receiver.is_empty() && !member.is_empty() {
            let arity = node
                .child_by_field_name("arguments")
                .map(|arguments| {
                    let mut cursor = arguments.walk();
                    arguments.named_children(&mut cursor).count()
                })
                .unwrap_or(0);
            namespace_member_calls
                .entry((receiver.to_string(), member.to_string()))
                .or_default()
                .insert(arity);
        }
    } else if node.kind() == "call_expression"
        && let Some(function) = node.child_by_field_name("function")
        && function.kind() == "member_expression"
        && let Some(object) = function.child_by_field_name("object")
        && let Some(specifier) = direct_require_specifier(object, source)?
        && let Some(property) = function.child_by_field_name("property")
        && property.kind() == "property_identifier"
        && let Ok(member) = node_text(property, source)
    {
        // Inline `require("./module").member(...)` resolves `member` within
        // the required module's namespace through the same machinery as
        // namespace member calls.
        let member = member.trim();
        if !member.is_empty() {
            let arity = node
                .child_by_field_name("arguments")
                .map(|arguments| {
                    let mut cursor = arguments.walk();
                    arguments.named_children(&mut cursor).count()
                })
                .unwrap_or(0);
            require_member_calls
                .entry((specifier, member.to_string()))
                .or_default()
                .insert(arity);
        }
    } else if node.kind() == "call_expression"
        && let Some(function) = node.child_by_field_name("function")
        && let Some(specifier) = direct_require_specifier(function, source)?
    {
        // Inline bare `require("./module")(...)` is a namespace-object call on
        // the required module's export object and resolves only CommonJS
        // callable exports.
        let arity = node
            .child_by_field_name("arguments")
            .map(|arguments| {
                let mut cursor = arguments.walk();
                arguments.named_children(&mut cursor).count()
            })
            .unwrap_or(0);
        require_object_calls
            .entry(specifier)
            .or_default()
            .insert(arity);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if is_javascript_symbol_node(child) {
            continue;
        }
        collect_direct_calls_from_node(
            child,
            source,
            deadline,
            references,
            call_arities_by_name,
            namespace_member_calls,
            require_member_calls,
            require_object_calls,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::index_javascript_symbols_with_deadline;
    use crate::language::parse_document;
    use crate::symbol_index_model::ReferenceLanguageDetails;

    #[test]
    fn extracts_javascript_and_typescript_callable_symbols_and_direct_calls() {
        for (path, source) in [
            (
                "sample.js",
                "export class Counter { increment(value) { return helper(value); } }\nexport const helper = (value) => value + 1;\n",
            ),
            (
                "sample.ts",
                "export interface Counter { increment(value: number): number; }\nexport function helper(value: number): number { return value + 1; }\n",
            ),
        ] {
            let document = parse_document(Path::new(path), source).unwrap();
            let symbols = index_javascript_symbols_with_deadline(
                Path::new(path),
                source,
                document.tree.root_node(),
                None,
            )
            .unwrap();
            assert!(
                symbols
                    .iter()
                    .any(|symbol| symbol.semantic_path == "Counter")
            );
            if path.ends_with(".js") {
                let increment = symbols
                    .iter()
                    .find(|symbol| symbol.semantic_path == "Counter::increment")
                    .unwrap();
                assert_eq!(increment.parameters, vec!["value"]);
                assert_eq!(
                    increment.call_arities_by_name.get("helper"),
                    Some(&BTreeSet::from([1]))
                );
            }
            let helper = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == "helper")
                .unwrap();

            if path.ends_with(".js") {
                assert_eq!(helper.parameters, vec!["value"]);
                assert!(
                    helper
                        .signature
                        .as_deref()
                        .is_some_and(|signature| signature.contains("=>"))
                );
            } else {
                assert_eq!(helper.return_type.as_deref(), Some("number"));
            }
        }
    }

    #[test]
    fn collects_namespace_member_call_facts() {
        let source = "import * as ns from \"./helper\";\nexport function caller(value) { return ns.helper(value) + ns.helper(value, 2); }\n";
        let path = Path::new("caller.ts");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_javascript_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "caller")
            .unwrap();
        assert!(caller.references_by_name.is_empty());
        assert_eq!(caller.reference_facts.len(), 1);
        let fact = &caller.reference_facts[0];
        assert_eq!(fact.spelling, "helper");
        assert_eq!(fact.call_arities, Some(BTreeSet::from([1, 2])));
        assert!(matches!(
            &fact.language_details,
            ReferenceLanguageDetails::JavaScript(details)
                if details.namespace_receiver.as_deref() == Some("ns")
        ));
    }
}
