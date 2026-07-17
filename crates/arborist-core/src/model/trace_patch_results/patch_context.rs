use std::collections::BTreeSet;

use anyhow::{Result, bail};

use super::super::ensure_nonblank;
use super::{
    DiscoveryContextPatchResult, GraphBackedPatchResult, NeighborhoodContextPatchResult,
    TraceBackedPatchResult, TracePatchImpactSummary,
};

impl TraceBackedPatchResult {
    pub(crate) fn trace_skip_reason_for_syntax_errors() -> &'static str {
        "trace skipped because patch validation reported syntax errors"
    }

    pub(crate) fn trace_skip_reason_for_patch_gate_rejection() -> &'static str {
        "trace skipped because patch validation rejected the patch"
    }

    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.patch.validate_public_output()?;
        ensure_nonblank(&self.trace_target, "trace_target")?;
        if self.trace_target != self.patch.resolved_symbol_id {
            bail!("invalid trace_target: expected trace_target to match patch.resolved_symbol_id");
        }

        if !self.patch.validation.syntax_errors.is_empty() || !self.patch.applied {
            if self.trace.is_some() {
                bail!("invalid trace: expected no trace when the patch was not safely applied");
            }
            if self.trace_validation.is_some() {
                bail!(
                    "invalid trace_validation: expected no trace validation when the patch was not safely applied"
                );
            }
            if self.impact.is_some() {
                bail!("invalid impact: expected no impact when the patch was not safely applied");
            }
            let trace_error = self
                .trace_error
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("invalid trace_error: expected trace_error when the patch was not safely applied"))?;
            ensure_nonblank(trace_error, "trace_error")?;
            let expected_reason = if !self.patch.validation.syntax_errors.is_empty() {
                Self::trace_skip_reason_for_syntax_errors()
            } else {
                Self::trace_skip_reason_for_patch_gate_rejection()
            };
            if trace_error != expected_reason {
                bail!(
                    "invalid trace_error: expected trace skip reason consistent with patch validation state"
                );
            }
            return Ok(());
        }

        let trace = self
            .trace
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("invalid trace: expected trace for applied patches"))?;
        trace.validate_public_output()?;
        let trace_validation = self.trace_validation.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "invalid trace_validation: expected trace validation for applied patches"
            )
        })?;
        trace_validation.validate_public_output()?;
        if let Some(impact) = &self.impact {
            impact.validate_public_output()?;
        }
        if self.trace_error.is_some() {
            bail!("invalid trace_error: expected no trace error for applied patches");
        }
        if trace.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid trace.symbol.symbol_id: expected trace root symbol id to match patch.resolved_symbol_id"
            );
        }
        if trace.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid trace.symbol.semantic_path: expected trace root semantic path to match patch.resolved_path"
            );
        }
        if trace.symbol.file_path != self.patch.file {
            bail!(
                "invalid trace.symbol.file_path: expected trace root file path to match patch.file"
            );
        }

        Ok(())
    }
}

impl TracePatchImpactSummary {
    fn validate_public_output(&self) -> Result<()> {
        let mut symbol_ids = BTreeSet::new();
        for (field, symbols) in [
            ("impact.added_callers", &self.added_callers),
            ("impact.removed_callers", &self.removed_callers),
            ("impact.added_callees", &self.added_callees),
            ("impact.removed_callees", &self.removed_callees),
        ] {
            for (index, symbol) in symbols.iter().enumerate() {
                symbol.validate_trace_replay_input(&format!("{field}[{index}]"))?;
                if !symbol_ids.insert(symbol.symbol_id.clone()) {
                    bail!("invalid {field}[{index}]: duplicate changed symbol id");
                }
            }
        }
        if self.affected_symbol_count != symbol_ids.len() {
            bail!("invalid impact.affected_symbol_count: expected distinct changed symbol count");
        }
        Ok(())
    }
}

