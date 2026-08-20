use anyhow::Result;

use crate::deadline::{CooperativeDeadline, DeadlineCheck};

mod graph;
mod neighborhood;

pub const MAX_TRACE_TIMEOUT_MS: u64 = 5 * 60 * 1_000;
pub const MAX_GRAPH_DEPTH: usize = 64;
pub const MAX_GRAPH_NODES: usize = 10_000;

#[derive(Debug, Clone, Copy)]
pub(crate) struct TraceQueryDeadline(CooperativeDeadline);

impl TraceQueryDeadline {
    pub(crate) fn new(timeout_ms: Option<u64>) -> Result<Self> {
        CooperativeDeadline::new(timeout_ms, MAX_TRACE_TIMEOUT_MS, "trace").map(Self)
    }

    pub(crate) fn check(&self, phase: &str) -> Result<()> {
        self.0.check(phase)
    }

    pub(crate) fn remaining_timeout_ms(&self, phase: &str) -> Result<Option<u64>> {
        self.0.remaining_timeout_ms(phase)
    }

    #[cfg(test)]
    pub(crate) fn expired_for_tests(timeout_ms: u64) -> Self {
        Self(CooperativeDeadline::expired_for_tests(timeout_ms, "trace"))
    }
}

impl DeadlineCheck for TraceQueryDeadline {
    fn check(&self, phase: &str) -> Result<()> {
        TraceQueryDeadline::check(self, phase)
    }

    fn remaining_timeout_micros(&self, phase: &str) -> Result<Option<u64>> {
        self.remaining_timeout_ms(phase)
            .map(|timeout_ms| timeout_ms.map(|timeout_ms| timeout_ms.saturating_mul(1_000)))
    }
}

pub(crate) use graph::trace_from_symbol_with_deadline;

pub(crate) use neighborhood::trace_neighborhood_from_symbol_with_deadline;

#[cfg(test)]
mod tests {
    use super::{
        MAX_GRAPH_DEPTH, MAX_GRAPH_NODES, MAX_TRACE_TIMEOUT_MS, TraceQueryDeadline,
        trace_from_symbol_with_deadline, trace_neighborhood_from_symbol_with_deadline,
    };
    use crate::model::{SymbolMeta, SymbolMetaInit, TraceDirection};
    use crate::symbol_summary::{
        summarize_symbols_with_deadline, trace_evidence_keys_with_deadline,
    };

    fn test_symbol() -> SymbolMeta {
        SymbolMeta::new(SymbolMetaInit {
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
        })
    }

    #[test]
    fn graph_with_diagnostics_sink_records_unresolved_and_ambiguous_references() {
        use crate::diagnostics::{DiagnosticCategory, DiagnosticsSink};

        let mut root = test_symbol();
        root.references = vec!["caller".to_string(), "missing".to_string()];

        let caller_a = SymbolMeta::new(SymbolMetaInit {
            symbol_id: "caller".to_string(),
            semantic_path: "caller".to_string(),
            scope_path: None,
            file_path: "a.py".to_string(),
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
        let caller_b = SymbolMeta {
            file_path: "b.py".to_string(),
            ..caller_a.clone()
        };

        let resolved = vec![root.clone(), caller_a, caller_b];
        let mut sink = DiagnosticsSink::default();
        let deadline = TraceQueryDeadline::new(None).expect("unbounded deadline should be valid");

        let result = super::graph::trace_from_symbol_with_deadline_and_diagnostics(
            &resolved,
            1,
            &root,
            TraceDirection::Callers,
            &deadline,
            Some(&mut sink),
        )
        .expect("graph expansion should succeed");

        assert_eq!(result.callers.len(), 1);
        let records = sink.records();
        assert!(records.iter().any(|record| {
            record.category == DiagnosticCategory::AmbiguousReference
                && record.semantic_path.as_deref() == Some("caller")
        }));
        assert!(records.iter().any(|record| {
            record.category == DiagnosticCategory::UnresolvedReference
                && record.semantic_path.as_deref() == Some("missing")
        }));
    }

    #[test]
    fn graph_expansion_reuses_the_callers_deadline() {
        let symbol = test_symbol();
        let deadline = TraceQueryDeadline::expired_for_tests(1);

        let error = trace_from_symbol_with_deadline(
            std::slice::from_ref(&symbol),
            1,
            &symbol,
            TraceDirection::Both,
            &deadline,
        )
        .expect_err("graph expansion should honor an already-expired deadline");

        assert!(error.to_string().contains("starting graph expansion"));
    }

    #[test]
    fn neighborhood_expansion_reuses_the_callers_deadline() {
        let symbol = test_symbol();
        let deadline = TraceQueryDeadline::expired_for_tests(1);

        let error = trace_neighborhood_from_symbol_with_deadline(
            std::slice::from_ref(&symbol),
            1,
            &symbol,
            TraceDirection::Both,
            1,
            1,
            &deadline,
        )
        .expect_err("neighborhood expansion should honor an already-expired deadline");

        assert!(
            error
                .to_string()
                .contains("starting neighborhood expansion")
        );
    }

    #[test]
    fn validates_trace_timeout_bounds() {
        assert!(TraceQueryDeadline::new(Some(0)).is_err());
        assert!(TraceQueryDeadline::new(Some(MAX_TRACE_TIMEOUT_MS + 1)).is_err());
        assert!(TraceQueryDeadline::new(Some(1)).is_ok());
    }

    #[test]
    fn reports_expired_trace_deadline() {
        let deadline = TraceQueryDeadline::expired_for_tests(1);

        let error = deadline
            .check("test phase")
            .expect_err("expired trace deadline should fail");
        assert!(error.to_string().contains("trace timeout exceeded"));
        assert!(error.to_string().contains("timeout_ms=1"));
    }

    #[test]
    fn reports_expired_remaining_trace_budget() {
        let deadline = TraceQueryDeadline::expired_for_tests(1);

        let error = deadline
            .remaining_timeout_ms("override loading")
            .expect_err("expired remaining budgets should fail");
        assert!(error.to_string().contains("override loading"));
    }

    #[test]
    fn neighborhood_expansion_validates_bounds() {
        let symbol = test_symbol();
        let deadline = TraceQueryDeadline::new(None).expect("unbounded deadline should be valid");

        for (max_depth, max_nodes, expected_message) in [
            (MAX_GRAPH_DEPTH + 1, 1, "max_depth must not exceed"),
            (0, 0, "max_nodes must be greater than zero"),
            (0, MAX_GRAPH_NODES + 1, "max_nodes must not exceed"),
        ] {
            let error = trace_neighborhood_from_symbol_with_deadline(
                std::slice::from_ref(&symbol),
                1,
                &symbol,
                TraceDirection::Both,
                max_depth,
                max_nodes,
                &deadline,
            )
            .expect_err("invalid neighborhood bounds should be rejected");

            assert!(error.to_string().contains(expected_message));
        }
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
        let deadline = TraceQueryDeadline::expired_for_tests(1);

        let error = summarize_symbols_with_deadline(
            &[symbol],
            &[String::from("helper")],
            None,
            &deadline,
            None,
        )
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
        let deadline = TraceQueryDeadline::expired_for_tests(1);

        let error = trace_evidence_keys_with_deadline(&symbol, &[], &[], &deadline)
            .expect_err("expired evidence-key deadline should fail");
        assert!(error.to_string().contains("trace timeout exceeded"));
    }
}
