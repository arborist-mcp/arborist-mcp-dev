use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};

use crate::model::{SymbolMeta, TraceDirection, TraceSymbolGraphResult};
use crate::symbol_summary::{summarize_symbols, trace_evidence_keys};

mod neighborhood;

pub const MAX_TRACE_TIMEOUT_MS: u64 = 5 * 60 * 1_000;

#[derive(Debug, Clone, Copy)]
pub(crate) struct TraceQueryDeadline {
    deadline: Option<Instant>,
    timeout_ms: Option<u64>,
}

impl TraceQueryDeadline {
    pub(crate) fn new(timeout_ms: Option<u64>) -> Result<Self> {
        if timeout_ms == Some(0) {
            return Err(anyhow!(
                "invalid trace timeout_ms: value must be greater than zero"
            ));
        }
        if timeout_ms.is_some_and(|value| value > MAX_TRACE_TIMEOUT_MS) {
            return Err(anyhow!(
                "invalid trace timeout_ms: value must not exceed {}",
                MAX_TRACE_TIMEOUT_MS
            ));
        }

        Ok(Self {
            deadline: timeout_ms.map(|value| Instant::now() + Duration::from_millis(value)),
            timeout_ms,
        })
    }

    pub(crate) fn check(&self, phase: &str) -> Result<()> {
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(anyhow!(
                "trace timeout exceeded during {phase}: timeout_ms={}",
                self.timeout_ms.unwrap_or_default()
            ));
        }
        Ok(())
    }
}

pub(crate) fn trace_from_symbol(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    trace_from_symbol_with_timeout(resolved_symbols, indexed_files, symbol, direction, None)
}

pub(crate) fn trace_from_symbol_with_timeout(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol: &SymbolMeta,
    direction: TraceDirection,
    timeout_ms: Option<u64>,
) -> Result<TraceSymbolGraphResult> {
    let deadline = TraceQueryDeadline::new(timeout_ms)?;
    deadline.check("starting graph expansion")?;
    let symbol = symbol.clone().with_origin_type("trace_root");

    let callers = if matches!(direction, TraceDirection::Callers | TraceDirection::Both) {
        deadline.check("expanding callers")?;
        summarize_symbols(resolved_symbols, &symbol.references, None)
    } else {
        Vec::new()
    };

    let callees = if matches!(direction, TraceDirection::Callees | TraceDirection::Both) {
        deadline.check("expanding callees")?;
        summarize_symbols(
            resolved_symbols,
            &symbol.dependencies,
            Some(&symbol.file_path),
        )
    } else {
        Vec::new()
    };
    deadline.check("validating graph output")?;

    let result = TraceSymbolGraphResult {
        evidence_keys: trace_evidence_keys(&symbol, &callers, &callees),
        symbol,
        callers,
        callees,
        indexed_files,
    };
    result.validate_public_output()?;
    Ok(result)
}

pub(crate) use neighborhood::trace_neighborhood_from_symbol_with_timeout;

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{MAX_TRACE_TIMEOUT_MS, TraceQueryDeadline};

    #[test]
    fn validates_trace_timeout_bounds() {
        assert!(TraceQueryDeadline::new(Some(0)).is_err());
        assert!(TraceQueryDeadline::new(Some(MAX_TRACE_TIMEOUT_MS + 1)).is_err());
        assert!(TraceQueryDeadline::new(Some(1)).is_ok());
    }

    #[test]
    fn reports_expired_trace_deadline() {
        let deadline = TraceQueryDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = deadline
            .check("test phase")
            .expect_err("expired trace deadline should fail");
        assert!(error.to_string().contains("trace timeout exceeded"));
        assert!(error.to_string().contains("timeout_ms=1"));
    }
}
