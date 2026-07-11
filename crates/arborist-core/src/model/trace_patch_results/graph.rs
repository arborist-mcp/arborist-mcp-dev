use anyhow::{Result, bail};

use super::super::{ensure_nonblank, ensure_unique_symbol_evidence_keys};
use super::{TraceSymbolGraphResult, TraceSymbolNeighborhoodEdge, TraceSymbolNeighborhoodNode};

impl TraceSymbolGraphResult {
    pub fn validate_trace_replay_input(&self) -> Result<()> {
        self.symbol.validate_trace_replay_input("trace.symbol")?;
        if self.symbol.origin_type != "trace_root" {
            bail!(
                "invalid trace.symbol.origin_type: expected traced root symbol origin type to be `trace_root`"
            );
        }
        for (index, caller) in self.callers.iter().enumerate() {
            caller.validate_trace_replay_input(&format!("trace.callers[{index}]"))?;
        }
        for (index, callee) in self.callees.iter().enumerate() {
            callee.validate_trace_replay_input(&format!("trace.callees[{index}]"))?;
        }
        ensure_unique_symbol_evidence_keys(&self.callers, "trace.callers")?;
        ensure_unique_symbol_evidence_keys(&self.callees, "trace.callees")?;

        let expected_callers = self
            .callers
            .iter()
            .map(|symbol| symbol.evidence_key.clone())
            .collect::<Vec<_>>();
        let expected_callees = self
            .callees
            .iter()
            .map(|symbol| symbol.evidence_key.clone())
            .collect::<Vec<_>>();

        if self.evidence_keys.symbol != self.symbol.evidence_key {
            bail!(
                "invalid trace.evidence_keys.symbol: expected traced symbol evidence key to match trace.symbol.evidence_key"
            );
        }
        if self.evidence_keys.callers != expected_callers {
            bail!(
                "invalid trace.evidence_keys.callers: expected caller evidence keys to match trace.callers"
            );
        }
        if self.evidence_keys.callees != expected_callees {
            bail!(
                "invalid trace.evidence_keys.callees: expected callee evidence keys to match trace.callees"
            );
        }

        Ok(())
    }

    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.validate_trace_replay_input()
    }
}

impl TraceSymbolNeighborhoodNode {
    pub(crate) fn validate_public_output(&self, index: usize) -> Result<()> {
        self.symbol
            .validate_trace_replay_input(&format!("trace_neighborhood.nodes[{index}].symbol"))?;
        Ok(())
    }
}

impl TraceSymbolNeighborhoodEdge {
    pub(crate) fn validate_public_output(&self, index: usize) -> Result<()> {
        ensure_nonblank(
            &self.from_symbol_id,
            &format!("trace_neighborhood.edges[{index}].from_symbol_id"),
        )?;
        ensure_nonblank(
            &self.to_symbol_id,
            &format!("trace_neighborhood.edges[{index}].to_symbol_id"),
        )?;
        if self.from_symbol_id == self.to_symbol_id {
            bail!("invalid trace_neighborhood.edges[{index}]: self-edges are not allowed");
        }
        Ok(())
    }
}
