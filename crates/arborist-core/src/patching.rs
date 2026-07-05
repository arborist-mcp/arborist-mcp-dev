use std::collections::{BTreeMap, BTreeSet};
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
    ascend_to_symbol, c_parameters, c_return_type, c_semantic_path, c_symbol_id_for_node,
    find_semantic_node, python_display_byte_range, python_display_header, python_docstring,
    python_parameters, python_return_type, semantic_parent_path, semantic_path,
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

#[derive(Debug, Clone)]
struct PythonAccessibleSymbol {
    name: String,
    summary: SymbolSummary,
    rank: usize,
    visibility: PythonSymbolVisibility,
}

#[derive(Debug, Clone)]
enum PythonSymbolVisibility {
    Always,
    ClassBodyLocal {
        class_range: (usize, usize),
    },
    NamedExpression {
        expression_range: (usize, usize),
        comprehension_range: Option<(usize, usize)>,
        comprehension_part_index: Option<usize>,
    },
    ComprehensionTarget {
        comprehension_range: (usize, usize),
        clause_index: usize,
    },
    ExceptTarget {
        except_clause_range: (usize, usize),
    },
    MatchCapture {
        case_clause_range: (usize, usize),
        match_statement_end: usize,
    },
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

#[derive(Debug, Clone)]
struct PythonReferenceTarget<'tree> {
    name: String,
    node: Node<'tree>,
    imported_symbol: Option<(String, String)>,
    import_fallback_name: Option<String>,
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
    let target_node = python_symbol_replacement_node(document.language_id, target_node);

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

fn python_symbol_replacement_node<'tree>(
    language_id: LanguageId,
    node: Node<'tree>,
) -> Node<'tree> {
    if language_id == LanguageId::Python {
        if let Some(parent) = node.parent() {
            if parent.kind() == "decorated_definition" {
                return parent;
            }
        }
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
            collect_python_reference_validation(path, document, source, symbol_node)
        }
        LanguageId::C => collect_c_reference_validation(path, document, source, symbol_node),
    }
}

fn collect_python_reference_validation(
    path: &Path,
    _document: &ParsedDocument,
    source: &str,
    symbol_node: Node<'_>,
) -> Result<ReferenceValidation> {
    let bindings = collect_visible_python_import_bindings(path, symbol_node, source)?;
    let reference_targets = collect_python_reference_targets(symbol_node, source, &bindings)?;
    let normalized_path = normalize_path(path);
    let mut validation = ReferenceValidation::default();

    for reference_target in reference_targets {
        let name = reference_target.name.clone();
        if PYTHON_BUILTINS.contains(&name.as_str()) {
            continue;
        }

        let candidates = python_binding_candidates_for_reference(
            path,
            source,
            &normalized_path,
            &reference_target,
        )?;
        match candidates.as_slice() {
            [] => {
                validation
                    .binding_decisions
                    .push(unresolved_binding_decision(&name));
                validation
                    .resolved_identifiers
                    .retain(|binding| binding.name != name);
                validation
                    .ambiguous_identifiers
                    .retain(|binding| binding.name != name);
                if !validation.unresolved_identifiers.contains(&name) {
                    validation.unresolved_identifiers.push(name);
                }
            }
            [single] => {
                validation
                    .binding_decisions
                    .push(resolved_binding_decision(&name, &single.summary));
                let is_blocked = validation
                    .unresolved_identifiers
                    .iter()
                    .any(|item| item == &name)
                    || validation
                        .ambiguous_identifiers
                        .iter()
                        .any(|binding| binding.name == name);
                if !is_blocked
                    && !validation
                        .resolved_identifiers
                        .iter()
                        .any(|binding| binding.name == name)
                {
                    validation.resolved_identifiers.push(ValidationBinding {
                        name,
                        symbol: single.summary.clone(),
                    });
                }
            }
            _ => {
                let candidate_summaries = candidates
                    .into_iter()
                    .map(|candidate| candidate.summary)
                    .collect::<Vec<_>>();
                let reason = "multiple equally-ranked visible Python bindings".to_string();
                validation
                    .binding_decisions
                    .push(ambiguous_binding_decision(
                        &name,
                        &reason,
                        &candidate_summaries,
                    ));
                if !validation
                    .unresolved_identifiers
                    .iter()
                    .any(|item| item == &name)
                {
                    validation
                        .resolved_identifiers
                        .retain(|binding| binding.name != name);
                    if !validation
                        .ambiguous_identifiers
                        .iter()
                        .any(|binding| binding.name == name)
                    {
                        validation.ambiguous_identifiers.push(ValidationAmbiguity {
                            name,
                            reason,
                            disambiguation_context: DisambiguationContext::default(),
                            candidates: candidate_summaries,
                        });
                    }
                }
            }
        }
    }

    Ok(validation)
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

fn collect_python_reference_targets<'tree>(
    symbol_node: Node<'tree>,
    source: &str,
    bindings: &BTreeMap<String, PythonImportBinding>,
) -> Result<Vec<PythonReferenceTarget<'tree>>> {
    let mut references = Vec::new();
    collect_python_reference_targets_inner(symbol_node, source, bindings, &mut references)?;
    Ok(references)
}

fn collect_python_reference_targets_inner<'tree>(
    node: Node<'tree>,
    source: &str,
    bindings: &BTreeMap<String, PythonImportBinding>,
    references: &mut Vec<PythonReferenceTarget<'tree>>,
) -> Result<()> {
    if node.kind() == "attribute" {
        if let (Some(object_node), Some(attribute_node)) = (
            node.child_by_field_name("object"),
            node.child_by_field_name("attribute"),
        ) {
            if object_node.kind() == "identifier" && attribute_node.kind() == "identifier" {
                let object_name = node_text(object_node, source)?.trim().to_string();
                let attribute_name = node_text(attribute_node, source)?.trim().to_string();
                if let Some(PythonImportBinding::Module { module_name }) =
                    bindings.get(&object_name)
                {
                    let display_name = format!("{object_name}.{attribute_name}");
                    references.push(PythonReferenceTarget {
                        name: display_name,
                        node: node,
                        imported_symbol: Some((module_name.clone(), attribute_name)),
                        import_fallback_name: Some(object_name),
                    });
                    return Ok(());
                }
            }

            collect_python_reference_targets_inner(object_node, source, bindings, references)?;
            return Ok(());
        }
    }

    if node.kind() == "identifier" && should_count_python_reference(node, source) {
        let name = node_text(node, source)?.trim().to_string();
        let imported_symbol = match bindings.get(&name) {
            Some(PythonImportBinding::Symbol {
                module_name: Some(module_name),
                symbol_name,
            }) => Some((module_name.clone(), symbol_name.clone())),
            _ => None,
        };
        references.push(PythonReferenceTarget {
            name,
            node,
            imported_symbol,
            import_fallback_name: None,
        });
        return Ok(());
    }

    let child_count = node.child_count();
    for index in 0..child_count {
        if let Some(child) = node.child(index) {
            collect_python_reference_targets_inner(child, source, bindings, references)?;
        }
    }

    Ok(())
}

