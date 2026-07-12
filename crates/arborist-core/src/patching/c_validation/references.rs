use std::collections::BTreeSet;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, visit_tree};

pub(super) fn collect_c_local_definitions(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if let Some(parent) = candidate.parent()
            && candidate.kind() == "identifier"
            && matches!(
                parent.kind(),
                "declaration"
                    | "init_declarator"
                    | "parameter_declaration"
                    | "function_declarator"
                    | "pointer_declarator"
                    | "array_declarator"
            )
        {
            let _ = node_text(candidate, source).map(|text| names.insert(text.trim().to_string()));
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}

pub(crate) fn collect_c_references(
    node: Node<'_>,
    source: &str,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() == "identifier" {
            let _ =
                node_text(candidate, source).map(|text| references.insert(text.trim().to_string()));
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}
