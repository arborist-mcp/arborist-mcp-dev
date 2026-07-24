use super::*;

#[test]
fn trace_from_missing_symbol_index_does_not_create_database() {
    let dir = temporary_dir();
    let missing_db_path = dir.join("missing-symbols.db");

    let error =
        trace_symbol_graph_from_index(&missing_db_path, "orchestrate", TraceDirection::Both)
            .unwrap_err();

    assert!(error.to_string().contains("does not exist"));
    assert!(!missing_db_path.exists());
}

#[test]
fn rejects_blank_trace_symbol_paths() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("helper.py"),
        "def helper() -> int:\n    return 1\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let error = trace_symbol_graph_from_index(&db_path, " \t", TraceDirection::Both)
        .expect_err("blank trace symbol paths should be rejected");

    assert!(error.to_string().contains("symbol_path"));
    assert!(error.to_string().contains("blank"));
}

#[test]
fn trace_from_existing_non_index_database_does_not_create_schema() {
    let dir = temporary_dir();
    let db_path = dir.join("not-symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    drop(connection);

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("empty databases should not be initialized by read paths");

    assert!(error.to_string().contains("missing symbol index table"));

    let connection = Connection::open(&db_path).unwrap();
    let table_count: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(table_count, 0);
}

#[test]
fn trace_from_index_rejects_negative_persisted_byte_ranges() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_minimal_symbol_index_schema(&connection);
    connection
        .execute_batch(
            "
                INSERT INTO symbols (
                    symbol_id, semantic_path, file_path, node_kind, start_byte, end_byte,
                    parameters_json, dependencies_json, references_json, reference_names_json
                ) VALUES (
                    'helper', 'helper', 'helper.py', 'function_definition', -1, 5,
                    '[]', '[]', '[]', '[]'
                );
                ",
        )
        .unwrap();

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("negative persisted byte ranges should be rejected");

    assert!(error.to_string().contains("non-negative integer"));
}

#[test]
fn trace_from_index_rejects_invalid_persisted_json_columns() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_minimal_symbol_index_schema(&connection);
    connection
        .execute_batch(
            "
                INSERT INTO symbols (
                    symbol_id, semantic_path, file_path, node_kind, start_byte, end_byte,
                    parameters_json, dependencies_json, references_json, reference_names_json
                ) VALUES (
                    'helper', 'helper', 'helper.py', 'function_definition', 0, 5,
                    '[]', '{not-json', '[]', '[]'
                );
                ",
        )
        .unwrap();

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("invalid persisted JSON columns should be rejected");

    assert!(error.to_string().contains("Conversion error"));
}

#[test]
fn trace_from_index_rejects_empty_persisted_symbol_identity() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_minimal_symbol_index_schema(&connection);
    connection
        .execute_batch(
            "
                INSERT INTO symbols (
                    symbol_id, semantic_path, file_path, node_kind, start_byte, end_byte,
                    parameters_json, dependencies_json, references_json, reference_names_json
                ) VALUES (
                    '', 'helper', 'helper.py', 'function_definition', 0, 5,
                    '[]', '[]', '[]', '[]'
                );
                ",
        )
        .unwrap();

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("empty persisted symbol identity should be rejected");

    assert!(error.to_string().contains("empty symbol_id"));
}

#[test]
fn trace_from_index_rejects_empty_persisted_graph_edges() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_minimal_symbol_index_schema(&connection);
    connection
        .execute_batch(
            "
                INSERT INTO symbols (
                    symbol_id, semantic_path, file_path, node_kind, start_byte, end_byte,
                    parameters_json, dependencies_json, references_json, reference_names_json
                ) VALUES (
                    'helper', 'helper', 'helper.py', 'function_definition', 0, 5,
                    '[]', '[\"\"]', '[]', '[]'
                );
                ",
        )
        .unwrap();

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("empty persisted graph edges should be rejected");

    assert!(error.to_string().contains("empty dependencies_json entry"));
}

#[test]
fn trace_from_index_rejects_invalid_indexed_files_metadata() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_minimal_symbol_index_schema(&connection);
    connection
        .execute(
            "UPDATE metadata SET value = 'many' WHERE key = 'indexed_files'",
            [],
        )
        .unwrap();

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("invalid indexed_files metadata should be rejected");

    assert!(error.to_string().contains("invalid indexed_files metadata"));
}

#[test]
fn trace_from_index_rejects_missing_indexed_files_metadata() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_minimal_symbol_index_schema(&connection);
    connection
        .execute(
            "INSERT INTO metadata(key, value) VALUES('workspace_root', ?1)",
            [normalize_path(&dir)],
        )
        .unwrap();
    connection
        .execute("DELETE FROM metadata WHERE key = 'indexed_files'", [])
        .unwrap();
    drop(connection);

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("missing indexed_files metadata should be rejected");

    assert!(error.to_string().contains("missing indexed_files metadata"));
}
