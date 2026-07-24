use crate::index_schema::{
    LEGACY_SYMBOL_INDEX_SCHEMA_VERSION, OLDEST_SYMBOL_INDEX_SCHEMA_VERSION,
    PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION, SYMBOL_INDEX_SCHEMA_VERSION,
};
use crate::model::SymbolIndexMigrationPlan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MigrationAction {
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

pub(super) fn plan(action: MigrationAction, reason: &str) -> SymbolIndexMigrationPlan {
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
