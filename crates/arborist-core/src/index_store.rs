use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use rusqlite::{Connection, Row, params, types::Type};
use serde::de::DeserializeOwned;

use crate::index_schema::{
    ensure_symbol_tables, load_indexed_files_metadata, persist_symbol_index_metadata,
};
use crate::model::{SymbolMeta, SymbolMetaInit};
use crate::semantic::semantic_parent_path;
use crate::symbol_index_model::{IndexedSymbol, PersistedFileState, symbol_base_name};

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

pub(crate) fn load_file_states(connection: &Connection) -> Result<BTreeMap<String, u64>> {
    let mut statement =
        connection.prepare("SELECT file_path, fingerprint FROM file_state ORDER BY file_path")?;
    let rows = statement.query_map([], |row| {
        Ok((
            nonempty_string_from_row(row, 0, "file_state.file_path")?,
            row.get::<_, i64>(1)? as u64,
        ))
    })?;

    let mut states = BTreeMap::new();
    for row in rows {
        let (file_path, fingerprint) = row?;
        states.insert(file_path, fingerprint);
    }
    Ok(states)
}

pub(crate) fn load_indexed_symbols_grouped_by_file(
    connection: &Connection,
) -> Result<BTreeMap<String, Vec<IndexedSymbol>>> {
    load_indexed_symbols_grouped_by_file_with_query(
        connection,
        "SELECT symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, reference_names_json,
                reference_call_arities_json
         FROM symbols
         ORDER BY file_path, semantic_path",
    )
}

pub(crate) fn validate_legacy_indexed_symbols(connection: &Connection) -> Result<()> {
    load_indexed_symbols_grouped_by_file_with_query(
        connection,
        "SELECT symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, reference_names_json,
                '{}' AS reference_call_arities_json
         FROM symbols
         ORDER BY file_path, semantic_path",
    )
    .context("invalid persisted legacy symbol row")?;
    Ok(())
}

fn load_indexed_symbols_grouped_by_file_with_query(
    connection: &Connection,
    query: &str,
) -> Result<BTreeMap<String, Vec<IndexedSymbol>>> {
    let mut statement = connection.prepare(query)?;
    let rows = statement.query_map([], |row| {
        let parameters_json: String = row.get(8)?;
        let reference_names_json: String = row.get(11)?;
        let reference_call_arities_json: String = row.get(12)?;
        let parameters = string_list_from_json_column(&parameters_json, 8, "parameters_json")?;
        let reference_names =
            string_list_from_json_column(&reference_names_json, 11, "reference_names_json")?;
        let call_arities_by_name = call_arities_from_json_column(&reference_call_arities_json, 12)?;
        let symbol_id = nonempty_string_from_row(row, 0, "symbol_id")?;
        let semantic_path = nonempty_string_from_row(row, 1, "semantic_path")?;
        let scope_path = validated_scope_path(row, 2, &semantic_path)?;
        Ok(IndexedSymbol {
            symbol_id,
            base_name: symbol_base_name(&semantic_path),
            semantic_path,
            scope_path,
            file_path: nonempty_string_from_row(row, 3, "file_path")?,
            node_kind: nonempty_string_from_row(row, 4, "node_kind")?,
            byte_range: byte_range_from_row(row, 5, 6)?,
            signature: optional_nonempty_string_from_row(row, 7, "signature")?,
            parameters,
            return_type: optional_nonempty_string_from_row(row, 9, "return_type")?,
            docstring: optional_nonempty_string_from_row(row, 10, "docstring")?,
            references_by_name: reference_names.into_iter().collect(),
            call_arities_by_name,
        })
    })?;

    let mut grouped = BTreeMap::new();
    for row in rows {
        let symbol = row?;
        grouped
            .entry(symbol.file_path.clone())
            .or_insert_with(Vec::new)
            .push(symbol);
    }
    Ok(grouped)
}

pub(crate) fn load_resolved_symbols(connection: &Connection) -> Result<(Vec<SymbolMeta>, usize)> {
    let indexed_files = load_indexed_files_metadata(connection)?;

    let mut statement = connection.prepare(
        "SELECT symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, dependencies_json,
                references_json
         FROM symbols",
    )?;
    let rows = statement.query_map([], |row| {
        let parameters_json: String = row.get(8)?;
        let dependencies_json: String = row.get(11)?;
        let references_json: String = row.get(12)?;
        let semantic_path = nonempty_string_from_row(row, 1, "semantic_path")?;
        Ok(SymbolMeta::new(SymbolMetaInit {
            symbol_id: nonempty_string_from_row(row, 0, "symbol_id")?,
            scope_path: validated_scope_path(row, 2, &semantic_path)?,
            semantic_path,
            file_path: nonempty_string_from_row(row, 3, "file_path")?,
            node_kind: nonempty_string_from_row(row, 4, "node_kind")?,
            origin_type: "workspace_symbol".to_string(),
            byte_range: byte_range_from_row(row, 5, 6)?,
            signature: optional_nonempty_string_from_row(row, 7, "signature")?,
            parameters: string_list_from_json_column(&parameters_json, 8, "parameters_json")?,
            return_type: optional_nonempty_string_from_row(row, 9, "return_type")?,
            docstring: optional_nonempty_string_from_row(row, 10, "docstring")?,
            dependencies: string_list_from_json_column(
                &dependencies_json,
                11,
                "dependencies_json",
            )?,
            references: string_list_from_json_column(&references_json, 12, "references_json")?,
        }))
    })?;

    let mut symbols = Vec::new();
    for row in rows {
        symbols.push(row?);
    }

    Ok((symbols, indexed_files))
}