fn python_binding_candidates_for_reference(
    path: &Path,
    source: &str,
    normalized_path: &str,
    reference_target: &PythonReferenceTarget<'_>,
) -> Result<Vec<PythonAccessibleSymbol>> {
    if let Some((module_name, symbol_name)) = &reference_target.imported_symbol {
        if let Some(summary) = resolve_local_python_imported_symbol(path, module_name, symbol_name)?
        {
            return Ok(vec![PythonAccessibleSymbol {
                name: reference_target.name.clone(),
                summary,
                rank: 4_000_000,
                visibility: PythonSymbolVisibility::Always,
            }]);
        }
    }

    if let Some(fallback_name) = &reference_target.import_fallback_name {
        let fallback = PythonReferenceTarget {
            name: fallback_name.clone(),
            node: reference_target.node,
            imported_symbol: None,
            import_fallback_name: None,
        };
        let fallback_candidates =
            python_binding_candidates_for_reference(path, source, normalized_path, &fallback)?;
        if !fallback_candidates.is_empty() {
            return Ok(fallback_candidates);
        }
    }

    let name = if let Some((_, symbol_name)) = &reference_target.imported_symbol {
        symbol_name.clone()
    } else {
        reference_target.name.clone()
    };
    let force_module_scope =
        python_reference_is_global_declared(reference_target.node, source, &name);
    let mut candidates = Vec::new();
    let mut seen_function_scope = false;
    let mut skipped_current_class_scope = false;
    let mut skipped_current_function_scope = false;
    let mut scope_rank = 3_000_000usize;
    let mut current = Some(reference_target.node);
    let skip_current_function_scope = is_python_default_parameter_value(reference_target.node);
    let skip_current_class_scope = is_python_class_header_expression(reference_target.node);

    while let Some(node) = current {
        let include_scope = if force_module_scope {
            node.kind() == "module"
        } else {
            match node.kind() {
                "function_definition" => {
                    if skip_current_function_scope && !skipped_current_function_scope {
                        skipped_current_function_scope = true;
                        false
                    } else {
                        seen_function_scope = true;
                        true
                    }
                }
                "lambda" => {
                    if skip_current_function_scope && !skipped_current_function_scope {
                        skipped_current_function_scope = true;
                        false
                    } else {
                        seen_function_scope = true;
                        true
                    }
                }
                "list_comprehension"
                | "set_comprehension"
                | "dictionary_comprehension"
                | "generator_expression" => {
                    seen_function_scope = true;
                    false
                }
                "class_definition" => {
                    if skip_current_class_scope && !skipped_current_class_scope {
                        skipped_current_class_scope = true;
                        false
                    } else {
                        !seen_function_scope
                    }
                }
                "module" => true,
                _ => false,
            }
        };

        if include_scope {
            collect_python_scope_symbols(
                node,
                source,
                normalized_path,
                scope_rank,
                &mut candidates,
            )?;
            scope_rank = scope_rank.saturating_sub(1_000_000);
        }

        current = node.parent();
    }

    candidates.retain(|candidate| candidate.name == name);
    let mut resolving_candidates = candidates
        .iter()
        .filter(|candidate| python_accessible_symbol_resolves_at(candidate, reference_target.node))
        .cloned()
        .collect::<Vec<_>>();
    let mut suppressing_candidates = candidates
        .iter()
        .filter(|candidate| {
            python_accessible_symbol_suppresses_at(candidate, reference_target.node)
        })
        .cloned()
        .collect::<Vec<_>>();

    resolving_candidates.sort_by(|left, right| {
        right
            .rank
            .cmp(&left.rank)
            .then_with(|| left.summary.symbol_id.cmp(&right.summary.symbol_id))
    });

    suppressing_candidates.sort_by(|left, right| {
        right
            .rank
            .cmp(&left.rank)
            .then_with(|| left.summary.symbol_id.cmp(&right.summary.symbol_id))
    });

    let best_suppressing_rank = suppressing_candidates
        .first()
        .map(|candidate| candidate.rank);
    let Some(best_rank) = resolving_candidates.first().map(|candidate| candidate.rank) else {
        return Ok(Vec::new());
    };

    if best_suppressing_rank.is_some_and(|rank| rank > best_rank) {
        return Ok(Vec::new());
    }

    Ok(resolving_candidates
        .into_iter()
        .filter(|candidate| candidate.rank == best_rank)
        .collect())
}

fn collect_python_scope_symbols(
    scope_node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let scope_path = python_binding_scope_path(scope_node, source)?;
    let origin_type = if scope_node.kind() == "module" {
        "module_scope"
    } else {
        "local_scope"
    };

    if matches!(scope_node.kind(), "function_definition" | "lambda") {
        collect_python_parameter_symbols(
            scope_node,
            source,
            normalized_path,
            scope_path.as_deref(),
            origin_type,
            scope_rank + 500_000,
            symbols,
        )?;
    }

    if scope_node.kind() == "lambda" {
        let Some(body_node) = scope_node.child_by_field_name("body") else {
            return Ok(());
        };
        collect_python_statement_symbols(
            body_node,
            source,
            normalized_path,
            scope_path.as_deref(),
            origin_type,
            scope_rank,
            symbols,
        )?;
        return Ok(());
    }

    let class_visibility = (scope_node.kind() == "class_definition")
        .then_some((scope_node.start_byte(), scope_node.end_byte()));
    let body_node = if scope_node.kind() == "module" {
        scope_node
    } else if let Some(body) = scope_node.child_by_field_name("body") {
        body
    } else {
        return Ok(());
    };

    let external_bindings = collect_python_external_binding_names(body_node, source)?;
    let mut cursor = body_node.walk();
    for child in body_node.named_children(&mut cursor) {
        let mut statement_symbols = Vec::new();
        collect_python_statement_symbols(
            child,
            source,
            normalized_path,
            scope_path.as_deref(),
            origin_type,
            scope_rank,
            &mut statement_symbols,
        )?;
        if let Some(class_range) = class_visibility {
            for symbol in &mut statement_symbols {
                if matches!(symbol.visibility, PythonSymbolVisibility::Always) {
                    symbol.visibility = PythonSymbolVisibility::ClassBodyLocal { class_range };
                }
            }
        }
        if scope_node.kind() != "module" && !external_bindings.is_empty() {
            statement_symbols.retain(|symbol| !external_bindings.contains(&symbol.name));
        }
        symbols.extend(statement_symbols);
    }

    Ok(())
}

fn collect_python_statement_symbols(
    statement_node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    scope_rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    collect_python_named_expression_symbols(
        statement_node,
        source,
        normalized_path,
        scope_path,
        origin_type,
        scope_rank + 350_000 + statement_node.start_byte(),
        symbols,
    )?;
    collect_python_comprehension_target_symbols(
        statement_node,
        source,
        normalized_path,
        scope_path,
        origin_type,
        scope_rank + 325_000 + statement_node.start_byte(),
        symbols,
    )?;

    match statement_node.kind() {
        "function_definition" | "class_definition" | "decorated_definition" => {
            if let Some(summary) =
                python_symbol_summary(statement_node, source, normalized_path, origin_type)?
            {
                symbols.push(PythonAccessibleSymbol {
                    name: summary
                        .semantic_path
                        .rsplit('.')
                        .next()
                        .unwrap_or(&summary.semantic_path)
                        .to_string(),
                    summary,
                    rank: scope_rank + 400_000 + statement_node.start_byte(),
                    visibility: PythonSymbolVisibility::Always,
                });
            }
        }
        "assignment" | "augmented_assignment" => {
            if let Some(left) = statement_node.child_by_field_name("left") {
                collect_python_target_symbols(
                    left,
                    source,
                    normalized_path,
                    scope_path,
                    origin_type,
                    "assignment",
                    scope_rank + 300_000 + statement_node.start_byte(),
                    symbols,
                )?;
            }
        }
        "for_statement" => {
            if let Some(left) = statement_node.child_by_field_name("left") {
                collect_python_target_symbols(
                    left,
                    source,
                    normalized_path,
                    scope_path,
                    origin_type,
                    "for_target",
                    scope_rank + 300_000 + statement_node.start_byte(),
                    symbols,
                )?;
            }
            collect_python_child_block_symbols(
                statement_node,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank,
                symbols,
            )?;
        }
        "with_statement" => {
            collect_python_with_target_symbols(
                statement_node,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank + 300_000 + statement_node.start_byte(),
                symbols,
            )?;
            collect_python_child_block_symbols(
                statement_node,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank,
                symbols,
            )?;
        }
        "try_statement" => {
            collect_python_except_target_symbols(
                statement_node,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank + 300_000 + statement_node.start_byte(),
                symbols,
            )?;
            collect_python_child_block_symbols(
                statement_node,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank,
                symbols,
            )?;
        }
        "match_statement" => {
            collect_python_match_target_symbols(
                statement_node,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank + 300_000 + statement_node.start_byte(),
                symbols,
            )?;
            collect_python_child_block_symbols(
                statement_node,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank,
                symbols,
            )?;
        }
        "if_statement" | "while_statement" => {
            collect_python_child_block_symbols(
                statement_node,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank,
                symbols,
            )?;
        }
        "import_statement" | "import_from_statement" => {
            collect_python_import_symbols(
                statement_node,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank + 300_000 + statement_node.start_byte(),
                symbols,
            )?;
        }
        "expression_statement" => {
            let mut cursor = statement_node.walk();
            for child in statement_node.named_children(&mut cursor) {
                collect_python_statement_symbols(
                    child,
                    source,
                    normalized_path,
                    scope_path,
                    origin_type,
                    scope_rank,
                    symbols,
                )?;
            }
        }
        _ => {}
    }

    Ok(())
}

