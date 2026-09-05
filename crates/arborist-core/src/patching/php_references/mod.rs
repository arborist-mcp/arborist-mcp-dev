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
use crate::semantic::php::{php_parameters, php_return_type, php_signature, php_symbol_name};

pub(crate) fn collect_php_reference_validation_with_deadline(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    symbol_node: Node<'_>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<ReferenceValidation> {
    let normalized_path = normalize_path(path);
    let mut file_functions: BTreeMap<String, Vec<PhpFunctionItem<'_>>> = BTreeMap::new();
    collect_php_file_functions(
        document.tree.root_node(),
        source,
        &mut file_functions,
        deadline,
    )?;
    let parameter_names: BTreeSet<String> =
        php_parameters(symbol_node, source).into_iter().collect();
    let references = collect_php_references(symbol_node, source, deadline)?;
    let mut validation = ReferenceValidation::default();
    for name in references {
        if parameter_names.contains(&name) {
            continue;
        }
        if let Some(deadline) = deadline {
            deadline.check("validating PHP references")?;
        }
        match file_functions.get(name.as_str()) {
            None => {
                validation
                    .binding_decisions
                    .push(unresolved_binding_decision(&name));
                validation.unresolved_identifiers.push(name);
            }
            Some(candidates) if candidates.len() == 1 => {
                let summary = php_function_symbol_summary(&normalized_path, source, &candidates[0]);
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
                    .map(|item| php_function_symbol_summary(&normalized_path, source, item))
                    .collect::<Vec<_>>();
                let reason =
                    "multiple PHP declarations match the referenced function name".to_string();
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

struct PhpFunctionItem<'tree> {
    name: String,
    node: Node<'tree>,
}

fn collect_php_file_functions<'tree>(
    node: Node<'tree>,
    source: &str,
    items: &mut BTreeMap<String, Vec<PhpFunctionItem<'tree>>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("collecting PHP file functions")?;
    }
    if node.kind() == "function_definition"
        && let Some(name) = php_symbol_name(node, source)?
    {
        items
            .entry(name.clone())
            .or_default()
            .push(PhpFunctionItem { name, node });
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_php_file_functions(child, source, items, deadline)?;
    }
    Ok(())
}

fn collect_php_references(
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeSet<String>> {
    let mut references = BTreeSet::new();
    let Some(body) = symbol_node.child_by_field_name("body") else {
        return Ok(references);
    };
    collect_php_references_from_node(body, source, deadline, &mut references)?;
    Ok(references)
}

fn collect_php_references_from_node(
    node: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("validating PHP references")?;
    }
    if node.kind() == "function_definition" || node.kind() == "method_declaration" {
        return Ok(());
    }
    if node.kind() == "function_call_expression"
        && let Some(function_node) = node.child_by_field_name("function")
        && function_node.kind() == "name"
    {
        let name = node_text(function_node, source)?.trim();
        if !name.is_empty() {
            references.insert(name.to_string());
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "function_definition" || child.kind() == "method_declaration" {
            continue;
        }
        collect_php_references_from_node(child, source, deadline, references)?;
    }
    Ok(())
}

fn php_function_symbol_summary(
    normalized_path: &str,
    source: &str,
    item: &PhpFunctionItem<'_>,
) -> SymbolSummary {
    SymbolSummary::new(SymbolSummaryInit {
        symbol_id: item.name.clone(),
        semantic_path: item.name.clone(),
        scope_path: None,
        file_path: normalized_path.to_string(),
        node_kind: "function_definition".to_string(),
        origin_type: "function_definition".to_string(),
        byte_range: (item.node.start_byte(), item.node.end_byte()),
        signature: php_signature(item.node, source),
        parameters: php_parameters(item.node, source),
        return_type: php_return_type(item.node, source).ok().flatten(),
        docstring: None,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::collect_php_reference_validation_with_deadline;
    use crate::language::parse_document;
    use crate::semantic::find_semantic_node_with_deadline;

    #[test]
    fn resolves_and_rejects_php_references() {
        let source = r#"<?php
function compute(int $value): int {
    return $value + 1;
}

function caller(int $value): int {
    return compute($value);
}
"#;
        let path = Path::new("sample.php");
        let document = parse_document(path, source).unwrap();
        let caller = find_semantic_node_with_deadline(
            crate::LanguageId::Php,
            path,
            &document.tree,
            source,
            "caller",
            None,
        )
        .unwrap()
        .expect("caller node should exist");
        let validation =
            collect_php_reference_validation_with_deadline(path, &document, source, caller, None)
                .unwrap();
        assert!(validation.unresolved_identifiers.is_empty());
        assert_eq!(validation.resolved_identifiers.len(), 1);
        assert_eq!(validation.resolved_identifiers[0].name, "compute");
    }

    #[test]
    fn rejects_unresolved_php_references() {
        let source = r#"<?php
function caller(int $value): int {
    return missing($value);
}
"#;
        let path = Path::new("sample.php");
        let document = parse_document(path, source).unwrap();
        let caller = find_semantic_node_with_deadline(
            crate::LanguageId::Php,
            path,
            &document.tree,
            source,
            "caller",
            None,
        )
        .unwrap()
        .expect("caller node should exist");
        let validation =
            collect_php_reference_validation_with_deadline(path, &document, source, caller, None)
                .unwrap();
        assert_eq!(
            validation.unresolved_identifiers,
            vec!["missing".to_string()]
        );
        assert!(validation.resolved_identifiers.is_empty());
    }
}
