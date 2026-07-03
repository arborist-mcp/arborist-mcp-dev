use std::collections::BTreeSet;
use std::fs;
use std::ops::Range;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use tree_sitter::Node;

use crate::language::{
    ParsedDocument, c_include_targets, contains_kind, contains_node, first_identifier,
    is_field_node, node_text, normalize_path, parse_document, position_from, read_source,
    resolve_local_c_include, visit_tree,
};
use crate::model::{
    DisambiguationContext, LanguageId, PatchAstNodeResult, PatchCommitGateReport,
    PatchEvidenceInvariantReport, PatchValidationReport, SymbolSummary, ValidationAmbiguity,
    ValidationBinding, ValidationBindingDecision, ValidationIssue,
};
use crate::semantic::{
    ascend_to_symbol, c_semantic_path, c_symbol_id_for_node, find_semantic_node, semantic_path,
};

#[derive(Default)]
struct ReferenceValidation {
    unresolved_identifiers: Vec<String>,
    resolved_identifiers: Vec<ValidationBinding>,
    ambiguous_identifiers: Vec<ValidationAmbiguity>,
    binding_decisions: Vec<ValidationBindingDecision>,
}

#[derive(Debug, Clone)]
struct CAccessibleSymbol {
    name: String,
    summary: SymbolSummary,
    rank: usize,
}

