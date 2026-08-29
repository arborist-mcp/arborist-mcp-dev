use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

use crate::deadline::DeadlineCheck;
use crate::language::{builtin_language_registry, normalize_absolute_path, normalize_path};

use super::schema::SYMBOL_INDEX_SCHEMA_VERSION;

const ANALYSIS_PROVENANCE_METADATA_KEY: &str = "analysis_provenance";
const ANALYSIS_PROVENANCE_SCHEMA_REVISION: &str = "1";
const REFERENCE_FACT_SCHEMA_REVISION: &str = "reference-facts-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnalysisProvenance {
    schema_revision: String,
    language_ids: Vec<String>,
    language_analysis_revisions: BTreeMap<String, String>,
    language_detection_policy_fingerprint: String,
    reference_fact_schema_revision: String,
}

fn current_analysis_provenance() -> AnalysisProvenance {
    let (language_ids, language_analysis_revisions, language_detection_policy_fingerprint) =
        builtin_language_registry().analysis_provenance();
    AnalysisProvenance {
        schema_revision: ANALYSIS_PROVENANCE_SCHEMA_REVISION.to_string(),
        language_ids,
        language_analysis_revisions,
        language_detection_policy_fingerprint,
        reference_fact_schema_revision: REFERENCE_FACT_SCHEMA_REVISION.to_string(),
    }
}

pub(crate) fn current_analysis_provenance_json() -> Result<String> {
    Ok(serde_json::to_string(&current_analysis_provenance())?)
}

