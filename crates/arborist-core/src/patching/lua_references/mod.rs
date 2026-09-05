use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::{
    ReferenceValidation, ambiguous_binding_decision, resolved_binding_decision,
    unresolved_binding_decision,
};
use crate::deadline::DeadlineCheck;
use crate::language::{ParsedDocument, node_text, normalize_path};
use crate::model::{
    DisambiguationContext, SymbolSummary, SymbolSummaryInit, ValidationAmbiguity, ValidationBinding,
};
use crate::semantic::lua::{
    is_lua_symbol_node, lua_parameters, lua_semantic_path, lua_signature, lua_symbol_name,
};

pub(crate) fn collect_lua_reference_validation_with_deadline(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    symbol_node: Node<'_>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<ReferenceValidation> {
    let normalized_path = normalize_path(path);
    let mut file_functions: BTreeMap<String, Vec<LuaFunctionItem<'_>>> = BTreeMap::new();
    collect_lua_file_functions(
        document.tree.root_node(),
        source,
        &mut file_functions,
        deadline,
    )?;
    let parameter_names: BTreeSet<String> =
        lua_parameters(symbol_node, source).into_iter().collect();
    let local_definitions = collect_lua_local_definitions(symbol_node, source, deadline)?;
    let references = collect_lua_references(symbol_node, source, deadline)?;
    let mut validation = ReferenceValidation::default();
    for name in references {
        if local_definitions.contains(&name) || parameter_names.contains(&name) {
            continue;
        }
        if let Some(deadline) = deadline {
            deadline.check("validating Lua references")?;
        }
        match file_functions.get(name.as_str()) {
            None => {
                validation
                    .binding_decisions
                    .push(unresolved_binding_decision(&name));
                validation.unresolved_identifiers.push(name);
            }
            Some(candidates) if candidates.len() == 1 => {
                let summary = lua_function_symbol_summary(&normalized_path, source, &candidates[0]);
                validation
                    .binding_decisions
                    .push(resolved_binding_decision(&name, &summary));
                validation.resolved_identifiers.push(ValidationBinding {
                    name,
                    symbol: summary,
                });
            }
            Some(candidates) => {
                let candidate_summaries = candidates
                    .iter()
                    .map(|item| lua_function_symbol_summary(&normalized_path, source, item))
                    .collect::<Vec<_>>();
                let reason =
                    "multiple Lua declarations match the referenced function name".to_string();
                validation
                    .binding_decisions
                    .push(ambiguous_binding_decision(
                        &name,
                        &reason,
                        &candidate_summaries,
                    ));
                validation.ambiguous_identifiers.push(ValidationAmbiguity {
                    name,
                    candidates: candidate_summaries,
                    reason,
                    disambiguation_context: DisambiguationContext::default(),
                });
            }
        }
    }
    Ok(validation)
}

struct LuaFunctionItem<'tree> {
    name: String,
    node: Node<'tree>,
}

fn collect_lua_file_functions<'tree>(
    node: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, Vec<LuaFunctionItem<'tree>>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("collecting Lua file functions")?;
    }
    if is_lua_symbol_node(node)
        && let Some(name) = lua_symbol_name(node, source)?
        && lua_semantic_path(name.as_str())?.is_some()
    {
        items
            .entry(name.clone())
            .or_default()
            .push(LuaFunctionItem { name, node });
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_lua_file_functions(child, source, items, deadline)?;
    }
    Ok(())
}

