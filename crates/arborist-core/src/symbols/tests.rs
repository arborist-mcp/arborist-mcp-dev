
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::index_schema::ensure_symbol_tables;
use crate::index_store::{
    SymbolRefreshPersistence, persist_symbol_index, persist_symbol_refresh, persisted_byte_range,
};
use crate::model::SymbolMeta;
use crate::symbol_index_model::{IndexedSymbol, PersistedFileState};
use crate::symbol_index_workspace::transitive_c_include_dependents;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn persisted_byte_range_rejects_inverted_ranges() {
    let symbol = SymbolMeta {
        semantic_path: "helper".to_string(),
        byte_range: (8, 4),
        ..Default::default()
    };

    let error = persisted_byte_range(&symbol)
        .expect_err("persisted byte ranges should reject inverted ranges");

    assert!(error.to_string().contains("start 8 is after end 4"));
}

#[test]
fn persist_symbol_index_rolls_back_metadata_on_row_failure() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let workspace = dir.join("workspace");
    let file_path = workspace.join("helper.py");
    let normalized_file = file_path.to_string_lossy().replace('\\', "/");
    seed_indexed_files_metadata(&db_path, "7");

    let raw_symbols = vec![invalid_indexed_symbol(&normalized_file)];
    let symbols = vec![invalid_symbol_meta(&normalized_file)];
    let file_states = vec![PersistedFileState {
        file_path: file_path.to_string_lossy().replace('\\', "/"),
        fingerprint: 1,
    }];

    let error = persist_symbol_index(
        &db_path,
        &workspace,
        &raw_symbols,
        &symbols,
        &file_states,
        1,
    )
    .expect_err("invalid rows should abort the full persistence transaction");

    assert!(error.to_string().contains("start 8 is after end 4"));
    assert_eq!(indexed_files_metadata(&db_path), "7");
}

#[test]
fn persist_symbol_refresh_rolls_back_metadata_on_row_failure() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let workspace = dir.join("workspace");
    let file_path = workspace.join("helper.py");
    let normalized_file = file_path.to_string_lossy().replace('\\', "/");
    seed_indexed_files_metadata(&db_path, "7");

    let raw_symbols = vec![invalid_indexed_symbol(&normalized_file)];
    let symbols = vec![invalid_symbol_meta(&normalized_file)];
    let file_states = BTreeMap::from([(normalized_file.clone(), 1)]);
    let changed_file_paths = BTreeSet::from([normalized_file]);
    let impacted_paths = BTreeSet::new();
    let resolved_symbols_by_id = BTreeMap::from([("helper".to_string(), symbols[0].clone())]);

    let error = persist_symbol_refresh(SymbolRefreshPersistence {
        db_path: &db_path,
        workspace_root: &workspace,
        raw_symbols: &raw_symbols,
        symbols: &symbols,
        resolved_symbols_by_id: &resolved_symbols_by_id,
        file_states: &file_states,
        changed_file_paths: &changed_file_paths,
        impacted_paths: &impacted_paths,
        indexed_files: 1,
    })
    .expect_err("invalid rows should abort the full refresh transaction");

    assert!(error.to_string().contains("start 8 is after end 4"));
    assert_eq!(indexed_files_metadata(&db_path), "7");
}

#[test]
fn transitive_c_include_dependents_skips_symlink_header_escape() {
    let root = temporary_dir();
    let workspace = root.join("workspace");
    let outside = root.join("outside");
    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(
        workspace.join("source.c"),
        "#include \"linked.h\"\n\nint value(void) {\n    return 1;\n}\n",
    )
    .unwrap();
    fs::write(outside.join("linked.h"), "int secret(void);\n").unwrap();

    let linked_header = workspace.join("linked.h");
    if !try_symlink_file(&outside.join("linked.h"), &linked_header) {
        let _ = fs::remove_dir_all(root);
        return;
    }

    let dependents = transitive_c_include_dependents(&workspace, &linked_header).unwrap();

    assert!(dependents.is_empty());
    fs::remove_dir_all(root).unwrap();
}

fn seed_indexed_files_metadata(db_path: &Path, value: &str) {
    let connection = Connection::open(db_path).unwrap();
    ensure_symbol_tables(&connection).unwrap();
    connection
        .execute(
            "INSERT INTO metadata(key, value) VALUES('indexed_files', ?1)",
            [value],
        )
        .unwrap();
}

fn indexed_files_metadata(db_path: &Path) -> String {
    let connection = Connection::open(db_path).unwrap();
    connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'indexed_files'",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

fn invalid_indexed_symbol(file_path: &str) -> IndexedSymbol {
    IndexedSymbol {
        symbol_id: "helper".to_string(),
        semantic_path: "helper".to_string(),
        base_name: "helper".to_string(),
        scope_path: None,
        file_path: file_path.to_string(),
        node_kind: "function_definition".to_string(),
        byte_range: (8, 4),
        signature: None,
        parameters: Vec::new(),
        return_type: None,
        docstring: None,
        references_by_name: BTreeSet::new(),
    }
}

fn invalid_symbol_meta(file_path: &str) -> SymbolMeta {
    SymbolMeta {
        symbol_id: "helper".to_string(),
        semantic_path: "helper".to_string(),
        file_path: file_path.to_string(),
        node_kind: "function_definition".to_string(),
        byte_range: (8, 4),
        ..Default::default()
    }
}

fn temporary_dir() -> std::path::PathBuf {
    let suffix = format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let dir = std::env::temp_dir().join(format!("arborist-symbols-{suffix}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[cfg(unix)]
fn try_symlink_file(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
fn try_symlink_file(target: &Path, link: &Path) -> bool {
    std::os::windows::fs::symlink_file(target, link).is_ok()
}
