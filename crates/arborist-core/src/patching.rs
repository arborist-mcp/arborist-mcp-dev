mod api;
mod c_validation;
mod commit_gate;
mod python_bindings;
mod python_imports;
mod python_patterns;
mod python_references;
mod python_visibility;

pub(crate) use c_validation::{collect_c_reference_validation, collect_c_references};
pub(crate) use commit_gate::evaluate_patch_commit_gate;
pub(crate) use python_imports::{
    resolve_local_python_imported_symbol, resolve_local_python_module_path,
};
pub(crate) use python_references::collect_python_references;

pub use api::{
    patch_ast_node, patch_ast_node_at_position, patch_ast_node_at_position_from_path,
    patch_ast_node_from_path, preview_patch_ast_node, preview_patch_ast_node_at_position,
    preview_patch_ast_node_at_position_from_path, preview_patch_ast_node_from_path,
};

use std::ops::Range;
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use tree_sitter::Node;

use crate::language::{
    ParsedDocument, contains_node, normalize_absolute_path, normalize_path, offset_for_position,
    parse_document, position_from, visit_tree,
};
use crate::model::{
    LanguageId, PatchAstNodeResult, PatchCommitGateReport, PatchValidationReport, Position,
    SymbolSummary, ValidationAmbiguity, ValidationBinding, ValidationBindingDecision,
    ValidationIssue,
};
use crate::semantic::{
    ascend_to_symbol, c_semantic_path, c_symbol_id_for_node, find_semantic_node, semantic_path,
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

pub fn semantic_target_at_position(
    path: &Path,
    source: &str,
    position: &Position,
) -> Result<String> {
    let path = normalize_absolute_path(path)?;
    let document = parse_document(&path, source)?;
    let byte_offset = offset_for_position(source, position)?;
    let node =
        node_at_byte_offset(document.tree.root_node(), source, byte_offset).ok_or_else(|| {
            anyhow!(
                "position {}:{} does not resolve to a syntax node in {}",
                position.row,
                position.column,
                path.display()
            )
        })?;
    let symbol_node = ascend_to_symbol(document.language_id, node).ok_or_else(|| {
        anyhow!(
            "position {}:{} does not resolve to a semantic symbol in {}",
            position.row,
            position.column,
            path.display()
        )
    })?;

    match document.language_id {
        LanguageId::Python => semantic_path(symbol_node, source),
        LanguageId::C => c_symbol_id_for_node(&path, symbol_node, source)?
            .ok_or_else(|| anyhow!("position does not resolve to a C symbol id")),
    }
}

pub(crate) fn semantic_target_range(
    path: &Path,
    source: &str,
    semantic_target: &str,
) -> Result<(usize, usize)> {
    validate_semantic_target(semantic_target)?;
    let document = parse_document(path, source)?;
    let target_node = find_semantic_node(
        document.language_id,
        path,
        &document.tree,
        source,
        semantic_target,
    )?
    .ok_or_else(|| anyhow!("semantic path not found: {semantic_target}"))?;
    let target_node = python_symbol_replacement_node(document.language_id, target_node);

    Ok((target_node.start_byte(), target_node.end_byte()))
}

fn validate_semantic_target(semantic_target: &str) -> Result<()> {
    if semantic_target.trim().is_empty() {
        bail!("invalid semantic target: selector must not be blank");
    }
    Ok(())
}

fn node_at_byte_offset<'tree>(
    root: Node<'tree>,
    source: &str,
    byte_offset: usize,
) -> Option<Node<'tree>> {
    root.named_descendant_for_byte_range(byte_offset, byte_offset)
        .or_else(|| {
            byte_offset
                .checked_sub(1)
                .and_then(|offset| root.named_descendant_for_byte_range(offset, offset))
        })
        .or_else(|| {
            if byte_offset < source.len() {
                root.descendant_for_byte_range(byte_offset, byte_offset)
            } else {
                byte_offset
                    .checked_sub(1)
                    .and_then(|offset| root.descendant_for_byte_range(offset, offset))
            }
        })
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
) -> Result<PatchAstNodeResult> {
    let virtual_document = parse_document(path, &updated_source)?;
    let syntax_errors = collect_syntax_errors(virtual_document.tree.root_node(), &updated_source);

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

fn locate_patched_symbol<'tree>(
    document: &'tree ParsedDocument,
    _source: &str,
    patch_start: usize,
    replacement_len: usize,
) -> Option<Node<'tree>> {
    let patch_end = patch_start + replacement_len.saturating_sub(1);
    let root = document.tree.root_node();
    let descendant = root
        .named_descendant_for_byte_range(patch_start, patch_end)
        .or_else(|| root.named_descendant_for_byte_range(patch_start, patch_start))?;
    ascend_to_symbol(document.language_id, descendant)
}

fn python_symbol_replacement_node<'tree>(
    language_id: LanguageId,
    node: Node<'tree>,
) -> Node<'tree> {
    if language_id == LanguageId::Python
        && let Some(parent) = node.parent()
        && parent.kind() == "decorated_definition"
    {
        return parent;
    }

    node
}

fn resolve_symbol_path(
    path: &Path,
    language_id: LanguageId,
    node: Node<'_>,
    source: &str,
) -> Result<String> {
    match language_id {
        LanguageId::Python => semantic_path(node, source),
        LanguageId::C => c_semantic_path(path, node, source)?
            .ok_or_else(|| anyhow!("failed to resolve patched C symbol path")),
    }
}

fn resolve_symbol_id(
    path: &Path,
    language_id: LanguageId,
    node: Node<'_>,
    source: &str,
) -> Result<String> {
    match language_id {
        LanguageId::Python => semantic_path(node, source),
        LanguageId::C => c_symbol_id_for_node(path, node, source)?
            .ok_or_else(|| anyhow!("failed to resolve patched C symbol id")),
    }
}

pub(crate) fn collect_syntax_errors(root: Node<'_>, source: &str) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let mut callback = |node: Node<'_>| {
        if node.is_error() || node.is_missing() {
            let kind = if node.is_missing() {
                "missing"
            } else {
                "error"
            };
            issues.push(ValidationIssue {
                kind: kind.to_string(),
                message: format!("Tree-sitter reported a {kind} node near `{}`", node.kind()),
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                start_point: position_from(node.start_position()),
                end_point: position_from(node.end_position()),
            });
        } else if node.kind() == "ERROR" {
            issues.push(ValidationIssue {
                kind: "error".to_string(),
                message: format!(
                    "Tree-sitter produced an ERROR node near `{}`",
                    node.utf8_text(source.as_bytes()).unwrap_or(node.kind())
                ),
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                start_point: position_from(node.start_position()),
                end_point: position_from(node.end_position()),
            });
        }
    };

    visit_tree(root, &mut callback);
    issues
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
        LanguageId::C => collect_c_reference_validation(path, document, source, symbol_node),
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