fn collect_python_comprehension_target_symbols(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if !matches!(
            candidate.kind(),
            "list_comprehension"
                | "set_comprehension"
                | "dictionary_comprehension"
                | "generator_expression"
        ) {
            return;
        }

        let comprehension_range = (candidate.start_byte(), candidate.end_byte());
        let mut clause_index = 0usize;
        let mut cursor = candidate.walk();
        for child in candidate.named_children(&mut cursor) {
            if child.kind() != "for_in_clause" {
                continue;
            }
            let Some(left) = child.child_by_field_name("left") else {
                clause_index += 1;
                continue;
            };
            collect_python_target_symbols_with_visibility(
                left,
                source,
                normalized_path,
                scope_path,
                origin_type,
                "comprehension_target",
                rank + child.start_byte(),
                PythonSymbolVisibility::ComprehensionTarget {
                    comprehension_range,
                    clause_index,
                },
                symbols,
            )
            .ok();
            clause_index += 1;
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}

fn collect_python_child_block_symbols(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    scope_rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() != "block" {
            continue;
        }

        let mut block_cursor = child.walk();
        for statement in child.named_children(&mut block_cursor) {
            collect_python_statement_symbols(
                statement,
                source,
                normalized_path,
                scope_path,
                origin_type,
                scope_rank,
                symbols,
            )?;
        }
    }

    Ok(())
}

fn collect_python_named_expression_symbols(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() != "named_expression" {
            return;
        }
        let Some(left) = candidate.child_by_field_name("name") else {
            return;
        };
        let mut target_callback = |target: Node<'_>| {
            if target.kind() != "identifier" {
                return;
            }
            if let Ok(name) = node_text(target, source) {
                symbols.push(PythonAccessibleSymbol {
                    name: name.trim().to_string(),
                    summary: python_synthetic_symbol_summary(
                        normalized_path,
                        scope_path,
                        name.trim(),
                        "named_expression",
                        origin_type,
                        (target.start_byte(), target.end_byte()),
                    ),
                    rank: rank + target.start_byte(),
                    visibility: PythonSymbolVisibility::NamedExpression {
                        expression_range: (candidate.start_byte(), candidate.end_byte()),
                        comprehension_range: python_enclosing_comprehension(candidate).map(
                            |comprehension| (comprehension.start_byte(), comprehension.end_byte()),
                        ),
                        comprehension_part_index: python_enclosing_comprehension(candidate)
                            .and_then(|comprehension| {
                                python_comprehension_part_index(comprehension, candidate)
                            }),
                    },
                });
            }
        };
        visit_tree(left, &mut target_callback);
    };
    visit_tree(node, &mut callback);
    Ok(())
}

fn python_symbol_summary(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    origin_type: &str,
) -> Result<Option<SymbolSummary>> {
    let Some(node) = python_symbol_node(node) else {
        return Ok(None);
    };

    let semantic_path = semantic_path(node, source)?;
    let scope_path = semantic_parent_path(&semantic_path);
    let signature = Some(python_display_header(node, source)?);
    let parameters = python_parameters(node, source)?;
    let return_type = python_return_type(node, source)?;
    let docstring = python_docstring(node, source)?;

    Ok(Some(SymbolSummary::new(
        semantic_path.clone(),
        semantic_path,
        scope_path,
        normalized_path.to_string(),
        node.kind().to_string(),
        origin_type.to_string(),
        python_display_byte_range(node),
        signature,
        parameters,
        return_type,
        docstring,
    )))
}

fn python_symbol_node(node: Node<'_>) -> Option<Node<'_>> {
    match node.kind() {
        "function_definition" | "class_definition" => Some(node),
        "decorated_definition" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|child| matches!(child.kind(), "function_definition" | "class_definition"))
        }
        _ => None,
    }
}

pub(crate) fn resolve_local_python_imported_symbol(
    current_path: &Path,
    module_name: &str,
    symbol_name: &str,
) -> Result<Option<SymbolSummary>> {
    let mut visited = BTreeSet::new();
    resolve_local_python_imported_symbol_inner(current_path, module_name, symbol_name, &mut visited)
}

fn resolve_local_python_imported_symbol_inner(
    current_path: &Path,
    module_name: &str,
    symbol_name: &str,
    visited: &mut BTreeSet<String>,
) -> Result<Option<SymbolSummary>> {
    let Some(module_path) = resolve_local_python_module_path(current_path, module_name) else {
        return Ok(None);
    };

    let visit_key = format!("{}::{symbol_name}", normalize_path(&module_path));
    if !visited.insert(visit_key) {
        return Ok(None);
    }

    let module_source = read_source(&module_path)?;
    let document = parse_document(&module_path, &module_source)?;
    if document.language_id != LanguageId::Python {
        return Ok(None);
    }

    let normalized_module_path = normalize_path(&module_path);
    let root = document.tree.root_node();
    let mut cursor = root.walk();
    let children = root.named_children(&mut cursor).collect::<Vec<_>>();

    for child in &children {
        let Some(summary) = python_symbol_summary(
            *child,
            &module_source,
            &normalized_module_path,
            "imported_module",
        )?
        else {
            continue;
        };

        if summary.semantic_path == symbol_name {
            return Ok(Some(summary));
        }
    }

    for child in children {
        let Some(binding) =
            python_reexport_binding_for_name(&module_path, child, &module_source, symbol_name)?
        else {
            continue;
        };

        let PythonImportBinding::Symbol {
            module_name: Some(reexport_module),
            symbol_name: reexported_symbol,
        } = binding
        else {
            continue;
        };

        if let Some(summary) = resolve_local_python_imported_symbol_inner(
            &module_path,
            &reexport_module,
            &reexported_symbol,
            visited,
        )? {
            return Ok(Some(summary));
        }
    }

    Ok(None)
}

fn python_reexport_binding_for_name(
    current_path: &Path,
    statement_node: Node<'_>,
    source: &str,
    symbol_name: &str,
) -> Result<Option<PythonImportBinding>> {
    if statement_node.kind() != "import_from_statement" {
        return Ok(None);
    }

    let mut cursor = statement_node.walk();
    let named_children = statement_node
        .named_children(&mut cursor)
        .collect::<Vec<_>>();
    let Some(module_node) = named_children.first() else {
        return Ok(None);
    };
    let module_name = node_text(*module_node, source)?.trim().to_string();

    for child in named_children.into_iter().skip(1) {
        match child.kind() {
            "aliased_import" => {
                let mut alias_cursor = child.walk();
                let alias_children = child.named_children(&mut alias_cursor).collect::<Vec<_>>();
                if alias_children.len() < 2 {
                    continue;
                }

                let imported_name = node_text(alias_children[0], source)?.trim().to_string();
                let Some(alias_node) = alias_children.last().copied() else {
                    continue;
                };
                let alias_name = node_text(alias_node, source)?.trim().to_string();
                if alias_name == symbol_name {
                    return Ok(Some(python_import_from_binding(
                        current_path,
                        &module_name,
                        &imported_name,
                    )));
                }
            }
            "dotted_name" | "identifier" => {
                let imported_name = node_text(child, source)?.trim().to_string();
                let binding_name = python_imported_symbol_name(&imported_name);
                if binding_name == symbol_name {
                    return Ok(Some(python_import_from_binding(
                        current_path,
                        &module_name,
                        &imported_name,
                    )));
                }
            }
            _ => {}
        }
    }

    Ok(None)
}