pub fn patch_ast_node_from_path(
    path: &Path,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchAstNodeResult> {
    let disk_source = read_source(path)?;
    let result = patch_ast_node(path, &disk_source, semantic_target, new_code, bypass_reason)?;

    if result.applied {
        fs::write(path, &result.updated_source)
            .with_context(|| format!("failed to write patched source to {}", path.display()))?;
    }

    Ok(result)
}

pub fn patch_ast_node(
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
) -> Result<PatchAstNodeResult> {
    let (start_byte, end_byte) = semantic_target_range(path, source, semantic_target)?;
    let updated_source = splice_source(source, start_byte..end_byte, new_code);
    build_patch_result(
        path,
        semantic_target,
        updated_source,
        bypass_reason,
        start_byte,
        new_code.len(),
    )
}

pub(crate) fn semantic_target_range(
    path: &Path,
    source: &str,
    semantic_target: &str,
) -> Result<(usize, usize)> {
    let document = parse_document(path, source)?;
    let target_node = find_semantic_node(
        document.language_id,
        path,
        &document.tree,
        source,
        semantic_target,
    )?
    .ok_or_else(|| anyhow!("semantic path not found: {semantic_target}"))?;

    Ok((target_node.start_byte(), target_node.end_byte()))
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

    if validation.syntax_errors.is_empty() {
        if let Some(symbol_node) = patched_symbol {
            let reference_validation = collect_reference_validation(
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

    Ok(PatchAstNodeResult {
        file: normalize_path(path),
        target_path: semantic_target.to_string(),
        resolved_path,
        resolved_symbol_id,
        applied,
        bypass_applied,
        updated_source,
        validation,
    })
}

fn evaluate_patch_commit_gate(
    validation: &PatchValidationReport,
    bypass_reason: Option<&str>,
) -> PatchCommitGateReport {
    let blocking_decisions = validation
        .binding_decisions
        .iter()
        .filter(|decision| decision.status != "resolved")
        .cloned()
        .collect::<Vec<_>>();
    let evidence_invariants = validation
        .binding_decisions
        .iter()
        .map(evaluate_binding_evidence_invariant)
        .collect::<Vec<_>>();
    let has_evidence_failure = evidence_invariants
        .iter()
        .any(|invariant| invariant.status == "failed");
    let bypass_reason = bypass_reason
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .map(str::to_string);

    if validation.syntax_errors.is_empty() && blocking_decisions.is_empty() && !has_evidence_failure
    {
        return PatchCommitGateReport {
            status: "allowed".to_string(),
            allowed: true,
            reason: "syntax and symbol binding validation passed".to_string(),
            bypass_reason: None,
            blocking_decisions,
            evidence_invariants,
            syntax_error_count: 0,
        };
    }

    if let Some(bypass_reason) = bypass_reason {
        return PatchCommitGateReport {
            status: "allowed_with_bypass".to_string(),
            allowed: true,
            reason:
                "validation reported blocking evidence, but an explicit bypass reason was provided"
                    .to_string(),
            bypass_reason: Some(bypass_reason),
            blocking_decisions,
            evidence_invariants,
            syntax_error_count: validation.syntax_errors.len(),
        };
    }

    PatchCommitGateReport {
        status: "rejected".to_string(),
        allowed: false,
        reason: rejected_patch_reason(validation, &blocking_decisions),
        bypass_reason: None,
        blocking_decisions,
        evidence_invariants,
        syntax_error_count: validation.syntax_errors.len(),
    }
}

fn evaluate_binding_evidence_invariant(
    decision: &ValidationBindingDecision,
) -> PatchEvidenceInvariantReport {
    let candidate_evidence_keys = decision
        .candidates
        .iter()
        .map(|candidate| candidate.evidence_key.clone())
        .collect::<Vec<_>>();

    match decision.status.as_str() {
        "resolved" => resolved_evidence_invariant(decision, candidate_evidence_keys),
        "ambiguous" => PatchEvidenceInvariantReport {
            name: decision.name.clone(),
            status: "blocked".to_string(),
            reason: "multiple candidate evidence keys remain visible".to_string(),
            selected_evidence_key: None,
            candidate_evidence_keys,
        },
        "unresolved" => PatchEvidenceInvariantReport {
            name: decision.name.clone(),
            status: "blocked".to_string(),
            reason: "no candidate evidence key is available for this binding".to_string(),
            selected_evidence_key: None,
            candidate_evidence_keys,
        },
        _ => PatchEvidenceInvariantReport {
            name: decision.name.clone(),
            status: "failed".to_string(),
            reason: format!("unknown binding decision status: {}", decision.status),
            selected_evidence_key: None,
            candidate_evidence_keys,
        },
    }
}

fn resolved_evidence_invariant(
    decision: &ValidationBindingDecision,
    candidate_evidence_keys: Vec<String>,
) -> PatchEvidenceInvariantReport {
    let selected_candidate = decision.selected_symbol_id.as_ref().and_then(|symbol_id| {
        decision
            .candidates
            .iter()
            .find(|candidate| &candidate.symbol_id == symbol_id)
    });
    let selected_evidence_key = selected_candidate.map(|candidate| candidate.evidence_key.clone());

    if decision.candidates.len() != 1 {
        return PatchEvidenceInvariantReport {
            name: decision.name.clone(),
            status: "failed".to_string(),
            reason: "resolved binding must have exactly one candidate".to_string(),
            selected_evidence_key,
            candidate_evidence_keys,
        };
    }

    if selected_evidence_key
        .as_ref()
        .is_none_or(|evidence_key| evidence_key.is_empty())
    {
        return PatchEvidenceInvariantReport {
            name: decision.name.clone(),
            status: "failed".to_string(),
            reason: "resolved binding is missing selected evidence key".to_string(),
            selected_evidence_key,
            candidate_evidence_keys,
        };
    }

    PatchEvidenceInvariantReport {
        name: decision.name.clone(),
        status: "passed".to_string(),
        reason: "resolved binding has one selected candidate evidence key".to_string(),
        selected_evidence_key,
        candidate_evidence_keys,
    }
}

fn rejected_patch_reason(
    validation: &PatchValidationReport,
    blocking_decisions: &[ValidationBindingDecision],
) -> String {
    if !validation.syntax_errors.is_empty() {
        return "syntax validation failed".to_string();
    }

    if blocking_decisions
        .iter()
        .any(|decision| decision.status == "ambiguous")
    {
        return "symbol binding is ambiguous".to_string();
    }

    if blocking_decisions
        .iter()
        .any(|decision| decision.status == "unresolved")
    {
        return "symbol binding is unresolved".to_string();
    }

    if validation
        .binding_decisions
        .iter()
        .any(|decision| decision.status == "resolved" && decision.candidates.len() != 1)
    {
        return "symbol evidence invariant failed".to_string();
    }

    "patch validation failed".to_string()
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
        LanguageId::Python => collect_python_reference_validation(document, source, symbol_node),
        LanguageId::C => collect_c_reference_validation(path, document, source, symbol_node),
    }
}

fn collect_python_reference_validation(
    document: &ParsedDocument,
    source: &str,
    symbol_node: Node<'_>,
) -> Result<ReferenceValidation> {
    let mut accessible = BTreeSet::new();
    collect_python_module_names(document.tree.root_node(), source, &mut accessible)?;

    let mut current = Some(symbol_node);
    while let Some(node) = current {
        collect_python_definitions(node, source, &mut accessible)?;
        current = node.parent();
    }

    let mut references = BTreeSet::new();
    collect_python_references(symbol_node, source, &mut references)?;

    let unresolved_identifiers = references
        .into_iter()
        .filter(|name| !accessible.contains(name) && !PYTHON_BUILTINS.contains(&name.as_str()))
        .collect::<Vec<_>>();

    Ok(ReferenceValidation {
        binding_decisions: unresolved_identifiers
            .iter()
            .map(|name| unresolved_binding_decision(name))
            .collect(),
        unresolved_identifiers,
        ..ReferenceValidation::default()
    })
}

fn collect_c_reference_validation(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    symbol_node: Node<'_>,
) -> Result<ReferenceValidation> {
    let mut accessible = BTreeSet::new();
    let mut visited = BTreeSet::new();
    collect_c_accessible_names(path, document, source, &mut accessible, &mut visited)?;
    let mut local_definitions = BTreeSet::new();
    collect_c_local_definitions(symbol_node, source, &mut local_definitions)?;

    let mut references = BTreeSet::new();
    collect_c_references(symbol_node, source, &mut references)?;

    let accessible_symbols = collect_c_accessible_symbols(path, document, source)?;
    let mut validation = ReferenceValidation::default();

    for name in references {
        if local_definitions.contains(&name) {
            continue;
        }

        let candidates = c_binding_candidates_for_name(&accessible_symbols, &name);
        match candidates.as_slice() {
            [] => {
                if !accessible.contains(&name) {
                    validation
                        .binding_decisions
                        .push(unresolved_binding_decision(&name));
                    validation.unresolved_identifiers.push(name);
                }
            }
            [single] => {
                validation
                    .binding_decisions
                    .push(resolved_binding_decision(&name, &single.summary));
                validation.resolved_identifiers.push(ValidationBinding {
                    name,
                    symbol: single.summary.clone(),
                });
            }
            _ => {
                let candidate_summaries = candidates
                    .into_iter()
                    .map(|candidate| candidate.summary)
                    .collect::<Vec<_>>();
                let reason = ambiguity_reason(&candidate_summaries);
                validation
                    .binding_decisions
                    .push(ambiguous_binding_decision(
                        &name,
                        &reason,
                        &candidate_summaries,
                    ));
                validation.ambiguous_identifiers.push(ValidationAmbiguity {
                    name,
                    reason,
                    disambiguation_context: ambiguity_disambiguation_context(
                        path,
                        document,
                        source,
                        &candidate_summaries,
                    )?,
                    candidates: candidate_summaries,
                });
            }
        }
    }

    Ok(validation)
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

fn ambiguity_reason(candidates: &[SymbolSummary]) -> String {
    let distinct_families = candidates
        .iter()
        .filter_map(|candidate| symbol_include_family(candidate))
        .collect::<BTreeSet<_>>();

    if distinct_families.len() > 1 {
        "multiple equally-ranked definitions across include families".to_string()
    } else {
        "multiple equally-ranked visible bindings".to_string()
    }
}

fn ambiguity_disambiguation_context(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    candidates: &[SymbolSummary],
) -> Result<DisambiguationContext> {
    let visible_include_families = collect_visible_include_families(path, document, source)?
        .into_iter()
        .collect::<Vec<_>>();
    let candidate_include_families = ordered_candidate_include_families(candidates);
    let matched_visible_families = visible_include_families
        .iter()
        .filter(|family| candidate_include_families.contains(family))
        .cloned()
        .collect::<Vec<_>>();
    let preferred_family = if matched_visible_families.len() == 1 {
        matched_visible_families.first().cloned()
    } else {
        None
    };
    let active_include_family = if candidate_include_families.len() == 1 {
        candidate_include_families.first().cloned()
    } else {
        preferred_family.clone()
    };

    Ok(DisambiguationContext {
        active_include_family,
        preferred_family,
        visible_include_families,
        candidate_include_families,
        candidate_symbol_ids: candidates
            .iter()
            .map(|candidate| candidate.symbol_id.clone())
            .collect(),
    })
}

fn symbol_include_family(candidate: &SymbolSummary) -> Option<String> {
    candidate
        .symbol_id
        .rsplit_once("::")
        .map(|(family, _)| family.to_string())
}

fn ordered_candidate_include_families(candidates: &[SymbolSummary]) -> Vec<String> {
    let mut families = Vec::new();
    for family in candidates
        .iter()
        .filter_map(|candidate| symbol_include_family(candidate))
    {
        push_unique(&mut families, family);
    }
    families
}

fn collect_visible_include_families(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
) -> Result<Vec<String>> {
    let mut families = Vec::new();
    let mut visited = BTreeSet::new();
    collect_visible_include_families_from_document(
        path,
        document,
        source,
        &mut families,
        &mut visited,
    )?;
    Ok(families)
}

fn collect_visible_include_families_from_document(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    families: &mut Vec<String>,
    visited: &mut BTreeSet<String>,
) -> Result<()> {
    let normalized = normalize_path(path);
    if !visited.insert(normalized) {
        return Ok(());
    }

    for include_target in c_include_targets(document.tree.root_node(), source)? {
        let Some(include_path) = resolve_local_c_include(path, &include_target) else {
            continue;
        };
        let include_family = normalize_path(&include_path);
        push_unique(families, include_family);

        let include_source = read_source(&include_path)?;
        let include_document = parse_document(&include_path, &include_source)?;
        collect_visible_include_families_from_document(
            &include_path,
            &include_document,
            &include_source,
            families,
            visited,
        )?;
    }

    Ok(())
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn collect_python_module_names(
    root: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        collect_python_module_child_names(child, source, names)?;
    }
    Ok(())
}

fn collect_python_module_child_names(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    match node.kind() {
        "function_definition" | "class_definition" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                names.insert(node_text(name_node, source)?.trim().to_string());
            }
        }
        "assignment" | "augmented_assignment" => {
            if let Some(left) = node.child_by_field_name("left") {
                collect_python_target_names(left, source, names)?;
            }
        }
        "import_statement" | "import_from_statement" => {
            collect_python_import_names(node, source, names)?;
        }
        _ => {}
    }
    Ok(())
}

fn collect_python_definitions(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        let _ = collect_python_definition_node(candidate, source, names);
    };
    visit_tree(node, &mut callback);
    Ok(())
}

fn collect_python_definition_node(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    match node.kind() {
        "function_definition" | "class_definition" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                names.insert(node_text(name_node, source)?.trim().to_string());
            }
        }
        "assignment" | "augmented_assignment" => {
            if let Some(left) = node.child_by_field_name("left") {
                collect_python_target_names(left, source, names)?;
            }
        }
        "for_statement" => {
            if let Some(left) = node.child_by_field_name("left") {
                collect_python_target_names(left, source, names)?;
            }
        }
        "import_statement" | "import_from_statement" => {
            collect_python_import_names(node, source, names)?;
        }
        parameter_kind if PYTHON_PARAMETER_KINDS.contains(&parameter_kind) => {
            if node.kind() == "identifier" {
                names.insert(node_text(node, source)?.trim().to_string());
            }
        }
        "identifier" if is_python_parameter_name(node) => {
            names.insert(node_text(node, source)?.trim().to_string());
        }
        _ => {}
    }
    Ok(())
}

