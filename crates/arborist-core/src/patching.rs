mod api;
mod c_validation;
mod commit_gate;
mod python_bindings;
mod python_imports;
mod python_patterns;
mod python_references;
mod python_visibility;

pub(crate) use c_validation::{
    collect_c_call_arities, collect_c_graph_references, collect_c_reference_validation,
    collect_cpp_call_arities,
};
pub(crate) use commit_gate::evaluate_patch_commit_gate;
pub(crate) use python_imports::{
    resolve_local_python_imported_symbol, resolve_local_python_module_path,
};
pub(crate) use python_references::collect_python_references;

pub(crate) use api::unified_diff;
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

pub(crate) struct PreparedPatchReplacement {
    pub(crate) start_byte: usize,
    pub(crate) end_byte: usize,
    pub(crate) replacement: String,
    pub(crate) validation_issues: Vec<ValidationIssue>,
}

struct SemanticTargetInfo {
    language_id: LanguageId,
    start_byte: usize,
    end_byte: usize,
    node_kind: String,
    start_point: Position,
    end_point: Position,
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
        LanguageId::C | LanguageId::Cpp => c_symbol_id_for_node(&path, symbol_node, source)?
            .ok_or_else(|| anyhow!("position does not resolve to a C symbol id")),
    }
}

fn semantic_target_info(
    path: &Path,
    source: &str,
    semantic_target: &str,
) -> Result<SemanticTargetInfo> {
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

    Ok(SemanticTargetInfo {
        language_id: document.language_id,
        start_byte: target_node.start_byte(),
        end_byte: target_node.end_byte(),
        node_kind: target_node.kind().to_string(),
        start_point: position_from(target_node.start_position()),
        end_point: position_from(target_node.end_position()),
    })
}

pub(crate) fn prepare_patch_replacement(
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
) -> Result<PreparedPatchReplacement> {
    let target = semantic_target_info(path, source, semantic_target)?;
    let replacement = match target.language_id {
        LanguageId::Python => normalize_python_replacement_indentation(
            source,
            target.start_byte,
            target.end_byte,
            new_code,
        ),
        LanguageId::C | LanguageId::Cpp => new_code.to_string(),
    };
    let mut validation_issues = Vec::new();
    if target.language_id == LanguageId::Python
        && target.node_kind == "decorated_definition"
        && !python_replacement_starts_with_decorator(&replacement)
    {
        validation_issues.push(ValidationIssue {
            kind: "decorator_guard".to_string(),
            message: "replacement would remove existing Python decorator(s); include decorators in new_code or provide an explicit bypass_reason".to_string(),
            start_byte: target.start_byte,
            end_byte: target.end_byte,
            start_point: target.start_point,
            end_point: target.end_point,
        });
    }

    Ok(PreparedPatchReplacement {
        start_byte: target.start_byte,
        end_byte: target.end_byte,
        replacement,
        validation_issues,
    })
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

fn locate_patched_symbol<'tree>(
    document: &'tree ParsedDocument,
    source: &str,
    patch_start: usize,
    replacement_len: usize,
) -> Option<Node<'tree>> {
    let patch_end = replacement_content_end(source, patch_start, replacement_len)?;
    let root = document.tree.root_node();
    let descendant = root
        .named_descendant_for_byte_range(patch_start, patch_end)
        .or_else(|| root.named_descendant_for_byte_range(patch_start, patch_start))?;
    ascend_to_symbol(document.language_id, descendant)
}

fn replacement_content_end(
    source: &str,
    patch_start: usize,
    replacement_len: usize,
) -> Option<usize> {
    let patch_end = patch_start.checked_add(replacement_len)?;
    let replacement = source.get(patch_start..patch_end)?;
    let content_len = replacement.trim_end().len();
    if content_len == 0 {
        return Some(patch_start);
    }
    Some(patch_start + content_len - 1)
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
        LanguageId::C | LanguageId::Cpp => c_semantic_path(path, node, source)?
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
        LanguageId::C | LanguageId::Cpp => c_symbol_id_for_node(path, node, source)?
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
    if root.kind() == "module" {
        issues.extend(collect_python_indentation_issues(source));
    }
    issues
}

fn normalize_python_replacement_indentation(
    source: &str,
    target_start: usize,
    target_end: usize,
    new_code: &str,
) -> String {
    let normalized_line_endings = normalize_line_endings(new_code, source_line_ending(source));
    let dedented = dedent_python_replacement(&normalized_line_endings);
    let ambient_indent = python_target_ambient_indent(source, target_start);
    let indent_unit = python_target_indent_unit(source, target_start, target_end)
        .or_else(|| infer_python_indent_unit(&dedented))
        .unwrap_or_else(|| ambient_indent.clone());

    if indent_unit.is_empty() {
        return reindent_python_replacement(&dedented, &ambient_indent);
    }

    reindent_python_replacement_with_unit(&dedented, &ambient_indent, &indent_unit)
}

fn dedent_python_replacement(new_code: &str) -> String {
    let indent = split_preserving_newline(new_code)
        .iter()
        .filter_map(|line| {
            let content = line.trim_end_matches(['\r', '\n']);
            (!content.trim().is_empty()).then(|| leading_indent_len(content))
        })
        .min()
        .unwrap_or(0);

    if indent == 0 {
        return new_code.to_string();
    }

    let mut dedented = String::with_capacity(new_code.len());
    for line in split_preserving_newline(new_code) {
        let remove = indent.min(leading_indent_len(line));
        dedented.push_str(&line[remove..]);
    }
    dedented
}