pub(crate) fn resolve_local_python_module_path(
    current_path: &Path,
    module_name: &str,
) -> Option<std::path::PathBuf> {
    let parent = current_path.parent()?;
    let (relative_levels, module_parts) = split_python_module_reference(module_name);
    if relative_levels > 0 {
        let mut candidate = parent.to_path_buf();
        for _ in 0..relative_levels.saturating_sub(1) {
            candidate = candidate.parent()?.to_path_buf();
        }
        return resolve_python_module_candidate(candidate, &module_parts);
    }

    let mut search_root = Some(parent);
    while let Some(root) = search_root {
        if let Some(candidate) = resolve_python_module_candidate(root.to_path_buf(), &module_parts)
        {
            return Some(candidate);
        }
        search_root = root.parent();
    }

    None
}

fn split_python_module_reference(module_name: &str) -> (usize, Vec<&str>) {
    let relative_levels = module_name.chars().take_while(|ch| *ch == '.').count();
    let trimmed = module_name.trim_start_matches('.');
    let parts = if trimmed.is_empty() {
        Vec::new()
    } else {
        trimmed
            .split('.')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
    };
    (relative_levels, parts)
}

fn resolve_python_module_candidate(
    mut base_dir: std::path::PathBuf,
    module_parts: &[&str],
) -> Option<std::path::PathBuf> {
    for part in module_parts {
        base_dir.push(part);
    }

    let file_candidate = base_dir.with_extension("py");
    if file_candidate.exists() {
        return Some(file_candidate);
    }

    let package_candidate = base_dir.join("__init__.py");
    package_candidate.exists().then_some(package_candidate)
}

fn collect_python_parameter_symbols(
    function_node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let Some(parameters_node) = function_node.child_by_field_name("parameters") else {
        return Ok(());
    };

    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() != "identifier" || !is_python_parameter_symbol_name(candidate) {
            return;
        }

        if let Ok(name) = node_text(candidate, source) {
            symbols.push(PythonAccessibleSymbol {
                name: name.trim().to_string(),
                summary: python_synthetic_symbol_summary(
                    normalized_path,
                    scope_path,
                    name.trim(),
                    "parameter",
                    origin_type,
                    (candidate.start_byte(), candidate.end_byte()),
                ),
                rank: rank + candidate.start_byte(),
                visibility: PythonSymbolVisibility::Always,
            });
        }
    };
    visit_tree(parameters_node, &mut callback);
    Ok(())
}

fn collect_python_target_symbols(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    node_kind: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    collect_python_target_symbols_with_visibility(
        node,
        source,
        normalized_path,
        scope_path,
        origin_type,
        node_kind,
        rank,
        PythonSymbolVisibility::Always,
        symbols,
    )
}

