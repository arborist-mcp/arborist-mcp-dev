use super::*;

#[test]
fn refresh_existing_database_with_unrelated_symbols_table_does_not_migrate() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("not-symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute("CREATE TABLE symbols (name TEXT NOT NULL)", [])
        .unwrap();
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject databases with non-index symbols tables");

    assert!(error.to_string().contains("missing symbol index table"));
    let connection = Connection::open(&db_path).unwrap();
    let created_table_count: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name IN ('metadata', 'file_state')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(created_table_count, 0);
}

#[test]
fn rebuild_existing_database_with_unrelated_symbols_table_does_not_migrate() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("not-symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute("CREATE TABLE symbols (name TEXT NOT NULL)", [])
        .unwrap();
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = rebuild_symbol_index(&dir, &db_path)
        .expect_err("rebuild should reject databases with non-index symbols tables");

    assert!(error.to_string().contains("missing symbol index table"));
    let connection = Connection::open(&db_path).unwrap();
    assert_eq!(symbol_table_columns(&connection), vec!["name"]);
    let created_table_count: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name IN ('metadata', 'file_state')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(created_table_count, 0);
}

#[test]
fn refresh_existing_database_with_incomplete_symbol_columns_does_not_migrate() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("not-symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    create_incomplete_symbol_index_tables(&connection);
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject tables that only resemble symbol indexes");

    assert!(error.to_string().contains("missing required column"));
    let connection = Connection::open(&db_path).unwrap();
    assert_eq!(symbol_table_columns(&connection), vec!["name"]);
}

#[test]
fn refresh_existing_database_with_incompatible_column_types_does_not_migrate() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("not-symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    create_symbol_index_schema_with_text_byte_columns(&connection);
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject incompatible symbol index column types");

    assert!(error.to_string().contains("incompatible type"));
    let connection = Connection::open(&db_path).unwrap();
    assert_eq!(
        symbol_table_column_type(&connection, "symbols", "start_byte"),
        "TEXT"
    );
}

#[test]
fn trace_existing_database_with_unrelated_symbols_table_does_not_migrate() {
    let dir = temporary_dir();
    let db_path = dir.join("not-symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute("CREATE TABLE symbols (name TEXT NOT NULL)", [])
        .unwrap();
    drop(connection);

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("trace should reject databases with non-index symbols tables");

    assert!(error.to_string().contains("missing symbol index table"));
    let connection = Connection::open(&db_path).unwrap();
    let created_table_count: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name IN ('metadata', 'file_state')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(created_table_count, 0);
}

#[test]
fn trace_existing_database_with_incomplete_symbol_columns_does_not_migrate() {
    let dir = temporary_dir();
    let db_path = dir.join("not-symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    create_incomplete_symbol_index_tables(&connection);
    drop(connection);

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("trace should reject tables that only resemble symbol indexes");

    assert!(error.to_string().contains("missing required column"));
    let connection = Connection::open(&db_path).unwrap();
    assert_eq!(symbol_table_columns(&connection), vec!["name"]);
}

#[test]
fn trace_existing_database_with_incompatible_column_types_does_not_migrate() {
    let dir = temporary_dir();
    let db_path = dir.join("not-symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    create_symbol_index_schema_with_text_byte_columns(&connection);
    drop(connection);

    let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
        .expect_err("trace should reject incompatible symbol index column types");

    assert!(error.to_string().contains("incompatible type"));
    let connection = Connection::open(&db_path).unwrap();
    assert_eq!(
        symbol_table_column_type(&connection, "symbols", "start_byte"),
        "TEXT"
    );
}
