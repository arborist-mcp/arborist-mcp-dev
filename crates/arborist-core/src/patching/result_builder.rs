use std::ops::Range;
use std::path::Path;

use anyhow::Result;

use crate::language::{normalize_path, parse_document};
use crate::model::{
    PatchAstNodeResult, PatchCommitGateReport, PatchValidationReport, ValidationIssue,
};

use super::{
    collect_syntax_errors, evaluate_patch_commit_gate, reference_validation, target_resolution,
};

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

    let patched_symbol = target_resolution::locate_patched_symbol(
        &virtual_document,
        &updated_source,
        patch_start,
        replacement_len,
    );

    if validation.syntax_errors.is_empty()
        && let Some(symbol_node) = patched_symbol
    {
        let reference_validation = reference_validation::collect_reference_validation(
            path,
            &virtual_document,
            &updated_source,
            symbol_node,
        )?;
        validation.unresolved_identifiers = reference_validation.unresolved_identifiers;
        validation.resolved_identifiers = reference_validation.resolved_identifiers;
        validation.ambiguous_identifiers = reference_validation.ambiguous_identifiers;
        validation.binding_decisions = reference_validation.binding_decisions;
    }

    validation.commit_gate = evaluate_patch_commit_gate(&validation, bypass_reason);
    let applied = validation.commit_gate.allowed;
    let bypass_applied = validation.commit_gate.status == "allowed_with_bypass";

    let resolved_path = patched_symbol
        .map(|node| {
            target_resolution::resolve_symbol_path(
                path,
                virtual_document.language_id,
                node,
                &updated_source,
            )
        })
        .transpose()?
        .unwrap_or_else(|| semantic_target.to_string());
    let resolved_symbol_id = patched_symbol
        .map(|node| {
            target_resolution::resolve_symbol_id(
                path,
                virtual_document.language_id,
                node,
                &updated_source,
            )
        })
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
