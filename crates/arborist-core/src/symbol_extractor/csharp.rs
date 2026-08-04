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
    let Some(body) = symbol_node.child_by_field_name("body") else {
        return Ok((BTreeSet::new(), BTreeMap::new()));
    };

    let bindings = collect_local_bindings(symbol_node, source)?;
    let mut references = BTreeSet::new();
    let mut call_arities_by_name = BTreeMap::new();
    collect_direct_same_type_calls_from_node(
        body,
        source,
        &bindings,
        &mut references,
        &mut call_arities_by_name,
    )?;
    Ok((references, call_arities_by_name))
}

fn collect_direct_same_type_calls_from_node(
    node: Node<'_>,
    source: &str,
    bindings: &BTreeSet<String>,
    references: &mut ReferenceNames,
    call_arities_by_name: &mut CallAritiesByName,
) -> Result<()> {
    if is_csharp_symbol_node(node) {
        return Ok(());
    }
    if node.kind() == "invocation_expression"
        && let Some(function) = node.child_by_field_name("function")
        && let Some(arguments) = node.child_by_field_name("arguments")
        && let Some(name) = csharp_direct_invocation_name(function, source)?
        && !bindings.contains(&name)
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

fn csharp_direct_invocation_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    if node.kind() == "member_access_expression" {
        let Some(receiver) = node.child_by_field_name("expression") else {
            return Ok(None);
        };
        if receiver.kind() != "this" {
            return Ok(None);
        }
        let Some(member) = node.child_by_field_name("name") else {
            return Ok(None);
        };
        return csharp_invocation_member_name(member, source)
            .map(|name| name.map(|name| format!("this.{name}")));
    }

    csharp_invocation_member_name(node, source)
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

fn collect_local_bindings(symbol_node: Node<'_>, source: &str) -> Result<BTreeSet<String>> {
    fn insert_name(node: Node<'_>, source: &str, bindings: &mut BTreeSet<String>) -> Result<()> {
        let name = crate::language::node_text(node, source)?.trim();
        if !name.is_empty() {
            bindings.insert(name.to_string());
        }
        Ok(())
    }

    fn collect(node: Node<'_>, source: &str, bindings: &mut BTreeSet<String>) -> Result<()> {
        if is_csharp_symbol_node(node) && node.parent().is_some() {
            return Ok(());
        }
        if node.kind() == "implicit_parameter" {
            insert_name(node, source, bindings)?;
        }
        if matches!(
            node.kind(),
            "parameter"
                | "variable_declarator"
                | "catch_declaration"
                | "declaration_expression"
                | "declaration_pattern"
                | "local_function_statement"
        ) && let Some(name) = node.child_by_field_name("name")
        {
            insert_name(name, source, bindings)?;
        }
        if node.kind() == "foreach_statement"
            && let Some(left) = node.child_by_field_name("left")
            && left.kind() == "identifier"
        {
            insert_name(left, source, bindings)?;
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            collect(child, source, bindings)?;
        }
        Ok(())
    }

    let mut bindings = BTreeSet::new();
    let mut cursor = symbol_node.walk();
    for child in symbol_node.named_children(&mut cursor) {
        collect(child, source, &mut bindings)?;
    }
    Ok(bindings)
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
    fn collects_only_unshadowed_unqualified_csharp_method_calls() {
        let source = r#"
using System;

class Counter {
    int Helper() => 1;
    T Generic<T>() => default;
    int Caller() => Helper();
    int GenericCaller() => Generic<int>();
    int ExplicitThis() => this.Helper();
    int ExplicitThisParameterShadow(Func<int> Helper) => this.Helper();
    int Other(Counter counter) => counter.Helper();
    int ParameterShadow(Func<int> Helper) => Helper();
    int LocalFunctionShadow() { int Helper() => 2; return Helper(); }
    int LambdaShadow() => ((Func<int, int>)(Helper => Helper())).Invoke(1);
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
        assert!(references("Counter::Other").is_empty());
        assert!(references("Counter::ParameterShadow").is_empty());
        assert!(references("Counter::LocalFunctionShadow").is_empty());
        assert!(references("Counter::LambdaShadow").is_empty());
    }
}
