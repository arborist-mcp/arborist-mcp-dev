mod execute;
mod plan;

pub(crate) use execute::migrate_symbol_index;
pub(crate) use plan::{
    failed_health_checks, healthy_index, incomplete_or_foreign_database,
    is_migratable_symbol_index_schema_version, missing_index, missing_schema_version,
    pending_inspection, unsupported_schema_version,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index_schema::{
        LEGACY_SYMBOL_INDEX_SCHEMA_VERSION, PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION,
    };

    #[test]
    fn migration_actions_map_to_public_plan_flags() {
        let none = plan::plan(plan::MigrationAction::None, "ready");
        assert!(!none.required);
        assert_eq!(none.action, "none");
        assert_eq!(none.reason, "ready");

        let rebuild = plan::plan(plan::MigrationAction::Rebuild, "refresh");
        assert!(rebuild.required);
        assert_eq!(rebuild.action, "rebuild");
        assert_eq!(rebuild.reason, "refresh");

        let migrate = plan::plan(plan::MigrationAction::Migrate, "upgrade");
        assert!(migrate.required);
        assert_eq!(migrate.action, "migrate");
        assert_eq!(migrate.reason, "upgrade");

        let manual = plan::plan(plan::MigrationAction::Manual, "inspect");
        assert!(manual.required);
        assert_eq!(manual.action, "manual");
        assert_eq!(manual.reason, "inspect");
    }

    #[test]
    fn unsupported_schema_versions_recommend_rebuild_until_migrations_exist() {
        let plan = unsupported_schema_version("99");
        assert!(plan.required);
        assert_eq!(plan.action, "rebuild");
    }

    #[test]
    fn previous_schema_version_recommends_in_place_migration() {
        let plan = unsupported_schema_version(PREVIOUS_SYMBOL_INDEX_SCHEMA_VERSION);
        assert!(plan.required);
        assert_eq!(plan.action, "migrate");
    }

    #[test]
    fn legacy_schema_version_recommends_in_place_migration() {
        let plan = unsupported_schema_version(LEGACY_SYMBOL_INDEX_SCHEMA_VERSION);
        assert!(plan.required);
        assert_eq!(plan.action, "migrate");
    }
}
