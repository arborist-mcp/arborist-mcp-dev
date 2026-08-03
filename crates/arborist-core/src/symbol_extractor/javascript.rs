use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, normalize_path};
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::symbol_reference_compat::reference_facts_from_legacy;
use crate::workspace_scan::WorkspaceScanDeadline;

type ReferenceNames = BTreeSet<String>;
type CallAritiesByName = BTreeMap<String, BTreeSet<usize>>;
type DirectCalls = (ReferenceNames, CallAritiesByName);

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
    if is_symbol_node(node)
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
    let Some(name) = symbol_name(node, source)? else {
        return Ok(None);
    };
    let semantic_path = semantic_path(node, source, &name)?;
    let scope_path = semantic_path
        .rsplit_once("::")
        .map(|(scope_path, _)| scope_path.to_string());
    let (references_by_name, call_arities_by_name) = collect_direct_calls(node, source, deadline)?;
    let reference_facts = reference_facts_from_legacy(&references_by_name, &call_arities_by_name);

    Ok(Some(IndexedSymbol {
        symbol_id: semantic_path.clone(),
        base_name: symbol_base_name(&semantic_path),
        semantic_path,
        scope_path,
        file_path: normalize_path(path),
        node_kind: node.kind().to_string(),
        byte_range: (node.start_byte(), node.end_byte()),
        signature: display_signature(node, source),
        is_overload: false,
        parameters: parameters(node, source),
        return_type: return_type(node, source),
        docstring: None,
        reference_facts,
        references_by_name,
        call_arities_by_name,
    }))
}

fn is_symbol_node(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "abstract_class_declaration"
            | "class_declaration"
            | "enum_declaration"
            | "function_declaration"
            | "generator_function_declaration"
            | "interface_declaration"
            | "method_definition"
    ) || is_callable_variable_declarator(node)
}

fn is_callable_variable_declarator(node: Node<'_>) -> bool {
    node.kind() == "variable_declarator"
        && node
            .child_by_field_name("value")
            .is_some_and(|value| matches!(value.kind(), "arrow_function" | "function_expression"))
}

fn symbol_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let name = node.child_by_field_name("name").or_else(|| {
        (node.kind() == "variable_declarator")
            .then(|| node.child_by_field_name("name"))
            .flatten()
    });
    name.map(|name| node_text(name, source).map(str::trim).map(str::to_string))
        .transpose()
        .map(|name| name.filter(|name| !name.is_empty()))
}

fn semantic_path(node: Node<'_>, source: &str, name: &str) -> Result<String> {
    let mut ancestors = Vec::new();
    let mut current = node.parent();
    while let Some(parent) = current {
        if is_symbol_node(parent)
            && let Some(parent_name) = symbol_name(parent, source)?
        {
            ancestors.push(parent_name);
        }
        current = parent.parent();
    }
    ancestors.reverse();
    ancestors.push(name.to_string());
    Ok(ancestors.join("::"))
}

fn callable_value(node: Node<'_>) -> Node<'_> {
    node.child_by_field_name("value")
        .filter(|value| matches!(value.kind(), "arrow_function" | "function_expression"))
        .unwrap_or(node)
}

fn display_signature(node: Node<'_>, source: &str) -> Option<String> {
    let callable = callable_value(node);
    let end_byte = callable
        .child_by_field_name("body")
        .map_or(node.end_byte(), |body| body.start_byte());
    source
        .get(node.start_byte()..end_byte)
        .map(str::trim)
        .filter(|signature| !signature.is_empty())
        .map(str::to_string)
}

fn parameters(node: Node<'_>, source: &str) -> Vec<String> {
    let callable = callable_value(node);
    let parameters = callable
        .child_by_field_name("parameters")
        .or_else(|| callable.child_by_field_name("parameter"));
    let Some(parameters) = parameters else {
        return Vec::new();
    };
    if parameters.kind() == "identifier" {
        return node_text(parameters, source)
            .ok()
            .map(str::trim)
            .filter(|parameter| !parameter.is_empty())
            .map(|parameter| vec![parameter.to_string()])
            .unwrap_or_default();
    }

    let mut cursor = parameters.walk();
    parameters
        .named_children(&mut cursor)
        .filter_map(|parameter| node_text(parameter, source).ok().map(str::trim))
        .filter(|parameter| !parameter.is_empty())
        .map(str::to_string)
        .collect()
}

fn return_type(node: Node<'_>, source: &str) -> Option<String> {
    callable_value(node)
        .child_by_field_name("return_type")
        .and_then(|return_type| node_text(return_type, source).ok())
        .map(str::trim)
        .map(|return_type| return_type.trim_start_matches(':').trim())
        .filter(|return_type| !return_type.is_empty())
        .map(str::to_string)
}

fn collect_direct_calls(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<DirectCalls> {
    let mut references = BTreeSet::new();
    let mut call_arities_by_name = BTreeMap::new();
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
        )?;
    }
    Ok((references, call_arities_by_name))
}

fn collect_direct_calls_from_node(
    node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
    references: &mut ReferenceNames,
    call_arities_by_name: &mut CallAritiesByName,
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
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if is_symbol_node(child) {
            continue;
        }
        collect_direct_calls_from_node(child, source, deadline, references, call_arities_by_name)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use super::index_javascript_symbols_with_deadline;
    use crate::language::parse_document;

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
}
