mod api;
mod c_validation;
mod commit_gate;
mod python_bindings;
mod python_imports;
mod python_patterns;
mod python_references;
mod python_replacement;
mod python_visibility;
mod reference_validation;
mod result_builder;
mod syntax_validation;
mod target_resolution;

pub(crate) use c_validation::{
    collect_c_call_arities, collect_c_graph_references, collect_cpp_call_arities,
};
pub(crate) use commit_gate::evaluate_patch_commit_gate;
pub(crate) use python_imports::{
    resolve_local_python_imported_symbol, resolve_local_python_module_path,
};
pub(crate) use python_references::collect_python_references;
pub(crate) use reference_validation::{
    ReferenceValidation, ambiguous_binding_decision, resolved_binding_decision,
    unresolved_binding_decision,
};
pub(super) use reference_validation::{
    is_python_class_header_expression, is_python_default_parameter_value,
};
pub(crate) use result_builder::{build_patch_result, splice_source};
pub(crate) use syntax_validation::collect_syntax_errors;
pub(crate) use target_resolution::{prepare_patch_replacement, semantic_target_at_position};

pub(crate) use api::unified_diff;
pub use api::{
    patch_ast_node, patch_ast_node_at_position, patch_ast_node_at_position_from_path,
    patch_ast_node_from_path, preview_patch_ast_node, preview_patch_ast_node_at_position,
    preview_patch_ast_node_at_position_from_path, preview_patch_ast_node_from_path,
};

use anyhow::{Result, bail};

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