fn collect_python_target_names(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() == "identifier" {
            let _ = node_text(candidate, source).map(|text| names.insert(text.trim().to_string()));
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}

fn collect_python_import_names(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() == "identifier" {
            let parent = candidate.parent();
            if parent.is_some_and(|p| p.kind() == "dotted_name") {
                return;
            }
            let _ = node_text(candidate, source).map(|text| names.insert(text.trim().to_string()));
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}

pub(crate) fn collect_python_references(
    node: Node<'_>,
    source: &str,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() != "identifier" || !should_count_python_reference(candidate) {
            return;
        }

        let _ = node_text(candidate, source).map(|text| references.insert(text.trim().to_string()));
    };
    visit_tree(node, &mut callback);
    Ok(())
}

fn should_count_python_reference(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    if is_field_node(parent, "name", node)
        && matches!(
            parent.kind(),
            "function_definition" | "class_definition" | "keyword_argument"
        )
    {
        return false;
    }

    if is_field_node(parent, "attribute", node) && parent.kind() == "attribute" {
        return false;
    }

    if matches!(parent.kind(), "import_statement" | "import_from_statement") {
        return false;
    }

    if matches!(
        parent.kind(),
        "parameters"
            | "lambda_parameters"
            | "typed_parameter"
            | "default_parameter"
            | "typed_default_parameter"
            | "list_splat_pattern"
            | "dictionary_splat_pattern"
            | "tuple_pattern"
    ) {
        return false;
    }

    if let Some(left) = parent.child_by_field_name("left") {
        if matches!(
            parent.kind(),
            "assignment" | "augmented_assignment" | "for_statement"
        ) && contains_node(left, node)
        {
            return false;
        }
    }

    true
}

fn is_python_parameter_name(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    PYTHON_PARAMETER_KINDS.contains(&parent.kind())
}

fn collect_c_top_level_names(
    root: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "type_definition" | "function_definition" => {
                if let Some(name) = first_identifier(child, source)? {
                    names.insert(name);
                }
            }
            "declaration" => {
                if let Some(name) = first_identifier(child, source)? {
                    names.insert(name);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn collect_c_accessible_names(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    names: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) -> Result<()> {
    let normalized = normalize_path(path);
    if !visited.insert(normalized) {
        return Ok(());
    }

    collect_c_top_level_names(document.tree.root_node(), source, names)?;
    for include_target in c_include_targets(document.tree.root_node(), source)? {
        let Some(include_path) = resolve_local_c_include(path, &include_target) else {
            continue;
        };
        let include_source = read_source(&include_path)?;
        let include_document = parse_document(&include_path, &include_source)?;
        collect_c_accessible_names(
            &include_path,
            &include_document,
            &include_source,
            names,
            visited,
        )?;
    }

    Ok(())
}

fn collect_c_accessible_symbols(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
) -> Result<Vec<CAccessibleSymbol>> {
    let mut symbols = Vec::new();
    let mut visited_files = BTreeSet::new();
    let mut visited_companion_sources = BTreeSet::new();
    let context_file = normalize_path(path);
    collect_c_accessible_symbols_from_document(
        path,
        document,
        source,
        1000,
        "local_file",
        true,
        &context_file,
        &mut symbols,
        &mut visited_files,
        &mut visited_companion_sources,
    )?;

    let mut deduped = std::collections::BTreeMap::new();
    for symbol in symbols {
        deduped
            .entry(symbol.summary.symbol_id.clone())
            .and_modify(|existing: &mut CAccessibleSymbol| {
                if symbol.rank > existing.rank {
                    *existing = symbol.clone();
                }
            })
            .or_insert(symbol);
    }

    Ok(deduped.into_values().collect())
}

fn collect_c_accessible_symbols_from_document(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    base_rank: usize,
    origin_type: &str,
    allow_companion_sources: bool,
    context_file: &str,
    symbols: &mut Vec<CAccessibleSymbol>,
    visited_files: &mut BTreeSet<String>,
    visited_companion_sources: &mut BTreeSet<String>,
) -> Result<()> {
    let normalized = normalize_path(path);
    if !visited_files.insert(normalized.clone()) {
        return Ok(());
    }

    collect_c_symbol_candidates_from_root(
        path,
        document.tree.root_node(),
        source,
        base_rank,
        origin_type,
        context_file,
        symbols,
    )?;

    for include_target in c_include_targets(document.tree.root_node(), source)? {
        let Some(include_path) = resolve_local_c_include(path, &include_target) else {
            continue;
        };

        let include_source = read_source(&include_path)?;
        let include_document = parse_document(&include_path, &include_source)?;
        collect_c_accessible_symbols_from_document(
            &include_path,
            &include_document,
            &include_source,
            500,
            "include_header",
            true,
            context_file,
            symbols,
            visited_files,
            visited_companion_sources,
        )?;

        if allow_companion_sources {
            if let Some(companion_source_path) = companion_c_source_path(&include_path) {
                let normalized_companion = normalize_path(&companion_source_path);
                if visited_companion_sources.insert(normalized_companion) {
                    let companion_source = read_source(&companion_source_path)?;
                    let companion_document =
                        parse_document(&companion_source_path, &companion_source)?;
                    collect_c_symbol_candidates_from_root(
                        &companion_source_path,
                        companion_document.tree.root_node(),
                        &companion_source,
                        600,
                        "companion_source",
                        context_file,
                        symbols,
                    )?;
                }
            }
        }
    }

    Ok(())
}

fn collect_c_symbol_candidates_from_root(
    path: &Path,
    root: Node<'_>,
    source: &str,
    base_rank: usize,
    origin_type: &str,
    context_file: &str,
    symbols: &mut Vec<CAccessibleSymbol>,
) -> Result<()> {
    let normalized_path = normalize_path(path);
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        let Some(name) = c_candidate_name(child, source)? else {
            continue;
        };
        let Some(semantic_path) = c_semantic_path(path, child, source)? else {
            continue;
        };
        if normalized_path != context_file && semantic_path.contains("::") {
            continue;
        }
        let Some(symbol_id) = c_symbol_id_for_node(path, child, source)? else {
            continue;
        };

        symbols.push(CAccessibleSymbol {
            name,
            summary: SymbolSummary::new(
                symbol_id,
                semantic_path,
                normalized_path.clone(),
                child.kind().to_string(),
                origin_type.to_string(),
                (child.start_byte(), child.end_byte()),
                c_candidate_signature(child, source)?,
            ),
            rank: base_rank + c_candidate_node_rank(child.kind()),
        });
    }

    Ok(())
}

fn c_candidate_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    match node.kind() {
        "type_definition" | "function_definition" => first_identifier(node, source),
        "declaration" if contains_kind(node, "function_declarator") => {
            first_identifier(node, source)
        }
        _ => Ok(None),
    }
}

fn c_candidate_signature(node: Node<'_>, source: &str) -> Result<Option<String>> {
    match node.kind() {
        "function_definition" => Ok(Some(crate::semantic::c_function_header(node, source)?)),
        "declaration" if contains_kind(node, "function_declarator") => {
            Ok(Some(node_text(node, source)?.trim().to_string()))
        }
        "type_definition" => Ok(Some(node_text(node, source)?.trim().to_string())),
        _ => Ok(None),
    }
}

fn c_candidate_node_rank(node_kind: &str) -> usize {
    match node_kind {
        "function_definition" => 30,
        "type_definition" => 20,
        "declaration" => 10,
        _ => 0,
    }
}

fn c_binding_candidates_for_name(
    accessible_symbols: &[CAccessibleSymbol],
    name: &str,
) -> Vec<CAccessibleSymbol> {
    let mut candidates = accessible_symbols
        .iter()
        .filter(|symbol| symbol.name == name)
        .cloned()
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .rank
            .cmp(&left.rank)
            .then_with(|| left.summary.symbol_id.cmp(&right.summary.symbol_id))
    });

    let Some(best_rank) = candidates.first().map(|candidate| candidate.rank) else {
        return Vec::new();
    };

    candidates
        .into_iter()
        .filter(|candidate| candidate.rank == best_rank)
        .collect()
}

fn companion_c_source_path(include_path: &Path) -> Option<std::path::PathBuf> {
    let extension = include_path.extension()?.to_str()?;
    if !matches!(extension, "h" | "hpp" | "hh") {
        return None;
    }

    let candidate = include_path.with_extension("c");
    candidate.exists().then_some(candidate)
}

fn collect_c_local_definitions(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if let Some(parent) = candidate.parent() {
            if candidate.kind() == "identifier"
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
                let _ =
                    node_text(candidate, source).map(|text| names.insert(text.trim().to_string()));
            }
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

const PYTHON_PARAMETER_KINDS: &[&str] = &[
    "parameters",
    "lambda_parameters",
    "typed_parameter",
    "default_parameter",
    "typed_default_parameter",
];

const PYTHON_BUILTINS: &[&str] = &[
    "abs",
    "all",
    "any",
    "bool",
    "dict",
    "enumerate",
    "filter",
    "float",
    "int",
    "len",
    "list",
    "map",
    "max",
    "min",
    "object",
    "open",
    "print",
    "range",
    "repr",
    "reversed",
    "set",
    "sorted",
    "str",
    "sum",
    "tuple",
    "zip",
];
