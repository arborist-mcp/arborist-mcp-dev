mod candidates;
mod filters;
mod targets;

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::python_bindings::{
    collect_python_local_bindings, collect_python_local_bindings_with_deadline,
};
use super::python_imports::{
    collect_visible_python_import_bindings, collect_visible_python_import_bindings_with_deadline,
};
use super::{
    ReferenceValidation, ambiguous_binding_decision, resolved_binding_decision,
    unresolved_binding_decision,
};
use crate::language::normalize_path;
use crate::model::{DisambiguationContext, ValidationAmbiguity, ValidationBinding};
use crate::workspace_scan::WorkspaceScanDeadline;

use self::candidates::python_binding_candidates_for_reference;
pub(super) use self::filters::{
    is_python_parameter_symbol_name, is_python_with_target_name, python_enclosing_except_clause,
    python_nearest_scope_node,
};
use self::targets::{
    collect_python_instance_type_bindings, collect_python_instance_type_bindings_with_deadline,
    collect_python_reference_entries, collect_python_reference_entries_with_deadline,
    collect_python_reference_targets,
};

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

pub(crate) fn collect_python_references(
    current_path: &Path,
    node: Node<'_>,
    source: &str,
    references: &mut BTreeSet<String>,
) -> Result<()> {
    collect_python_references_with_deadline(current_path, node, source, references, None)
}

pub(crate) fn collect_python_references_with_deadline(
    current_path: &Path,
    node: Node<'_>,
    source: &str,
    references: &mut BTreeSet<String>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<()> {
    let bindings = match deadline {
        Some(deadline) => collect_visible_python_import_bindings_with_deadline(
            current_path,
            node,
            source,
            Some(deadline),
        )?,
        None => collect_visible_python_import_bindings(current_path, node, source)?,
    };
    let local_bindings = match deadline {
        Some(deadline) => {
            collect_python_local_bindings_with_deadline(current_path, node, source, Some(deadline))?
        }
        None => collect_python_local_bindings(current_path, node, source)?,
    };
    let instance_bindings = match deadline {
        Some(deadline) => {
            let mut bindings = std::collections::BTreeMap::new();
            collect_python_instance_type_bindings_with_deadline(
                node,
                source,
                &mut bindings,
                Some(deadline),
            )?;
            bindings
        }
        None => collect_python_instance_type_bindings(node, source)?,
    };
    match deadline {
        Some(deadline) => collect_python_reference_entries_with_deadline(
            current_path,
            node,
            source,
            &bindings,
            &local_bindings,
            &instance_bindings,
            references,
            Some(deadline),
        ),
        None => collect_python_reference_entries(
            current_path,
            node,
            source,
            &bindings,
            &local_bindings,
            &instance_bindings,
            references,
        ),
    }
}

#[derive(Debug, Clone)]
pub(super) struct PythonReferenceTarget<'tree> {
    pub(super) name: String,
    pub(super) node: Node<'tree>,
    pub(super) imported_symbol: Option<(String, String)>,
    pub(super) import_fallback_name: Option<String>,
}

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
