use arborist_core::{
    PatchAstNodeResult, TraceSymbolGraphResult, replay_patch_evidence_against_trace_with_timeout,
};
use pyo3::prelude::*;

use crate::{ArboristCore, parse_json_arg, to_json_result, to_py_error};

#[pymethods]
impl ArboristCore {
    #[pyo3(
        name = "replay_patch_evidence_against_trace_json",
        signature = (patch_json, trace_json, timeout_ms=None)
    )]
    fn replay_patch_evidence_against_trace_json_binding(
        &self,
        patch_json: &str,
        trace_json: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        self.replay_patch_evidence_against_trace_json_with_timeout_impl(
            patch_json, trace_json, timeout_ms,
        )
    }
}

impl ArboristCore {
    #[cfg(test)]
    pub(crate) fn replay_patch_evidence_against_trace_json(
        &self,
        patch_json: &str,
        trace_json: &str,
    ) -> PyResult<String> {
        self.replay_patch_evidence_against_trace_json_with_timeout_impl(
            patch_json, trace_json, None,
        )
    }

    pub(crate) fn replay_patch_evidence_against_trace_json_with_timeout_impl(
        &self,
        patch_json: &str,
        trace_json: &str,
        timeout_ms: Option<u64>,
    ) -> PyResult<String> {
        let patch: PatchAstNodeResult = parse_json_arg(patch_json)?;
        let trace: TraceSymbolGraphResult = parse_json_arg(trace_json)?;
        let result = replay_patch_evidence_against_trace_with_timeout(&patch, &trace, timeout_ms)
            .map_err(to_py_error)?;
        to_json_result(&result)
    }
}
