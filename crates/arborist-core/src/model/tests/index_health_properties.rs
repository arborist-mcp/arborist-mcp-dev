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

fn file_path_strategy() -> impl Strategy<Value = String> {
    nonblank_strategy().prop_map(|name| format!("src/{name}.py"))
}

/// A healthy index report with consistent freshness counts and a complete
/// current inspection.
fn healthy_report_strategy() -> impl Strategy<Value = SymbolIndexHealth> {
    (
        file_path_strategy(),
        1usize..8usize,
        prop::collection::vec(file_path_strategy(), 0..=3),
    )
        .prop_map(|(db_path, fresh_file_count, stale_names)| {
            let missing_files: Vec<String> = Vec::new();
            let unreadable_files: Vec<String> = Vec::new();
            // Position suffixes keep freshness paths unique; the checker
            // rejects duplicate paths across categories.
            let stale_files: Vec<String> = stale_names
                .into_iter()
                .enumerate()
                .map(|(position, name)| format!("src/stale{position}/{name}.py"))
                .collect();
            let file_state_entries =
                fresh_file_count + stale_files.len() + missing_files.len() + unreadable_files.len();
            SymbolIndexHealth {
                response_schema_version: "4".to_string(),
                db_path,
                exists: true,
                ok: true,
                schema_version: Some("cpp-v1".to_string()),
                expected_schema_version: "cpp-v1".to_string(),
                migration: SymbolIndexMigrationPlan::none("up to date"),
                workspace_root: Some("E:/workspace/sample".to_string()),
                indexed_files: Some(10),
                indexed_symbols: Some(40),
                file_state_entries: Some(file_state_entries),
                fresh_file_count: Some(fresh_file_count),
                stale_files,
                missing_files,
                unreadable_files,
                unindexed_files: Vec::new(),
                issues: Vec::new(),
            }
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// A healthy report with consistent freshness counts validates.
    #[test]
    fn healthy_reports_validate(report in healthy_report_strategy()) {
        prop_assert!(report.validate_public_output().is_ok());
    }

    /// Freshness count conservation must be enforced: inflating any single
    /// category without removing another breaks the sum and is rejected.
    #[test]
    fn reports_reject_freshness_count_drift(
        mut report in healthy_report_strategy(),
        extra in 1usize..=3usize,
        which in 0usize..2,
    ) {
        prop_assume!(report.fresh_file_count.unwrap() >= extra);
        match which {
            // Shrinking file_state_entries below the category sum.
            0 => {
                report.file_state_entries =
                    Some(report.file_state_entries.unwrap().saturating_sub(extra));
            }
            // Adding a stale file without updating file_state_entries.
            _ => {
                let path = format!("src/extra{}.py", report.stale_files.len());
                report.stale_files.push(path);
            }
        }
        prop_assert!(report.validate_public_output().is_err());
    }

    /// Health/issue contradictions must be rejected in both directions, and a
    /// healthy report must not require migration.
    #[test]
    fn reports_reject_health_contradictions(mut report in healthy_report_strategy()) {
        // Healthy but claiming issues.
        report.issues.push("stale schema".to_string());
        prop_assert!(report.validate_public_output().is_err());
    }

    /// Stats whose rebuilt+reused does not equal indexed_files are rejected;
    /// consistent stats validate.
    #[test]
    fn stats_reject_inconsistent_counts(
        db_path in file_path_strategy(),
        rebuilt in 0usize..16usize,
        reused in 0usize..16usize,
        drift in 1usize..=4usize,
    ) {
        let indexed_files = rebuilt + reused;
        let stats = SymbolIndexStats {
            db_path: db_path.clone(),
            indexed_files,
            indexed_symbols: 0,
            rebuilt_files: rebuilt,
            reused_files: reused,
        };
        prop_assert!(stats.validate_public_output().is_ok());

        let broken = SymbolIndexStats {
            db_path,
            indexed_files: indexed_files + drift,
            indexed_symbols: 0,
            rebuilt_files: rebuilt,
            reused_files: reused,
        };
        prop_assert!(broken.validate_public_output().is_err());
    }
}
