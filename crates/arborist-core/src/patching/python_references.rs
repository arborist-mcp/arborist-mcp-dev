use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::python_bindings::{
    PythonAccessibleSymbol, PythonSymbolVisibility, collect_python_local_bindings,
    collect_python_scope_symbols,
};
use super::python_imports::{
    collect_visible_python_import_bindings, resolve_local_python_imported_symbol,
};
use super::python_patterns::{
    is_python_as_pattern_alias, is_python_match_capture_name, is_python_match_keyword_name,
};
use super::python_visibility::{
    python_accessible_symbol_resolves_at, python_accessible_symbol_suppresses_at,
    python_local_binding_visible,
};
use super::{
    PythonImportBinding, ReferenceValidation, ambiguous_binding_decision,
    is_python_class_header_expression, is_python_default_parameter_value,
    resolved_binding_decision, unresolved_binding_decision,
};
use crate::language::{contains_node, is_field_node, node_text, normalize_path};
use crate::model::{DisambiguationContext, ValidationAmbiguity, ValidationBinding};

#[derive(Debug, Clone)]
struct PythonReferenceTarget<'tree> {
    name: String,
    node: Node<'tree>,
    imported_symbol: Option<(String, String)>,
    import_fallback_name: Option<String>,
}

pub(super) fn collect_python_reference_validation(
    path: &Path,
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
    if node.kind() == "attribute"
        && let (Some(object_node), Some(attribute_node)) = (
            node.child_by_field_name("object"),
            node.child_by_field_name("attribute"),
        )
    {
        if object_node.kind() == "identifier" && attribute_node.kind() == "identifier" {
            let object_name = node_text(object_node, source)?.trim().to_string();
            let attribute_name = node_text(attribute_node, source)?.trim().to_string();
            if let Some(PythonImportBinding::Module { module_name }) = bindings.get(&object_name) {
                let display_name = format!("{object_name}.{attribute_name}");
                references.push(PythonReferenceTarget {
                    name: display_name,
                    node,
                    imported_symbol: Some((module_name.clone(), attribute_name)),
                    import_fallback_name: Some(object_name),
                });
                return Ok(());
            }
        }

        collect_python_reference_targets_inner(object_node, source, bindings, references)?;
        return Ok(());
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
    if let Some((module_name, symbol_name)) = &reference_target.imported_symbol
        && let Some(summary) = resolve_local_python_imported_symbol(path, module_name, symbol_name)?
    {
        return Ok(vec![PythonAccessibleSymbol {
            name: reference_target.name.clone(),
            summary,
            rank: 4_000_000,
            visibility: PythonSymbolVisibility::Always,
        }]);
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
    if node.kind() == "attribute"
        && let (Some(object_node), Some(attribute_node)) = (
            node.child_by_field_name("object"),
            node.child_by_field_name("attribute"),
        )
    {
        if object_node.kind() == "identifier" && attribute_node.kind() == "identifier" {
            let object_name = node_text(object_node, source)?.trim().to_string();
            let attribute_name = node_text(attribute_node, source)?.trim().to_string();
            if let Some(PythonImportBinding::Module { module_name }) = bindings.get(&object_name) {
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
        } else if (!is_python_default_parameter_value(node)
            && python_local_binding_visible(local_bindings, &name, node))
            || python_enclosing_local_binding_should_suppress_reference(
                current_path,
                node,
                source,
                &name,
            )?
        {
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

    if let Some(left) = parent.child_by_field_name("left")
        && matches!(
            parent.kind(),
            "assignment" | "augmented_assignment" | "for_statement" | "for_in_clause"
        )
        && contains_node(left, node)
    {
        return false;
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

pub(super) fn is_python_parameter_symbol_name(node: Node<'_>) -> bool {
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

pub(super) fn is_python_with_target_name(node: Node<'_>, source: &str) -> bool {
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

pub(super) fn python_enclosing_except_clause(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate.kind() == "except_clause" {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
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

fn python_reference_is_global_declared(node: Node<'_>, source: &str, name: &str) -> bool {
    python_nearest_scope_node(node).is_some_and(|scope| {
        super::python_bindings::python_scope_declares_external_name(
            scope,
            source,
            name,
            "global_statement",
        )
    })
}

pub(super) fn python_nearest_scope_node(node: Node<'_>) -> Option<Node<'_>> {
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
