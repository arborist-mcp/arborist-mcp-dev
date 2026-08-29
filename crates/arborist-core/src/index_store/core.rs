use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Result, anyhow};
use rusqlite::{Connection, params};

use super::metadata::persisted_fingerprint;
use crate::deadline::DeadlineCheck;
use crate::index_schema::{ensure_symbol_tables, persist_symbol_index_metadata};
use crate::model::SymbolMeta;
use crate::symbol_index_model::{IndexedSymbol, PersistedFileState};

pub(crate) fn persist_symbol_index(
    db_path: &Path,
    workspace_root: &Path,
    raw_symbols: &[IndexedSymbol],
    symbols: &[SymbolMeta],
    file_states: &[PersistedFileState],
    indexed_files: usize,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("opening symbol index database")?;
    }
    let connection = Connection::open(db_path)?;
    if let Some(deadline) = deadline {
        deadline.check("preparing symbol index schema")?;
    }
    ensure_symbol_tables(&connection)?;

    let tx = connection.unchecked_transaction()?;
    if let Some(deadline) = deadline {
        deadline.check("persisting symbol index metadata")?;
    }
    persist_symbol_index_metadata(&tx, workspace_root, indexed_files)?;
    tx.execute("DELETE FROM symbols", [])?;
    tx.execute("DELETE FROM file_state", [])?;
    let raw_symbol_rows = raw_symbol_row_map(raw_symbols)?;
    {
        let mut statement = tx.prepare(
            "INSERT INTO symbols (
                symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, extension_receiver,
                dependencies_json, references_json, reference_names_json, reference_call_arities_json,
                reference_facts_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        )?;

        for symbol in symbols {
            if let Some(deadline) = deadline {
                deadline.check("writing persisted symbol rows")?;
            }
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
                raw_symbol.extension_receiver,
                serde_json::to_string(&symbol.dependencies)?,
                serde_json::to_string(&symbol.references)?,
                serde_json::to_string(&reference_names(raw_symbol))?,
                serde_json::to_string(&raw_symbol.call_arities_by_name)?,
                serde_json::to_string(&raw_symbol.reference_facts)?,
            ])?;
        }
    }
    {
        let mut statement =
            tx.prepare("INSERT INTO file_state (file_path, fingerprint) VALUES (?1, ?2)")?;

        for file_state in file_states {
            if let Some(deadline) = deadline {
                deadline.check("writing persisted file states")?;
            }
            statement.execute(params![
                file_state.file_path,
                persisted_fingerprint(file_state.fingerprint)?
            ])?;
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

pub(super) fn raw_symbol_row_map(
    symbols: &[IndexedSymbol],
) -> Result<BTreeMap<(String, String, usize, usize), IndexedSymbol>> {
    let mut rows = BTreeMap::new();
    for symbol in symbols {
        let key = (
            symbol.semantic_path.clone(),
            symbol.file_path.clone(),
            symbol.byte_range.0,
            symbol.byte_range.1,
        );
        if rows.insert(key, symbol.clone()).is_some() {
            return Err(anyhow!(
                "duplicate raw symbol row for {} in {} at {}..{}",
                symbol.semantic_path,
                symbol.file_path,
                symbol.byte_range.0,
                symbol.byte_range.1
            ));
        }
    }
    Ok(rows)
}

pub(super) fn reference_names(symbol: &IndexedSymbol) -> Vec<String> {
    symbol.references_by_name.iter().cloned().collect()
}

pub(super) fn symbol_row_key(symbol: &SymbolMeta) -> (String, String, usize, usize) {
    (
        symbol.semantic_path.clone(),
        symbol.file_path.clone(),
        symbol.byte_range.0,
        symbol.byte_range.1,
    )
}
