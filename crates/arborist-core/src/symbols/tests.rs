use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::deadline::DeadlineCheck;
use crate::index_schema::ensure_symbol_tables_with_deadline;
use crate::index_store::{
    SymbolRefreshPersistence, load_file_states, persist_symbol_index, persist_symbol_refresh,
    persisted_byte_range,
};
use crate::model::SymbolMeta;
use crate::symbol_index_model::{IndexedSymbol, PersistedFileState};
use crate::symbol_index_workspace::transitive_local_file_dependents;
use crate::workspace_scan::WorkspaceScanDeadline;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct FailOnSecondDeadlineCheck {
    checks: std::sync::atomic::AtomicUsize,
}

impl FailOnSecondDeadlineCheck {
    fn new() -> Self {
        Self {
            checks: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

impl DeadlineCheck for FailOnSecondDeadlineCheck {
    fn check(&self, phase: &str) -> anyhow::Result<()> {
        if self.checks.fetch_add(1, Ordering::Relaxed) >= 1 {
            anyhow::bail!("deadline expired during {phase}");
        }
        Ok(())
    }
}

struct FailOnPhase {
    phase: &'static str,
}

impl DeadlineCheck for FailOnPhase {
    fn check(&self, phase: &str) -> anyhow::Result<()> {
        if phase == self.phase {
            anyhow::bail!("deadline expired during {phase}");
        }
        Ok(())
    }
}

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
        None,
    )
    .expect_err("invalid rows should abort the full persistence transaction");

    assert!(error.to_string().contains("start 8 is after end 4"));
    assert_eq!(indexed_files_metadata(&db_path), "7");
}

#[test]
fn persist_symbol_index_rolls_back_when_deadline_expires_before_commit() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let workspace = dir.join("workspace");
    let file_path = workspace.join("helper.py");
    let normalized_file = file_path.to_string_lossy().replace('\\', "/");
    seed_indexed_files_metadata(&db_path, "7");
    let deadline = FailOnPhase {
        phase: "committing symbol index persistence",
    };

    let error = persist_symbol_index(
        &db_path,
        &workspace,
        &[valid_indexed_symbol(&normalized_file)],
        &[valid_symbol_meta(&normalized_file)],
        &[PersistedFileState {
            file_path: normalized_file,
            fingerprint: 1,
        }],
        1,
        Some(&deadline),
    )
    .expect_err("expired deadline should roll back the rebuild transaction");

    assert!(
        error
            .to_string()
            .contains("deadline expired during committing symbol index persistence")
    );
    assert_eq!(indexed_files_metadata(&db_path), "7");
}
#[test]
fn persist_symbol_index_rejects_duplicate_raw_symbol_rows() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let workspace = dir.join("workspace");
    let file_path = workspace.join("helper.py");
    let normalized_file = file_path.to_string_lossy().replace('\\', "/");
    let raw_symbol = IndexedSymbol {
        extension_receiver: None,
        symbol_id: "helper".to_string(),
        semantic_path: "helper".to_string(),
        base_name: "helper".to_string(),
        scope_path: None,
        file_path: normalized_file.clone(),
        node_kind: "function_definition".to_string(),
        byte_range: (0, 4),
        signature: None,
        is_overload: false,
        parameters: Vec::new(),
        return_type: None,
        docstring: None,
        reference_facts: Vec::new(),
        references_by_name: BTreeSet::new(),
        call_arities_by_name: BTreeMap::new(),
    };
    let symbol = SymbolMeta {
        symbol_id: "helper".to_string(),
        semantic_path: "helper".to_string(),
        file_path: normalized_file,
        node_kind: "function_definition".to_string(),
        byte_range: (0, 4),
        ..Default::default()
    };

    let error = persist_symbol_index(
        &db_path,
        &workspace,
        &[raw_symbol.clone(), raw_symbol],
        &[symbol],
        &[],
        1,
        None,
    )
    .expect_err("duplicate raw symbol rows should be rejected");

    assert!(error.to_string().contains("duplicate raw symbol row"));
}

