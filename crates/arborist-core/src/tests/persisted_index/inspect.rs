use super::*;

#[test]
fn inspect_symbol_index_rejects_invalid_timeout_before_opening_database() {
    let dir = temporary_dir();
    let db_path = dir.join("missing.db");

    for timeout_ms in [0, MAX_WORKSPACE_SCAN_TIMEOUT_MS + 1] {
        let error = inspect_symbol_index_with_timeout(&db_path, Some(timeout_ms)).expect_err(
            "invalid inspection timeout should be rejected before reading the database",
        );

        assert!(
            error
                .to_string()
                .contains("invalid workspace scan timeout_ms")
        );
    }
    assert!(!db_path.exists());
}

#[test]
fn inspect_symbol_index_reports_healthy_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let health = inspect_symbol_index(&db_path).unwrap();

    assert_eq!(health.response_schema_version, "4");
    assert!(health.exists);
    assert!(health.ok);
    assert_eq!(health.schema_version.as_deref(), Some("4"));
    assert_eq!(health.expected_schema_version, "4");
    assert!(!health.migration.required);
    assert_eq!(health.migration.action, "none");
    assert_eq!(
        health.workspace_root.as_deref(),
        Some(normalize_path(&dir).as_str())
    );
    assert_eq!(health.indexed_files, Some(1));
    assert_eq!(health.indexed_symbols, Some(1));
    assert_eq!(health.file_state_entries, Some(1));
    assert_eq!(health.fresh_file_count, Some(1));
    assert!(health.stale_files.is_empty());
    assert!(health.missing_files.is_empty());
    assert!(health.unreadable_files.is_empty());
    assert!(health.unindexed_files.is_empty());
    assert!(health.issues.is_empty());
}

#[test]
fn inspect_and_queries_reject_unindexed_workspace_files() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let added = dir.join("added.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(&added, "def added() -> int:\n    return 2\n").unwrap();

    let health = inspect_symbol_index(&db_path).unwrap();

    assert!(!health.ok);
    assert_eq!(health.unindexed_files, vec![normalize_path(&added)]);
    assert!(
        health
            .issues
            .iter()
            .any(|issue| issue.contains("workspace source file is not indexed"))
    );

    for error in [
        read_symbol_from_index(&db_path, "helper")
            .expect_err("read_symbol should reject incomplete persisted indexes")
            .to_string(),
        search_symbols_from_index(&db_path, "helper", 10)
            .expect_err("search_symbols should reject incomplete persisted indexes")
            .to_string(),
        trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
            .expect_err("trace_symbol_graph should reject incomplete persisted indexes")
            .to_string(),
    ] {
        assert!(error.contains("symbol index"));
        assert!(error.contains("is stale"));
        assert!(error.contains("workspace source file is not indexed"));
        assert!(error.contains("added.py"));
    }
}

#[test]
fn inspect_and_queries_reject_inconsistent_indexed_file_counts() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute(
            "UPDATE metadata SET value = '2' WHERE key = 'indexed_files'",
            [],
        )
        .unwrap();
    drop(connection);

    let health = inspect_symbol_index(&db_path).unwrap();

    assert!(!health.ok);
    assert!(
        health
            .issues
            .iter()
            .any(|issue| issue.contains("does not match file_state entries"))
    );

    for error in [
        read_symbol_from_index(&db_path, "helper")
            .expect_err("read_symbol should reject inconsistent file counts")
            .to_string(),
        search_symbols_from_index(&db_path, "helper", 10)
            .expect_err("search_symbols should reject inconsistent file counts")
            .to_string(),
        trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
            .expect_err("trace_symbol_graph should reject inconsistent file counts")
            .to_string(),
    ] {
        assert!(error.contains("indexed_files metadata 2"));
        assert!(error.contains("file_state entries 1"));
    }
}

#[test]
fn inspect_and_queries_reject_persisted_symbol_paths_outside_workspace() {
    let root = temporary_dir();
    let dir = root.join("workspace");
    let outside = root.join("outside.py");
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&dir).unwrap();
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(&outside, "def outside() -> int:\n    return 2\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute(
            "UPDATE symbols SET file_path = ?1 WHERE semantic_path = 'helper'",
            [normalize_path(&outside)],
        )
        .unwrap();
    drop(connection);

    let health = inspect_symbol_index(&db_path).unwrap();
    assert!(!health.ok);
    assert!(
        health
            .issues
            .iter()
            .any(|issue| issue.contains("symbols.file_path")
                && issue.contains("outside indexed workspace"))
    );

    let error = read_symbol_from_index(&db_path, "helper")
        .expect_err("persisted reads must reject symbol paths outside the workspace");
    assert!(error.to_string().contains("symbols.file_path"));
    assert!(error.to_string().contains("outside indexed workspace"));
}