impl GraphBackedPatchResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.patch.validate_public_output()?;
        ensure_nonblank(&self.trace_target, "trace_target")?;
        if self.trace_target != self.patch.resolved_symbol_id {
            bail!("invalid trace_target: expected trace_target to match patch.resolved_symbol_id");
        }

        if !self.patch.validation.syntax_errors.is_empty() || !self.patch.applied {
            if self.trace.is_some() {
                bail!("invalid trace: expected no trace when the patch was not safely applied");
            }
            if self.neighborhood.is_some() {
                bail!(
                    "invalid neighborhood: expected no neighborhood when the patch was not safely applied"
                );
            }
            if self.trace_validation.is_some() {
                bail!(
                    "invalid trace_validation: expected no trace validation when the patch was not safely applied"
                );
            }
            let trace_error = self
                .trace_error
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("invalid trace_error: expected trace_error when the patch was not safely applied"))?;
            ensure_nonblank(trace_error, "trace_error")?;
            let expected_reason = if !self.patch.validation.syntax_errors.is_empty() {
                TraceBackedPatchResult::trace_skip_reason_for_syntax_errors()
            } else {
                TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection()
            };
            if trace_error != expected_reason {
                bail!(
                    "invalid trace_error: expected trace skip reason consistent with patch validation state"
                );
            }
            return Ok(());
        }

        let trace = self
            .trace
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("invalid trace: expected trace for applied patches"))?;
        trace.validate_public_output()?;
        let neighborhood = self.neighborhood.as_ref().ok_or_else(|| {
            anyhow::anyhow!("invalid neighborhood: expected neighborhood for applied patches")
        })?;
        neighborhood.validate_public_output()?;
        let trace_validation = self.trace_validation.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "invalid trace_validation: expected trace validation for applied patches"
            )
        })?;
        trace_validation.validate_public_output()?;
        if self.trace_error.is_some() {
            bail!("invalid trace_error: expected no trace error for applied patches");
        }
        if trace.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid trace.symbol.symbol_id: expected trace root symbol id to match patch.resolved_symbol_id"
            );
        }
        if trace.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid trace.symbol.semantic_path: expected trace root semantic path to match patch.resolved_path"
            );
        }
        if trace.symbol.file_path != self.patch.file {
            bail!(
                "invalid trace.symbol.file_path: expected trace root file path to match patch.file"
            );
        }
        if neighborhood.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid neighborhood.symbol.symbol_id: expected neighborhood root symbol id to match patch.resolved_symbol_id"
            );
        }
        if neighborhood.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid neighborhood.symbol.semantic_path: expected neighborhood root semantic path to match patch.resolved_path"
            );
        }
        if neighborhood.symbol.file_path != self.patch.file {
            bail!(
                "invalid neighborhood.symbol.file_path: expected neighborhood root file path to match patch.file"
            );
        }
        if neighborhood.symbol.symbol_id != trace.symbol.symbol_id {
            bail!(
                "invalid neighborhood.symbol.symbol_id: expected neighborhood root to match trace root symbol id"
            );
        }

        Ok(())
    }
}

impl NeighborhoodContextPatchResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.patch.validate_public_output()?;
        ensure_nonblank(&self.trace_target, "trace_target")?;
        if self.trace_target != self.patch.resolved_symbol_id {
            bail!("invalid trace_target: expected trace_target to match patch.resolved_symbol_id");
        }

        if !self.patch.validation.syntax_errors.is_empty() || !self.patch.applied {
            if self.trace.is_some() {
                bail!("invalid trace: expected no trace when the patch was not safely applied");
            }
            if self.neighborhood_context.is_some() {
                bail!(
                    "invalid neighborhood_context: expected no neighborhood_context when the patch was not safely applied"
                );
            }
            if self.trace_validation.is_some() {
                bail!(
                    "invalid trace_validation: expected no trace validation when the patch was not safely applied"
                );
            }
            let trace_error = self
                .trace_error
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("invalid trace_error: expected trace_error when the patch was not safely applied"))?;
            ensure_nonblank(trace_error, "trace_error")?;
            let expected_reason = if !self.patch.validation.syntax_errors.is_empty() {
                TraceBackedPatchResult::trace_skip_reason_for_syntax_errors()
            } else {
                TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection()
            };
            if trace_error != expected_reason {
                bail!(
                    "invalid trace_error: expected trace skip reason consistent with patch validation state"
                );
            }
            return Ok(());
        }

        let trace = self
            .trace
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("invalid trace: expected trace for applied patches"))?;
        trace.validate_public_output()?;
        let neighborhood_context = self.neighborhood_context.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "invalid neighborhood_context: expected neighborhood_context for applied patches"
            )
        })?;
        neighborhood_context.validate_public_output()?;
        let trace_validation = self.trace_validation.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "invalid trace_validation: expected trace validation for applied patches"
            )
        })?;
        trace_validation.validate_public_output()?;
        if self.trace_error.is_some() {
            bail!("invalid trace_error: expected no trace error for applied patches");
        }
        if trace.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid trace.symbol.symbol_id: expected trace root symbol id to match patch.resolved_symbol_id"
            );
        }
        if trace.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid trace.symbol.semantic_path: expected trace root semantic path to match patch.resolved_path"
            );
        }
        if trace.symbol.file_path != self.patch.file {
            bail!(
                "invalid trace.symbol.file_path: expected trace root file path to match patch.file"
            );
        }

        let neighborhood = &neighborhood_context.neighborhood;
        if neighborhood.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.symbol_id: expected neighborhood root symbol id to match patch.resolved_symbol_id"
            );
        }
        if neighborhood.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.semantic_path: expected neighborhood root semantic path to match patch.resolved_path"
            );
        }
        if neighborhood.symbol.file_path != self.patch.file {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.file_path: expected neighborhood root file path to match patch.file"
            );
        }
        if neighborhood.symbol.symbol_id != trace.symbol.symbol_id {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.symbol_id: expected neighborhood root to match trace root symbol id"
            );
        }

        Ok(())
    }
}