pub(crate) fn open_symbol_index_read_only_with_deadline(
    db_path: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Connection> {
    check_deadline(deadline, "opening persisted index")?;
    let connection = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    check_deadline(deadline, "opening persisted index")?;
    Ok(connection)
}

pub(crate) fn persist_symbol_index_metadata(
    tx: &Transaction<'_>,
    workspace_root: &Path,
    indexed_files: usize,
) -> Result<()> {
    tx.execute(
        "INSERT INTO metadata(key, value) VALUES('schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [SYMBOL_INDEX_SCHEMA_VERSION],
    )?;
    tx.execute(
        "INSERT INTO metadata(key, value) VALUES('workspace_root', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [normalize_path(workspace_root)],
    )?;
    tx.execute(
        "INSERT INTO metadata(key, value) VALUES('indexed_files', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [indexed_files.to_string()],
    )?;
    persist_current_analysis_provenance(tx)?;
    Ok(())
}

pub(crate) fn persist_current_analysis_provenance(tx: &Transaction<'_>) -> Result<()> {
    let provenance = current_analysis_provenance_json()?;
    tx.execute(
        "INSERT INTO metadata(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        (ANALYSIS_PROVENANCE_METADATA_KEY, provenance),
    )?;
    Ok(())
}

pub(crate) fn validate_symbol_index_analysis_provenance_with_deadline(
    connection: &Connection,
    db_path: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    check_deadline(deadline, "validating index analysis provenance")?;
    let Some(stored) = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = ?1",
            [ANALYSIS_PROVENANCE_METADATA_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    else {
        bail!(
            "missing analysis_provenance metadata in symbol index {}; rebuild the index",
            db_path.display()
        );
    };
    check_deadline(deadline, "validating index analysis provenance")?;
    let stored: AnalysisProvenance = serde_json::from_str(&stored).map_err(|error| {
        anyhow!(
            "invalid analysis_provenance metadata in symbol index {}: {error}",
            db_path.display()
        )
    })?;
    let expected = current_analysis_provenance();
    check_deadline(deadline, "validating index analysis provenance")?;
    if stored != expected {
        bail!(
            "analysis_provenance metadata in symbol index {} does not match current analysis behavior; rebuild the index",
            db_path.display()
        );
    }
    Ok(())
}

pub(crate) fn load_symbol_index_workspace_root(
    connection: &Connection,
    db_path: &Path,
) -> Result<PathBuf> {
    load_symbol_index_workspace_root_with_deadline(connection, db_path, None)
}

pub(crate) fn load_symbol_index_workspace_root_with_deadline(
    connection: &Connection,
    db_path: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<PathBuf> {
    check_deadline(deadline, "loading indexed workspace root")?;
    let Some(stored_workspace) = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'workspace_root'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    else {
        return Err(anyhow!(
            "missing workspace_root metadata in symbol index {}",
            db_path.display()
        ));
    };
    check_deadline(deadline, "loading indexed workspace root")?;

    let stored_workspace_path = Path::new(&stored_workspace);
    if !stored_workspace_path.is_absolute() {
        return Err(anyhow!(
            "workspace_root metadata in symbol index {} is not a normalized absolute path: {}",
            db_path.display(),
            stored_workspace
        ));
    }

    let normalized_workspace = normalize_absolute_path(stored_workspace_path)?;
    check_deadline(deadline, "validating indexed workspace root")?;
    if normalize_path(&normalized_workspace) != stored_workspace {
        return Err(anyhow!(
            "workspace_root metadata in symbol index {} is not a normalized absolute path: {}",
            db_path.display(),
            stored_workspace
        ));
    }

    Ok(normalized_workspace)
}

pub(crate) fn validate_symbol_index_schema_version_with_deadline(
    connection: &Connection,
    db_path: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    check_deadline(deadline, "validating index schema version")?;
    let Some(value) = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    else {
        return Err(anyhow!(
            "missing schema_version metadata in symbol index {}",
            db_path.display()
        ));
    };
    check_deadline(deadline, "validating index schema version")?;

    if value != SYMBOL_INDEX_SCHEMA_VERSION {
        return Err(anyhow!(
            "unsupported symbol index schema_version `{}` in {}; expected `{}`",
            value,
            db_path.display(),
            SYMBOL_INDEX_SCHEMA_VERSION
        ));
    }

    Ok(())
}

fn check_deadline(deadline: Option<&dyn DeadlineCheck>, phase: &str) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check(phase)?;
    }
    Ok(())
}

pub(crate) fn load_optional_metadata_value(
    connection: &Connection,
    key: &str,
) -> Result<Option<String>> {
    load_optional_metadata_value_with_deadline(connection, key, None)
}

pub(crate) fn load_optional_metadata_value_with_deadline(
    connection: &Connection,
    key: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<String>> {
    check_deadline(deadline, "loading index metadata")?;
    let value = connection
        .query_row("SELECT value FROM metadata WHERE key = ?1", [key], |row| {
            row.get::<_, String>(0)
        })
        .optional()?;
    check_deadline(deadline, "loading index metadata")?;
    Ok(value)
}

pub(crate) fn validate_symbol_index_workspace_with_deadline(
    connection: &Connection,
    workspace_root: &Path,
    db_path: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let expected_workspace = normalize_path(workspace_root);
    let stored_workspace =
        load_symbol_index_workspace_root_with_deadline(connection, db_path, deadline)?;
    let stored_workspace = normalize_path(&stored_workspace);

    if stored_workspace != expected_workspace {
        return Err(anyhow!(
            "symbol index {} belongs to workspace {}, not {}",
            db_path.display(),
            stored_workspace,
            expected_workspace
        ));
    }

    check_deadline(deadline, "validating indexed workspace")?;
    Ok(())
}

pub(crate) fn load_indexed_files_metadata_with_deadline(
    connection: &Connection,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<usize> {
    check_deadline(deadline, "loading indexed file metadata")?;
    let Some(value) = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'indexed_files'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    else {
        return Err(anyhow!("missing indexed_files metadata"));
    };

    let indexed_files = value
        .parse::<usize>()
        .map_err(|error| anyhow!("invalid indexed_files metadata `{value}`: {error}"))?;
    check_deadline(deadline, "loading indexed file metadata")?;
    Ok(indexed_files)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::{Duration, Instant};

    use rusqlite::Connection;

    use crate::deadline::DeadlineCheck;
    use crate::workspace_scan::WorkspaceScanDeadline;

    use super::{
        load_indexed_files_metadata_with_deadline, load_optional_metadata_value_with_deadline,
        load_symbol_index_workspace_root_with_deadline, open_symbol_index_read_only_with_deadline,
        validate_symbol_index_workspace_with_deadline,
    };

    struct ExpiredDeadline;

    impl DeadlineCheck for ExpiredDeadline {
        fn check(&self, phase: &str) -> anyhow::Result<()> {
            assert_eq!(phase, "opening persisted index");
            anyhow::bail!("test deadline expired during {phase}");
        }
    }

    #[test]
    fn opening_read_only_index_checks_deadline_before_opening_database() {
        let error = open_symbol_index_read_only_with_deadline(
            Path::new("unreachable-symbol-index.db"),
            Some(&ExpiredDeadline),
        )
        .expect_err("expired deadline should stop before opening the database");

        assert!(
            error
                .to_string()
                .contains("test deadline expired during opening persisted index")
        );
    }

    #[test]
    fn load_indexed_files_metadata_checks_deadline_before_query() {
        let connection = Connection::open_in_memory().unwrap();
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = load_indexed_files_metadata_with_deadline(
            &connection,
            Some(&deadline as &dyn DeadlineCheck),
        )
        .expect_err("expired deadline should stop before metadata loading");

        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }

    #[test]
    fn load_optional_metadata_checks_deadline_before_query() {
        let connection = Connection::open_in_memory().unwrap();
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = load_optional_metadata_value_with_deadline(
            &connection,
            "schema_version",
            Some(&deadline),
        )
        .expect_err("expired deadline should stop before metadata loading");

        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }

    #[test]
    fn load_workspace_root_checks_deadline_before_query() {
        let connection = Connection::open_in_memory().unwrap();
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = load_symbol_index_workspace_root_with_deadline(
            &connection,
            Path::new("symbols.db"),
            Some(&deadline),
        )
        .expect_err("expired deadline should stop before workspace metadata loading");

        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }

    #[test]
    fn validate_workspace_checks_deadline_before_query() {
        let connection = Connection::open_in_memory().unwrap();
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error = validate_symbol_index_workspace_with_deadline(
            &connection,
            Path::new("workspace"),
            Path::new("symbols.db"),
            Some(&deadline),
        )
        .expect_err("expired deadline should stop before workspace validation");

        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }
}