#[test]
fn inspect_and_queries_reject_persisted_file_states_outside_workspace() {
    let root = temporary_dir();
    let dir = root.join("workspace");
    let outside = root.join("outside.py");
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&dir).unwrap();
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(&outside, "def outside() -> int:\n    return 2\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute(
            "UPDATE file_state SET file_path = ?1",
            [normalize_path(&outside)],
        )
        .unwrap();
    drop(connection);

    let health = inspect_symbol_index(&db_path).unwrap();
    assert!(!health.ok);
    assert_eq!(health.fresh_file_count, None);
    assert!(
        health
            .issues
            .iter()
            .any(|issue| issue.contains("file_state.file_path")
                && issue.contains("outside indexed workspace"))
    );

    let error = read_symbol_from_index(&db_path, "helper")
        .expect_err("persisted reads must reject file states outside the workspace");
    assert!(error.to_string().contains("file_state.file_path"));
    assert!(error.to_string().contains("outside indexed workspace"));
}

#[test]
fn inspect_and_queries_reject_persisted_symbol_paths_in_ignored_directories() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let ignored = dir.join(".venv").join("installed.py");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(ignored.parent().unwrap()).unwrap();
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(&ignored, "def installed() -> int:\n    return 2\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute(
            "UPDATE symbols SET file_path = ?1 WHERE semantic_path = 'helper'",
            [normalize_path(&ignored)],
        )
        .unwrap();
    drop(connection);

    let health = inspect_symbol_index(&db_path).unwrap();
    assert!(!health.ok);
    assert!(health.issues.iter().any(|issue| {
        issue.contains("symbols.file_path") && issue.contains("ignored workspace directory")
    }));

    let error = read_symbol_from_index(&db_path, "helper")
        .expect_err("persisted reads must reject symbol paths in ignored directories");
    assert!(error.to_string().contains("symbols.file_path"));
    assert!(error.to_string().contains("ignored workspace directory"));
}

#[test]
fn inspect_and_queries_reject_persisted_file_states_in_ignored_directories() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let ignored = dir.join("node_modules").join("installed.py");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(ignored.parent().unwrap()).unwrap();
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(&ignored, "def installed() -> int:\n    return 2\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute(
            "UPDATE file_state SET file_path = ?1",
            [normalize_path(&ignored)],
        )
        .unwrap();
    drop(connection);

    let health = inspect_symbol_index(&db_path).unwrap();
    assert!(!health.ok);
    assert_eq!(health.fresh_file_count, None);
    assert!(health.issues.iter().any(|issue| {
        issue.contains("file_state.file_path") && issue.contains("ignored workspace directory")
    }));

    let error = read_symbol_from_index(&db_path, "helper")
        .expect_err("persisted reads must reject file states in ignored directories");
    assert!(error.to_string().contains("file_state.file_path"));
    assert!(error.to_string().contains("ignored workspace directory"));
}

#[test]
fn inspect_and_queries_reject_non_normalized_workspace_root_metadata() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    for invalid_workspace_root in [
        "relative/workspace".to_string(),
        normalize_path(&dir.join("nested").join("..")),
    ] {
        let connection = Connection::open(&db_path).unwrap();
        connection
            .execute(
                "UPDATE metadata SET value = ?1 WHERE key = 'workspace_root'",
                [&invalid_workspace_root],
            )
            .unwrap();
        drop(connection);

        let health = inspect_symbol_index(&db_path).unwrap();
        assert!(!health.ok);
        assert!(
            health
                .issues
                .iter()
                .any(|issue| issue.contains("workspace_root metadata")
                    && issue.contains("normalized absolute path"))
        );

        let error = read_symbol_from_index(&db_path, "helper")
            .expect_err("persisted reads must reject non-normalized workspace roots");
        assert!(error.to_string().contains("workspace_root metadata"));
        assert!(error.to_string().contains("normalized absolute path"));

        let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
            .expect_err("persisted refreshes must reject non-normalized workspace roots");
        assert!(error.to_string().contains("workspace_root metadata"));
        assert!(error.to_string().contains("normalized absolute path"));

        let error = rebuild_symbol_index(&dir, &db_path)
            .expect_err("persisted rebuilds must reject non-normalized workspace roots");
        assert!(error.to_string().contains("workspace_root metadata"));
        assert!(error.to_string().contains("normalized absolute path"));
    }
}

