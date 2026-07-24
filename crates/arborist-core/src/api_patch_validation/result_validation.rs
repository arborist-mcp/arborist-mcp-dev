use anyhow::{Result, bail};

use super::{validate_patch_commit_with_trace, validate_replay_trace_target};
use crate::model::{
    DiscoveryContextPatchResult, GraphBackedPatchResult, NeighborhoodContextPatchResult,
    PatchTraceValidationResult, TraceBackedPatchResult, TracePatchEvidenceReplayResult,
};

pub(crate) fn validate_trace_patch_evidence_replay_result(
    replay: &TracePatchEvidenceReplayResult,
) -> Result<()> {
    replay.validate_public_output()
}

pub(crate) fn validate_patch_trace_validation_result(
    result: &PatchTraceValidationResult,
) -> Result<()> {
    result.validate_public_output()
}

pub(crate) fn validate_trace_backed_patch_result(result: &TraceBackedPatchResult) -> Result<()> {
    result.validate_public_output()?;
    if !result.patch.validation.syntax_errors.is_empty() || !result.patch.applied {
        return Ok(());
    }

    let trace = result
        .trace
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("invalid trace: expected trace for applied patches"))?;
    let trace_validation = result.trace_validation.as_ref().ok_or_else(|| {
        anyhow::anyhow!("invalid trace_validation: expected trace validation for applied patches")
    })?;
    if result.trace_error.is_some() {
        bail!("invalid trace_error: expected no trace error for applied patches");
    }

    validate_replay_trace_target(&result.patch, trace)?;
    let expected = validate_patch_commit_with_trace(&result.patch, trace)?;
    if trace_validation != &expected {
        bail!(
            "invalid trace_validation: expected trace-backed validation derived from patch and trace"
        );
    }

    Ok(())
}

pub(crate) fn validate_graph_backed_patch_result(result: &GraphBackedPatchResult) -> Result<()> {
    result.validate_public_output()?;
    if !result.patch.validation.syntax_errors.is_empty() || !result.patch.applied {
        return Ok(());
    }

    let trace = result
        .trace
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("invalid trace: expected trace for applied patches"))?;
    let neighborhood = result.neighborhood.as_ref().ok_or_else(|| {
        anyhow::anyhow!("invalid neighborhood: expected neighborhood for applied patches")
    })?;
    let trace_validation = result.trace_validation.as_ref().ok_or_else(|| {
        anyhow::anyhow!("invalid trace_validation: expected trace validation for applied patches")
    })?;
    if result.trace_error.is_some() {
        bail!("invalid trace_error: expected no trace error for applied patches");
    }

    validate_replay_trace_target(&result.patch, trace)?;
    let expected = validate_patch_commit_with_trace(&result.patch, trace)?;
    if trace_validation != &expected {
        bail!(
            "invalid trace_validation: expected trace-backed validation derived from patch and trace"
        );
    }
    if neighborhood.symbol.symbol_id != trace.symbol.symbol_id {
        bail!(
            "invalid neighborhood.symbol.symbol_id: expected neighborhood root to match trace root symbol id"
        );
    }

    Ok(())
}

pub(crate) fn validate_neighborhood_context_patch_result(
    result: &NeighborhoodContextPatchResult,
) -> Result<()> {
    result.validate_public_output()?;
    if !result.patch.validation.syntax_errors.is_empty() || !result.patch.applied {
        return Ok(());
    }

    let trace = result
        .trace
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("invalid trace: expected trace for applied patches"))?;
    let neighborhood_context = result.neighborhood_context.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "invalid neighborhood_context: expected neighborhood_context for applied patches"
        )
    })?;
    let trace_validation = result.trace_validation.as_ref().ok_or_else(|| {
        anyhow::anyhow!("invalid trace_validation: expected trace validation for applied patches")
    })?;
    if result.trace_error.is_some() {
        bail!("invalid trace_error: expected no trace error for applied patches");
    }

    validate_replay_trace_target(&result.patch, trace)?;
    let expected = validate_patch_commit_with_trace(&result.patch, trace)?;
    if trace_validation != &expected {
        bail!(
            "invalid trace_validation: expected trace-backed validation derived from patch and trace"
        );
    }
    if neighborhood_context.neighborhood.symbol.symbol_id != trace.symbol.symbol_id {
        bail!(
            "invalid neighborhood_context.neighborhood.symbol.symbol_id: expected neighborhood root to match trace root symbol id"
        );
    }

    Ok(())
}

pub(crate) fn validate_discovery_context_patch_result(
    result: &DiscoveryContextPatchResult,
) -> Result<()> {
    result.validate_public_output()?;
    if !result.patch.validation.syntax_errors.is_empty() || !result.patch.applied {
        return Ok(());
    }

    let trace = result
        .trace
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("invalid trace: expected trace for applied patches"))?;
    let read = result
        .read
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("invalid read: expected read for applied patches"))?;
    let neighborhood_context = result.neighborhood_context.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "invalid neighborhood_context: expected neighborhood_context for applied patches"
        )
    })?;
    let trace_validation = result.trace_validation.as_ref().ok_or_else(|| {
        anyhow::anyhow!("invalid trace_validation: expected trace validation for applied patches")
    })?;
    if result.trace_error.is_some() {
        bail!("invalid trace_error: expected no trace error for applied patches");
    }

    validate_replay_trace_target(&result.patch, trace)?;
    let expected = validate_patch_commit_with_trace(&result.patch, trace)?;
    if trace_validation != &expected {
        bail!(
            "invalid trace_validation: expected trace-backed validation derived from patch and trace"
        );
    }
    if read.symbol.symbol_id != trace.symbol.symbol_id {
        bail!("invalid read.symbol.symbol_id: expected read symbol id to match trace root");
    }
    if neighborhood_context.neighborhood.symbol.symbol_id != trace.symbol.symbol_id {
        bail!(
            "invalid neighborhood_context.neighborhood.symbol.symbol_id: expected neighborhood root to match trace root symbol id"
        );
    }

    Ok(())
}
