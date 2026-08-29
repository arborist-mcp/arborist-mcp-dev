mod accessibility;
mod ambiguity;
mod cpp_syntax;
mod cpp_types;
mod cpp_wrappers;
mod references;

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::{
    ReferenceValidation, ambiguous_binding_decision, resolved_binding_decision,
    unresolved_binding_decision,
};
use crate::deadline::DeadlineCheck;
use crate::language::ParsedDocument;
use crate::model::{ValidationAmbiguity, ValidationBinding};

use accessibility::{
    c_binding_candidates_for_name, collect_c_accessible_names,
    collect_c_accessible_names_with_deadline, collect_c_accessible_symbols,
    collect_c_accessible_symbols_with_deadline,
};
use ambiguity::{ambiguity_disambiguation_context, ambiguity_reason};
use references::{
    collect_c_local_definitions, collect_c_local_definitions_with_deadline,
    collect_c_references_with_deadline, collect_c_scope_escaped_local_definition_names,
    collect_c_scope_escaped_local_definition_names_with_deadline,
};

pub(crate) fn collect_c_reference_validation_with_deadline(
    path: &Path,
    document: &ParsedDocument,
    source: &str,
    symbol_node: Node<'_>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<ReferenceValidation> {
    let mut accessible = BTreeSet::new();
    let mut visited = BTreeSet::new();
    match deadline {
        Some(deadline) => collect_c_accessible_names_with_deadline(
            path,
            document,
            source,
            &mut accessible,
            &mut visited,
            Some(deadline),
        )?,
        None => collect_c_accessible_names(path, document, source, &mut accessible, &mut visited)?,
    }
    let mut local_definitions = BTreeSet::new();
    match deadline {
        Some(deadline) => collect_c_local_definitions_with_deadline(
            symbol_node,
            source,
            &mut local_definitions,
            Some(deadline),
        )?,
        None => collect_c_local_definitions(symbol_node, source, &mut local_definitions)?,
    }

    let mut scope_escaped_local_definitions = BTreeSet::new();
    match deadline {
        Some(deadline) => collect_c_scope_escaped_local_definition_names_with_deadline(
            symbol_node,
            source,
            &mut scope_escaped_local_definitions,
            Some(deadline),
        )?,
        None => collect_c_scope_escaped_local_definition_names(
            symbol_node,
            source,
            &mut scope_escaped_local_definitions,
        )?,
    }

    let mut references = BTreeSet::new();
    match deadline {
        Some(deadline) => collect_c_references_with_deadline(
            symbol_node,
            source,
            &mut references,
            Some(deadline),
        )?,
        None => references::collect_c_references(symbol_node, source, &mut references)?,
    }

    let accessible_symbols = match deadline {
        Some(deadline) => {
            collect_c_accessible_symbols_with_deadline(path, document, source, Some(deadline))?
        }
        None => collect_c_accessible_symbols(path, document, source)?,
    };
    let mut validation = ReferenceValidation::default();

    for name in references {
        if let Some(deadline) = deadline {
            deadline.check("validating C/C++ references")?;
        }
        if local_definitions.contains(&name) && !scope_escaped_local_definitions.contains(&name) {
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

pub(crate) use references::{
    CPP_CONST_LVALUE_TEMPORARY_MEMBER_CALL_PREFIX, CPP_CONST_LVALUE_THIS_CALL_PREFIX,
    CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_CONST_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
    CPP_CONST_RVALUE_THIS_CALL_PREFIX, CPP_CONST_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
    CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
    CPP_RVALUE_THIS_CALL_PREFIX, CPP_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
    CPP_TEMPORARY_MEMBER_CALL_SEPARATOR, collect_c_call_arities,
    collect_c_call_arities_with_deadline, collect_c_graph_references,
    collect_c_graph_references_with_deadline, collect_cpp_call_arities,
    collect_cpp_call_arities_with_deadline,
};