#[test]
fn inspect_and_queries_reject_empty_persisted_scope_paths() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    for invalid_scope_path in ["", " \t"] {
        let connection = Connection::open(&db_path).unwrap();
        connection
            .execute(
                "UPDATE symbols SET scope_path = ?1 WHERE semantic_path = 'helper'",
                [invalid_scope_path],
            )
            .unwrap();
        drop(connection);

        let health = inspect_symbol_index(&db_path).unwrap();
        assert!(!health.ok);
        assert!(
            health
                .issues
                .iter()
                .any(|issue| issue.contains("empty scope_path"))
        );

        let error = read_symbol_from_index(&db_path, "helper")
            .expect_err("persisted reads must reject empty scope paths");
        assert!(error.to_string().contains("empty scope_path"));

        let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
            .expect_err("persisted refreshes must reject empty scope paths");
        assert!(error.to_string().contains("empty scope_path"));
    }
}

#[test]
fn inspect_and_queries_reject_inconsistent_persisted_scope_paths() {
    let dir = temporary_dir();
    let source_path = dir.join("module.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int helper() { return 1; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute(
            "UPDATE symbols SET scope_path = 'other' WHERE semantic_path = 'api::helper'",
            [],
        )
        .unwrap();
    drop(connection);

    let health = inspect_symbol_index(&db_path).unwrap();
    assert!(!health.ok);
    assert!(
        health.issues.iter().any(|issue| {
            issue.contains("scope_path does not match semantic_path `api::helper`")
        })
    );

    let error = read_symbol_from_index(&db_path, "api::helper")
        .expect_err("persisted reads must reject inconsistent scope paths");
    assert!(
        error
            .to_string()
            .contains("scope_path does not match semantic_path `api::helper`")
    );

    let error = refresh_symbol_index_for_file(&dir, &db_path, &source_path)
        .expect_err("persisted refreshes must reject inconsistent scope paths");
    assert!(
        error
            .to_string()
            .contains("scope_path does not match semantic_path `api::helper`")
    );
}

#[test]
fn inspect_and_queries_reject_persisted_byte_ranges_outside_source() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute(
            "UPDATE symbols SET end_byte = 999 WHERE semantic_path = 'helper'",
            [],
        )
        .unwrap();
    drop(connection);

    let expected_error = "persisted symbol byte range";
    let health = inspect_symbol_index(&db_path).unwrap();
    assert!(!health.ok);
    assert!(
        health
            .issues
            .iter()
            .any(|issue| issue.contains(expected_error))
    );

    for error in [
        read_symbol_from_index(&db_path, "helper")
            .expect_err("persisted reads must reject byte ranges outside source")
            .to_string(),
        trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
            .expect_err("persisted traces must reject byte ranges outside source")
            .to_string(),
        refresh_symbol_index_for_file(&dir, &db_path, &helper)
            .expect_err("persisted refreshes must reject byte ranges outside source")
            .to_string(),
    ] {
        assert!(error.contains(expected_error), "{error}");
    }
}

#[test]
fn inspect_queries_and_refresh_reject_inconsistent_persisted_call_arities() {
    let dir = temporary_dir();
    let helper = dir.join("helper.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper,
        "int helper(int value) { return value; }\nint caller() { return helper(1); }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute(
            "UPDATE symbols SET reference_names_json = '[]' WHERE semantic_path = 'caller'",
            [],
        )
        .unwrap();
    drop(connection);

    let expected_error =
        "reference_call_arities_json contains a name absent from reference_names_json";
    let health = inspect_symbol_index(&db_path).unwrap();
    assert!(!health.ok);
    assert!(
        health
            .issues
            .iter()
            .any(|issue| issue.contains(expected_error))
    );

    for error in [
        read_symbol_from_index(&db_path, "caller")
            .expect_err("persisted reads must reject inconsistent call arities")
            .to_string(),
        search_symbols_from_index(&db_path, "caller", 10)
            .expect_err("persisted searches must reject inconsistent call arities")
            .to_string(),
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both)
            .expect_err("persisted traces must reject inconsistent call arities")
            .to_string(),
        refresh_symbol_index_for_file(&dir, &db_path, &helper)
            .expect_err("persisted refreshes must reject inconsistent call arities")
            .to_string(),
    ] {
        assert!(error.contains(expected_error), "{error}");
    }
}

