use std::sync::LazyLock;

use super::*;
use proptest::prelude::*;

/// Characters valid in identifier-like non-blank strings.
static IDENTIFIER_CHARACTERS: LazyLock<Vec<char>> =
    LazyLock::new(|| vec!['a', 'z', '0', '9', '_', 'A', 'Z']);

fn nonblank_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(&*IDENTIFIER_CHARACTERS), 1..=12)
        .prop_map(String::from_iter)
}

/// A summary whose evidence key matches the derived identity so it passes
/// replay-input validation. Position-suffixing keeps evidence keys unique
/// across decisions.
fn summary_strategy(position: usize) -> impl Strategy<Value = SymbolSummary> {
    (nonblank_strategy(), nonblank_strategy(), 1usize..32usize).prop_map(
        move |(symbol_id, file_path, end_byte)| {
            let symbol_id = format!("{position}::{symbol_id}");
            let node_kind = "function_definition".to_string();
            let origin_type = "patch_binding";
            let evidence_key =
                format!("{symbol_id}|{file_path}|{node_kind}|{origin_type}|0..{end_byte}|");
            SymbolSummary {
                symbol_id: symbol_id.clone(),
                semantic_path: format!("path::{symbol_id}"),
                scope_path: None,
                file_path,
                node_kind,
                origin_type: origin_type.to_string(),
                evidence_key,
                byte_range: (0, end_byte),
                signature: None,
                parameters: Vec::new(),
                return_type: None,
                docstring: None,
            }
        },
    )
}

/// Resolved decisions whose selected id mirrors their only candidate.
fn resolved_decisions_strategy() -> impl Strategy<Value = Vec<ValidationBindingDecision>> {
    prop::collection::vec((nonblank_strategy(), summary_strategy(0)), 0..=3).prop_map(|entries| {
        entries
            .into_iter()
            .enumerate()
            .map(|(position, (name, mut symbol))| {
                let suffixed_id = format!("{position}::{}", symbol.symbol_id);
                symbol.semantic_path = format!("path::{suffixed_id}");
                symbol.evidence_key = format!(
                    "{suffixed_id}|{}|{}|{}|{}..{}|",
                    symbol.file_path,
                    symbol.node_kind,
                    symbol.origin_type,
                    symbol.byte_range.0,
                    symbol.byte_range.1
                );
                symbol.symbol_id = suffixed_id;
                ValidationBindingDecision {
                    name: format!("{position}::{name}"),
                    status: "resolved".to_string(),
                    reason: "single candidate".to_string(),
                    selected_symbol_id: Some(symbol.symbol_id.clone()),
                    candidates: vec![symbol],
                }
            })
            .collect()
    })
}

/// Ambiguous decisions carrying two distinct-candidate summaries.
fn ambiguous_decisions_strategy() -> impl Strategy<Value = Vec<ValidationBindingDecision>> {
    prop::collection::vec(
        (
            nonblank_strategy(),
            summary_strategy(10),
            summary_strategy(20),
            nonblank_strategy(),
        ),
        0..=3,
    )
    .prop_map(|entries| {
        entries
            .into_iter()
            .enumerate()
            .map(
                |(position, (name, first, second, reason))| ValidationBindingDecision {
                    name: format!("{position}::{name}"),
                    status: "ambiguous".to_string(),
                    reason,
                    selected_symbol_id: None,
                    candidates: vec![first, second],
                },
            )
            .collect()
    })
}

/// Unresolved decisions without a selection or candidates.
fn unresolved_decisions_strategy() -> impl Strategy<Value = Vec<ValidationBindingDecision>> {
    prop::collection::vec(nonblank_strategy(), 0..=3).prop_map(|names| {
        names
            .into_iter()
            .enumerate()
            .map(|(position, name)| ValidationBindingDecision {
                name: format!("{position}::{name}"),
                status: "unresolved".to_string(),
                reason: "not found".to_string(),
                selected_symbol_id: None,
                candidates: Vec::new(),
            })
            .collect()
    })
}

/// A report whose summaries are derived from the given decisions in the
/// same order the checker expects.
fn report_from_decisions(decisions: Vec<ValidationBindingDecision>) -> PatchValidationReport {
    PatchValidationReport {
        syntax_errors: Vec::new(),
        unresolved_identifiers: decisions
            .iter()
            .filter(|decision| decision.status == "unresolved")
            .map(|decision| decision.name.clone())
            .collect(),
        resolved_identifiers: decisions
            .iter()
            .filter(|decision| decision.status == "resolved")
            .map(|decision| ValidationBinding {
                name: decision.name.clone(),
                symbol: decision.candidates[0].clone(),
            })
            .collect(),
        ambiguous_identifiers: decisions
            .iter()
            .filter(|decision| decision.status == "ambiguous")
            .map(|decision| ValidationAmbiguity {
                name: decision.name.clone(),
                candidates: decision.candidates.clone(),
                reason: decision.reason.clone(),
                disambiguation_context: DisambiguationContext::default(),
            })
            .collect(),
        binding_decisions: decisions,
        commit_gate: PatchCommitGateReport::default(),
    }
}

