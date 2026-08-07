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
        if receiver.kind() == "this" {
            return csharp_invocation_member_name(member, source)
                .map(|name| name.map(|name| format!("this.{name}")));
        }
        if receiver.kind() == "base" {
            return csharp_invocation_member_name(member, source)
                .map(|name| name.map(|name| format!("base.{name}")));
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
            return csharp_qualified_type_static_invocation_name(receiver, member, source);
        }
        return csharp_global_qualified_static_invocation_name(receiver, member, source);
    }

    let name = csharp_invocation_member_name(node, source)?;
    Ok(name.filter(|name| !csharp_reference_is_shadowed(name, bindings)))
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
                        // `var helper = new Helper()`; other initializers bind
                        // an empty type and fail closed.
                        None => csharp_constructor_type_from_declarator(node, source)?,
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

/// Infers a receiver type for `var` locals whose initializer is a constructor
/// call such as `var helper = new Helper()` or `var helper = new Outer.Inner()`.
/// Non-constructor initializers, target-typed creations, array creations, and
/// malformed type spellings return `None` and fail closed.
fn csharp_constructor_type_from_declarator(
    declarator: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    let Some(initializer) = csharp_declarator_initializer(declarator) else {
        return Ok(None);
    };
    if initializer.kind() != "object_creation_expression" {
        return Ok(None);
    }
    let Some(type_node) = initializer.child_by_field_name("type") else {
        return Ok(None);
    };
    let type_name = crate::language::node_text(type_node, source)?.trim();
    if type_name.is_empty() || type_name == "var" {
        return Ok(None);
    }
    Ok(Some(type_name.to_string()))
}

/// Returns the initializer expression of a `variable_declarator` such as
/// `helper = new Helper()`. The grammar does not name the `= expression` child,
/// so the last named child that is not the declared name or a tuple/indexer
/// suffix is the initializer.
fn csharp_declarator_initializer<'a>(declarator: Node<'a>) -> Option<Node<'a>> {
    let mut cursor = declarator.walk();
    let mut initializer = None;
    for child in declarator.named_children(&mut cursor) {
        if !matches!(
            child.kind(),
            "identifier" | "tuple_pattern" | "bracketed_argument_list"
        ) {
            initializer = Some(child);
        }
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
}
