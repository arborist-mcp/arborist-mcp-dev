use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use super::super::{SYMBOL_INDEX_HEALTH_RESPONSE_SCHEMA_VERSION, ensure_nonblank};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SymbolIndexStats {
    pub db_path: String,
    pub indexed_files: usize,
    pub indexed_symbols: usize,
    pub rebuilt_files: usize,
    pub reused_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RegisteredSymbolIndex {
    pub workspace_root: String,
    pub db_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SymbolIndexHealth {
    pub response_schema_version: String,
    pub db_path: String,
    pub exists: bool,
    pub ok: bool,
    pub schema_version: Option<String>,
    pub expected_schema_version: String,
    pub migration: SymbolIndexMigrationPlan,
    pub workspace_root: Option<String>,
    pub indexed_files: Option<usize>,
    pub indexed_symbols: Option<usize>,
    pub file_state_entries: Option<usize>,
    pub fresh_file_count: Option<usize>,
    pub stale_files: Vec<String>,
    pub missing_files: Vec<String>,
    pub unreadable_files: Vec<String>,
    pub unindexed_files: Vec<String>,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SymbolIndexMigrationPlan {
    pub required: bool,
    pub action: String,
    pub reason: String,
}

impl SymbolIndexStats {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        ensure_nonblank(&self.db_path, "symbol_index.db_path")?;
        if self.rebuilt_files + self.reused_files != self.indexed_files {
            bail!(
                "invalid symbol_index.indexed_files: expected indexed_files to equal rebuilt_files + reused_files"
            );
        }
        Ok(())
    }
}

impl RegisteredSymbolIndex {
    pub(crate) fn validate_public_output(&self, index: usize) -> Result<()> {
        let prefix = format!("registered_symbol_indexes[{index}]");
        ensure_nonblank(&self.workspace_root, &format!("{prefix}.workspace_root"))?;
        ensure_nonblank(&self.db_path, &format!("{prefix}.db_path"))?;
        Ok(())
    }
}

impl SymbolIndexHealth {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        if self.response_schema_version != SYMBOL_INDEX_HEALTH_RESPONSE_SCHEMA_VERSION {
            bail!(
                "invalid symbol_index_health.response_schema_version: expected response schema version {}",
                SYMBOL_INDEX_HEALTH_RESPONSE_SCHEMA_VERSION
            );
        }
        ensure_nonblank(&self.db_path, "symbol_index_health.db_path")?;
        ensure_nonblank(
            &self.expected_schema_version,
            "symbol_index_health.expected_schema_version",
        )?;
        self.migration.validate_public_output()?;
        if self.ok && !self.issues.is_empty() {
            bail!("invalid symbol_index_health.ok: expected healthy indexes to have no issues");
        }
        if self.ok && self.migration.required {
            bail!(
                "invalid symbol_index_health.migration: healthy indexes must not require migration"
            );
        }
        if !self.ok && self.issues.is_empty() {
            bail!(
                "invalid symbol_index_health.issues: expected unhealthy indexes to report issues"
            );
        }
        if !self.exists
            && (self.schema_version.is_some()
                || self.workspace_root.is_some()
                || self.indexed_files.is_some()
                || self.indexed_symbols.is_some()
                || self.file_state_entries.is_some()
                || self.fresh_file_count.is_some()
                || !self.stale_files.is_empty()
                || !self.missing_files.is_empty()
                || !self.unreadable_files.is_empty()
                || !self.unindexed_files.is_empty())
        {
            bail!("invalid symbol_index_health: missing indexes must not report loaded metadata");
        }
        if let Some(fresh_file_count) = self.fresh_file_count {
            let Some(file_state_entries) = self.file_state_entries else {
                bail!(
                    "invalid symbol_index_health.fresh_file_count: expected file_state_entries when freshness is inspected"
                );
            };
            if fresh_file_count
                + self.stale_files.len()
                + self.missing_files.len()
                + self.unreadable_files.len()
                != file_state_entries
            {
                bail!(
                    "invalid symbol_index_health freshness counts: expected fresh, stale, missing, and unreadable files to equal file_state_entries"
                );
            }
        }
        for (index, file_path) in self.stale_files.iter().enumerate() {
            ensure_nonblank(
                file_path,
                &format!("symbol_index_health.stale_files[{index}]"),
            )?;
        }
        for (index, file_path) in self.missing_files.iter().enumerate() {
            ensure_nonblank(
                file_path,
                &format!("symbol_index_health.missing_files[{index}]"),
            )?;
        }
        for (index, file_path) in self.unreadable_files.iter().enumerate() {
            ensure_nonblank(
                file_path,
                &format!("symbol_index_health.unreadable_files[{index}]"),
            )?;
        }
        for (index, file_path) in self.unindexed_files.iter().enumerate() {
            ensure_nonblank(
                file_path,
                &format!("symbol_index_health.unindexed_files[{index}]"),
            )?;
        }
        for (index, issue) in self.issues.iter().enumerate() {
            ensure_nonblank(issue, &format!("symbol_index_health.issues[{index}]"))?;
        }
        Ok(())
    }
}

impl SymbolIndexMigrationPlan {
    pub(crate) fn none(reason: &str) -> Self {
        Self {
            required: false,
            action: "none".to_string(),
            reason: reason.to_string(),
        }
    }

    pub(crate) fn rebuild(reason: &str) -> Self {
        Self {
            required: true,
            action: "rebuild".to_string(),
            reason: reason.to_string(),
        }
    }

    pub(crate) fn migrate(reason: &str) -> Self {
        Self {
            required: true,
            action: "migrate".to_string(),
            reason: reason.to_string(),
        }
    }

    pub(crate) fn manual(reason: &str) -> Self {
        Self {
            required: true,
            action: "manual".to_string(),
            reason: reason.to_string(),
        }
    }

    fn validate_public_output(&self) -> Result<()> {
        ensure_nonblank(&self.action, "symbol_index_health.migration.action")?;
        ensure_nonblank(&self.reason, "symbol_index_health.migration.reason")?;
        match self.action.as_str() {
            "none" | "migrate" | "rebuild" | "manual" => {}
            action => {
                bail!("invalid symbol_index_health.migration.action: unsupported action `{action}`")
            }
        }
        if !self.required && self.action != "none" {
            bail!(
                "invalid symbol_index_health.migration.required: optional migration must use action `none`"
            );
        }
        Ok(())
    }
}
