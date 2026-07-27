use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};

mod graph;
mod neighborhood;

pub const MAX_TRACE_TIMEOUT_MS: u64 = 5 * 60 * 1_000;
pub const MAX_GRAPH_DEPTH: usize = 64;
pub const MAX_GRAPH_NODES: usize = 10_000;

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

    pub(crate) fn remaining_timeout_ms(&self, phase: &str) -> Result<Option<u64>> {
        let Some(deadline) = self.deadline else {
            return Ok(None);
        };
        let remaining_ms = ceil_duration_millis(deadline.saturating_duration_since(Instant::now()));
        if remaining_ms == 0 {
            return Err(anyhow!(
                "trace timeout exceeded during {phase}: timeout_ms={}",
                self.timeout_ms.unwrap_or_default()
            ));
        }
        Ok(Some(remaining_ms))
    }
}

fn ceil_duration_millis(duration: Duration) -> u64 {
    duration
        .as_micros()
        .saturating_add(999)
        .saturating_div(1_000)
        .min(u128::from(u64::MAX)) as u64
}

pub(crate) use graph::trace_from_symbol_with_timeout;

pub(crate) use neighborhood::trace_neighborhood_from_symbol_with_timeout;

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{
        MAX_GRAPH_DEPTH, MAX_GRAPH_NODES, MAX_TRACE_TIMEOUT_MS, TraceQueryDeadline,
        neighborhood::validate_neighborhood_bounds,
    };
    use crate::model::{SymbolMeta, SymbolMetaInit};
    use crate::symbol_summary::{
        summarize_symbols_with_deadline, trace_evidence_keys_with_deadline,
    };

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

    #[test]
    fn reports_expired_remaining_trace_budget() {
        let deadline = TraceQueryDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = deadline
            .remaining_timeout_ms("override loading")
            .expect_err("expired remaining budgets should fail");
        assert!(error.to_string().contains("override loading"));
    }

    #[test]
    fn rounds_remaining_trace_budget_up_to_milliseconds() {
        assert_eq!(super::ceil_duration_millis(Duration::from_micros(1)), 1);
        assert_eq!(super::ceil_duration_millis(Duration::from_millis(1)), 1);
        assert_eq!(super::ceil_duration_millis(Duration::from_micros(1_001)), 2);
    }

    #[test]
    fn validates_neighborhood_bounds() {
        assert!(validate_neighborhood_bounds(MAX_GRAPH_DEPTH, 1).is_ok());
        assert!(validate_neighborhood_bounds(MAX_GRAPH_DEPTH + 1, 1).is_err());
        assert!(validate_neighborhood_bounds(0, 0).is_err());
        assert!(validate_neighborhood_bounds(0, MAX_GRAPH_NODES).is_ok());
        assert!(validate_neighborhood_bounds(0, MAX_GRAPH_NODES + 1).is_err());
    }

    #[test]
    fn summarizing_symbols_checks_expired_deadline() {
        let symbol = SymbolMeta::new(SymbolMetaInit {
            symbol_id: "helper".to_string(),
            semantic_path: "helper".to_string(),
            scope_path: None,
            file_path: "helper.py".to_string(),
            node_kind: "function_definition".to_string(),
            origin_type: "workspace_symbol".to_string(),
            byte_range: (0, 1),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
            dependencies: Vec::new(),
            references: Vec::new(),
        });
        let deadline = TraceQueryDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error =
            summarize_symbols_with_deadline(&[symbol], &[String::from("helper")], None, &deadline)
                .expect_err("expired summary deadline should fail");
        assert!(error.to_string().contains("trace timeout exceeded"));
    }

    #[test]
    fn building_trace_evidence_keys_checks_expired_deadline() {
        let symbol = SymbolMeta::new(SymbolMetaInit {
            symbol_id: "helper".to_string(),
            semantic_path: "helper".to_string(),
            scope_path: None,
            file_path: "helper.py".to_string(),
            node_kind: "function_definition".to_string(),
            origin_type: "workspace_symbol".to_string(),
            byte_range: (0, 1),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
            dependencies: Vec::new(),
            references: Vec::new(),
        });
        let deadline = TraceQueryDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = trace_evidence_keys_with_deadline(&symbol, &[], &[], &deadline)
            .expect_err("expired evidence-key deadline should fail");
        assert!(error.to_string().contains("trace timeout exceeded"));
    }
}