fn collect_lua_references(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeSet<String>> {
    let mut references = BTreeSet::new();
    let Some(body) = symbol_node.child_by_field_name("body") else {
        return Ok(references);
    };
    collect_lua_references_from_node(body, source, deadline, &mut references)?;
    Ok(references)
}

fn collect_lua_references_from_node(
    node: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("collecting Lua references")?;
    }
    // Nested function declarations own their calls; the enclosing symbol's
    // validation should only account for references reachable from its body.
    if node.kind() == "function_declaration" {
        return Ok(());
    }
    if node.kind() == "function_call"
        && let Some(name_node) = node.child_by_field_name("name")
        && name_node.kind() == "identifier"
    {
        let name = node_text(name_node, source)?.trim();
        if !matches!(name, "require" | "dofile") && !name.is_empty() {
            references.insert(name.to_string());
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_lua_references_from_node(child, source, deadline, references)?;
    }
    Ok(())
}

fn collect_lua_local_definitions(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    let Some(body) = symbol_node.child_by_field_name("body") else {
        return Ok(names);
    };
    collect_lua_local_definitions_from_node(body, source, &mut names, deadline)?;
    Ok(names)
}

fn collect_lua_local_definitions_from_node(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("collecting Lua local definitions")?;
    }
    if node.kind() == "function_declaration" {
        if let Some(name_node) = node.child_by_field_name("name") {
            insert_lua_identifier_name(name_node, source, names);
        }
        return Ok(());
    }
    if node.kind() == "for_numeric_clause"
        && let Some(name_node) = node.child_by_field_name("name")
    {
        insert_lua_identifier_name(name_node, source, names);
    }
    collect_lua_variable_list_names(node, source, names)?;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_lua_local_definitions_from_node(child, source, names, deadline)?;
    }
    Ok(())
}

fn collect_lua_variable_list_names(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    if node.kind() == "variable_list" {
        let mut cursor = node.walk();
        for name_node in node.children_by_field_name("name", &mut cursor) {
            insert_lua_identifier_name(name_node, source, names);
        }
        return Ok(());
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_lua_variable_list_names(child, source, names)?;
    }
    Ok(())
}

fn insert_lua_identifier_name(node: Node<'_>, source: &str, names: &mut BTreeSet<String>) {
    if let Ok(name) = node_text(node, source)
        && !name.trim().is_empty()
    {
        names.insert(name.trim().to_string());
    }
}

fn lua_function_symbol_summary(
    normalized_path: &str,
    source: &str,
    item: &LuaFunctionItem<'_>,
) -> SymbolSummary {
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: format!(
            "{normalized_path}::lua::<module>::function::{name}",
            name = item.name
        ),
        semantic_path: item.name.clone(),
        scope_path: None,
        file_path: normalized_path.to_string(),
        node_kind: "function_declaration".to_string(),
        origin_type: "function_declaration".to_string(),
        byte_range: (item.node.start_byte(), item.node.end_byte()),
        signature: lua_signature(item.node, source),
        parameters: lua_parameters(item.node, source),
        return_type: None,
        docstring: None,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::collect_lua_reference_validation_with_deadline;
    use crate::language::parse_document;
    use crate::semantic::find_semantic_node_with_deadline;

    #[test]
    fn resolves_and_rejects_lua_references() {
        let source = r#"local function compute(value)
    return value + 1
end

local function caller(value)
    return compute(value)
end
"#;
        let path = Path::new("sample.lua");
        let document = parse_document(path, source).unwrap();
        let caller = find_semantic_node_with_deadline(
            crate::LanguageId::Lua,
            path,
            &document.tree,
            source,
            "caller",
            None,
        )
        .unwrap()
        .expect("caller node should exist");
        let validation =
            collect_lua_reference_validation_with_deadline(path, &document, source, caller, None)
                .unwrap();
        assert!(validation.unresolved_identifiers.is_empty());
        assert_eq!(validation.resolved_identifiers.len(), 1);
        assert_eq!(validation.resolved_identifiers[0].name, "compute");
    }

    #[test]
    fn rejects_unresolved_lua_references() {
        let source = r#"local function compute(value)
    return value + 1
end

local function caller(value)
    return missing(value)
end
"#;
        let path = Path::new("sample.lua");
        let document = parse_document(path, source).unwrap();
        let caller = find_semantic_node_with_deadline(
            crate::LanguageId::Lua,
            path,
            &document.tree,
            source,
            "caller",
            None,
        )
        .unwrap()
        .expect("caller node should exist");
        let validation =
            collect_lua_reference_validation_with_deadline(path, &document, source, caller, None)
                .unwrap();
        assert_eq!(
            validation.unresolved_identifiers,
            vec!["missing".to_string()]
        );
        assert!(validation.resolved_identifiers.is_empty());
    }
}
