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
    collect_c_call_arities, collect_c_call_arities_with_deadline, collect_c_graph_references,
    collect_c_graph_references_with_deadline, collect_cpp_call_arities,
    collect_cpp_call_arities_with_deadline,
};
pub(crate) use commit_gate::evaluate_patch_commit_gate;
pub(crate) use python_imports::{
    resolve_local_python_imported_symbol, resolve_local_python_module_path,
};
pub(crate) use python_references::{
    collect_python_references, collect_python_references_with_deadline,
};
pub(crate) use reference_validation::{
    ReferenceValidation, ambiguous_binding_decision, resolved_binding_decision,
    unresolved_binding_decision,
};
pub(super) use reference_validation::{
    is_python_class_header_expression, is_python_default_parameter_value,
};
pub(crate) use result_builder::{
    PatchBuildInput, build_patch_result, build_patch_result_with_deadline, splice_source,
};
pub(crate) use syntax_validation::{collect_syntax_errors, collect_syntax_errors_with_deadline};
pub(crate) use target_resolution::{
    prepare_patch_replacement, prepare_patch_replacement_with_deadline,
    semantic_target_at_position_with_deadline,
};

pub use api::{
    patch_ast_node, patch_ast_node_at_position, patch_ast_node_at_position_from_path,
    patch_ast_node_at_position_from_path_with_timeout, patch_ast_node_at_position_with_timeout,
    patch_ast_node_from_path, patch_ast_node_from_path_with_timeout, patch_ast_node_with_timeout,
    preview_patch_ast_node, preview_patch_ast_node_at_position,
    preview_patch_ast_node_at_position_from_path,
    preview_patch_ast_node_at_position_from_path_with_timeout,
    preview_patch_ast_node_at_position_with_timeout, preview_patch_ast_node_from_path,
    preview_patch_ast_node_from_path_with_timeout, preview_patch_ast_node_with_timeout,
};
pub(crate) use api::{patch_ast_node_with_deadline, unified_diff};

use anyhow::{Result, bail};

pub const MAX_PATCH_REPLACEMENT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_PATCH_TIMEOUT_MS: u64 = 5 * 60 * 1_000;
pub const MAX_PATCH_PREVIEW_TIMEOUT_MS: u64 = 5 * 60 * 1_000;
pub const MAX_BYPASS_REASON_BYTES: usize = 4 * 1024;

pub(crate) fn patch_deadline(
    timeout_ms: Option<u64>,
) -> Result<crate::deadline::CooperativeDeadline> {
    crate::deadline::CooperativeDeadline::new(timeout_ms, MAX_PATCH_TIMEOUT_MS, "patch")
}

pub(crate) fn validate_bypass_reason(bypass_reason: Option<&str>) -> Result<()> {
    if bypass_reason.is_some_and(|reason| reason.trim().is_empty()) {
        bail!("invalid bypass_reason: reason must not be blank");
    }
    if bypass_reason.is_some_and(|reason| reason.len() > MAX_BYPASS_REASON_BYTES) {
        bail!(
            "invalid bypass_reason: reason exceeds max bytes ({})",
            MAX_BYPASS_REASON_BYTES
        );
    }
    Ok(())
}

pub(crate) fn validate_patch_replacement(new_code: &str) -> Result<()> {
    if new_code.trim().is_empty() {
        bail!("invalid new_code: replacement must not be blank");
    }
    if new_code.len() > MAX_PATCH_REPLACEMENT_BYTES {
        bail!(
            "invalid new_code: replacement exceeds max bytes ({})",
            MAX_PATCH_REPLACEMENT_BYTES
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_BYPASS_REASON_BYTES, MAX_PATCH_REPLACEMENT_BYTES, validate_bypass_reason,
        validate_patch_replacement,
    };

    #[test]
    fn validates_patch_replacement_size() {
        assert!(validate_patch_replacement("return 1").is_ok());
        assert!(validate_patch_replacement(&"x".repeat(MAX_PATCH_REPLACEMENT_BYTES)).is_ok());
        assert!(validate_patch_replacement(&"x".repeat(MAX_PATCH_REPLACEMENT_BYTES + 1)).is_err());
    }

    #[test]
    fn validates_bypass_reason_size() {
        assert!(validate_bypass_reason(None).is_ok());
        assert!(validate_bypass_reason(Some("reason")).is_ok());
        assert!(validate_bypass_reason(Some(&"x".repeat(MAX_BYPASS_REASON_BYTES))).is_ok());
        assert!(validate_bypass_reason(Some(&"x".repeat(MAX_BYPASS_REASON_BYTES + 1))).is_err());
    }
}