#[test]
fn persist_symbol_index_round_trips_full_u64_fingerprint() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let workspace = dir.join("workspace");
    let file_path = workspace.join("helper.py");
    let normalized_file = file_path.to_string_lossy().replace('\\', "/");
    let raw_symbol = valid_indexed_symbol(&normalized_file);
    let symbol = valid_symbol_meta(&normalized_file);
    let file_states = vec![PersistedFileState {
        file_path: normalized_file.clone(),
        fingerprint: u64::MAX,
    }];

    persist_symbol_index(
        &db_path,
        &workspace,
        &[raw_symbol],
        &[symbol],
        &file_states,
        1,
        None,
    )
    .expect("full-range fingerprints should persist");

    let connection = Connection::open(&db_path).unwrap();
    let loaded = load_file_states(&connection).unwrap();
    assert_eq!(loaded[&normalized_file], u64::MAX);
}

#[test]
fn persist_symbol_refresh_round_trips_full_u64_fingerprint() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let workspace = dir.join("workspace");
    let file_path = workspace.join("helper.py");
    let normalized_file = file_path.to_string_lossy().replace('\\', "/");
    let raw_symbol = valid_indexed_symbol(&normalized_file);
    let symbol = valid_symbol_meta(&normalized_file);

    persist_symbol_index(
        &db_path,
        &workspace,
        std::slice::from_ref(&raw_symbol),
        std::slice::from_ref(&symbol),
        &[PersistedFileState {
            file_path: normalized_file.clone(),
            fingerprint: 1,
        }],
        1,
        None,
    )
    .expect("initial index should persist");

    persist_symbol_refresh(SymbolRefreshPersistence {
        db_path: &db_path,
        workspace_root: &workspace,
        raw_symbols: &[raw_symbol],
        symbols: std::slice::from_ref(&symbol),
        resolved_symbols_by_id: &BTreeMap::from([("helper".to_string(), symbol.clone())]),
        file_states: &BTreeMap::from([(normalized_file.clone(), u64::MAX)]),
        changed_file_paths: &BTreeSet::from([normalized_file.clone()]),
        impacted_paths: &BTreeSet::new(),
        indexed_files: 1,
        deadline: None,
    })
    .expect("full-range refresh fingerprints should persist");

    let connection = Connection::open(&db_path).unwrap();
    let loaded = load_file_states(&connection).unwrap();
    assert_eq!(loaded[&normalized_file], u64::MAX);
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
        deadline: None,
    })
    .expect_err("invalid rows should abort the full refresh transaction");

    assert!(error.to_string().contains("start 8 is after end 4"));
    assert_eq!(indexed_files_metadata(&db_path), "7");
}

#[test]
fn persist_symbol_refresh_rolls_back_when_deadline_expires_before_commit() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let workspace = dir.join("workspace");
    let file_path = workspace.join("helper.py");
    let normalized_file = file_path.to_string_lossy().replace('\\', "/");
    seed_indexed_files_metadata(&db_path, "7");
    let raw_symbol = valid_indexed_symbol(&normalized_file);
    let symbol = valid_symbol_meta(&normalized_file);
    let deadline = FailOnPhase {
        phase: "committing symbol index refresh",
    };

    let error = persist_symbol_refresh(SymbolRefreshPersistence {
        db_path: &db_path,
        workspace_root: &workspace,
        raw_symbols: std::slice::from_ref(&raw_symbol),
        symbols: std::slice::from_ref(&symbol),
        resolved_symbols_by_id: &BTreeMap::from([("helper".to_string(), symbol.clone())]),
        file_states: &BTreeMap::from([(normalized_file.clone(), 1)]),
        changed_file_paths: &BTreeSet::from([normalized_file]),
        impacted_paths: &BTreeSet::new(),
        indexed_files: 1,
        deadline: Some(&deadline),
    })
    .expect_err("expired deadline should roll back the refresh transaction");

    assert!(
        error
            .to_string()
            .contains("deadline expired during committing symbol index refresh")
    );
    assert_eq!(indexed_files_metadata(&db_path), "7");
}
#[test]
fn persist_symbol_refresh_checks_expired_deadline_before_writes() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    let workspace = dir.join("workspace");
    let file_path = workspace.join("helper.py");
    let normalized_file = file_path.to_string_lossy().replace('\\', "/");
    seed_indexed_files_metadata(&db_path, "7");

    let raw_symbols = vec![valid_indexed_symbol(&normalized_file)];
    let symbols = vec![valid_symbol_meta(&normalized_file)];
    let deadline = WorkspaceScanDeadline {
        deadline: Some(Instant::now() - Duration::from_millis(1)),
        timeout_ms: Some(1),
    };

    let error = persist_symbol_refresh(SymbolRefreshPersistence {
        db_path: &db_path,
        workspace_root: &workspace,
        raw_symbols: &raw_symbols,
        symbols: &symbols,
        resolved_symbols_by_id: &BTreeMap::new(),
        file_states: &BTreeMap::from([(normalized_file.clone(), 1)]),
        changed_file_paths: &BTreeSet::from([normalized_file]),
        impacted_paths: &BTreeSet::new(),
        indexed_files: 1,
        deadline: Some(&deadline),
    })
    .expect_err("expired deadline should stop before any persistence work");

    assert!(
        error
            .to_string()
            .contains("workspace scan timeout exceeded")
    );
    assert_eq!(indexed_files_metadata(&db_path), "7");
}

