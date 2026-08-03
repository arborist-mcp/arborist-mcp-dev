use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, anyhow};
use rusqlite::{Connection, Transaction};

use crate::language::detect_language;
use crate::model::LanguageId;
use crate::semantic::cpp_callable_symbol_id;

use super::SYMBOL_INDEX_SCHEMA_VERSION;
use crate::symbol_reference_compat::reference_facts_from_legacy;

pub(crate) fn migrate_symbol_index_schema_to_current(connection: &mut Connection) -> Result<()> {
    let transaction = connection.transaction()?;
    let legacy_has_call_arities =
        symbols_table_has_column(&transaction, "reference_call_arities_json")?;
    let legacy_call_arities = if legacy_has_call_arities {
        "COALESCE(reference_call_arities_json, '{}')"
    } else {
        "'{}'"
    };
    transaction.execute_batch(&format!(
        "
        DROP INDEX IF EXISTS idx_symbols_file_path;
        ALTER TABLE symbols RENAME TO symbols_legacy;
        CREATE TABLE symbols (
            symbol_id TEXT NOT NULL,
            semantic_path TEXT NOT NULL,
            scope_path TEXT,
            file_path TEXT NOT NULL,
            node_kind TEXT NOT NULL,
            start_byte INTEGER NOT NULL,
            end_byte INTEGER NOT NULL,
            signature TEXT,
            parameters_json TEXT NOT NULL DEFAULT '[]',
            return_type TEXT,
            docstring TEXT,
            dependencies_json TEXT NOT NULL,
            references_json TEXT NOT NULL,
            reference_names_json TEXT NOT NULL DEFAULT '[]',
            reference_call_arities_json TEXT NOT NULL DEFAULT '{{}}',
            reference_facts_json TEXT NOT NULL DEFAULT '[]',
            PRIMARY KEY (symbol_id, file_path, start_byte, end_byte)
        );
        INSERT INTO symbols (
            symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
            signature, parameters_json, return_type, docstring, dependencies_json,
            references_json, reference_names_json, reference_call_arities_json
        )
        SELECT
            COALESCE(NULLIF(symbol_id, ''), semantic_path),
            semantic_path, scope_path, file_path, node_kind, start_byte, end_byte, signature,
            COALESCE(parameters_json, '[]'), return_type, docstring,
            dependencies_json, references_json,
            COALESCE(reference_names_json, '[]'),
            {legacy_call_arities}
        FROM symbols_legacy;
        DROP TABLE symbols_legacy;
        CREATE INDEX idx_symbols_file_path ON symbols(file_path);
        DELETE FROM file_state;
        "
    ))?;
    migrate_reference_facts(&transaction)?;
    migrate_cpp_callable_symbol_ids(&transaction)?;
    transaction.execute(
        "UPDATE metadata SET value = ?1 WHERE key = 'schema_version'",
        [SYMBOL_INDEX_SCHEMA_VERSION],
    )?;
    transaction.commit()?;
    Ok(())
}

fn symbols_table_has_column(transaction: &Transaction<'_>, column_name: &str) -> Result<bool> {
    let mut statement = transaction.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == column_name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn migrate_reference_facts(transaction: &Transaction<'_>) -> Result<()> {
    let mut statement = transaction
        .prepare("SELECT rowid, reference_names_json, reference_call_arities_json FROM symbols")?;
    let mut rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut update =
        transaction.prepare("UPDATE symbols SET reference_facts_json = ?1 WHERE rowid = ?2")?;

    for row in &mut rows {
        let (rowid, reference_names_json, reference_call_arities_json) = row?;
        let reference_names: BTreeSet<String> = serde_json::from_str(&reference_names_json)
            .map_err(|error| {
                anyhow!("invalid reference_names_json while migrating symbol row {rowid}: {error}")
            })?;
        let call_arities_by_name: BTreeMap<String, BTreeSet<usize>> = serde_json::from_str(
            &reference_call_arities_json,
        )
        .map_err(|error| {
            anyhow!(
                "invalid reference_call_arities_json while migrating symbol row {rowid}: {error}"
            )
        })?;
        let reference_facts = reference_facts_from_legacy(&reference_names, &call_arities_by_name);
        let reference_facts_json = serde_json::to_string(&reference_facts)?;
        update.execute((&reference_facts_json, rowid))?;
    }

    Ok(())
}

fn migrate_cpp_callable_symbol_ids(transaction: &Transaction<'_>) -> Result<()> {
    let mut statement = transaction.prepare(
        "SELECT rowid, semantic_path, file_path, node_kind, signature, parameters_json FROM symbols",
    )?;
    let mut rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;
    let mut update = transaction.prepare("UPDATE symbols SET symbol_id = ?1 WHERE rowid = ?2")?;

    for row in &mut rows {
        let (rowid, semantic_path, file_path, node_kind, signature, parameters_json) = row?;
        if detect_language(Path::new(&file_path)).ok() != Some(LanguageId::Cpp)
            || !matches!(
                node_kind.as_str(),
                "function_definition" | "declaration" | "field_declaration"
            )
        {
            continue;
        }

        let parameters =
            serde_json::from_str::<Vec<String>>(&parameters_json).map_err(|error| {
                anyhow!(
                    "invalid parameters_json while migrating C++ symbol `{semantic_path}`: {error}"
                )
            })?;
        let symbol_id = cpp_callable_symbol_id(&semantic_path, &parameters, signature.as_deref());
        update.execute((&symbol_id, rowid))?;
    }

    Ok(())
}
