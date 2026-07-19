use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::{c_validation::collect_c_reference_validation, python_references};
use crate::language::{ParsedDocument, contains_node};
use crate::model::{
    LanguageId, SymbolSummary, ValidationAmbiguity, ValidationBinding, ValidationBindingDecision,
};

#[derive(Default)]
pub(crate) struct ReferenceValidation {
    pub(super) unresolved_identifiers: Vec<String>,
    pub(super) resolved_identifiers: Vec<ValidationBinding>,
    pub(super) ambiguous_identifiers: Vec<ValidationAmbiguity>,
    pub(super) binding_decisions: Vec<ValidationBindingDecision>,
}

pub(super) fn collect_reference_validation(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    symbol_node: Node<'_>,
) -> Result<ReferenceValidation> {
    match document.language_id {
        LanguageId::Python => {
            python_references::collect_python_reference_validation(path, source, symbol_node)
        }
        LanguageId::C | LanguageId::Cpp => {
            collect_c_reference_validation(path, document, source, symbol_node)
        }
    }
}

pub(crate) fn unresolved_binding_decision(name: &str) -> ValidationBindingDecision {
    ValidationBindingDecision {
        name: name.to_string(),
        status: "unresolved".to_string(),
        reason: "identifier is not visible from the patched symbol scope".to_string(),
        selected_symbol_id: None,
        candidates: Vec::new(),
    }
}

pub(crate) fn resolved_binding_decision(
    name: &str,
    symbol: &SymbolSummary,
) -> ValidationBindingDecision {
    ValidationBindingDecision {
        name: name.to_string(),
        status: "resolved".to_string(),
        reason: "exactly one visible binding candidate remained after scope and include filtering"
            .to_string(),
        selected_symbol_id: Some(symbol.symbol_id.clone()),
        candidates: vec![symbol.clone()],
    }
}

pub(crate) fn ambiguous_binding_decision(
    name: &str,
    reason: &str,
    candidates: &[SymbolSummary],
) -> ValidationBindingDecision {
    ValidationBindingDecision {
        name: name.to_string(),
        status: "ambiguous".to_string(),
        reason: reason.to_string(),
        selected_symbol_id: None,
        candidates: candidates.to_vec(),
    }
}

pub(crate) fn is_python_default_parameter_value(node: Node<'_>) -> bool {
    let mut current = node.parent();

    while let Some(candidate) = current {
        if candidate.kind() == "default_parameter" || candidate.kind() == "typed_default_parameter"
        {
            return candidate
                .child_by_field_name("value")
                .is_some_and(|value| contains_node(value, node));
        }

        if matches!(
            candidate.kind(),
            "function_definition" | "class_definition" | "module"
        ) {
            return false;
        }

        current = candidate.parent();
    }

    false
}

pub(crate) fn is_python_class_header_expression(node: Node<'_>) -> bool {
    let mut current = Some(node);

    while let Some(candidate) = current {
        if candidate.kind() == "block" {
            return false;
        }

        if candidate.kind() == "class_definition" {
            return true;
        }

        if matches!(candidate.kind(), "function_definition" | "module") {
            return false;
        }

        current = candidate.parent();
    }

    false
}
