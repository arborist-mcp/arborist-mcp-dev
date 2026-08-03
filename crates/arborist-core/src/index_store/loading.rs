use std::collections::BTreeMap;

use anyhow::{Context, Result};
use rusqlite::{Connection, types::Type};

pub(super) use super::loading_values::nonempty_string_from_row;
use super::loading_values::{
    byte_range_from_row, call_arities_from_json_column, optional_nonempty_string_from_row,
    reference_facts_from_json_column, string_list_from_json_column, validated_scope_path,
};
use crate::deadline::DeadlineCheck;
use crate::index_schema::load_indexed_files_metadata_with_deadline;
use crate::model::{SymbolMeta, SymbolMetaInit};
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) fn load_indexed_symbols_grouped_by_file(
    connection: &Connection,
) -> Result<BTreeMap<String, Vec<IndexedSymbol>>> {
    load_indexed_symbols_grouped_by_file_with_query(
        connection,
        "SELECT symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, reference_names_json,
                reference_call_arities_json, reference_facts_json
         FROM symbols
         ORDER BY file_path, semantic_path",
    )
}

pub(crate) fn load_indexed_symbols_grouped_by_file_with_deadline(
    connection: &Connection,
    deadline: &WorkspaceScanDeadline,
) -> Result<BTreeMap<String, Vec<IndexedSymbol>>> {
    load_indexed_symbols_grouped_by_file_with_query_and_deadline(
        connection,
        "SELECT symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, reference_names_json,
                reference_call_arities_json, reference_facts_json
         FROM symbols
         ORDER BY file_path, semantic_path",
        Some(deadline),
    )
}

pub(crate) fn validate_previous_indexed_symbols(connection: &Connection) -> Result<()> {
    validate_previous_indexed_symbols_with_deadline(connection, None)
}

pub(crate) fn validate_previous_indexed_symbols_with_deadline(
    connection: &Connection,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    load_indexed_symbols_grouped_by_file_with_query_and_deadline(
        connection,
        "SELECT symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, reference_names_json,
                reference_call_arities_json, '[]' AS reference_facts_json
         FROM symbols
         ORDER BY file_path, semantic_path",
        deadline,
    )
    .context("invalid persisted previous symbol row")?;
    Ok(())
}

pub(crate) fn validate_legacy_indexed_symbols(connection: &Connection) -> Result<()> {
    validate_legacy_indexed_symbols_with_deadline(connection, None)
}

pub(crate) fn validate_legacy_indexed_symbols_with_deadline(
    connection: &Connection,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    load_indexed_symbols_grouped_by_file_with_query_and_deadline(
        connection,
        "SELECT symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, reference_names_json,
                '{}' AS reference_call_arities_json, '[]' AS reference_facts_json
         FROM symbols
         ORDER BY file_path, semantic_path",
        deadline,
    )
    .context("invalid persisted legacy symbol row")?;
    Ok(())
}

fn load_indexed_symbols_grouped_by_file_with_query(
    connection: &Connection,
    query: &str,
) -> Result<BTreeMap<String, Vec<IndexedSymbol>>> {
    load_indexed_symbols_grouped_by_file_with_query_and_deadline(connection, query, None)
}

