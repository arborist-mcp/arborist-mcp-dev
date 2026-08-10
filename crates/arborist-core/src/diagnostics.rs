use serde::{Deserialize, Serialize};

/// Structured internal categories for why an analysis step produced no result
/// or was skipped.
///
/// These are internal diagnostics: public responses stay concise by default,
/// and only an inspection or debug path that opts in should surface them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiagnosticCategory {
    /// A requested capability is not advertised for the detected language.
    UnsupportedCapability,
    /// The language could not be determined unambiguously.
    DetectionAmbiguity,
    /// A reference spelled a name that no indexed symbol resolves to.
    UnresolvedReference,
    /// A reference matched multiple indexed symbols with no decisive winner.
    AmbiguousReference,
    /// The persisted analysis revision is older than the current analyzer.
    StaleAnalysisRevision,
    /// A reference crossed a language boundary with no approved bridge.
    UnsupportedCrossLanguageBridge,
}

/// A single structured diagnostic record.
///
/// Callers should branch on [`DiagnosticCategory`] and the structured value
/// fields rather than parsing the message prose. Records never include source
/// contents beyond existing permitted output or paths outside the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Diagnostic {
    pub(crate) category: DiagnosticCategory,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) semantic_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) context_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) language_id: Option<String>,
}

/// Collects structured diagnostics while an analysis runs.
///
/// Pass `Option<&mut DiagnosticsSink>` through internal analysis entry points;
/// `None` keeps the hot public path allocation-free and unchanged.
#[derive(Debug, Default)]
pub(crate) struct DiagnosticsSink {
    records: Vec<Diagnostic>,
}

impl DiagnosticsSink {
    pub(crate) fn record(&mut self, diagnostic: Diagnostic) {
        self.records.push(diagnostic);
    }

    // Exercised by tests today; inspection/debug consumers arrive in later slices.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn records(&self) -> &[Diagnostic] {
        &self.records
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn into_records(self) -> Vec<Diagnostic> {
        self.records
    }
}

#[cfg(test)]
mod tests {
    use super::{Diagnostic, DiagnosticCategory, DiagnosticsSink};

    #[test]
    fn categories_serialize_as_snake_case() {
        for (category, expected) in [
            (
                DiagnosticCategory::UnsupportedCapability,
                "unsupported_capability",
            ),
            (
                DiagnosticCategory::DetectionAmbiguity,
                "detection_ambiguity",
            ),
            (
                DiagnosticCategory::UnresolvedReference,
                "unresolved_reference",
            ),
            (
                DiagnosticCategory::AmbiguousReference,
                "ambiguous_reference",
            ),
            (
                DiagnosticCategory::StaleAnalysisRevision,
                "stale_analysis_revision",
            ),
            (
                DiagnosticCategory::UnsupportedCrossLanguageBridge,
                "unsupported_cross_language_bridge",
            ),
        ] {
            let json = serde_json::to_string(&category).expect("category must serialize");
            assert_eq!(json, format!("\"{expected}\""));
        }
    }

    #[test]
    fn sink_collects_records_in_order() {
        let mut sink = DiagnosticsSink::default();
        sink.record(Diagnostic {
            category: DiagnosticCategory::UnresolvedReference,
            message: "no indexed symbol matches reference".to_string(),
            semantic_path: Some("pkg::missing".to_string()),
            context_file: None,
            language_id: None,
        });
        sink.record(Diagnostic {
            category: DiagnosticCategory::AmbiguousReference,
            message: "reference matched multiple indexed symbols".to_string(),
            semantic_path: Some("pkg::caller".to_string()),
            context_file: None,
            language_id: None,
        });

        assert_eq!(sink.records().len(), 2);
        let records = sink.into_records();
        assert_eq!(records[0].category, DiagnosticCategory::UnresolvedReference);
        assert_eq!(records[0].semantic_path.as_deref(), Some("pkg::missing"));
        assert_eq!(records[1].category, DiagnosticCategory::AmbiguousReference);
        assert_eq!(records[1].semantic_path.as_deref(), Some("pkg::caller"));
    }

