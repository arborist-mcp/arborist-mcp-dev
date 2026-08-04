use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, normalize_path};
use crate::semantic::go::{
    go_parameters, go_return_type, go_semantic_path, go_signature, go_symbol_name,
    is_go_symbol_node,
};
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::symbol_reference_compat::reference_facts_from_legacy;
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) fn index_go_symbols_with_deadline(
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
        deadline.check("extracting Go symbols")?;
    }
    if is_go_symbol_node(node)
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
    let Some(name) = go_symbol_name(node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = go_semantic_path(node, source, &name)? else {
        return Ok(None);
    };
    let scope_path = semantic_path
        .rsplit_once("::")
        .map(|(scope_path, _)| scope_path.to_string());
    let references_by_name = collect_direct_local_calls(node, source, deadline)?;
    let call_arities_by_name = BTreeMap::new();

    Ok(Some(IndexedSymbol {
        symbol_id: semantic_path.clone(),
        base_name: symbol_base_name(&semantic_path),
        semantic_path,
        scope_path,
        file_path: normalize_path(path),
        node_kind: node.kind().to_string(),
        byte_range: (node.start_byte(), node.end_byte()),
        signature: go_signature(node, source),
        is_overload: false,
        parameters: go_parameters(node, source),
        return_type: go_return_type(node, source),
        docstring: None,
        reference_facts: reference_facts_from_legacy(&references_by_name, &call_arities_by_name),
        references_by_name,
        call_arities_by_name,
    }))
}

fn collect_direct_local_calls(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<BTreeSet<String>> {
    if !matches!(
        symbol_node.kind(),
        "function_declaration" | "method_declaration"
    ) {
        return Ok(BTreeSet::new());
    }
    let Some(body) = symbol_node.child_by_field_name("body") else {
        return Ok(BTreeSet::new());
    };
    let local_functions = source_file_function_paths(symbol_node, source)?;

    let mut bindings = BTreeSet::new();
    collect_function_bindings(symbol_node, source, &mut bindings)?;
    let mut references = BTreeSet::new();
    collect_direct_local_calls_from_node(
        body,
        source,
        deadline,
        &local_functions,
        &bindings,
        &mut references,
    )?;
    Ok(references)
}

fn source_file_function_paths(
    symbol_node: Node<'_>,
    source: &str,
) -> Result<BTreeMap<String, String>> {
    let mut root = symbol_node;
    while let Some(parent) = root.parent() {
        root = parent;
    }

    let mut paths_by_name = BTreeMap::<String, Vec<String>>::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() != "function_declaration" {
            continue;
        }
        let Some(name) = go_symbol_name(child, source)? else {
            continue;
        };
        let Some(path) = go_semantic_path(child, source, &name)? else {
            continue;
        };
        paths_by_name.entry(name).or_default().push(path);
    }

    Ok(paths_by_name
        .into_iter()
        .filter_map(|(name, paths)| (paths.len() == 1).then(|| (name, paths[0].clone())))
        .collect())
}

fn collect_function_bindings(
    symbol_node: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    for field_name in ["receiver", "parameters"] {
        if let Some(parameters) = symbol_node.child_by_field_name(field_name) {
            collect_parameter_bindings(parameters, source, bindings)?;
        }
    }
    if let Some(body) = symbol_node.child_by_field_name("body") {
        collect_body_bindings(body, source, bindings)?;
    }
    Ok(())
}

fn collect_parameter_bindings(
    parameters: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    let mut cursor = parameters.walk();
    for parameter in parameters.named_children(&mut cursor) {
        collect_field_name_bindings(parameter, source, bindings)?;
    }
    Ok(())
}

