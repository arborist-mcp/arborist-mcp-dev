use std::path::Path;

use anyhow::{Result, anyhow, bail};
use rusqlite::Connection;

use crate::index_schema::{
    LEGACY_SYMBOL_INDEX_SCHEMA_VERSION, OLDEST_SYMBOL_INDEX_SCHEMA_VERSION,
    PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION, SYMBOL_INDEX_SCHEMA_VERSION, load_indexed_files_metadata,
    load_optional_metadata_value, load_symbol_index_workspace_root,
    migrate_symbol_index_schema_to_current, require_legacy_symbol_index_schema,
    require_previous_symbol_index_schema, require_symbol_index_tables,
};
use crate::model::SymbolIndexMigrationPlan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MigrationAction {
    None,
    Migrate,
    Rebuild,
    Manual,
}

pub(crate) fn pending_inspection() -> SymbolIndexMigrationPlan {
    plan(MigrationAction::None, "symbol index inspection is pending")
}

pub(crate) fn missing_index() -> SymbolIndexMigrationPlan {
    plan(
        MigrationAction::Rebuild,
        "symbol index is missing; rebuild_symbol_index can create it",
    )
}

pub(crate) fn incomplete_or_foreign_database() -> SymbolIndexMigrationPlan {
    plan(
        MigrationAction::Manual,
        "database is not a complete Arborist symbol index; choose a new db_path or replace it explicitly",
    )
}

pub(crate) fn missing_schema_version() -> SymbolIndexMigrationPlan {
    plan(
        MigrationAction::Manual,
        "schema_version metadata is missing; choose a new db_path or explicitly rebuild this index",
    )
}

pub(crate) fn unsupported_schema_version(stored_version: &str) -> SymbolIndexMigrationPlan {
    let action = schema_version_action(stored_version);
    let reason = if action == MigrationAction::Migrate {
        supported_schema_migration_reason(stored_version)
    } else {
        unsupported_schema_reason().to_string()
    };
    plan(action, &reason)
}

pub(crate) fn is_migratable_symbol_index_schema_version(stored_version: &str) -> bool {
    matches!(
        stored_version,
        OLDEST_SYMBOL_INDEX_SCHEMA_VERSION
            | LEGACY_SYMBOL_INDEX_SCHEMA_VERSION
            | PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION
    )
}

pub(crate) fn migrate_symbol_index(connection: &mut Connection, db_path: &Path) -> Result<()> {
    require_symbol_index_tables(connection, db_path)?;
    let stored_version =
        load_optional_metadata_value(connection, "schema_version")?.ok_or_else(|| {
            anyhow!(
                "missing schema_version metadata in symbol index {}",
                db_path.display()
            )
        })?;

    if !is_migratable_symbol_index_schema_version(&stored_version) {
        bail!(
            "symbol index schema_version `{stored_version}` in {} cannot be migrated by this Arborist build; expected `{OLDEST_SYMBOL_INDEX_SCHEMA_VERSION}`, `{LEGACY_SYMBOL_INDEX_SCHEMA_VERSION}`, or `{PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION}`",
            db_path.display()
        );
    }

    if stored_version == PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION {
        require_previous_symbol_index_schema(connection, db_path)?;
    } else {
        require_legacy_symbol_index_schema(connection, db_path)?;
    }
    load_symbol_index_workspace_root(connection, db_path)?;
    load_indexed_files_metadata(connection)?;
    migrate_symbol_index_schema_to_current(connection)
}

pub(crate) fn healthy_index() -> SymbolIndexMigrationPlan {
    plan(
        MigrationAction::None,
        "index schema and persisted file fingerprints are current",
    )
}

pub(crate) fn failed_health_checks() -> SymbolIndexMigrationPlan {
    plan(
        MigrationAction::Rebuild,
        "index health checks failed; rebuild after reviewing reported issues",
    )
}

fn plan(action: MigrationAction, reason: &str) -> SymbolIndexMigrationPlan {
    match action {
        MigrationAction::None => SymbolIndexMigrationPlan::none(reason),
        MigrationAction::Migrate => SymbolIndexMigrationPlan::migrate(reason),
        MigrationAction::Rebuild => SymbolIndexMigrationPlan::rebuild(reason),
        MigrationAction::Manual => SymbolIndexMigrationPlan::manual(reason),
    }
}

fn schema_version_action(stored_version: &str) -> MigrationAction {
    if is_migratable_symbol_index_schema_version(stored_version) {
        MigrationAction::Migrate
    } else {
        MigrationAction::Rebuild
    }
}

fn unsupported_schema_reason() -> &'static str {
    "stored schema_version is unsupported by this Arborist build; rebuild the symbol index"
}

fn supported_schema_migration_reason(stored_version: &str) -> String {
    format!(
        "stored schema_version {stored_version} can migrate in place to schema_version {SYMBOL_INDEX_SCHEMA_VERSION}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_actions_map_to_public_plan_flags() {
        let none = plan(MigrationAction::None, "ready");
        assert!(!none.required);
        assert_eq!(none.action, "none");
        assert_eq!(none.reason, "ready");

        let rebuild = plan(MigrationAction::Rebuild, "refresh");
        assert!(rebuild.required);
        assert_eq!(rebuild.action, "rebuild");
        assert_eq!(rebuild.reason, "refresh");

        let migrate = plan(MigrationAction::Migrate, "upgrade");
        assert!(migrate.required);
        assert_eq!(migrate.action, "migrate");
        assert_eq!(migrate.reason, "upgrade");

        let manual = plan(MigrationAction::Manual, "inspect");
        assert!(manual.required);
        assert_eq!(manual.action, "manual");
        assert_eq!(manual.reason, "inspect");
    }

    #[test]
    fn unsupported_schema_versions_recommend_rebuild_until_migrations_exist() {
        let plan = unsupported_schema_version("99");
        assert!(plan.required);
        assert_eq!(plan.action, "rebuild");
        assert_eq!(plan.reason, unsupported_schema_reason());
    }

    #[test]
    fn previous_schema_version_recommends_in_place_migration() {
        let plan = unsupported_schema_version(PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION);
        assert!(plan.required);
        assert_eq!(plan.action, "migrate");
        assert_eq!(
            plan.reason,
            supported_schema_migration_reason(PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION)
        );
    }

    #[test]
    fn legacy_schema_version_recommends_in_place_migration() {
        let plan = unsupported_schema_version(LEGACY_SYMBOL_INDEX_SCHEMA_VERSION);
        assert!(plan.required);
        assert_eq!(plan.action, "migrate");
        assert_eq!(
            plan.reason,
            supported_schema_migration_reason(LEGACY_SYMBOL_INDEX_SCHEMA_VERSION)
        );
    }
}
