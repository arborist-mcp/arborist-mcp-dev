use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use rusqlite::{Row, types::Type};
use serde::de::{self, DeserializeOwned, MapAccess, Visitor};

use crate::semantic::semantic_parent_path;

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

pub(super) fn optional_nonempty_string_from_row(
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

pub(super) fn validated_scope_path(
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

pub(super) fn call_arities_from_json_column(
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