fn collect_python_target_symbols_with_visibility(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    node_kind: &str,
    rank: usize,
    visibility: PythonSymbolVisibility,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() != "identifier" {
            return;
        }

        if let Ok(name) = node_text(candidate, source) {
            symbols.push(PythonAccessibleSymbol {
                name: name.trim().to_string(),
                summary: python_synthetic_symbol_summary(
                    normalized_path,
                    scope_path,
                    name.trim(),
                    node_kind,
                    origin_type,
                    (candidate.start_byte(), candidate.end_byte()),
                ),
                rank: rank + candidate.start_byte(),
                visibility: visibility.clone(),
            });
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}

fn collect_python_with_target_symbols(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() != "identifier" || !is_python_with_target_name(candidate, source) {
            return;
        }

        if let Ok(name) = node_text(candidate, source) {
            symbols.push(PythonAccessibleSymbol {
                name: name.trim().to_string(),
                summary: python_synthetic_symbol_summary(
                    normalized_path,
                    scope_path,
                    name.trim(),
                    "with_target",
                    origin_type,
                    (candidate.start_byte(), candidate.end_byte()),
                ),
                rank: rank + candidate.start_byte(),
                visibility: PythonSymbolVisibility::Always,
            });
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}

fn collect_python_except_target_symbols(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() != "as_pattern_target" {
            return;
        }

        let Some(except_clause) = python_enclosing_except_clause(candidate) else {
            return;
        };
        let Some(_scope_node) = python_nearest_scope_node(candidate) else {
            return;
        };

        if let Ok(name) = node_text(candidate, source) {
            symbols.push(PythonAccessibleSymbol {
                name: name.trim().to_string(),
                summary: python_synthetic_symbol_summary(
                    normalized_path,
                    scope_path,
                    name.trim(),
                    "except_target",
                    origin_type,
                    (candidate.start_byte(), candidate.end_byte()),
                ),
                rank: rank + candidate.start_byte(),
                visibility: PythonSymbolVisibility::ExceptTarget {
                    except_clause_range: (except_clause.start_byte(), except_clause.end_byte()),
                },
            });
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}

fn collect_python_match_target_symbols(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() != "case_pattern" {
            return;
        }

        let Some(case_clause) = python_enclosing_case_clause(candidate) else {
            return;
        };

        for name in python_match_capture_names(candidate, source) {
            symbols.push(PythonAccessibleSymbol {
                name: name.clone(),
                summary: python_synthetic_symbol_summary(
                    normalized_path,
                    scope_path,
                    &name,
                    "match_capture",
                    origin_type,
                    (candidate.start_byte(), candidate.end_byte()),
                ),
                rank: rank + candidate.start_byte(),
                visibility: PythonSymbolVisibility::MatchCapture {
                    case_clause_range: (case_clause.start_byte(), case_clause.end_byte()),
                    match_statement_end: node.end_byte(),
                },
            });
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}

fn collect_python_import_symbols(
    node: Node<'_>,
    source: &str,
    normalized_path: &str,
    scope_path: Option<&str>,
    origin_type: &str,
    rank: usize,
    symbols: &mut Vec<PythonAccessibleSymbol>,
) -> Result<()> {
    let mut callback = |candidate: Node<'_>| {
        if candidate.kind() != "identifier" {
            return;
        }
        if candidate
            .parent()
            .is_some_and(|parent| parent.kind() == "dotted_name")
        {
            return;
        }

        if let Ok(name) = node_text(candidate, source) {
            symbols.push(PythonAccessibleSymbol {
                name: name.trim().to_string(),
                summary: python_synthetic_symbol_summary(
                    normalized_path,
                    scope_path,
                    name.trim(),
                    "import",
                    origin_type,
                    (candidate.start_byte(), candidate.end_byte()),
                ),
                rank: rank + candidate.start_byte(),
                visibility: PythonSymbolVisibility::Always,
            });
        }
    };
    visit_tree(node, &mut callback);
    Ok(())
}

fn python_synthetic_symbol_summary(
    normalized_path: &str,
    scope_path: Option<&str>,
    name: &str,
    node_kind: &str,
    origin_type: &str,
    byte_range: (usize, usize),
) -> SymbolSummary {
    let scope_fragment = scope_path.unwrap_or("<module>");
    SymbolSummary::new(
        format!("{normalized_path}::python::{scope_fragment}::{node_kind}::{name}"),
        name.to_string(),
        scope_path.map(str::to_string),
        normalized_path.to_string(),
        node_kind.to_string(),
        origin_type.to_string(),
        byte_range,
        None,
        Vec::new(),
        None,
        None,
    )
}

pub(crate) fn collect_python_references(
    current_path: &Path,
    node: Node<'_>,
    source: &str,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    let bindings = collect_visible_python_import_bindings(current_path, node, source)?;
    let local_bindings = collect_python_local_bindings(current_path, node, source)?;
    collect_python_reference_entries(
        current_path,
        node,
        source,
        &bindings,
        &local_bindings,
        references,
    )
}

fn collect_python_reference_entries(
    current_path: &Path,
    node: Node<'_>,
    source: &str,
    bindings: &BTreeMap<String, PythonImportBinding>,
    local_bindings: &[PythonAccessibleSymbol],
    references: &mut BTreeSet<String>,
) -> Result<()> {
    if node.kind() == "attribute" {
        if let (Some(object_node), Some(attribute_node)) = (
            node.child_by_field_name("object"),
            node.child_by_field_name("attribute"),
        ) {
            if object_node.kind() == "identifier" && attribute_node.kind() == "identifier" {
                let object_name = node_text(object_node, source)?.trim().to_string();
                let attribute_name = node_text(attribute_node, source)?.trim().to_string();
                if let Some(PythonImportBinding::Module { module_name }) =
                    bindings.get(&object_name)
                {
                    references.insert(format!("{module_name}.{attribute_name}"));
                    return Ok(());
                }
            }

            collect_python_reference_entries(
                current_path,
                object_node,
                source,
                bindings,
                local_bindings,
                references,
            )?;
            return Ok(());
        }
    }

    if node.kind() == "identifier" && should_count_python_reference(node, source) {
        let name = node_text(node, source)?.trim().to_string();
        if let Some(binding) = bindings.get(&name) {
            match binding {
                PythonImportBinding::Module { .. } => {
                    references.insert(name);
                }
                PythonImportBinding::Symbol {
                    module_name,
                    symbol_name,
                } => {
                    if let Some(module_name) = module_name {
                        references.insert(format!("{module_name}.{symbol_name}"));
                    } else {
                        references.insert(symbol_name.clone());
                    }
                }
            }
        } else if !is_python_default_parameter_value(node)
            && python_local_binding_visible(local_bindings, &name, node)
        {
            return Ok(());
        } else if python_enclosing_local_binding_should_suppress_reference(
            current_path,
            node,
            source,
            &name,
        )? {
            return Ok(());
        } else {
            references.insert(name);
        }
        return Ok(());
    }

    let child_count = node.child_count();
    for index in 0..child_count {
        if let Some(child) = node.child(index) {
            collect_python_reference_entries(
                current_path,
                child,
                source,
                bindings,
                local_bindings,
                references,
            )?;
        }
    }

    Ok(())
}

fn collect_python_local_bindings(
    current_path: &Path,
    node: Node<'_>,
    source: &str,
) -> Result<Vec<PythonAccessibleSymbol>> {
    let normalized_path = normalize_path(current_path);
    let scope_path = python_binding_scope_path(node, source)?;
    let origin_type = if node.kind() == "module" {
        "module_scope"
    } else {
        "local_scope"
    };

    let mut symbols = Vec::new();
    if node.kind() == "lambda" {
        if node.child_by_field_name("body").is_none() {
            return Ok(Vec::new());
        }
        collect_python_scope_symbols(node, source, &normalized_path, 0, &mut symbols)?;
        return Ok(symbols);
    }

    let body_node = if node.kind() == "module" {
        node
    } else if let Some(body) = node.child_by_field_name("body") {
        body
    } else {
        return Ok(Vec::new());
    };

    let class_visibility =
        (node.kind() == "class_definition").then_some((node.start_byte(), node.end_byte()));
    let mut cursor = body_node.walk();
    for statement in body_node.named_children(&mut cursor) {
        let mut statement_symbols = Vec::new();
        collect_python_statement_symbols(
            statement,
            source,
            &normalized_path,
            scope_path.as_deref(),
            origin_type,
            0,
            &mut statement_symbols,
        )?;
        if let Some(class_range) = class_visibility {
            for symbol in &mut statement_symbols {
                if matches!(symbol.visibility, PythonSymbolVisibility::Always) {
                    symbol.visibility = PythonSymbolVisibility::ClassBodyLocal { class_range };
                }
            }
        }
        symbols.extend(statement_symbols);
    }

    let external_bindings = collect_python_external_binding_names(body_node, source)?;
    if !external_bindings.is_empty() {
        symbols.retain(|symbol| !external_bindings.contains(&symbol.name));
    }
    Ok(symbols)
}

fn collect_python_external_binding_names(
    body_node: Node<'_>,
    source: &str,
) -> Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    collect_python_external_binding_names_in_scope(body_node, source, &mut names)?;
    Ok(names)
}

fn collect_python_external_binding_names_in_scope(
    node: Node<'_>,
    source: &str,
    names: &mut BTreeSet<String>,
) -> Result<()> {
    if matches!(
        node.kind(),
        "function_definition" | "class_definition" | "lambda"
    ) {
        return Ok(());
    }

    if matches!(node.kind(), "global_statement" | "nonlocal_statement") {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() != "identifier" {
                continue;
            }
            if let Ok(name) = node_text(child, source) {
                names.insert(name.trim().to_string());
            }
        }
        return Ok(());
    }

    for index in 0..node.child_count() {
        if let Some(child) = node.child(index) {
            collect_python_external_binding_names_in_scope(child, source, names)?;
        }
    }

    Ok(())
}

fn collect_visible_python_import_bindings(
    current_path: &Path,
    node: Node<'_>,
    source: &str,
) -> Result<BTreeMap<String, PythonImportBinding>> {
    let mut scopes = Vec::new();
    let mut current = Some(node);
    while let Some(candidate) = current {
        if matches!(
            candidate.kind(),
            "module" | "function_definition" | "class_definition" | "lambda"
        ) {
            scopes.push(candidate);
        }
        current = candidate.parent();
    }
    scopes.reverse();

    let mut bindings = BTreeMap::new();
    for scope in scopes {
        collect_python_scope_import_bindings(current_path, scope, source, &mut bindings)?;
    }

    Ok(bindings)
}

fn collect_python_scope_import_bindings(
    current_path: &Path,
    scope_node: Node<'_>,
    source: &str,
    bindings: &mut BTreeMap<String, PythonImportBinding>,
) -> Result<()> {
    let body_node = if scope_node.kind() == "module" {
        scope_node
    } else if let Some(body) = scope_node.child_by_field_name("body") {
        body
    } else {
        return Ok(());
    };

    let mut cursor = body_node.walk();
    for child in body_node.named_children(&mut cursor) {
        collect_python_import_bindings_from_statement(current_path, child, source, bindings)?;
    }
    Ok(())
}

fn collect_python_import_bindings_from_statement(
    current_path: &Path,
    statement_node: Node<'_>,
    source: &str,
    bindings: &mut BTreeMap<String, PythonImportBinding>,
) -> Result<()> {
    match statement_node.kind() {
        "import_statement" => {
            collect_python_import_statement_bindings(statement_node, source, bindings)
        }
        "import_from_statement" => collect_python_import_from_statement_bindings(
            current_path,
            statement_node,
            source,
            bindings,
        ),
        "expression_statement" => {
            let mut cursor = statement_node.walk();
            for child in statement_node.named_children(&mut cursor) {
                collect_python_import_bindings_from_statement(
                    current_path,
                    child,
                    source,
                    bindings,
                )?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn collect_python_import_statement_bindings(
    statement_node: Node<'_>,
    source: &str,
    bindings: &mut BTreeMap<String, PythonImportBinding>,
) -> Result<()> {
    let mut cursor = statement_node.walk();
    for child in statement_node.named_children(&mut cursor) {
        match child.kind() {
            "aliased_import" => {
                let mut alias_cursor = child.walk();
                let named_children = child.named_children(&mut alias_cursor).collect::<Vec<_>>();
                if named_children.len() >= 2 {
                    let module_name = node_text(named_children[0], source)?.trim().to_string();
                    let Some(alias_node) = named_children.last().copied() else {
                        continue;
                    };
                    let alias_name = node_text(alias_node, source)?.trim().to_string();
                    bindings.insert(alias_name, PythonImportBinding::Module { module_name });
                }
            }
            "dotted_name" | "identifier" => {
                let module_name = node_text(child, source)?.trim().to_string();
                let binding_name = python_import_statement_binding_name(&module_name);
                bindings.insert(binding_name, PythonImportBinding::Module { module_name });
            }
            _ => {}
        }
    }
    Ok(())
}

fn collect_python_import_from_statement_bindings(
    current_path: &Path,
    statement_node: Node<'_>,
    source: &str,
    bindings: &mut BTreeMap<String, PythonImportBinding>,
) -> Result<()> {
    let mut cursor = statement_node.walk();
    let named_children = statement_node
        .named_children(&mut cursor)
        .collect::<Vec<_>>();
    let Some(module_node) = named_children.first() else {
        return Ok(());
    };
    let module_name = node_text(*module_node, source)?.trim().to_string();

    for child in named_children.into_iter().skip(1) {
        match child.kind() {
            "aliased_import" => {
                let mut alias_cursor = child.walk();
                let alias_children = child.named_children(&mut alias_cursor).collect::<Vec<_>>();
                if alias_children.len() >= 2 {
                    let imported_name = node_text(alias_children[0], source)?.trim().to_string();
                    let Some(alias_node) = alias_children.last().copied() else {
                        continue;
                    };
                    let alias_name = node_text(alias_node, source)?.trim().to_string();
                    bindings.insert(
                        alias_name,
                        python_import_from_binding(current_path, &module_name, &imported_name),
                    );
                }
            }
            "dotted_name" | "identifier" => {
                let imported_name = node_text(child, source)?.trim().to_string();
                let binding_name = python_imported_symbol_name(&imported_name);
                bindings.insert(
                    binding_name.clone(),
                    python_import_from_binding(current_path, &module_name, &imported_name),
                );
            }
            _ => {}
        }
    }

    Ok(())
}

fn python_import_from_binding(
    current_path: &Path,
    module_name: &str,
    imported_name: &str,
) -> PythonImportBinding {
    let imported_symbol_name = python_imported_symbol_name(imported_name);
    let module_candidate = join_python_module_reference(module_name, imported_name);
    if resolve_local_python_module_path(current_path, &module_candidate).is_some() {
        PythonImportBinding::Module {
            module_name: module_candidate,
        }
    } else {
        PythonImportBinding::Symbol {
            module_name: Some(module_name.to_string()),
            symbol_name: imported_symbol_name,
        }
    }
}

fn python_import_statement_binding_name(module_name: &str) -> String {
    module_name
        .split('.')
        .next()
        .unwrap_or(module_name)
        .to_string()
}

fn python_imported_symbol_name(imported_name: &str) -> String {
    imported_name
        .rsplit('.')
        .next()
        .unwrap_or(imported_name)
        .to_string()
}

fn join_python_module_reference(module_name: &str, imported_name: &str) -> String {
    if module_name.is_empty() {
        imported_name.to_string()
    } else if module_name.ends_with('.') {
        format!("{module_name}{imported_name}")
    } else {
        format!("{module_name}.{imported_name}")
    }
}

fn should_count_python_reference(node: Node<'_>, source: &str) -> bool {
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

    if is_python_with_target_name(node, source) {
        return false;
    }

    if is_python_except_target_name(node, source) {
        return false;
    }

    if is_python_match_capture_name(node, source) {
        return false;
    }

    if is_python_match_keyword_name(node) {
        return false;
    }

    if matches!(parent.kind(), "import_statement" | "import_from_statement") {
        return false;
    }

    if has_python_type_annotation_ancestor(node) {
        return false;
    }

    if is_python_parameter_name(node) {
        return false;
    }

    if is_python_parameter_declaration_node(node) {
        return false;
    }

    if is_python_named_expression_target(node) {
        return false;
    }

    if matches!(
        parent.kind(),
        "list_splat_pattern" | "dictionary_splat_pattern" | "tuple_pattern"
    ) {
        return false;
    }

    if let Some(left) = parent.child_by_field_name("left") {
        if matches!(
            parent.kind(),
            "assignment" | "augmented_assignment" | "for_statement" | "for_in_clause"
        ) && contains_node(left, node)
        {
            return false;
        }
    }

    true
}

fn has_python_type_annotation_ancestor(node: Node<'_>) -> bool {
    let mut current = node.parent();

    while let Some(candidate) = current {
        if candidate.kind() == "type" {
            return true;
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

fn is_python_parameter_name(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if !PYTHON_PARAMETER_KINDS.contains(&parent.kind()) {
        return false;
    }

    parent
        .child_by_field_name("name")
        .is_some_and(|candidate| candidate.id() == node.id())
}

fn is_python_parameter_symbol_name(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if !PYTHON_PARAMETER_KINDS.contains(&parent.kind()) {
        return false;
    }

    parent
        .child_by_field_name("value")
        .is_none_or(|value| !contains_node(value, node))
        && !has_python_type_annotation_ancestor(node)
}

fn is_python_parameter_declaration_node(node: Node<'_>) -> bool {
    let mut current = node.parent();

    while let Some(candidate) = current {
        if candidate.kind() == "default_parameter" || candidate.kind() == "typed_default_parameter"
        {
            if let Some(value) = candidate.child_by_field_name("value") {
                return !contains_node(value, node);
            }
            return true;
        }

        if matches!(candidate.kind(), "parameters" | "lambda_parameters") {
            return true;
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

fn is_python_named_expression_target(node: Node<'_>) -> bool {
    let mut current = node.parent();

    while let Some(candidate) = current {
        if candidate.kind() == "named_expression" {
            return candidate
                .child_by_field_name("name")
                .is_some_and(|left| contains_node(left, node));
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

fn is_python_default_parameter_value(node: Node<'_>) -> bool {
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

fn is_python_with_target_name(node: Node<'_>, source: &str) -> bool {
    let mut current = node.parent();

    while let Some(candidate) = current {
        if matches!(candidate.kind(), "as_pattern" | "with_item") {
            if let Some(alias) = candidate.child_by_field_name("alias") {
                return contains_node(alias, node);
            }
            if source
                .get(candidate.start_byte()..node.start_byte())
                .is_some_and(|prefix| prefix.contains(" as "))
            {
                return true;
            }
        }

        if matches!(
            candidate.kind(),
            "with_statement" | "function_definition" | "class_definition" | "module"
        ) {
            return false;
        }

        current = candidate.parent();
    }

    false
}

fn is_python_except_target_name(node: Node<'_>, source: &str) -> bool {
    let mut current = node.parent();

    while let Some(candidate) = current {
        if candidate.kind() == "except_clause" {
            if node.kind() == "as_pattern_target" {
                return true;
            }
            return is_python_as_pattern_alias(node, candidate, source);
        }

        if matches!(
            candidate.kind(),
            "try_statement" | "function_definition" | "class_definition" | "module"
        ) {
            return false;
        }

        current = candidate.parent();
    }

    false
}

fn python_match_capture_names(case_pattern: Node<'_>, source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = case_pattern.walk();
    for child in case_pattern.named_children(&mut cursor) {
        python_collect_direct_match_capture_names(child, source, &mut names);
    }
    names
}

fn python_splat_pattern_capture_name(splat_pattern: Node<'_>, source: &str) -> Option<String> {
    let identifier = only_named_child(splat_pattern)?;
    let name = node_text(identifier, source).ok()?.trim();
    is_python_capture_name_text(name).then(|| name.to_string())
}

fn python_as_pattern_alias_name(as_pattern: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = as_pattern.walk();
    as_pattern
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "identifier" || child.kind() == "as_pattern_target")
        .last()
        .and_then(|alias| node_text(alias, source).ok())
        .map(str::trim)
        .filter(|name| is_python_capture_name_text(name))
        .map(str::to_string)
}

fn python_collect_direct_match_capture_names(
    node: Node<'_>,
    source: &str,
    names: &mut Vec<String>,
) {
    match node.kind() {
        "case_pattern" => {}
        "as_pattern" => {
            if let Some(name) = python_as_pattern_alias_name(node, source) {
                push_python_match_capture_name(names, &name);
            }
        }
        "keyword_pattern" => {
            let mut cursor = node.walk();
            if let Some(value) = node.named_children(&mut cursor).last() {
                python_collect_direct_match_capture_names(value, source, names);
            }
        }
        "splat_pattern" => {
            if let Some(name) = python_splat_pattern_capture_name(node, source) {
                push_python_match_capture_name(names, &name);
            }
        }
        "pattern" => {
            if let Ok(name) = node_text(node, source) {
                push_python_match_capture_name(names, name.trim());
            }
        }
        "dotted_name" => {
            if let Some(identifier) = only_named_child(node) {
                if let Ok(name) = node_text(identifier, source) {
                    push_python_match_capture_name(names, name.trim());
                }
            }
        }
        "class_pattern" => {
            let mut cursor = node.walk();
            for (index, child) in node.named_children(&mut cursor).enumerate() {
                if index == 0 || child.kind() == "case_pattern" {
                    continue;
                }
                python_collect_direct_match_capture_names(child, source, names);
            }
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if child.kind() == "case_pattern" {
                    continue;
                }
                python_collect_direct_match_capture_names(child, source, names);
            }
        }
    }
}

fn push_python_match_capture_name(names: &mut Vec<String>, name: &str) {
    if !is_python_capture_name_text(name) {
        return;
    }
    if !names.iter().any(|existing| existing == name) {
        names.push(name.to_string());
    }
}

fn python_enclosing_case_clause(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate.kind() == "case_clause" {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

fn python_enclosing_except_clause(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate.kind() == "except_clause" {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

fn python_enclosing_comprehension(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if matches!(
            candidate.kind(),
            "list_comprehension"
                | "set_comprehension"
                | "dictionary_comprehension"
                | "generator_expression"
        ) {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

fn python_comprehension_visible_clause_count(
    comprehension: Node<'_>,
    reference_node: Node<'_>,
) -> Option<usize> {
    if !contains_node(comprehension, reference_node) {
        return None;
    }

    if comprehension
        .child_by_field_name("body")
        .is_some_and(|body| contains_node(body, reference_node))
    {
        let mut total_for_clauses = 0usize;
        let mut cursor = comprehension.walk();
        for child in comprehension.named_children(&mut cursor) {
            if child.kind() == "for_in_clause" {
                total_for_clauses += 1;
            }
        }
        return Some(total_for_clauses);
    }

    let mut completed_for_clauses = 0usize;
    let mut cursor = comprehension.walk();
    for child in comprehension.named_children(&mut cursor) {
        if child.kind() == "for_in_clause" {
            if contains_node(child, reference_node) {
                return Some(completed_for_clauses);
            }
            completed_for_clauses += 1;
            continue;
        }

        if child.kind() == "if_clause" && contains_node(child, reference_node) {
            return Some(completed_for_clauses);
        }
    }

    None
}

fn python_comprehension_part_index(comprehension: Node<'_>, node: Node<'_>) -> Option<usize> {
    if !contains_node(comprehension, node) {
        return None;
    }

    let mut part_index = 0usize;
    let mut cursor = comprehension.walk();
    for child in comprehension.named_children(&mut cursor) {
        if child.kind() == "for_in_clause" {
            if child
                .child_by_field_name("right")
                .is_some_and(|right| contains_node(right, node))
            {
                return Some(part_index);
            }
            part_index += 1;
            continue;
        }

        if child.kind() == "if_clause" {
            if contains_node(child, node) {
                return Some(part_index);
            }
            part_index += 1;
            continue;
        }
    }

    comprehension
        .child_by_field_name("body")
        .filter(|body| contains_node(*body, node))
        .map(|_| part_index)
}

fn python_accessible_symbol_resolves_at(
    symbol: &PythonAccessibleSymbol,
    reference_node: Node<'_>,
) -> bool {
    match symbol.visibility {
        PythonSymbolVisibility::Always => true,
        PythonSymbolVisibility::ClassBodyLocal { class_range } => {
            python_reference_uses_direct_class_scope(reference_node, class_range)
        }
        PythonSymbolVisibility::NamedExpression {
            expression_range,
            comprehension_range,
            comprehension_part_index,
        } => {
            if let (Some(expected_range), Some(named_part_index)) =
                (comprehension_range, comprehension_part_index)
            {
                if python_enclosing_comprehension(reference_node).is_some_and(|comprehension| {
                    (comprehension.start_byte(), comprehension.end_byte()) == expected_range
                        && python_comprehension_part_index(comprehension, reference_node)
                            .is_some_and(|reference_part_index| {
                                reference_part_index > named_part_index
                                    || (reference_part_index == named_part_index
                                        && reference_node.start_byte() > expression_range.1)
                            })
                }) {
                    return true;
                }
            }

            reference_node.start_byte() > expression_range.1
        }
        PythonSymbolVisibility::ComprehensionTarget {
            comprehension_range,
            clause_index,
        } => python_enclosing_comprehension(reference_node).is_some_and(|comprehension| {
            (comprehension.start_byte(), comprehension.end_byte()) == comprehension_range
                && python_comprehension_visible_clause_count(comprehension, reference_node)
                    .is_some_and(|visible_clause_count| clause_index < visible_clause_count)
        }),
        PythonSymbolVisibility::ExceptTarget {
            except_clause_range,
            ..
        } => {
            let start = reference_node.start_byte();
            let end = reference_node.end_byte();
            start >= except_clause_range.0 && end <= except_clause_range.1
        }
        PythonSymbolVisibility::MatchCapture {
            case_clause_range,
            match_statement_end,
        } => {
            let start = reference_node.start_byte();
            let end = reference_node.end_byte();
            (start >= case_clause_range.0 && end <= case_clause_range.1)
                || start > match_statement_end
        }
    }
}

fn python_accessible_symbol_suppresses_at(
    symbol: &PythonAccessibleSymbol,
    reference_node: Node<'_>,
) -> bool {
    match symbol.visibility {
        PythonSymbolVisibility::Always => true,
        PythonSymbolVisibility::ClassBodyLocal { class_range } => {
            python_reference_uses_direct_class_scope(reference_node, class_range)
        }
        PythonSymbolVisibility::NamedExpression { .. } => true,
        PythonSymbolVisibility::ComprehensionTarget { .. } => {
            python_accessible_symbol_resolves_at(symbol, reference_node)
        }
        PythonSymbolVisibility::ExceptTarget {
            except_clause_range: _,
        } => true,
        PythonSymbolVisibility::MatchCapture { .. } => true,
    }
}

fn python_local_binding_visible(
    local_bindings: &[PythonAccessibleSymbol],
    name: &str,
    reference_node: Node<'_>,
) -> bool {
    local_bindings.iter().any(|binding| {
        binding.name == name && python_accessible_symbol_suppresses_at(binding, reference_node)
    })
}

fn python_enclosing_local_binding_should_suppress_reference(
    current_path: &Path,
    reference_node: Node<'_>,
    source: &str,
    name: &str,
) -> Result<bool> {
    if python_reference_is_global_declared(reference_node, source, name) {
        return Ok(false);
    }

    let normalized_path = normalize_path(current_path);
    let mut candidates = Vec::new();
    let mut seen_scope = false;
    let include_immediate_scope = is_python_decorator_expression(reference_node);
    let mut scope_rank = 2_000_000usize;
    let mut current = reference_node.parent();

    while let Some(node) = current {
        let include_scope = match node.kind() {
            "lambda" => {
                seen_scope = true;
                true
            }
            "list_comprehension"
            | "set_comprehension"
            | "dictionary_comprehension"
            | "generator_expression" => {
                seen_scope = true;
                false
            }
            "function_definition" | "class_definition" | "module" => {
                if seen_scope {
                    true
                } else {
                    seen_scope = true;
                    include_immediate_scope
                }
            }
            _ => false,
        };

        if include_scope {
            collect_python_scope_symbols(
                node,
                source,
                &normalized_path,
                scope_rank,
                &mut candidates,
            )?;
            scope_rank = scope_rank.saturating_sub(1_000_000);
        }

        current = node.parent();
    }

    candidates.retain(|candidate| {
        candidate.name == name && python_accessible_symbol_suppresses_at(candidate, reference_node)
    });
    candidates.sort_by(|left, right| {
        right
            .rank
            .cmp(&left.rank)
            .then_with(|| left.summary.symbol_id.cmp(&right.summary.symbol_id))
    });

    let Some(best) = candidates.first() else {
        return Ok(false);
    };

    Ok(!matches!(
        best.summary.node_kind.as_str(),
        "function_definition" | "class_definition"
    ))
}

fn is_python_decorator_expression(node: Node<'_>) -> bool {
    let mut current = Some(node);

    while let Some(candidate) = current {
        if candidate.kind() == "decorator" {
            return true;
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

fn is_python_class_header_expression(node: Node<'_>) -> bool {
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

fn python_reference_is_global_declared(node: Node<'_>, source: &str, name: &str) -> bool {
    python_nearest_scope_node(node).is_some_and(|scope| {
        python_scope_declares_external_name(scope, source, name, "global_statement")
    })
}

fn python_reference_uses_direct_class_scope(
    reference_node: Node<'_>,
    class_range: (usize, usize),
) -> bool {
    let mut skipped_current_class_scope = false;
    let mut skipped_current_function_scope = false;
    let skip_current_class_scope = is_python_class_header_expression(reference_node);
    let skip_current_function_scope = is_python_default_parameter_value(reference_node);
    let mut current = Some(reference_node);
    while let Some(candidate) = current {
        if skip_current_class_scope
            && !skipped_current_class_scope
            && candidate.kind() == "class_definition"
        {
            skipped_current_class_scope = true;
            current = candidate.parent();
            continue;
        }

        if candidate.kind() == "class_definition"
            && (candidate.start_byte(), candidate.end_byte()) == class_range
        {
            return true;
        }

        if skip_current_function_scope
            && !skipped_current_function_scope
            && matches!(candidate.kind(), "function_definition" | "lambda")
        {
            skipped_current_function_scope = true;
            current = candidate.parent();
            continue;
        }

        if matches!(
            candidate.kind(),
            "function_definition"
                | "lambda"
                | "list_comprehension"
                | "set_comprehension"
                | "dictionary_comprehension"
                | "generator_expression"
                | "class_definition"
        ) {
            return false;
        }

        current = candidate.parent();
    }

    false
}

fn python_nearest_scope_node(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = Some(node);
    while let Some(candidate) = current {
        if matches!(
            candidate.kind(),
            "function_definition" | "class_definition" | "module" | "lambda"
        ) {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

fn python_binding_scope_path(scope_node: Node<'_>, source: &str) -> Result<Option<String>> {
    if scope_node.kind() == "module" {
        return Ok(None);
    }

    if matches!(
        scope_node.kind(),
        "function_definition" | "class_definition"
    ) {
        return Ok(Some(semantic_path(scope_node, source)?));
    }

    if scope_node.kind() == "lambda" {
        let mut current = scope_node.parent();
        while let Some(candidate) = current {
            if candidate.kind() == "module" {
                return Ok(None);
            }
            if matches!(candidate.kind(), "function_definition" | "class_definition") {
                return Ok(Some(semantic_path(candidate, source)?));
            }
            current = candidate.parent();
        }
    }

    Ok(None)
}

fn python_scope_declares_external_name(
    scope_node: Node<'_>,
    source: &str,
    name: &str,
    statement_kind: &str,
) -> bool {
    let body_node = if scope_node.kind() == "module" {
        scope_node
    } else if let Some(body) = scope_node.child_by_field_name("body") {
        body
    } else {
        return false;
    };

    python_scope_declares_external_name_in_scope(body_node, source, name, statement_kind)
}

fn python_scope_declares_external_name_in_scope(
    node: Node<'_>,
    source: &str,
    name: &str,
    statement_kind: &str,
) -> bool {
    if matches!(
        node.kind(),
        "function_definition" | "class_definition" | "lambda"
    ) {
        return false;
    }

    if node.kind() == statement_kind {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() != "identifier" {
                continue;
            }
            if node_text(child, source)
                .ok()
                .is_some_and(|text| text.trim() == name)
            {
                return true;
            }
        }
        return false;
    }

    for index in 0..node.child_count() {
        if let Some(child) = node.child(index) {
            if python_scope_declares_external_name_in_scope(child, source, name, statement_kind) {
                return true;
            }
        }
    }

    false
}

fn only_named_child(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    let mut children = node.named_children(&mut cursor);
    let child = children.next()?;
    children.next().is_none().then_some(child)
}

fn is_python_capture_name_text(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first == '_' && chars.clone().next().is_some()
        || first.is_ascii_alphabetic() && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn is_python_match_capture_name(node: Node<'_>, source: &str) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    let mut current = Some(parent);
    while let Some(candidate) = current {
        if candidate.kind() == "case_pattern" {
            return python_match_capture_names(candidate, source)
                .into_iter()
                .any(|name| {
                    node_text(node, source)
                        .ok()
                        .is_some_and(|node_name| node_name.trim() == name)
                });
        }

        if matches!(
            candidate.kind(),
            "case_clause" | "function_definition" | "class_definition" | "module"
        ) {
            return false;
        }

        current = candidate.parent();
    }

    false
}

fn is_python_match_keyword_name(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if parent.kind() != "keyword_pattern" {
        return false;
    }

    let mut cursor = parent.walk();
    parent
        .named_children(&mut cursor)
        .next()
        .is_some_and(|keyword| keyword.id() == node.id())
}

fn is_python_as_pattern_alias(node: Node<'_>, ancestor: Node<'_>, source: &str) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if parent.kind() != "as_pattern" || !contains_node(ancestor, parent) {
        return false;
    }

    let Some(pattern_text) = source.get(parent.start_byte()..parent.end_byte()) else {
        return false;
    };
    let Some(as_index) = pattern_text.rfind(" as ") else {
        return false;
    };
    let relative_start = node.start_byte().saturating_sub(parent.start_byte());
    relative_start > as_index
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
        let scope_path = semantic_parent_path(&semantic_path);

        symbols.push(CAccessibleSymbol {
            name,
            summary: SymbolSummary::new(
                symbol_id,
                semantic_path,
                scope_path,
                normalized_path.clone(),
                child.kind().to_string(),
                origin_type.to_string(),
                (child.start_byte(), child.end_byte()),
                c_candidate_signature(child, source)?,
                c_parameters(child, source)?,
                c_return_type(child, source)?,
                None,
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
    "ArithmeticError",
    "AssertionError",
    "AttributeError",
    "BaseException",
    "Exception",
    "ImportError",
    "IndexError",
    "KeyError",
    "LookupError",
    "NameError",
    "OSError",
    "RuntimeError",
    "StopIteration",
    "SyntaxError",
    "TypeError",
    "ValueError",
    "ZeroDivisionError",
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