/// A mixed-status report with unique position-suffixed names and fully
/// derived summaries.
fn aligned_report_strategy() -> impl Strategy<Value = PatchValidationReport> {
    (
        resolved_decisions_strategy(),
        ambiguous_decisions_strategy(),
        unresolved_decisions_strategy(),
    )
        .prop_map(|(resolved, mut ambiguous, mut unresolved)| {
            let mut decisions = resolved;
            decisions.append(&mut ambiguous);
            decisions.append(&mut unresolved);
            // Rename with a global index so decision names stay unique
            // across status groups, then re-derive the summaries.
            for (position, decision) in decisions.iter_mut().enumerate() {
                let trimmed = decision.name.split("::").last().unwrap_or(&decision.name);
                decision.name = format!("{position}::{trimmed}");
            }
            report_from_decisions(decisions)
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Reports whose summaries mirror their binding decisions validate.
    #[test]
    fn aligned_reports_validate(report in aligned_report_strategy()) {
        prop_assert!(report.validate_trace_replay_input().is_ok());
    }

    /// Dropping a resolved summary entry breaks the derived-summary
    /// invariant and must be rejected.
    #[test]
    fn reports_reject_missing_resolved_summary(mut report in aligned_report_strategy()) {
        prop_assume!(!report.resolved_identifiers.is_empty());
        report.resolved_identifiers.pop();
        prop_assert!(report.validate_trace_replay_input().is_err());
    }

    /// Renaming an unresolved summary entry breaks the derived-summary
    /// invariant and must be rejected.
    #[test]
    fn reports_reject_unresolved_name_drift(mut report in aligned_report_strategy()) {
        prop_assume!(!report.unresolved_identifiers.is_empty());
        report.unresolved_identifiers[0] = format!("drifted::{}", report.unresolved_identifiers[0]);
        prop_assert!(report.validate_trace_replay_input().is_err());
    }

    /// Duplicating an ambiguous summary entry must trip the duplicate-name
    /// guard even when every other field stays aligned.
    #[test]
    fn reports_reject_duplicate_ambiguous_names(mut report in aligned_report_strategy()) {
        prop_assume!(!report.ambiguous_identifiers.is_empty());
        let duplicate = report.ambiguous_identifiers.last().unwrap().clone();
        report.ambiguous_identifiers.push(duplicate);
        prop_assert!(report.validate_trace_replay_input().is_err());
    }

    /// A full patch result pairing an allowed gate with consistent flags
    /// validates through validate_trace_replay_input.
    #[test]
    fn patch_results_with_allowed_gate_validate(seed in nonblank_strategy()) {
        let gate = PatchCommitGateReport {
            status: "allowed".to_string(),
            allowed: true,
            reason: "no blockers".to_string(),
            bypass_reason: None,
            blocking_decisions: Vec::new(),
            evidence_invariants: Vec::new(),
            syntax_error_count: 0,
        };
        let validation = PatchValidationReport {
            commit_gate: gate,
            ..PatchValidationReport::default()
        };
        let result = PatchAstNodeResult {
            file: format!("{seed}.py"),
            target_path: format!("src/{seed}.py"),
            resolved_path: format!("src/{seed}.py"),
            resolved_symbol_id: seed.clone(),
            applied: true,
            bypass_applied: false,
            updated_source: format!("def {seed}():\n    pass\n"),
            validation,
        };
        prop_assert!(result.validate_trace_replay_input().is_ok());
    }

    /// An applied patch whose gate is rejected must fail the pairing check.
    #[test]
    fn patch_results_reject_applied_flag_drift(seed in nonblank_strategy()) {
        let gate = PatchCommitGateReport {
            status: "rejected".to_string(),
            allowed: false,
            reason: "syntax errors present".to_string(),
            bypass_reason: None,
            blocking_decisions: Vec::new(),
            evidence_invariants: Vec::new(),
            syntax_error_count: 1,
        };
        let issue = ValidationIssue {
            kind: "error".to_string(),
            message: "broken".to_string(),
            start_byte: 0,
            end_byte: 3,
            start_point: Position { row: 0, column: 0 },
            end_point: Position { row: 0, column: 3 },
        };
        let validation = PatchValidationReport {
            syntax_errors: vec![issue],
            commit_gate: gate,
            ..PatchValidationReport::default()
        };
        let result = PatchAstNodeResult {
            file: format!("{seed}.py"),
            target_path: format!("src/{seed}.py"),
            resolved_path: format!("src/{seed}.py"),
            resolved_symbol_id: seed.clone(),
            applied: true,
            bypass_applied: false,
            updated_source: format!("def {seed}():\n    pass\n"),
            validation,
        };
        prop_assert!(result.validate_trace_replay_input().is_err());
    }

    /// Public output validation additionally rejects a blank updated source.
    #[test]
    fn patch_results_reject_blank_updated_source(seed in nonblank_strategy()) {
        let validation = PatchValidationReport {
            commit_gate: PatchCommitGateReport {
                status: "allowed".to_string(),
                allowed: true,
                reason: "no blockers".to_string(),
                bypass_reason: None,
                blocking_decisions: Vec::new(),
                evidence_invariants: Vec::new(),
                syntax_error_count: 0,
            },
            ..PatchValidationReport::default()
        };
        let result = PatchAstNodeResult {
            file: format!("{seed}.py"),
            target_path: format!("src/{seed}.py"),
            resolved_path: format!("src/{seed}.py"),
            resolved_symbol_id: seed.clone(),
            applied: true,
            bypass_applied: false,
            updated_source: "   ".to_string(),
            validation,
        };
        prop_assert!(result.validate_public_output().is_err());
    }
}
