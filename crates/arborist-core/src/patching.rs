mod api;
mod c_validation;
mod commit_gate;
mod python_bindings;
mod python_imports;
mod python_patterns;
mod python_references;
mod python_replacement;
mod python_visibility;
mod syntax_validation;
mod target_resolution;

pub(crate) use c_validation::{
    collect_c_call_arities, collect_c_graph_references, collect_c_reference_validation,
    collect_cpp_call_arities,
};
pub(crate) use commit_gate::evaluate_patch_commit_gate;
pub(crate) use python_imports::{
    resolve_local_python_imported_symbol, resolve_local_python_module_path,
};
pub(crate) use python_references::collect_python_references;
pub(crate) use syntax_validation::collect_syntax_errors;
use target_resolution::{locate_patched_symbol, resolve_symbol_id, resolve_symbol_path};
pub(crate) use target_resolution::{prepare_patch_replacement, semantic_target_at_position};

pub(crate) use api::unified_diff;
pub use api::{
    patch_ast_node, patch_ast_node_at_position, patch_ast_node_at_position_from_path,
    patch_ast_node_from_path, preview_patch_ast_node, preview_patch_ast_node_at_position,
    preview_patch_ast_node_at_position_from_path, preview_patch_ast_node_from_path,
};

use std::ops::Range;
use std::path::Path;

use anyhow::{Result, bail};
use tree_sitter::Node;

use crate::language::{ParsedDocument, contains_node, normalize_path, parse_document};
use crate::model::{
    LanguageId, PatchAstNodeResult, PatchCommitGateReport, PatchValidationReport, SymbolSummary,
    ValidationAmbiguity, ValidationBinding, ValidationBindingDecision, ValidationIssue,
};
#[derive(Default)]
pub(crate) struct ReferenceValidation {
    unresolved_identifiers: Vec<String>,
    resolved_identifiers: Vec<ValidationBinding>,
    ambiguous_identifiers: Vec<ValidationAmbiguity>,
    binding_decisions: Vec<ValidationBindingDecision>,
}

#[derive(Debug, Clone)]
enum PythonImportBinding {
    Module {
        module_name: String,
    },
    Symbol {
        module_name: Option<String>,
        symbol_name: String,
    },
}

pub(crate) fn validate_bypass_reason(bypass_reason: Option<&str>) -> Result<()> {
    if bypass_reason.is_some_and(|reason| reason.trim().is_empty()) {
        bail!("invalid bypass_reason: reason must not be blank");
    }
    Ok(())
}

pub(crate) fn validate_patch_replacement(new_code: &str) -> Result<()> {
    if new_code.trim().is_empty() {
        bail!("invalid new_code: replacement must not be blank");
    }
    Ok(())
}

pub(crate) fn build_patch_result(
    path: &Path,
    semantic_target: &str,
    updated_source: String,
    bypass_reason: Option<&str>,
    patch_start: usize,
    replacement_len: usize,
    mut preflight_issues: Vec<ValidationIssue>,
) -> Result<PatchAstNodeResult> {
    let virtual_document = parse_document(path, &updated_source)?;
    let mut syntax_errors =
        collect_syntax_errors(virtual_document.tree.root_node(), &updated_source);
    syntax_errors.append(&mut preflight_issues);

    let mut validation = PatchValidationReport {
        syntax_errors,
        unresolved_identifiers: Vec::new(),
        resolved_identifiers: Vec::new(),
        ambiguous_identifiers: Vec::new(),
        binding_decisions: Vec::new(),
        commit_gate: PatchCommitGateReport::default(),
    };

    let patched_symbol = locate_patched_symbol(
        &virtual_document,
        &updated_source,
        patch_start,
        replacement_len,
    );

    if validation.syntax_errors.is_empty()
        && let Some(symbol_node) = patched_symbol
    {
        let reference_validation =
            collect_reference_validation(path, &virtual_document, &updated_source, symbol_node)?;
        validation.unresolved_identifiers = reference_validation.unresolved_identifiers;
        validation.resolved_identifiers = reference_validation.resolved_identifiers;
        validation.ambiguous_identifiers = reference_validation.ambiguous_identifiers;
        validation.binding_decisions = reference_validation.binding_decisions;
    }

    validation.commit_gate = evaluate_patch_commit_gate(&validation, bypass_reason);
    let applied = validation.commit_gate.allowed;
    let bypass_applied = validation.commit_gate.status == "allowed_with_bypass";

    let resolved_path = patched_symbol
        .map(|node| resolve_symbol_path(path, virtual_document.language_id, node, &updated_source))
        .transpose()?
        .unwrap_or_else(|| semantic_target.to_string());
    let resolved_symbol_id = patched_symbol
        .map(|node| resolve_symbol_id(path, virtual_document.language_id, node, &updated_source))
        .transpose()?
        .unwrap_or_else(|| resolved_path.clone());

    let result = PatchAstNodeResult {
        file: normalize_path(path),
        target_path: semantic_target.to_string(),
        resolved_path,
        resolved_symbol_id,
        applied,
        bypass_applied,
        updated_source,
        validation,
    };
    result.validate_public_output()?;
    Ok(result)
}

pub(crate) fn splice_source(source: &str, range: Range<usize>, replacement: &str) -> String {
    let mut updated =
        String::with_capacity(source.len() - (range.end - range.start) + replacement.len());
    updated.push_str(&source[..range.start]);
    updated.push_str(replacement);
    updated.push_str(&source[range.end..]);
    updated
}

fn collect_reference_validation(
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

fn unresolved_binding_decision(name: &str) -> ValidationBindingDecision {
    ValidationBindingDecision {
        name: name.to_string(),
        status: "unresolved".to_string(),
        reason: "identifier is not visible from the patched symbol scope".to_string(),
        selected_symbol_id: None,
        candidates: Vec::new(),
    }
}

fn resolved_binding_decision(name: &str, symbol: &SymbolSummary) -> ValidationBindingDecision {
    ValidationBindingDecision {
        name: name.to_string(),
        status: "resolved".to_string(),
        reason: "exactly one visible binding candidate remained after scope and include filtering"
            .to_string(),
        selected_symbol_id: Some(symbol.symbol_id.clone()),
        candidates: vec![symbol.clone()],
    }
}

fn ambiguous_binding_decision(
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

pub(super) fn is_python_default_parameter_value(node: Node<'_>) -> bool {
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

pub(super) fn is_python_class_header_expression(node: Node<'_>) -> bool {
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