#[test]
fn persistence_checks_deadline_before_initializing_schema() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let refresh_db_path = dir.join("refresh.db");
    let rebuild_db_path = dir.join("rebuild.db");
    let rebuild_deadline = FailOnSecondDeadlineCheck::new();
    let refresh_deadline = FailOnSecondDeadlineCheck::new();

    persist_symbol_index(
        &rebuild_db_path,
        &workspace,
        &[],
        &[],
        &[],
        0,
        Some(&rebuild_deadline),
    )
    .expect_err("rebuild persistence should stop before schema setup");
    persist_symbol_refresh(SymbolRefreshPersistence {
        db_path: &refresh_db_path,
        workspace_root: &workspace,
        raw_symbols: &[],
        symbols: &[],
        resolved_symbols_by_id: &BTreeMap::new(),
        file_states: &BTreeMap::new(),
        changed_file_paths: &BTreeSet::new(),
        impacted_paths: &BTreeSet::new(),
        indexed_files: 0,
        deadline: Some(&refresh_deadline),
    })
    .expect_err("refresh persistence should stop before schema setup");

    for db_path in [&rebuild_db_path, &refresh_db_path] {
        let connection = Connection::open(db_path).unwrap();
        let symbol_tables: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('symbols', 'file_state', 'index_metadata')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            symbol_tables, 0,
            "schema should not be initialized for {db_path:?}"
        );
    }
}

#[test]
fn transitive_local_file_dependents_skips_symlink_header_escape() {
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

    let dependents = transitive_local_file_dependents(&workspace, &linked_header).unwrap();

    assert!(dependents.is_empty());
    fs::remove_dir_all(root).unwrap();
}

fn seed_indexed_files_metadata(db_path: &Path, value: &str) {
    let connection = Connection::open(db_path).unwrap();
    ensure_symbol_tables_with_deadline(&connection, None).unwrap();
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
        extension_receiver: None,
        symbol_id: "helper".to_string(),
        semantic_path: "helper".to_string(),
        base_name: "helper".to_string(),
        scope_path: None,
        file_path: file_path.to_string(),
        node_kind: "function_definition".to_string(),
        byte_range: (8, 4),
        signature: None,
        is_overload: false,
        parameters: Vec::new(),
        return_type: None,
        docstring: None,
        reference_facts: Vec::new(),
        references_by_name: BTreeSet::new(),
        call_arities_by_name: BTreeMap::new(),
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

fn valid_indexed_symbol(file_path: &str) -> IndexedSymbol {
    IndexedSymbol {
        extension_receiver: None,
        symbol_id: "helper".to_string(),
        semantic_path: "helper".to_string(),
        base_name: "helper".to_string(),
        scope_path: None,
        file_path: file_path.to_string(),
        node_kind: "function_definition".to_string(),
        byte_range: (0, 4),
        signature: None,
        is_overload: false,
        parameters: Vec::new(),
        return_type: None,
        docstring: None,
        reference_facts: Vec::new(),
        references_by_name: BTreeSet::new(),
        call_arities_by_name: BTreeMap::new(),
    }
}

fn valid_symbol_meta(file_path: &str) -> SymbolMeta {
    SymbolMeta {
        symbol_id: "helper".to_string(),
        semantic_path: "helper".to_string(),
        file_path: file_path.to_string(),
        node_kind: "function_definition".to_string(),
        byte_range: (0, 4),
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
