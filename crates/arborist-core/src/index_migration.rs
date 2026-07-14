use crate::model::SymbolIndexMigrationPlan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MigrationAction {
    None,
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

pub(crate) fn unsupported_schema_version() -> SymbolIndexMigrationPlan {
    plan(
        MigrationAction::Rebuild,
        "stored schema_version is unsupported by this Arborist build; rebuild the symbol index",
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

fn plan(action: MigrationAction, reason: &str) -> SymbolIndexMigrationPlan {
    match action {
        MigrationAction::None => SymbolIndexMigrationPlan::none(reason),
        MigrationAction::Rebuild => SymbolIndexMigrationPlan::rebuild(reason),
        MigrationAction::Manual => SymbolIndexMigrationPlan::manual(reason),
    }
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

        let manual = plan(MigrationAction::Manual, "inspect");
        assert!(manual.required);
        assert_eq!(manual.action, "manual");
        assert_eq!(manual.reason, "inspect");
    }
}