    #[test]
    fn serialized_records_omit_none_fields() {
        let record = Diagnostic {
            category: DiagnosticCategory::UnresolvedReference,
            message: "no indexed symbol matches reference".to_string(),
            semantic_path: Some("pkg::missing".to_string()),
            context_file: None,
            language_id: None,
        };
        let json = serde_json::to_string(&record).expect("record must serialize");
        assert!(json.contains("\"category\":\"unresolved_reference\""));
        assert!(json.contains("\"semantic_path\":\"pkg::missing\""));
        assert!(!json.contains("context_file"));
        assert!(!json.contains("language_id"));
    }
}

#[cfg(test)]
mod resolution_tests {
    use super::{DiagnosticCategory, DiagnosticsSink};
    use crate::model::{SymbolMeta, SymbolMetaInit};
    use crate::symbol_summary::summarize_symbols_with_deadline;
    use crate::symbol_trace::TraceQueryDeadline;

    fn symbol(symbol_id: &str, file_path: &str, byte_range: (usize, usize)) -> SymbolMeta {
        SymbolMeta::new(SymbolMetaInit {
            symbol_id: symbol_id.to_string(),
            semantic_path: symbol_id.to_string(),
            scope_path: None,
            file_path: file_path.to_string(),
            node_kind: "function_definition".to_string(),
            origin_type: "workspace_symbol".to_string(),
            byte_range,
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
            dependencies: Vec::new(),
            references: Vec::new(),
        })
    }

    fn deadline() -> TraceQueryDeadline {
        TraceQueryDeadline::new(None).expect("unbounded deadline must be valid")
    }

    #[test]
    fn unresolved_reference_is_recorded_and_dropped() {
        let symbols = vec![symbol("pkg::present", "a.py", (0, 1))];
        let mut sink = DiagnosticsSink::default();
        let summaries = summarize_symbols_with_deadline(
            &symbols,
            &["pkg::missing".to_string()],
            None,
            &deadline(),
            Some(&mut sink),
        )
        .expect("summaries must build");

        assert!(summaries.is_empty());
        let records = sink.into_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].category, DiagnosticCategory::UnresolvedReference);
        assert_eq!(records[0].semantic_path.as_deref(), Some("pkg::missing"));
        assert_eq!(records[0].context_file, None);
    }

    #[test]
    fn ambiguous_reference_is_recorded_but_still_resolves_deterministically() {
        let symbols = vec![
            symbol("pkg::caller", "a.py", (0, 1)),
            symbol("pkg::caller", "b.py", (5, 6)),
        ];
        let mut sink = DiagnosticsSink::default();
        let summaries = summarize_symbols_with_deadline(
            &symbols,
            &["pkg::caller".to_string()],
            None,
            &deadline(),
            Some(&mut sink),
        )
        .expect("summaries must build");

        assert_eq!(summaries.len(), 1);
        // The deterministic tiebreak prefers the lexicographically smallest
        // file_path among equal-rank candidates.
        assert_eq!(summaries[0].file_path, "a.py");
        let records = sink.into_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].category, DiagnosticCategory::AmbiguousReference);
        assert_eq!(records[0].semantic_path.as_deref(), Some("pkg::caller"));
    }

    #[test]
    fn resolved_reference_records_no_diagnostic() {
        let symbols = vec![symbol("pkg::caller", "a.py", (0, 1))];
        let mut sink = DiagnosticsSink::default();
        let summaries = summarize_symbols_with_deadline(
            &symbols,
            &["pkg::caller".to_string()],
            None,
            &deadline(),
            Some(&mut sink),
        )
        .expect("summaries must build");

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].symbol_id, "pkg::caller");
        assert!(sink.records().is_empty());
    }

    #[test]
    fn trace_without_sink_keeps_public_behavior_unchanged() {
        let symbols = vec![symbol("pkg::caller", "a.py", (0, 1))];
        let summaries = summarize_symbols_with_deadline(
            &symbols,
            &["pkg::missing".to_string(), "pkg::caller".to_string()],
            None,
            &deadline(),
            None,
        )
        .expect("summaries must build");

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].symbol_id, "pkg::caller");
    }
}
