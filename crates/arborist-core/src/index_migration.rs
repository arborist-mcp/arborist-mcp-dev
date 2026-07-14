use crate::model::SymbolIndexMigrationPlan;

pub(crate) fn pending_inspection() -> SymbolIndexMigrationPlan {
    SymbolIndexMigrationPlan::none("symbol index inspection is pending")
}

pub(crate) fn missing_index() -> SymbolIndexMigrationPlan {
    SymbolIndexMigrationPlan::rebuild("symbol index is missing; rebuild_symbol_index can create it")
}

pub(crate) fn incomplete_or_foreign_database() -> SymbolIndexMigrationPlan {
    SymbolIndexMigrationPlan::manual(
        "database is not a complete Arborist symbol index; choose a new db_path or replace it explicitly",
    )
}

pub(crate) fn missing_schema_version() -> SymbolIndexMigrationPlan {
    SymbolIndexMigrationPlan::manual(
        "schema_version metadata is missing; choose a new db_path or explicitly rebuild this index",
    )
}

pub(crate) fn unsupported_schema_version() -> SymbolIndexMigrationPlan {
    SymbolIndexMigrationPlan::rebuild(
        "stored schema_version is unsupported by this Arborist build; rebuild the symbol index",
    )
}

pub(crate) fn healthy_index() -> SymbolIndexMigrationPlan {
    SymbolIndexMigrationPlan::none("index schema and persisted file fingerprints are current")
}

pub(crate) fn failed_health_checks() -> SymbolIndexMigrationPlan {
    SymbolIndexMigrationPlan::rebuild(
        "index health checks failed; rebuild after reviewing reported issues",
    )
}
