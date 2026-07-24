use super::*;

#[test]
fn rejects_refresh_path_that_escapes_workspace_after_normalization() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let nested = workspace.join("child");
    let helper = workspace.join("helper.py");
    let db_path = workspace.join("symbols.db");
    let outside = dir.join("outside.py");

    fs::create_dir_all(&nested).unwrap();
    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &outside,
        "def outside(value: int) -> int:\n    return value + 2\n",
    )
    .unwrap();

    rebuild_symbol_index(&workspace, &db_path).unwrap();

    let escaping_path = nested.join("..").join("..").join("outside.py");
    let error = refresh_symbol_index_for_file(&workspace, &db_path, &escaping_path)
        .expect_err("refresh should reject paths outside the workspace");
    assert!(error.to_string().contains("outside workspace"));
}

#[test]
fn rejects_refresh_path_outside_workspace_before_missing_index_rebuild() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let outside = dir.join("outside.py");
    let missing_db_path = workspace.join("missing-symbols.db");

    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        workspace.join("helper.py"),
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &outside,
        "def outside(value: int) -> int:\n    return value + 2\n",
    )
    .unwrap();

    let error = refresh_symbol_index_for_file(&workspace, &missing_db_path, &outside)
        .expect_err("refresh should reject outside files before rebuilding a missing index");
    assert!(error.to_string().contains("outside workspace"));
    assert!(!missing_db_path.exists());
}

#[test]
fn rejects_refresh_with_symbol_index_from_different_workspace() {
    let dir = temporary_dir();
    let workspace_a = dir.join("workspace-a");
    let workspace_b = dir.join("workspace-b");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&workspace_a).unwrap();
    fs::create_dir_all(&workspace_b).unwrap();
    fs::write(
        workspace_a.join("helper.py"),
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        workspace_b.join("helper.py"),
        "def helper(value: int) -> int:\n    return value + 2\n",
    )
    .unwrap();

    rebuild_symbol_index(&workspace_a, &db_path).unwrap();

    let error =
        refresh_symbol_index_for_file(&workspace_b, &db_path, &workspace_b.join("helper.py"))
            .expect_err("refresh should reject a database built for another workspace");

    assert!(error.to_string().contains("belongs to workspace"));
}

#[test]
fn rejects_rebuild_with_symbol_index_from_different_workspace() {
    let dir = temporary_dir();
    let workspace_a = dir.join("workspace-a");
    let workspace_b = dir.join("workspace-b");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&workspace_a).unwrap();
    fs::create_dir_all(&workspace_b).unwrap();
    fs::write(
        workspace_a.join("helper.py"),
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        workspace_b.join("helper.py"),
        "def helper(value: int) -> int:\n    return value + 2\n",
    )
    .unwrap();

    rebuild_symbol_index(&workspace_a, &db_path).unwrap();

    let error = rebuild_symbol_index(&workspace_b, &db_path)
        .expect_err("rebuild should reject a database built for another workspace");

    assert!(error.to_string().contains("belongs to workspace"));
}

#[test]
fn refresh_rejects_different_workspace_before_legacy_migration() {
    let dir = temporary_dir();
    let workspace_a = dir.join("workspace-a");
    let workspace_b = dir.join("workspace-b");
    let db_path = dir.join("symbols.db");
    let helper = workspace_b.join("helper.py");

    fs::create_dir_all(&workspace_a).unwrap();
    fs::create_dir_all(&workspace_b).unwrap();
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    let connection = Connection::open(&db_path).unwrap();
    create_legacy_symbol_index_schema_without_reference_names(
        &connection,
        Some(&normalize_path(&workspace_a)),
        Some("0"),
    );
    drop(connection);

    let error = refresh_symbol_index_for_file(&workspace_b, &db_path, &helper)
        .expect_err("wrong-workspace refresh should reject before legacy migration");

    assert!(error.to_string().contains("belongs to workspace"));
    let connection = Connection::open(&db_path).unwrap();
    assert!(!symbol_table_columns(&connection).contains(&"reference_names_json".to_string()));
}

#[test]
fn refresh_rejects_empty_persisted_symbol_identity_without_rewrite() {
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
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject persisted rows with empty identity fields");

    assert!(error.to_string().contains("empty symbol_id"));
    let connection = Connection::open(&db_path).unwrap();
    let persisted_symbol_id: String = connection
        .query_row("SELECT symbol_id FROM symbols", [], |row| row.get(0))
        .unwrap();
    assert_eq!(persisted_symbol_id, "");
}

#[test]
fn refresh_rejects_empty_persisted_reference_names_without_rewrite() {
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
        .execute_batch(
            "
                INSERT INTO symbols (
                    symbol_id, semantic_path, file_path, node_kind, start_byte, end_byte,
                    parameters_json, dependencies_json, references_json, reference_names_json
                ) VALUES (
                    'helper', 'helper', 'helper.py', 'function_definition', 0, 5,
                    '[]', '[]', '[]', '[\"\"]'
                );
                ",
        )
        .unwrap();
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject empty persisted reference names");

    assert!(
        error
            .to_string()
            .contains("empty reference_names_json entry")
    );
    let connection = Connection::open(&db_path).unwrap();
    let reference_names_json: String = connection
        .query_row("SELECT reference_names_json FROM symbols", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(reference_names_json, "[\"\"]");
}

#[test]
fn refresh_rejects_empty_persisted_file_state_path_without_rewrite() {
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
            "INSERT INTO file_state(file_path, fingerprint) VALUES('', 1)",
            [],
        )
        .unwrap();
    drop(connection);
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh should reject empty persisted file_state paths");

    assert!(error.to_string().contains("empty file_state.file_path"));
    let connection = Connection::open(&db_path).unwrap();
    let persisted_file_path: String = connection
        .query_row("SELECT file_path FROM file_state", [], |row| row.get(0))
        .unwrap();
    assert_eq!(persisted_file_path, "");
}

#[test]
fn refresh_rejects_persisted_symbol_paths_outside_workspace_without_rewrite() {
    let root = temporary_dir();
    let dir = root.join("workspace");
    let helper = dir.join("helper.py");
    let outside = root.join("outside.py");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&dir).unwrap();
    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(&outside, "def outside() -> int:\n    return 2\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let outside_path = normalize_path(&outside);
    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute(
            "UPDATE symbols SET file_path = ?1 WHERE semantic_path = 'helper'",
            [&outside_path],
        )
        .unwrap();
    drop(connection);

    let error = refresh_symbol_index_for_file(&dir, &db_path, &helper)
        .expect_err("refresh must reject persisted paths outside the workspace");
    assert!(error.to_string().contains("symbols.file_path"));
    assert!(error.to_string().contains("outside indexed workspace"));

    let connection = Connection::open(&db_path).unwrap();
    let persisted_path: String = connection
        .query_row("SELECT file_path FROM symbols", [], |row| row.get(0))
        .unwrap();
    assert_eq!(persisted_path, outside_path);
}