fn load_indexed_symbols_grouped_by_file_with_query_and_deadline(
    connection: &Connection,
    query: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeMap<String, Vec<IndexedSymbol>>> {
    let mut statement = connection.prepare(query)?;
    let rows = statement.query_map([], |row| {
        let parameters_json: String = row.get(8)?;
        let reference_names_json: String = row.get(11)?;
        let reference_call_arities_json: String = row.get(12)?;
        let reference_facts_json: String = row.get(13)?;
        let parameters = string_list_from_json_column(&parameters_json, 8, "parameters_json")?;
        let reference_names: std::collections::BTreeSet<_> =
            string_list_from_json_column(&reference_names_json, 11, "reference_names_json")?
                .into_iter()
                .collect();
        let call_arities_by_name = call_arities_from_json_column(&reference_call_arities_json, 12)?;
        if call_arities_by_name
            .keys()
            .any(|name| !reference_names.contains(name))
        {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                12,
                Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "reference_call_arities_json contains a name absent from reference_names_json",
                )),
            ));
        }
        let symbol_id = nonempty_string_from_row(row, 0, "symbol_id")?;
        let semantic_path = nonempty_string_from_row(row, 1, "semantic_path")?;
        let file_path = nonempty_string_from_row(row, 3, "file_path")?;
        let overload_prefix = format!("{file_path}::{semantic_path}#");
        let is_overload = symbol_id
            .strip_prefix(&overload_prefix)
            .is_some_and(|suffix| suffix.starts_with("overload["));
        let scope_path = validated_scope_path(row, 2, &semantic_path)?;
        let reference_facts = reference_facts_from_json_column(&reference_facts_json, 13)?;
        Ok(IndexedSymbol {
            symbol_id,
            base_name: symbol_base_name(&semantic_path),
            semantic_path,
            scope_path,
            file_path,
            node_kind: nonempty_string_from_row(row, 4, "node_kind")?,
            byte_range: byte_range_from_row(row, 5, 6)?,
            is_overload,
            signature: optional_nonempty_string_from_row(row, 7, "signature")?,
            parameters,
            return_type: optional_nonempty_string_from_row(row, 9, "return_type")?,
            docstring: optional_nonempty_string_from_row(row, 10, "docstring")?,
            reference_facts,
            references_by_name: reference_names,
            call_arities_by_name,
        })
    })?;

    let mut grouped = BTreeMap::new();
    for row in rows {
        if let Some(deadline) = deadline {
            deadline.check("loading indexed symbol rows")?;
        }
        let symbol = row?;
        grouped
            .entry(symbol.file_path.clone())
            .or_insert_with(Vec::new)
            .push(symbol);
    }
    if let Some(deadline) = deadline {
        deadline.check("loading indexed symbol rows")?;
    }
    Ok(grouped)
}

pub(crate) fn load_resolved_symbols(connection: &Connection) -> Result<(Vec<SymbolMeta>, usize)> {
    load_resolved_symbols_with_deadline(connection, None)
}

pub(crate) fn load_resolved_symbols_with_deadline(
    connection: &Connection,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    let indexed_files = load_indexed_files_metadata_with_deadline(connection, deadline)?;

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
        if let Some(deadline) = deadline {
            deadline.check("loading resolved symbol rows")?;
        }
        symbols.push(row?);
    }

    if let Some(deadline) = deadline {
        deadline.check("loading resolved symbol rows")?;
    }
    Ok((symbols, indexed_files))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use rusqlite::Connection;

    use crate::workspace_scan::WorkspaceScanDeadline;

    use super::super::loading_values::{
        call_arities_from_json_column, string_list_from_json_column,
    };
    use super::load_resolved_symbols_with_deadline;

    #[test]
    fn string_list_from_json_column_rejects_duplicate_entries() {
        let error = string_list_from_json_column(r#"["helper", "helper"]"#, 0, "dependencies_json")
            .expect_err("duplicate persisted list entries should be rejected");

        assert!(error.to_string().contains("duplicate"));
    }

    #[test]
    fn call_arities_from_json_column_rejects_duplicate_object_keys() {
        let error = call_arities_from_json_column(r#"{"helper":[1],"helper":[2]}"#, 0)
            .expect_err("duplicate persisted map keys should be rejected");

        assert!(error.to_string().contains("duplicate object key"));
    }

    #[test]
    fn load_resolved_symbols_checks_deadline_before_metadata_query() {
        let connection = Connection::open_in_memory().unwrap();
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = load_resolved_symbols_with_deadline(&connection, Some(&deadline))
            .expect_err("expired deadline should stop before metadata loading");

        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }
}