fn reindent_python_replacement(replacement: &str, ambient_indent: &str) -> String {
    let mut adjusted = String::with_capacity(replacement.len() + ambient_indent.len());
    for (index, line) in split_preserving_newline(replacement)
        .into_iter()
        .enumerate()
    {
        if index > 0 && !line.trim().is_empty() {
            adjusted.push_str(ambient_indent);
        }
        adjusted.push_str(line);
    }
    adjusted
}

fn reindent_python_replacement_with_unit(
    replacement: &str,
    ambient_indent: &str,
    indent_unit: &str,
) -> String {
    let indent_step = infer_python_indent_step(replacement);
    if indent_step == 0 {
        return reindent_python_replacement(replacement, ambient_indent);
    }

    let mut adjusted = String::with_capacity(
        replacement.len() + ambient_indent.len() + indent_unit.len() * replacement.lines().count(),
    );
    for (index, line) in split_preserving_newline(replacement)
        .into_iter()
        .enumerate()
    {
        let (content, newline) = split_line_ending(line);
        if content.trim().is_empty() {
            adjusted.push_str(content);
            adjusted.push_str(newline);
            continue;
        }

        let leading = leading_indent_len(content);
        let depth = leading / indent_step;
        if index > 0 {
            adjusted.push_str(ambient_indent);
        }
        for _ in 0..depth {
            adjusted.push_str(indent_unit);
        }
        adjusted.push_str(&content[leading..]);
        adjusted.push_str(newline);
    }
    adjusted
}

fn python_target_ambient_indent(source: &str, target_start: usize) -> String {
    let line_start = source[..target_start]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let prefix = &source[line_start..target_start];
    if prefix.chars().all(|ch| ch == ' ' || ch == '\t') {
        prefix.to_string()
    } else {
        String::new()
    }
}

fn python_target_indent_unit(
    source: &str,
    target_start: usize,
    target_end: usize,
) -> Option<String> {
    let base_indent = python_target_ambient_indent(source, target_start);
    let target_text = &source[target_start..target_end];
    for line in split_preserving_newline(target_text).into_iter().skip(1) {
        let (content, _) = split_line_ending(line);
        if content.trim().is_empty() {
            continue;
        }
        let indent_len = leading_indent_len(content);
        if indent_len > base_indent.len() && content.starts_with(&base_indent) {
            return Some(content[base_indent.len()..indent_len].to_string());
        }
    }
    None
}

fn infer_python_indent_unit(replacement: &str) -> Option<String> {
    for line in split_preserving_newline(replacement) {
        let (content, _) = split_line_ending(line);
        if content.trim().is_empty() {
            continue;
        }
        let indent_len = leading_indent_len(content);
        if indent_len > 0 {
            return Some(content[..indent_len].to_string());
        }
    }
    None
}

fn infer_python_indent_step(replacement: &str) -> usize {
    let mut step = 0usize;
    for line in split_preserving_newline(replacement) {
        let (content, _) = split_line_ending(line);
        if content.trim().is_empty() {
            continue;
        }
        let indent_len = leading_indent_len(content);
        if indent_len == 0 {
            continue;
        }
        step = if step == 0 {
            indent_len
        } else {
            gcd(step, indent_len)
        };
        if step == 1 {
            break;
        }
    }
    step
}

fn source_line_ending(source: &str) -> &'static str {
    if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn normalize_line_endings(value: &str, line_ending: &str) -> String {
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    if line_ending == "\n" {
        normalized
    } else {
        normalized.replace('\n', line_ending)
    }
}

fn python_replacement_starts_with_decorator(replacement: &str) -> bool {
    replacement
        .lines()
        .map(str::trim_start)
        .find(|line| !line.trim().is_empty())
        .is_some_and(|line| line.starts_with('@'))
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(body) = line.strip_suffix("\r\n") {
        (body, "\r\n")
    } else if let Some(body) = line.strip_suffix('\n') {
        (body, "\n")
    } else {
        (line, "")
    }
}

fn split_preserving_newline(value: &str) -> Vec<&str> {
    if value.is_empty() {
        return vec![""];
    }

    let mut lines = value.split_inclusive('\n').collect::<Vec<_>>();
    if !value.ends_with('\n')
        && let Some(last_newline) = value.rfind('\n')
        && last_newline + 1 < value.len()
        && lines.is_empty()
    {
        lines.push(&value[last_newline + 1..]);
    }
    lines
}

fn leading_indent_len(line: &str) -> usize {
    line.as_bytes()
        .iter()
        .take_while(|byte| **byte == b' ' || **byte == b'\t')
        .count()
}

fn gcd(mut left: usize, mut right: usize) -> usize {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn collect_python_indentation_issues(source: &str) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let mut pending_block: Option<(usize, usize, usize)> = None;
    let mut byte_start = 0usize;

    for (row, line) in source.split_inclusive('\n').enumerate() {
        let content = line.trim_end_matches(['\r', '\n']);
        let trimmed = content.trim();
        let indent = leading_indent_len(content);

        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            if let Some((header_indent, header_row, header_start)) = pending_block.take()
                && indent <= header_indent
            {
                issues.push(ValidationIssue {
                    kind: "indentation".to_string(),
                    message: format!(
                        "Python indentation appears invalid: expected an indented block after line {}",
                        header_row + 1
                    ),
                    start_byte: byte_start,
                    end_byte: byte_start + content.len(),
                    start_point: Position {
                        row,
                        column: 0,
                    },
                    end_point: Position {
                        row,
                        column: content.len(),
                    },
                });
                pending_block = Some((header_indent, header_row, header_start));
            }

            if trimmed.ends_with(':') {
                pending_block = Some((indent, row, byte_start));
            }
        }

        byte_start += line.len();
    }

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