fn collect_body_bindings(
    node: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    if node.kind() == "function_literal" {
        return Ok(());
    }
    match node.kind() {
        "var_spec" | "const_spec" | "parameter_declaration" | "variadic_parameter_declaration" => {
            collect_field_name_bindings(node, source, bindings)?
        }
        "short_var_declaration" | "range_clause" => {
            if let Some(left) = node.child_by_field_name("left") {
                collect_expression_list_identifier_bindings(left, source, bindings)?;
            }
        }
        "type_switch_statement" => {
            if let Some(alias) = node.child_by_field_name("alias") {
                collect_expression_list_identifier_bindings(alias, source, bindings)?;
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_body_bindings(child, source, bindings)?;
    }
    Ok(())
}

fn collect_field_name_bindings(
    node: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    let mut cursor = node.walk();
    for name in node.children_by_field_name("name", &mut cursor) {
        collect_identifier_binding(name, source, bindings)?;
    }
    Ok(())
}

fn collect_expression_list_identifier_bindings(
    node: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "identifier" {
            collect_identifier_binding(child, source, bindings)?;
        }
    }
    Ok(())
}

fn collect_identifier_binding(
    node: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    let name = node_text(node, source)?.trim();
    if !name.is_empty() && name != "_" {
        bindings.insert(name.to_string());
    }
    Ok(())
}

fn go_imported_selector_reference(
    selector: Node<'_>,
    source: &str,
    bindings: &BTreeSet<String>,
) -> Result<Option<String>> {
    let Some(operand) = selector.child_by_field_name("operand") else {
        return Ok(None);
    };
    let Some(field) = selector.child_by_field_name("field") else {
        return Ok(None);
    };
    if operand.kind() != "identifier" || field.kind() != "field_identifier" {
        return Ok(None);
    }

    let local_name = node_text(operand, source)?.trim();
    let imported_name = node_text(field, source)?.trim();
    if local_name.is_empty()
        || imported_name.is_empty()
        || bindings.contains(local_name)
        || !imported_name.chars().next().is_some_and(char::is_uppercase)
    {
        return Ok(None);
    }

    Ok(Some(format!("{local_name}.{imported_name}")))
}

fn collect_direct_local_calls_from_node(
    node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
    local_functions: &BTreeMap<String, String>,
    bindings: &BTreeSet<String>,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("collecting Go direct calls")?;
    }
    if node.kind() == "function_literal" {
        return Ok(());
    }
    if node.kind() == "call_expression"
        && let Some(function) = node.child_by_field_name("function")
    {
        match function.kind() {
            "identifier" => {
                let name = node_text(function, source)?.trim();
                if !name.is_empty()
                    && !bindings.contains(name)
                    && let Some(path) = local_functions.get(name)
                {
                    references.insert(path.clone());
                }
            }
            "selector_expression" => {
                if let Some(reference) = go_imported_selector_reference(function, source, bindings)?
                {
                    references.insert(reference);
                }
            }
            _ => {}
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_direct_local_calls_from_node(
            child,
            source,
            deadline,
            local_functions,
            bindings,
            references,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::index_go_symbols_with_deadline;
    use crate::language::parse_document;

    #[test]
    fn indexes_go_named_types_functions_methods_and_unshadowed_direct_calls() {
        let source = r#"
package metrics

type Counter struct { value int }
type Alias = Counter

func helper() int { return 1 }
func direct() int { return helper() }
func shadowed_parameter(helper func() int) int { return helper() }
func shadowed_variable() int {
    helper := func() int { return 2 }
    return helper()
}
func NewCounter(value int) Counter { return Counter{value: value} }
func imported() int { return service.Value() }
func shadowed_selector() int {
    service := Counter{}
    return service.Value()
}
func (counter *Counter) Increment(amount int) int { return helper() + amount }
"#;
        let path = Path::new("metrics.go");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_go_symbols_with_deadline(path, source, document.tree.root_node(), None).unwrap();

        assert_eq!(
            symbols
                .iter()
                .map(|symbol| symbol.semantic_path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Counter",
                "Alias",
                "helper",
                "direct",
                "shadowed_parameter",
                "shadowed_variable",
                "NewCounter",
                "imported",
                "shadowed_selector",
                "Counter::Increment",
            ]
        );
        let increment = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "Counter::Increment")
            .unwrap();
        assert_eq!(increment.parameters, vec!["amount int"]);
        assert_eq!(increment.return_type.as_deref(), Some("int"));
        assert_eq!(increment.references_by_name, ["helper".to_string()].into());

        for caller_path in ["direct", "Counter::Increment"] {
            let caller = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == caller_path)
                .unwrap();
            assert_eq!(caller.references_by_name, ["helper".to_string()].into());
            assert_eq!(caller.reference_facts.len(), 1);
        }
        let imported = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "imported")
            .unwrap();
        assert_eq!(
            imported.references_by_name,
            ["service.Value".to_string()].into()
        );
        assert_eq!(imported.reference_facts.len(), 1);
        for caller_path in [
            "shadowed_parameter",
            "shadowed_variable",
            "shadowed_selector",
        ] {
            let caller = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == caller_path)
                .unwrap();
            assert!(caller.references_by_name.is_empty(), "{caller_path}");
            assert!(caller.reference_facts.is_empty(), "{caller_path}");
        }
    }
}
