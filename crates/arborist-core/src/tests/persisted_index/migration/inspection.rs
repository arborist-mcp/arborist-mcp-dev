use super::*;

#[test]
fn inspect_symbol_index_reports_schema_version_issues_without_migration() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();
    create_legacy_symbol_index_schema_without_reference_names(
        &connection,
        Some(&normalize_path(&dir)),
        Some("0"),
    );
    drop(connection);

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
            .any(|issue| issue.contains("missing schema_version metadata"))
    );
    let connection = Connection::open(&db_path).unwrap();
    assert!(!symbol_table_columns(&connection).contains(&"reference_names_json".to_string()));
}

#[test]
fn inspect_symbol_index_reports_unsupported_schema_version_without_rewrite() {
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
        .execute(
            "UPDATE metadata SET value = '99' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();
    drop(connection);

    let health = inspect_symbol_index(&db_path).unwrap();

    assert!(!health.ok);
    assert_eq!(health.schema_version.as_deref(), Some("99"));
    assert!(health.migration.required);
    assert_eq!(health.migration.action, "rebuild");
    assert!(
        health
            .issues
            .iter()
            .any(|issue| issue.contains("unsupported symbol index schema_version"))
    );
    let connection = Connection::open(&db_path).unwrap();
    let schema_version: String = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(schema_version, "99");
}

#[test]
fn refresh_existing_database_with_missing_indexed_files_metadata_does_not_migrate() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
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
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject databases with missing indexed_files metadata");

    assert!(error.to_string().contains("missing indexed_files metadata"));
    let connection = Connection::open(&db_path).unwrap();
    let metadata_count: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM metadata WHERE key = 'indexed_files'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(metadata_count, 0);
}

#[test]
fn refresh_existing_database_with_missing_schema_version_does_not_migrate() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_legacy_symbol_index_schema_without_reference_names(
        &connection,
        Some(&normalize_path(&dir)),
        Some("0"),
    );
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject databases with missing schema_version metadata");

    assert!(
        error
            .to_string()
            .contains("missing schema_version metadata")
    );
    let connection = Connection::open(&db_path).unwrap();
    assert!(!symbol_table_columns(&connection).contains(&"reference_names_json".to_string()));
}

#[test]
fn refresh_existing_database_with_unsupported_schema_version_does_not_rewrite() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
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
        .execute(
            "UPDATE metadata SET value = '99' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject unsupported schema versions");

    assert!(
        error
            .to_string()
            .contains("unsupported symbol index schema_version")
    );
    let connection = Connection::open(&db_path).unwrap();
    let schema_version: String = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(schema_version, "99");
}

#[test]
fn refresh_existing_database_with_missing_workspace_metadata_does_not_migrate() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    let connection = Connection::open(&db_path).unwrap();

    create_minimal_symbol_index_schema(&connection);
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject databases with missing workspace_root metadata");

    assert!(
        error
            .to_string()
            .contains("missing workspace_root metadata")
    );
    let connection = Connection::open(&db_path).unwrap();
    let metadata_count: usize = connection
        .query_row(
            "SELECT COUNT(*) FROM metadata WHERE key = 'workspace_root'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(metadata_count, 0);
}
