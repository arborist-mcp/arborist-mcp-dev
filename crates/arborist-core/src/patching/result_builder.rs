use std::ops::Range;
use std::path::Path;

use anyhow::Result;

use crate::deadline::DeadlineCheck;
use crate::language::{normalize_path, parse_document};
use crate::model::{
    LanguageId, PatchAstNodeResult, PatchCommitGateReport, PatchValidationReport, ValidationIssue,
};

use super::{
    collect_syntax_errors_with_deadline, evaluate_patch_commit_gate, reference_validation,
    target_resolution,
};

pub(crate) struct PatchBuildInput<'a> {
    pub(crate) path: &'a Path,
    pub(crate) semantic_target: &'a str,
    pub(crate) updated_source: String,
    pub(crate) bypass_reason: Option<&'a str>,
    pub(crate) patch_start: usize,
    pub(crate) replacement_len: usize,
    pub(crate) preflight_issues: Vec<ValidationIssue>,
}

pub(crate) fn build_patch_result(
    path: &Path,
    semantic_target: &str,
    updated_source: String,
    bypass_reason: Option<&str>,
    patch_start: usize,
    replacement_len: usize,
    preflight_issues: Vec<ValidationIssue>,
) -> Result<PatchAstNodeResult> {
    build_patch_result_with_deadline(
        PatchBuildInput {
            path,
            semantic_target,
            updated_source,
            bypass_reason,
            patch_start,
            replacement_len,
            preflight_issues,
        },
        None,
    )
}

pub(crate) fn build_patch_result_with_deadline(
    input: PatchBuildInput<'_>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<PatchAstNodeResult> {
    let PatchBuildInput {
        path,
        semantic_target,
        updated_source,
        bypass_reason,
        patch_start,
        replacement_len,
        mut preflight_issues,
    } = input;
    check_deadline(deadline, "updated source parse")?;
    let virtual_document = parse_document(path, &updated_source)?;
    check_deadline(deadline, "syntax validation")?;
    let mut syntax_errors = collect_syntax_errors_with_deadline(
        virtual_document.tree.root_node(),
        &updated_source,
        deadline,
    )?;
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
        check_deadline(deadline, "reference validation")?;
        let reference_validation = match deadline {
            Some(deadline) => reference_validation::collect_reference_validation_with_deadline(
                path,
                &virtual_document,
                &updated_source,
                symbol_node,
                Some(deadline),
            )?,
            None => reference_validation::collect_reference_validation(
                path,
                &virtual_document,
                &updated_source,
                symbol_node,
            )?,
        };
        validation.unresolved_identifiers = reference_validation.unresolved_identifiers;
        validation.resolved_identifiers = reference_validation.resolved_identifiers;
        validation.ambiguous_identifiers = reference_validation.ambiguous_identifiers;
        validation.binding_decisions = reference_validation.binding_decisions;
    }

    check_deadline(deadline, "patch commit gate")?;
    validation.commit_gate = evaluate_patch_commit_gate(&validation, bypass_reason);
    let applied = validation.commit_gate.allowed;
    let bypass_applied = validation.commit_gate.status == "allowed_with_bypass";

    check_deadline(deadline, "patched symbol resolution")?;
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
    let mut resolved_symbol_id = patched_symbol
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
    if virtual_document.language_id == LanguageId::Python
        && resolved_symbol_id == resolved_path
        && semantic_target.ends_with(&format!("::{resolved_path}"))
    {
        resolved_symbol_id = semantic_target.to_string();
    }

    check_deadline(deadline, "patch result validation")?;
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

fn check_deadline(deadline: Option<&dyn DeadlineCheck>, phase: &str) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check(phase)?;
    }
    Ok(())
}

pub(crate) fn splice_source(source: &str, range: Range<usize>, replacement: &str) -> String {
    let mut updated =
        String::with_capacity(source.len() - (range.end - range.start) + replacement.len());
    updated.push_str(&source[..range.start]);
    updated.push_str(replacement);
    updated.push_str(&source[range.end..]);
    updated
}
