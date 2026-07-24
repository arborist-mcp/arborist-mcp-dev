use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, anyhow};
use rusqlite::{Connection, params};

use crate::index_schema::{ensure_symbol_tables, persist_symbol_index_metadata};
use crate::model::SymbolMeta;
use crate::symbol_index_model::{IndexedSymbol, PersistedFileState};

pub(crate) struct SymbolRefreshPersistence<'a> {
    pub(crate) db_path: &'a Path,
    pub(crate) workspace_root: &'a Path,
    pub(crate) raw_symbols: &'a [IndexedSymbol],
    pub(crate) symbols: &'a [SymbolMeta],
    pub(crate) resolved_symbols_by_id: &'a BTreeMap<String, SymbolMeta>,
    pub(crate) file_states: &'a BTreeMap<String, u64>,
    pub(crate) changed_file_paths: &'a BTreeSet<String>,
    pub(crate) impacted_paths: &'a BTreeSet<String>,
    pub(crate) indexed_files: usize,
}

pub(crate) fn persist_symbol_index(
    db_path: &Path,
    workspace_root: &Path,
    raw_symbols: &[IndexedSymbol],
    symbols: &[SymbolMeta],
    file_states: &[PersistedFileState],
    indexed_files: usize,
) -> Result<()> {
    let connection = Connection::open(db_path)?;
    ensure_symbol_tables(&connection)?;

    let tx = connection.unchecked_transaction()?;
    persist_symbol_index_metadata(&tx, workspace_root, indexed_files)?;
    tx.execute("DELETE FROM symbols", [])?;
    tx.execute("DELETE FROM file_state", [])?;
    let raw_symbol_rows = raw_symbol_row_map(raw_symbols);
    {
        let mut statement = tx.prepare(
            "INSERT INTO symbols (
                symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, dependencies_json,
                references_json, reference_names_json, reference_call_arities_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        )?;

        for symbol in symbols {
            let raw_symbol = raw_symbol_rows
                .get(&symbol_row_key(symbol))
                .ok_or_else(|| anyhow!("missing raw symbol for {}", symbol.semantic_path))?;
            let (start_byte, end_byte) = persisted_byte_range(symbol)?;
            statement.execute(params![
                symbol.symbol_id,
                symbol.semantic_path,
                symbol.scope_path,
                symbol.file_path,
                symbol.node_kind,
                start_byte,
                end_byte,
                symbol.signature,
                serde_json::to_string(&symbol.parameters)?,
                symbol.return_type,
                symbol.docstring,
                serde_json::to_string(&symbol.dependencies)?,
                serde_json::to_string(&symbol.references)?,
                serde_json::to_string(&reference_names(raw_symbol))?,
                serde_json::to_string(&raw_symbol.call_arities_by_name)?,
            ])?;
        }
    }
    {
        let mut statement =
            tx.prepare("INSERT INTO file_state (file_path, fingerprint) VALUES (?1, ?2)")?;

        for file_state in file_states {
            statement.execute(params![file_state.file_path, file_state.fingerprint as i64])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub(crate) fn persist_symbol_refresh(context: SymbolRefreshPersistence<'_>) -> Result<()> {
    let connection = Connection::open(context.db_path)?;
    ensure_symbol_tables(&connection)?;

    let raw_symbol_rows = raw_symbol_row_map(context.raw_symbols);
    let changed_symbols: Vec<_> = context
        .symbols
        .iter()
        .filter(|symbol| context.changed_file_paths.contains(&symbol.file_path))
        .cloned()
        .collect();

    let tx = connection.unchecked_transaction()?;
    persist_symbol_index_metadata(&tx, context.workspace_root, context.indexed_files)?;
    {
        let mut delete_statement = tx.prepare("DELETE FROM symbols WHERE file_path = ?1")?;
        for changed_file_path in context.changed_file_paths {
            delete_statement.execute([changed_file_path])?;
        }
    }

    {
        let mut insert_statement = tx.prepare(
            "INSERT INTO symbols (
                symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, dependencies_json,
                references_json, reference_names_json, reference_call_arities_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        )?;

        for symbol in &changed_symbols {
            let raw_symbol = raw_symbol_rows
                .get(&symbol_row_key(symbol))
                .ok_or_else(|| anyhow!("missing raw symbol for {}", symbol.semantic_path))?;
            let (start_byte, end_byte) = persisted_byte_range(symbol)?;
            insert_statement.execute(params![
                symbol.symbol_id,
                symbol.semantic_path,
                symbol.scope_path,
                symbol.file_path,
                symbol.node_kind,
                start_byte,
                end_byte,
                symbol.signature,
                serde_json::to_string(&symbol.parameters)?,
                symbol.return_type,
                symbol.docstring,
                serde_json::to_string(&symbol.dependencies)?,
                serde_json::to_string(&symbol.references)?,
                serde_json::to_string(&reference_names(raw_symbol))?,
                serde_json::to_string(&raw_symbol.call_arities_by_name)?,
            ])?;
        }
    }

    {
        let mut update_statement = tx.prepare(
            "UPDATE symbols
             SET dependencies_json = ?1, references_json = ?2
             WHERE symbol_id = ?3",
        )?;

        for impacted_path in context.impacted_paths {
            let Some(symbol) = context.resolved_symbols_by_id.get(impacted_path) else {
                continue;
            };
            if context.changed_file_paths.contains(&symbol.file_path) {
                continue;
            }
            update_statement.execute(params![
                serde_json::to_string(&symbol.dependencies)?,
                serde_json::to_string(&symbol.references)?,
                symbol.symbol_id,
            ])?;
        }
    }

    for changed_file_path in context.changed_file_paths {
        tx.execute(
            "DELETE FROM file_state WHERE file_path = ?1",
            [changed_file_path],
        )?;
        if let Some(fingerprint) = context.file_states.get(changed_file_path) {
            tx.execute(
                "INSERT INTO file_state (file_path, fingerprint) VALUES (?1, ?2)",
                params![changed_file_path, *fingerprint as i64],
            )?;
        }
    }

    tx.commit()?;
    Ok(())
}

pub(crate) fn persisted_byte_range(symbol: &SymbolMeta) -> Result<(i64, i64)> {
    if symbol.byte_range.0 > symbol.byte_range.1 {
        return Err(anyhow!(
            "invalid byte range for {}: start {} is after end {}",
            symbol.semantic_path,
            symbol.byte_range.0,
            symbol.byte_range.1
        ));
    }

    Ok((
        i64::try_from(symbol.byte_range.0).map_err(|error| {
            anyhow!("invalid start byte for {}: {}", symbol.semantic_path, error)
        })?,
        i64::try_from(symbol.byte_range.1)
            .map_err(|error| anyhow!("invalid end byte for {}: {}", symbol.semantic_path, error))?,
    ))
}

fn raw_symbol_row_map(
    symbols: &[IndexedSymbol],
) -> BTreeMap<(String, String, usize, usize), IndexedSymbol> {
    symbols
        .iter()
        .cloned()
        .map(|symbol| {
            (
                (
                    symbol.semantic_path.clone(),
                    symbol.file_path.clone(),
                    symbol.byte_range.0,
                    symbol.byte_range.1,
                ),
                symbol,
            )
        })
        .collect()
}

fn reference_names(symbol: &IndexedSymbol) -> Vec<String> {
    symbol.references_by_name.iter().cloned().collect()
}

fn symbol_row_key(symbol: &SymbolMeta) -> (String, String, usize, usize) {
    (
        symbol.semantic_path.clone(),
        symbol.file_path.clone(),
        symbol.byte_range.0,
        symbol.byte_range.1,
    )
}
