use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use anyhow::{Context, Result};
use rusqlite::{Connection, Row, types::Type};
use serde::de::{self, DeserializeOwned, MapAccess, Visitor};

use crate::index_schema::load_indexed_files_metadata;
use crate::model::{SymbolMeta, SymbolMetaInit};
use crate::semantic::semantic_parent_path;
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::workspace_scan::WorkspaceScanDeadline;

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

pub(crate) fn load_indexed_symbols_grouped_by_file_with_deadline(
    connection: &Connection,
    deadline: &WorkspaceScanDeadline,
) -> Result<BTreeMap<String, Vec<IndexedSymbol>>> {
    load_indexed_symbols_grouped_by_file_with_query_and_deadline(
        connection,
        "SELECT symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, reference_names_json,
                reference_call_arities_json
         FROM symbols
         ORDER BY file_path, semantic_path",
        Some(deadline),
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
    load_indexed_symbols_grouped_by_file_with_query_and_deadline(connection, query, None)
}

fn load_indexed_symbols_grouped_by_file_with_query_and_deadline(
    connection: &Connection,
    query: &str,
    deadline: Option<&WorkspaceScanDeadline>,
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
        if let Some(deadline) = deadline {
            deadline.check("loading indexed symbol rows")?;
        }
        let symbol = row?;
        grouped
            .entry(symbol.file_path.clone())
            .or_insert_with(Vec::new)
            .push(symbol);
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
    if let Some(deadline) = deadline {
        deadline.check("loading indexed file metadata")?;
    }
    let indexed_files = load_indexed_files_metadata(connection)?;
    if let Some(deadline) = deadline {
        deadline.check("loading indexed file metadata")?;
    }

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

    Ok((symbols, indexed_files))
}

pub(super) fn nonempty_string_from_row(
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
    let mut seen = BTreeSet::new();
    if values.iter().any(|value| !seen.insert(value.clone())) {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            column,
            Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("duplicate {column_name} entry"),
            )),
        ));
    }
    Ok(values)
}

fn call_arities_from_json_column(
    json: &str,
    column: usize,
) -> rusqlite::Result<BTreeMap<String, BTreeSet<usize>>> {
    let call_arities = serde_json::from_str::<UniqueStringMap<BTreeSet<usize>>>(json)
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(column, Type::Text, Box::new(error))
        })?
        .0;
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

struct UniqueStringMap<T>(BTreeMap<String, T>);

impl<'de, T> serde::Deserialize<'de> for UniqueStringMap<T>
where
    T: serde::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct UniqueStringMapVisitor<T>(std::marker::PhantomData<T>);

        impl<'de, T> Visitor<'de> for UniqueStringMapVisitor<T>
        where
            T: serde::Deserialize<'de>,
        {
            type Value = UniqueStringMap<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object with unique string keys")
            }

            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = BTreeMap::new();
                while let Some((key, value)) = access.next_entry::<String, T>()? {
                    if values.insert(key, value).is_some() {
                        return Err(de::Error::custom("duplicate object key"));
                    }
                }
                Ok(UniqueStringMap(values))
            }
        }

        deserializer.deserialize_map(UniqueStringMapVisitor(std::marker::PhantomData))
    }
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use rusqlite::Connection;

    use crate::workspace_scan::WorkspaceScanDeadline;

    use super::{
        call_arities_from_json_column, load_resolved_symbols_with_deadline,
        string_list_from_json_column,
    };

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