#[test]
fn inspect_and_queries_reject_persisted_graph_edges_to_missing_symbols() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute(
            "UPDATE symbols SET dependencies_json = '[\"missing\"]' WHERE semantic_path = 'helper'",
            [],
        )
        .unwrap();
    drop(connection);

    let health = inspect_symbol_index(&db_path).unwrap();
    assert!(!health.ok);
    assert!(health.issues.iter().any(|issue| {
        issue.contains("persisted dependency `missing` for symbol `helper` does not exist")
    }));

    for error in [
        read_symbol_from_index(&db_path, "helper")
            .expect_err("persisted reads must reject missing dependency targets")
            .to_string(),
        trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
            .expect_err("persisted traces must reject missing dependency targets")
            .to_string(),
        refresh_symbol_index_for_file(&dir, &db_path, &helper)
            .expect_err("persisted refreshes must reject missing dependency targets")
            .to_string(),
    ] {
        assert!(
            error.contains("persisted dependency `missing` for symbol `helper` does not exist"),
            "{error}"
        );
    }
}

#[test]
fn inspect_and_queries_reject_inconsistent_persisted_graph_edges() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper,
        "def helper() -> int:\n    return 1\n\ndef caller() -> int:\n    return helper()\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute(
            "UPDATE symbols SET references_json = '[]' WHERE semantic_path = 'helper'",
            [],
        )
        .unwrap();
    drop(connection);

    let expected_error =
        "persisted dependency `helper` for symbol `caller` has no matching reference";
    let health = inspect_symbol_index(&db_path).unwrap();
    assert!(!health.ok);
    assert!(
        health
            .issues
            .iter()
            .any(|issue| issue.contains(expected_error))
    );

    for error in [
        read_symbol_from_index(&db_path, "caller")
            .expect_err("persisted reads must reject inconsistent graph edges")
            .to_string(),
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both)
            .expect_err("persisted traces must reject inconsistent graph edges")
            .to_string(),
        refresh_symbol_index_for_file(&dir, &db_path, &helper)
            .expect_err("persisted refreshes must reject inconsistent graph edges")
            .to_string(),
    ] {
        assert!(error.contains(expected_error), "{error}");
    }
}

#[test]
fn inspect_and_queries_reject_empty_persisted_symbol_metadata() {
    for (column, invalid_value, expected_error) in [
        ("signature", " \t", "empty signature"),
        ("parameters_json", "[\"\"]", "empty parameters_json entry"),
        ("return_type", " \t", "empty return_type"),
        ("docstring", " \t", "empty docstring"),
    ] {
        let dir = temporary_dir();
        let helper = dir.join("helper.py");
        let db_path = dir.join("symbols.db");
        fs::write(
            &helper,
            "\"\"\"Helper documentation.\"\"\"\ndef helper(value: int) -> int:\n    return value\n",
        )
        .unwrap();
        rebuild_symbol_index(&dir, &db_path).unwrap();

        let connection = Connection::open(&db_path).unwrap();
        connection
            .execute(
                &format!("UPDATE symbols SET {column} = ?1 WHERE semantic_path = 'helper'"),
                [invalid_value],
            )
            .unwrap();
        drop(connection);

        let health = inspect_symbol_index(&db_path).unwrap();
        assert!(
            !health.ok,
            "expected {column} corruption to make index unhealthy"
        );
        assert!(
            health
                .issues
                .iter()
                .any(|issue| issue.contains(expected_error)),
            "expected {column} corruption issue, got {:#?}",
            health.issues
        );

        let error = read_symbol_from_index(&db_path, "helper")
            .expect_err("persisted reads must reject empty symbol metadata");
        assert!(error.to_string().contains(expected_error));

        let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
            .expect_err("persisted refreshes must reject empty symbol metadata");
        assert!(error.to_string().contains(expected_error));
    }
}

