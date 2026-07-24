use super::*;

#[test]
fn symbol_index_health_rejects_required_migration_without_action() {
    let health = SymbolIndexHealth {
        response_schema_version: "4".to_string(),
        db_path: "symbols.db".to_string(),
        exists: false,
        ok: false,
        schema_version: None,
        expected_schema_version: "4".to_string(),
        migration: SymbolIndexMigrationPlan {
            required: true,
            action: "none".to_string(),
            reason: "index must be rebuilt".to_string(),
        },
        workspace_root: None,
        indexed_files: None,
        indexed_symbols: None,
        file_state_entries: None,
        fresh_file_count: None,
        stale_files: Vec::new(),
        missing_files: Vec::new(),
        unreadable_files: Vec::new(),
        unindexed_files: Vec::new(),
        issues: vec!["symbol index does not exist".to_string()],
    };

    let error = health
        .validate_public_output()
        .expect_err("required migrations must provide a concrete action");

    assert!(error.to_string().contains("migration.required"));
}

#[test]
fn symbol_index_health_rejects_non_rebuild_action_for_missing_index() {
    let health = SymbolIndexHealth {
        response_schema_version: "4".to_string(),
        db_path: "symbols.db".to_string(),
        exists: false,
        ok: false,
        schema_version: None,
        expected_schema_version: "4".to_string(),
        migration: SymbolIndexMigrationPlan {
            required: true,
            action: "manual".to_string(),
            reason: "index cannot be opened".to_string(),
        },
        workspace_root: None,
        indexed_files: None,
        indexed_symbols: None,
        file_state_entries: None,
        fresh_file_count: None,
        stale_files: Vec::new(),
        missing_files: Vec::new(),
        unreadable_files: Vec::new(),
        unindexed_files: Vec::new(),
        issues: vec!["symbol index does not exist".to_string()],
    };

    let error = health
        .validate_public_output()
        .expect_err("missing indexes must recommend rebuild");

    assert!(error.to_string().contains("migration.action"));
}

#[test]
fn symbol_index_health_rejects_incomplete_healthy_inspection() {
    let health = SymbolIndexHealth {
        response_schema_version: "4".to_string(),
        db_path: "symbols.db".to_string(),
        exists: true,
        ok: true,
        schema_version: Some("4".to_string()),
        expected_schema_version: "4".to_string(),
        migration: SymbolIndexMigrationPlan {
            required: false,
            action: "none".to_string(),
            reason: "index schema and persisted file fingerprints are current".to_string(),
        },
        workspace_root: Some("workspace".to_string()),
        indexed_files: Some(1),
        indexed_symbols: Some(1),
        file_state_entries: Some(1),
        fresh_file_count: None,
        stale_files: Vec::new(),
        missing_files: Vec::new(),
        unreadable_files: Vec::new(),
        unindexed_files: Vec::new(),
        issues: Vec::new(),
    };

    let error = health
        .validate_public_output()
        .expect_err("healthy indexes must include a complete inspection snapshot");

    assert!(error.to_string().contains("complete current inspection"));
}

#[test]
fn symbol_index_health_rejects_duplicate_freshness_file_paths() {
    let health = SymbolIndexHealth {
        response_schema_version: "4".to_string(),
        db_path: "symbols.db".to_string(),
        exists: true,
        ok: false,
        schema_version: Some("4".to_string()),
        expected_schema_version: "4".to_string(),
        migration: SymbolIndexMigrationPlan {
            required: true,
            action: "rebuild".to_string(),
            reason: "index health checks failed".to_string(),
        },
        workspace_root: Some("workspace".to_string()),
        indexed_files: Some(1),
        indexed_symbols: Some(1),
        file_state_entries: Some(2),
        fresh_file_count: Some(0),
        stale_files: vec!["workspace/helper.py".to_string()],
        missing_files: vec!["workspace/helper.py".to_string()],
        unreadable_files: Vec::new(),
        unindexed_files: Vec::new(),
        issues: vec!["indexed file is stale".to_string()],
    };

    let error = health
        .validate_public_output()
        .expect_err("freshness categories must not overlap");

    assert!(error.to_string().contains("duplicate freshness file paths"));
}

#[test]
fn symbol_index_stats_reject_unknown_fields() {
    let error = serde_json::from_str::<SymbolIndexStats>(
        r#"{
                "db_path":"symbols.db",
                "indexed_files":1,
                "indexed_symbols":2,
                "rebuilt_files":1,
                "reused_files":0,
                "unexpected":true
            }"#,
    )
    .expect_err("symbol index stats should reject unknown fields");

    assert!(error.to_string().contains("unknown field `unexpected`"));
}

#[test]
fn symbol_index_stats_validation_rejects_inconsistent_totals() {
    let stats = SymbolIndexStats {
        db_path: "symbols.db".to_string(),
        indexed_files: 3,
        indexed_symbols: 4,
        rebuilt_files: 1,
        reused_files: 1,
    };

    let error = stats
        .validate_public_output()
        .expect_err("symbol index stats validation should reject inconsistent totals");

    assert!(error.to_string().contains("symbol_index.indexed_files"));
}
