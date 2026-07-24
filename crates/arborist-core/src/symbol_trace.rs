use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};

mod graph;
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

pub(crate) use graph::{trace_from_symbol, trace_from_symbol_with_timeout};

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