pub(crate) fn validate_resolved_symbol_edges(symbols: &[SymbolMeta]) -> Result<()> {
    let symbol_ids = symbols
        .iter()
        .map(|symbol| symbol.symbol_id.as_str())
        .collect::<BTreeSet<_>>();

    for symbol in symbols {
        for dependency in &symbol.dependencies {
            if !symbol_ids.contains(dependency.as_str()) {
                return Err(anyhow!(
                    "persisted dependency `{dependency}` for symbol `{}` does not exist",
                    symbol.symbol_id
                ));
            }
        }
        for reference in &symbol.references {
            if !symbol_ids.contains(reference.as_str()) {
                return Err(anyhow!(
                    "persisted reference `{reference}` for symbol `{}` does not exist",
                    symbol.symbol_id
                ));
            }
        }
    }

    Ok(())
}

pub(crate) fn count_table_rows(connection: &Connection, table_name: &str) -> Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM {table_name}");
    let count = connection.query_row(&sql, [], |row| row.get::<_, i64>(0))?;
    usize::try_from(count).map_err(|error| anyhow!("invalid row count in `{table_name}`: {error}"))
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

pub(crate) fn nonempty_string_from_row(
    row: &Row<'_>,
    column: usize,
    column_name: &str,
) -> rusqlite::Result<String> {
    let value: String = row.get(column)?;
    if value.trim().is_empty() {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            column,
            Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("empty {column_name}"),
            )),
        ));
    }
    Ok(value)
}

fn optional_nonempty_string_from_row(
    row: &Row<'_>,
    column: usize,
    column_name: &str,
) -> rusqlite::Result<Option<String>> {
    let value: Option<String> = row.get(column)?;
    if value
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            column,
            Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("empty {column_name}"),
            )),
        ));
    }
    Ok(value)
}

fn validated_scope_path(
    row: &Row<'_>,
    column: usize,
    semantic_path: &str,
) -> rusqlite::Result<Option<String>> {
    let scope_path = optional_nonempty_string_from_row(row, column, "scope_path")?;
    let expected_scope_path = semantic_parent_path(semantic_path);
    if scope_path != expected_scope_path {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            column,
            Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("scope_path does not match semantic_path `{semantic_path}`"),
            )),
        ));
    }
    Ok(scope_path)
}

pub(crate) fn json_from_column<T: DeserializeOwned>(
    json: &str,
    column: usize,
) -> rusqlite::Result<T> {
    serde_json::from_str(json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(column, Type::Text, Box::new(error))
    })
}

pub(crate) fn string_list_from_json_column(
    json: &str,
    column: usize,
    column_name: &str,
) -> rusqlite::Result<Vec<String>> {
    let values: Vec<String> = json_from_column(json, column)?;
    if values.iter().any(|value| value.trim().is_empty()) {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            column,
            Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("empty {column_name} entry"),
            )),
        ));
    }
    Ok(values)
}

fn call_arities_from_json_column(
    json: &str,
    column: usize,
) -> rusqlite::Result<BTreeMap<String, BTreeSet<usize>>> {
    let call_arities: BTreeMap<String, BTreeSet<usize>> = json_from_column(json, column)?;
    if call_arities
        .iter()
        .any(|(name, arities)| name.trim().is_empty() || arities.is_empty())
    {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            column,
            Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "empty reference_call_arities_json entry",
            )),
        ));
    }
    Ok(call_arities)
}

pub(crate) fn byte_range_from_row(
    row: &Row<'_>,
    start_column: usize,
    end_column: usize,
) -> rusqlite::Result<(usize, usize)> {
    let start = nonnegative_i64_as_usize(row.get(start_column)?, start_column)?;
    let end = nonnegative_i64_as_usize(row.get(end_column)?, end_column)?;
    if start > end {
        return Err(integer_conversion_error(
            end_column,
            format!("end_byte {end} is before start_byte {start}"),
        ));
    }
    Ok((start, end))
}

fn nonnegative_i64_as_usize(value: i64, column: usize) -> rusqlite::Result<usize> {
    if value < 0 {
        return Err(integer_conversion_error(
            column,
            format!("expected non-negative integer, got {value}"),
        ));
    }
    usize::try_from(value).map_err(|error| integer_conversion_error(column, error.to_string()))
}

fn integer_conversion_error(column: usize, message: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column,
        Type::Integer,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message,
        )),
    )
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
