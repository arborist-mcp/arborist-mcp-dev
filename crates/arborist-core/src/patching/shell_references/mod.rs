use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::{
    ReferenceValidation, ambiguous_binding_decision, resolved_binding_decision,
    unresolved_binding_decision,
};
use crate::deadline::DeadlineCheck;
use crate::language::{ParsedDocument, normalize_path};
use crate::model::{
    DisambiguationContext, SymbolSummary, SymbolSummaryInit, ValidationAmbiguity, ValidationBinding,
};
use crate::semantic::shell::{
    is_shell_symbol_node, shell_parameters, shell_signature, shell_symbol_name,
};
use crate::symbol_extractor::shell::shell_command_name;

pub(crate) fn collect_shell_reference_validation_with_deadline(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    symbol_node: Node<'_>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<ReferenceValidation> {
    let normalized_path = normalize_path(path);
    let mut file_functions: BTreeMap<String, Vec<ShellFunctionItem<'_>>> = BTreeMap::new();
    collect_shell_file_functions(
        document.tree.root_node(),
        source,
        &mut file_functions,
        deadline,
    )?;
    let parameter_names: BTreeSet<String> =
        shell_parameters(symbol_node, source).into_iter().collect();
    let references = collect_shell_references(symbol_node, source, deadline)?;
    let mut validation = ReferenceValidation::default();
    for name in references {
        if parameter_names.contains(&name) {
            continue;
        }
        if let Some(deadline) = deadline {
            deadline.check("validating shell references")?;
        }
        match file_functions.get(name.as_str()) {
            None => {
                validation
                    .binding_decisions
                    .push(unresolved_binding_decision(&name));
                validation.unresolved_identifiers.push(name);
            }
            Some(candidates) if candidates.len() == 1 => {
                let summary =
                    shell_function_symbol_summary(&normalized_path, source, &candidates[0]);
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
                    .map(|item| shell_function_symbol_summary(&normalized_path, source, item))
                    .collect::<Vec<_>>();
                let reason =
                    "multiple shell declarations match the referenced function name".to_string();
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

struct ShellFunctionItem<'tree> {
    name: String,
    node: Node<'tree>,
}

fn collect_shell_file_functions<'tree>(
    node: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, Vec<ShellFunctionItem<'tree>>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("collecting shell file functions")?;
    }
    if is_shell_symbol_node(node)
        && let Some(name) = shell_symbol_name(node, source)?
    {
        items
            .entry(name.clone())
            .or_default()
            .push(ShellFunctionItem { name, node });
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_shell_file_functions(child, source, items, deadline)?;
    }
    Ok(())
}

fn collect_shell_references(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeSet<String>> {
    let mut references = BTreeSet::new();
    let Some(body) = symbol_node.child_by_field_name("body") else {
        return Ok(references);
    };
    collect_shell_references_from_node(body, source, deadline, &mut references)?;
    Ok(references)
}

fn collect_shell_references_from_node(
    node: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("validating shell references")?;
    }
    if node.kind() == "function_definition" {
        return Ok(());
    }
    if let Some(name) = shell_command_name(node, source)? {
        references.insert(name);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "function_definition" {
            continue;
        }
        collect_shell_references_from_node(child, source, deadline, references)?;
    }
    Ok(())
}

fn shell_function_symbol_summary(
    normalized_path: &str,
    source: &str,
    item: &ShellFunctionItem<'_>,
) -> SymbolSummary {
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: item.name.clone(),
        semantic_path: item.name.clone(),
        scope_path: None,
        file_path: normalized_path.to_string(),
        node_kind: "function_definition".to_string(),
        origin_type: "function_definition".to_string(),
        byte_range: (item.node.start_byte(), item.node.end_byte()),
        signature: shell_signature(item.node, source),
        parameters: shell_parameters(item.node, source),
        return_type: None,
        docstring: None,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::collect_shell_reference_validation_with_deadline;
    use crate::language::parse_document;
    use crate::semantic::find_semantic_node_with_deadline;

    #[test]
    fn resolves_and_rejects_shell_references() {
        let source = r#"compute() {
    value=1
}

caller() {
    compute
}
"#;
        let path = Path::new("sample.sh");
        let document = parse_document(path, source).unwrap();
        let caller = find_semantic_node_with_deadline(
            crate::LanguageId::Shell,
            path,
            &document.tree,
            source,
            "caller",
            None,
        )
        .unwrap()
        .expect("caller node should exist");
        let validation =
            collect_shell_reference_validation_with_deadline(path, &document, source, caller, None)
                .unwrap();
        assert!(validation.unresolved_identifiers.is_empty());
        assert_eq!(validation.resolved_identifiers.len(), 1);
        assert_eq!(validation.resolved_identifiers[0].name, "compute");
    }

    #[test]
    fn rejects_unresolved_shell_references() {
        let source = r#"caller() {
    missing_helper
}
"#;
        let path = Path::new("sample.sh");
        let document = parse_document(path, source).unwrap();
        let caller = find_semantic_node_with_deadline(
            crate::LanguageId::Shell,
            path,
            &document.tree,
            source,
            "caller",
            None,
        )
        .unwrap()
        .expect("caller node should exist");
        let validation =
            collect_shell_reference_validation_with_deadline(path, &document, source, caller, None)
                .unwrap();
        assert_eq!(
            validation.unresolved_identifiers,
            vec!["missing_helper".to_string()]
        );
        assert!(validation.resolved_identifiers.is_empty());
    }
}