impl DiscoveryContextPatchResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.patch.validate_public_output()?;
        ensure_nonblank(&self.trace_target, "trace_target")?;
        if self.trace_target != self.patch.resolved_symbol_id {
            bail!("invalid trace_target: expected trace_target to match patch.resolved_symbol_id");
        }

        if !self.patch.validation.syntax_errors.is_empty() || !self.patch.applied {
            if self.trace.is_some() {
                bail!("invalid trace: expected no trace when the patch was not safely applied");
            }
            if self.read.is_some() {
                bail!("invalid read: expected no read when the patch was not safely applied");
            }
            if self.neighborhood_context.is_some() {
                bail!(
                    "invalid neighborhood_context: expected no neighborhood_context when the patch was not safely applied"
                );
            }
            if self.trace_validation.is_some() {
                bail!(
                    "invalid trace_validation: expected no trace validation when the patch was not safely applied"
                );
            }
            let trace_error = self
                .trace_error
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("invalid trace_error: expected trace_error when the patch was not safely applied"))?;
            ensure_nonblank(trace_error, "trace_error")?;
            let expected_reason = if !self.patch.validation.syntax_errors.is_empty() {
                TraceBackedPatchResult::trace_skip_reason_for_syntax_errors()
            } else {
                TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection()
            };
            if trace_error != expected_reason {
                bail!(
                    "invalid trace_error: expected trace skip reason consistent with patch validation state"
                );
            }
            return Ok(());
        }

        let trace = self
            .trace
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("invalid trace: expected trace for applied patches"))?;
        trace.validate_public_output()?;
        let read = self
            .read
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("invalid read: expected read for applied patches"))?;
        read.validate_public_output()?;
        let neighborhood_context = self.neighborhood_context.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "invalid neighborhood_context: expected neighborhood_context for applied patches"
            )
        })?;
        neighborhood_context.validate_public_output()?;
        let trace_validation = self.trace_validation.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "invalid trace_validation: expected trace validation for applied patches"
            )
        })?;
        trace_validation.validate_public_output()?;
        if self.trace_error.is_some() {
            bail!("invalid trace_error: expected no trace error for applied patches");
        }
        if trace.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid trace.symbol.symbol_id: expected trace root symbol id to match patch.resolved_symbol_id"
            );
        }
        if trace.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid trace.symbol.semantic_path: expected trace root semantic path to match patch.resolved_path"
            );
        }
        if trace.symbol.file_path != self.patch.file {
            bail!(
                "invalid trace.symbol.file_path: expected trace root file path to match patch.file"
            );
        }
        if read.indexed_files != trace.indexed_files {
            bail!(
                "invalid read.indexed_files: expected read.indexed_files to match trace.indexed_files"
            );
        }
        if read.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid read.symbol.symbol_id: expected read symbol id to match patch.resolved_symbol_id"
            );
        }
        if read.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid read.symbol.semantic_path: expected read semantic path to match patch.resolved_path"
            );
        }
        if read.symbol.file_path != self.patch.file {
            bail!("invalid read.symbol.file_path: expected read file path to match patch.file");
        }
        let neighborhood = &neighborhood_context.neighborhood;
        if neighborhood.symbol.symbol_id != self.patch.resolved_symbol_id {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.symbol_id: expected neighborhood root symbol id to match patch.resolved_symbol_id"
            );
        }
        if neighborhood.symbol.semantic_path != self.patch.resolved_path {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.semantic_path: expected neighborhood root semantic path to match patch.resolved_path"
            );
        }
        if neighborhood.symbol.file_path != self.patch.file {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.file_path: expected neighborhood root file path to match patch.file"
            );
        }
        if neighborhood.indexed_files != trace.indexed_files {
            bail!(
                "invalid neighborhood_context.neighborhood.indexed_files: expected neighborhood indexed_files to match trace.indexed_files"
            );
        }
        if read.symbol.symbol_id != trace.symbol.symbol_id {
            bail!("invalid read.symbol.symbol_id: expected read symbol id to match trace root");
        }
        if neighborhood.symbol.symbol_id != trace.symbol.symbol_id {
            bail!(
                "invalid neighborhood_context.neighborhood.symbol.symbol_id: expected neighborhood root to match trace root symbol id"
            );
        }

        Ok(())
    }
}
