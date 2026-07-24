use super::*;

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