#[test]
fn persisted_queries_reject_symbol_paths_for_unsupported_files() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let secret = dir.join("secret.txt");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(&secret, "not source data\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute(
            "UPDATE symbols SET file_path = ?1 WHERE semantic_path = 'helper'",
            [normalize_path(&secret)],
        )
        .unwrap();
    drop(connection);

    let error = read_symbol_from_index(&db_path, "helper")
        .expect_err("persisted reads must reject paths for unsupported source types");
    assert!(error.to_string().contains("symbols.file_path"));
    assert!(error.to_string().contains("not a supported source file"));
}

#[test]
fn inspect_symbol_index_reports_stale_files() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(&helper, "def helper() -> int:\n    return 2\n").unwrap();

    let health = inspect_symbol_index(&db_path).unwrap();

    assert!(health.exists);
    assert!(!health.ok);
    assert_eq!(health.file_state_entries, Some(1));
    assert_eq!(health.fresh_file_count, Some(0));
    assert_eq!(health.stale_files, vec![normalize_path(&helper)]);
    assert!(health.missing_files.is_empty());
    assert!(health.unreadable_files.is_empty());
    assert!(
        health
            .issues
            .iter()
            .any(|issue| issue.contains("indexed file is stale"))
    );
}

#[test]
fn persisted_index_queries_reject_stale_file_states() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(&helper, "def helper() -> int:\n    return 2\n").unwrap();

    for error in [
        read_symbol_from_index(&db_path, "helper")
            .expect_err("read_symbol should reject stale persisted indexes")
            .to_string(),
        search_symbols_from_index(&db_path, "helper", 10)
            .expect_err("search_symbols should reject stale persisted indexes")
            .to_string(),
        trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
            .expect_err("trace_symbol_graph should reject stale persisted indexes")
            .to_string(),
    ] {
        assert!(error.contains("symbol index"));
        assert!(error.contains("is stale"));
        assert!(error.contains("indexed file is stale"));
        assert!(error.contains("helper.py"));
    }
}

#[test]
fn inspect_symbol_index_reports_missing_indexed_files() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::remove_file(&helper).unwrap();

    let health = inspect_symbol_index(&db_path).unwrap();

    assert!(health.exists);
    assert!(!health.ok);
    assert_eq!(health.file_state_entries, Some(1));
    assert_eq!(health.fresh_file_count, Some(0));
    assert!(health.stale_files.is_empty());
    assert_eq!(health.missing_files, vec![normalize_path(&helper)]);
    assert!(health.unreadable_files.is_empty());
    assert!(
        health
            .issues
            .iter()
            .any(|issue| issue.contains("indexed file is missing"))
    );
}

#[test]
fn inspect_symbol_index_reports_missing_database_without_creating_it() {
    let dir = temporary_dir();
    let db_path = dir.join("missing.db");

    let health = inspect_symbol_index(&db_path).unwrap();

    assert_eq!(health.response_schema_version, "4");
    assert!(!health.exists);
    assert!(!health.ok);
    assert!(health.migration.required);
    assert_eq!(health.migration.action, "rebuild");
    assert!(health.issues[0].contains("does not exist"));
    assert!(!db_path.exists());
}

#[test]
fn inspect_symbol_index_reports_manual_action_when_database_cannot_be_opened() {
    let dir = temporary_dir();
    let db_path = dir.join("not-a-database");
    fs::create_dir_all(&db_path).unwrap();

    let health = inspect_symbol_index(&db_path).unwrap();

    assert!(health.exists);
    assert!(!health.ok);
    assert!(health.schema_version.is_none());
    assert!(health.migration.required);
    assert_eq!(health.migration.action, "manual");
    assert!(
        health
            .issues
            .iter()
            .any(|issue| issue.contains("failed to open symbol index"))
    );
    assert!(db_path.is_dir());
}

#[test]
fn inspect_symbol_index_reports_manual_action_for_non_index_database() {
    let dir = temporary_dir();
    let db_path = dir.join("not-symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute("CREATE TABLE symbols (name TEXT NOT NULL)", [])
        .unwrap();
    drop(connection);

    let health = inspect_symbol_index(&db_path).unwrap();

    assert!(health.exists);
    assert!(!health.ok);
    assert!(health.migration.required);
    assert_eq!(health.migration.action, "manual");
    assert!(
        health
            .migration
            .reason
            .contains("not a complete Arborist symbol index")
    );
}
