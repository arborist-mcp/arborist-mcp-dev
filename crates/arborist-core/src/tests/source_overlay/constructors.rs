use super::*;

#[test]
fn index_overlay_counts_new_unsaved_files_in_indexed_file_totals() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let new_file = dir.join("helper_alias.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let context = SymbolQueryContext::index(&db_path)
        .unwrap()
        .with_source_overlay(
            &new_file,
            "from helper import helper\n\n\ndef helper_alias() -> int:\n    return helper()\n",
        )
        .unwrap();

    let listed = context.list_symbols(10, None, None).unwrap();

    assert_eq!(listed.indexed_files, 2);
    assert_eq!(listed.total_symbols, 2);
    assert!(
        listed
            .symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "helper_alias")
    );
    assert!(!new_file.exists());
}

#[test]
fn index_overlay_accepts_new_disk_file_when_source_is_overridden() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let new_file = dir.join("helper_alias.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(&new_file, "def stale_alias() -> int:\n    return 0\n").unwrap();

    let context = SymbolQueryContext::index(&db_path)
        .unwrap()
        .with_source_overlay(
            &new_file,
            "from helper import helper\n\n\ndef helper_alias() -> int:\n    return helper()\n",
        )
        .unwrap();

    let listed = context.list_symbols(10, None, None).unwrap();

    assert_eq!(listed.indexed_files, 2);
    assert!(
        listed
            .symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "helper_alias")
    );
    assert!(
        listed
            .symbols
            .iter()
            .all(|symbol| symbol.semantic_path != "stale_alias")
    );
}
