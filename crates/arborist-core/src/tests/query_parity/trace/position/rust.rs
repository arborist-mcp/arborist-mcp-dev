use super::*;
use crate::{trace_symbol_graph_from_index_with_source, trace_symbol_graph_with_source};

#[test]
fn traces_rust_unshadowed_local_direct_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("api.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "mod api {\n    pub fn caller() { helper(); }\n    pub fn helper() {}\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "api::helper", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, "api::helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "api::caller");

    let position = Position { row: 2, column: 11 };
    let live_at_position =
        trace_symbol_graph_at_position(&dir, &source_path, &position, TraceDirection::Callers)
            .unwrap();
    assert_eq!(live_at_position.symbol.symbol_id, "api::helper");
    assert_eq!(live_at_position.callers[0].symbol_id, "api::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "api::helper", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, "api::helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "api::caller");

    let persisted_at_position = trace_symbol_graph_at_position_from_index(
        &db_path,
        &source_path,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted_at_position.symbol.symbol_id, "api::helper");
    assert_eq!(persisted_at_position.callers[0].symbol_id, "api::caller");
}

#[test]
fn traces_rust_qualified_inline_module_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("api.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "fn caller() { api::helper(); }\n\nmod api {\n    pub fn helper() {}\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "api::helper", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, "api::helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "api::helper", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, "api::helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_rust_direct_out_of_line_module_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_path = dir.join("api.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api;\nfn caller() { api::helper(); }\nfn crate_caller() { crate::api::helper(); }\n",
    )
    .unwrap();
    fs::write(&api_path, "pub fn helper() {}\n").unwrap();

    for caller in ["caller", "crate_caller"] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert_eq!(live.indexed_files, 2);
        assert_eq!(
            live.callees.len(),
            1,
            "{caller} should resolve its direct module call"
        );
        assert_eq!(live.callees[0].symbol_id, "helper");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in ["caller", "crate_caller"] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert_eq!(persisted.indexed_files, 2);
        assert_eq!(
            persisted.callees.len(),
            1,
            "{caller} should resolve its direct module call from the persisted index"
        );
        assert_eq!(persisted.callees[0].symbol_id, "helper");
    }
}

#[test]
fn traces_rust_root_function_import_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_path = dir.join("api.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api;\nuse crate::api::helper;\nuse crate::api::helper as aliased_helper;\nfn caller() { helper(); }\nfn alias_caller() { aliased_helper(); }\n",
    )
    .unwrap();
    fs::write(&api_path, "pub fn helper() {}\n").unwrap();

    for caller in ["caller", "alias_caller"] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert_eq!(live.indexed_files, 2);
        assert_eq!(
            live.callees.len(),
            1,
            "{caller} should resolve its imported function"
        );
        assert_eq!(live.callees[0].symbol_id, "helper");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in ["caller", "alias_caller"] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert_eq!(persisted.indexed_files, 2);
        assert_eq!(
            persisted.callees.len(),
            1,
            "{caller} should resolve its imported function from the persisted index"
        );
        assert_eq!(persisted.callees[0].symbol_id, "helper");
    }
}

#[test]
fn traces_rust_self_function_import_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_directory = dir.join("api");
    let api_path = api_directory.join("mod.rs");
    let nested_path = api_directory.join("nested.rs");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&api_directory).unwrap();
    fs::write(&root_path, "mod api;\n").unwrap();
    fs::write(
        &api_path,
        "mod nested;\nuse self::nested::value;\nuse self::{nested::value as grouped_value};\nfn caller() { value(); }\nfn grouped_caller() { grouped_value(); }\n",
    )
    .unwrap();
    fs::write(&nested_path, "pub fn value() {}\n").unwrap();

    for caller in ["caller", "grouped_caller"] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert_eq!(live.indexed_files, 3);
        assert_eq!(
            live.callees.len(),
            1,
            "{caller} should resolve its self import"
        );
        assert_eq!(live.callees[0].symbol_id, "value");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in ["caller", "grouped_caller"] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert_eq!(persisted.indexed_files, 3);
        assert_eq!(
            persisted.callees.len(),
            1,
            "{caller} should resolve its self import from the persisted index"
        );
        assert_eq!(persisted.callees[0].symbol_id, "value");
    }
}

#[test]
fn traces_rust_self_function_import_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_directory = dir.join("api");
    let api_path = api_directory.join("mod.rs");
    let nested_path = api_directory.join("nested.rs");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&api_directory).unwrap();
    fs::write(&root_path, "mod api;\n").unwrap();
    fs::write(&api_path, "mod stale;\n").unwrap();
    fs::write(&nested_path, "pub fn value() {}\n").unwrap();
    let api_overlay =
        "mod nested;\nuse self::{nested::value as selected};\nfn caller() { selected(); }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &api_path,
        api_overlay,
        "caller",
        TraceDirection::Callees,
    )
    .unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "value");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &api_path,
        api_overlay,
        "caller",
        TraceDirection::Callees,
    )
    .unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "value");
}

#[test]
fn traces_rust_parent_qualified_calls_from_out_of_line_children() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_path = dir.join("api.rs");
    let api_directory = dir.join("api");
    let nested_path = api_directory.join("nested.rs");
    let sibling_path = dir.join("sibling.rs");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&api_directory).unwrap();
    fs::write(&root_path, "mod api;\nmod sibling;\nfn root_helper() {}\n").unwrap();
    fs::write(
        &api_path,
        "mod nested;\nmod inline {\n    fn inline_caller() { crate::sibling::helper(); super::super::root_helper(); }\n}\nfn caller() { crate::sibling::helper(); super::root_helper(); }\n",
    )
    .unwrap();
    fs::write(
        &nested_path,
        "fn nested_caller() { super::super::sibling::helper(); }\n",
    )
    .unwrap();
    fs::write(&sibling_path, "pub fn helper() {}\n").unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 2);
    assert!(
        live.callees
            .iter()
            .any(|symbol| symbol.symbol_id == "helper")
    );
    assert!(
        live.callees
            .iter()
            .any(|symbol| symbol.symbol_id == "root_helper")
    );
    let nested_live = trace_symbol_graph(&dir, "nested_caller", TraceDirection::Callees).unwrap();
    assert_eq!(nested_live.callees.len(), 1);
    assert_eq!(nested_live.callees[0].symbol_id, "helper");
    let inline_live =
        trace_symbol_graph(&dir, "inline::inline_caller", TraceDirection::Callees).unwrap();
    assert_eq!(inline_live.callees.len(), 2);
    assert!(
        inline_live
            .callees
            .iter()
            .any(|symbol| symbol.symbol_id == "helper")
    );
    assert!(
        inline_live
            .callees
            .iter()
            .any(|symbol| symbol.symbol_id == "root_helper")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 2);
    assert!(
        persisted
            .callees
            .iter()
            .any(|symbol| symbol.symbol_id == "helper")
    );
    assert!(
        persisted
            .callees
            .iter()
            .any(|symbol| symbol.symbol_id == "root_helper")
    );
    let nested_persisted =
        trace_symbol_graph_from_index(&db_path, "nested_caller", TraceDirection::Callees).unwrap();
    assert_eq!(nested_persisted.callees.len(), 1);
    assert_eq!(nested_persisted.callees[0].symbol_id, "helper");
    let inline_persisted =
        trace_symbol_graph_from_index(&db_path, "inline::inline_caller", TraceDirection::Callees)
            .unwrap();
    assert_eq!(inline_persisted.callees.len(), 2);
    assert!(
        inline_persisted
            .callees
            .iter()
            .any(|symbol| symbol.symbol_id == "helper")
    );
    assert!(
        inline_persisted
            .callees
            .iter()
            .any(|symbol| symbol.symbol_id == "root_helper")
    );
}

#[test]
fn traces_rust_parent_qualified_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_path = dir.join("api.rs");
    let sibling_path = dir.join("sibling.rs");
    let db_path = dir.join("symbols.db");
    fs::write(&root_path, "mod api;\nmod sibling;\nfn root_helper() {}\n").unwrap();
    fs::write(&api_path, "fn stale() {}\n").unwrap();
    fs::write(&sibling_path, "pub fn helper() {}\n").unwrap();
    let api_overlay = "fn caller() { crate::sibling::helper(); super::root_helper(); }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &api_path,
        api_overlay,
        "caller",
        TraceDirection::Callees,
    )
    .unwrap();
    assert_eq!(live.callees.len(), 2);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &api_path,
        api_overlay,
        "caller",
        TraceDirection::Callees,
    )
    .unwrap();
    assert_eq!(persisted.callees.len(), 2);
}

#[test]
fn does_not_trace_rust_parent_qualified_calls_with_ambiguous_parent_modules() {
    let dir = temporary_dir();
    let lib_path = dir.join("lib.rs");
    let main_path = dir.join("main.rs");
    let api_path = dir.join("api.rs");
    let sibling_path = dir.join("sibling.rs");
    fs::write(&lib_path, "mod api;\nmod sibling;\n").unwrap();
    fs::write(&main_path, "mod api;\nmod sibling;\n").unwrap();
    fs::write(
        &api_path,
        "fn caller() { crate::sibling::helper(); super::root_helper(); }\n",
    )
    .unwrap();
    fs::write(&sibling_path, "pub fn helper() {}\n").unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());
}

#[test]
fn traces_rust_crate_and_super_function_import_calls_from_out_of_line_children() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_path = dir.join("api.rs");
    let sibling_path = dir.join("sibling.rs");
    let db_path = dir.join("symbols.db");
    fs::write(&root_path, "mod api;\nmod sibling;\nfn root_helper() {}\n").unwrap();
    fs::write(
        &api_path,
        "use crate::{sibling::helper as crate_helper};\nuse super::root_helper;\nfn caller() { crate_helper(); root_helper(); }\n",
    )
    .unwrap();
    fs::write(&sibling_path, "pub fn helper() {}\n").unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callees.len(), 2);
    assert!(
        live.callees
            .iter()
            .any(|symbol| symbol.symbol_id == "helper")
    );
    assert!(
        live.callees
            .iter()
            .any(|symbol| symbol.symbol_id == "root_helper")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callees.len(), 2);
    assert!(
        persisted
            .callees
            .iter()
            .any(|symbol| symbol.symbol_id == "helper")
    );
    assert!(
        persisted
            .callees
            .iter()
            .any(|symbol| symbol.symbol_id == "root_helper")
    );
}

#[test]
fn traces_rust_repeated_super_function_import_calls() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_directory = dir.join("api");
    let api_path = api_directory.join("mod.rs");
    let nested_path = api_directory.join("nested.rs");
    let sibling_path = dir.join("sibling.rs");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&api_directory).unwrap();
    fs::write(&root_path, "mod api;\nmod sibling;\n").unwrap();
    fs::write(&api_path, "mod nested;\n").unwrap();
    fs::write(
        &nested_path,
        "use super::super::{sibling::helper as selected};\nfn caller() { selected(); }\n",
    )
    .unwrap();
    fs::write(&sibling_path, "pub fn helper() {}\n").unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "helper");
}

#[test]
fn traces_rust_crate_and_super_function_import_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_path = dir.join("api.rs");
    let sibling_path = dir.join("sibling.rs");
    let db_path = dir.join("symbols.db");
    fs::write(&root_path, "mod api;\nmod sibling;\nfn root_helper() {}\n").unwrap();
    fs::write(&api_path, "use crate::sibling::stale;\n").unwrap();
    fs::write(&sibling_path, "pub fn helper() {}\n").unwrap();
    let api_overlay = "use crate::{sibling::helper as crate_helper};\nuse super::root_helper;\nfn caller() { crate_helper(); root_helper(); }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &api_path,
        api_overlay,
        "caller",
        TraceDirection::Callees,
    )
    .unwrap();
    assert_eq!(live.callees.len(), 2);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &api_path,
        api_overlay,
        "caller",
        TraceDirection::Callees,
    )
    .unwrap();
    assert_eq!(persisted.callees.len(), 2);
}

#[test]
fn traces_rust_grouped_function_import_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_directory = dir.join("api");
    let api_path = api_directory.join("mod.rs");
    let nested_path = api_directory.join("nested.rs");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&api_directory).unwrap();
    fs::write(
        &root_path,
        "mod api;\nuse crate::api::{helper, nested::value as aliased_value};\nuse crate::{api::helper as root_alias};\nfn caller() { helper(); }\nfn alias_caller() { aliased_value(); }\nfn root_alias_caller() { root_alias(); }\n",
    )
    .unwrap();
    fs::write(&api_path, "mod nested;\npub fn helper() {}\n").unwrap();
    fs::write(&nested_path, "pub fn value() {}\n").unwrap();

    for (caller, target) in [
        ("caller", "helper"),
        ("alias_caller", "value"),
        ("root_alias_caller", "helper"),
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert_eq!(live.indexed_files, 3);
        assert_eq!(
            live.callees.len(),
            1,
            "{caller} should resolve its grouped import"
        );
        assert_eq!(live.callees[0].symbol_id, target);
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, target) in [
        ("caller", "helper"),
        ("alias_caller", "value"),
        ("root_alias_caller", "helper"),
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert_eq!(persisted.indexed_files, 3);
        assert_eq!(
            persisted.callees.len(),
            1,
            "{caller} should resolve its grouped import from the persisted index"
        );
        assert_eq!(persisted.callees[0].symbol_id, target);
    }
}

#[test]
fn traces_rust_grouped_function_import_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_path = dir.join("api.rs");
    let db_path = dir.join("symbols.db");
    fs::write(&root_path, "mod api;\nuse crate::api::stale;\n").unwrap();
    fs::write(&api_path, "pub fn helper() {}\n").unwrap();
    let root_overlay =
        "mod api;\nuse crate::api::{helper as selected};\nfn caller() { selected(); }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &root_path,
        root_overlay,
        "caller",
        TraceDirection::Callees,
    )
    .unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &root_path,
        root_overlay,
        "caller",
        TraceDirection::Callees,
    )
    .unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "helper");
}

#[test]
fn traces_rust_root_function_import_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_path = dir.join("api.rs");
    let db_path = dir.join("symbols.db");
    fs::write(&root_path, "mod api;\nuse crate::api::stale;\n").unwrap();
    fs::write(&api_path, "pub fn helper() {}\n").unwrap();
    let root_overlay = "mod api;\nuse crate::api::helper;\nfn caller() { helper(); }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &root_path,
        root_overlay,
        "caller",
        TraceDirection::Callees,
    )
    .unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &root_path,
        root_overlay,
        "caller",
        TraceDirection::Callees,
    )
    .unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "helper");
}

#[test]
fn does_not_trace_ambiguous_or_wildcard_rust_function_import_calls() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_path = dir.join("api.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api;\nuse crate::api::helper;\nuse crate::api::helper as helper;\nuse crate::api::*;\nfn ambiguous_caller() { helper(); }\nfn wildcard_caller() { other(); }\n",
    )
    .unwrap();
    fs::write(&api_path, "pub fn helper() {}\npub fn other() {}\n").unwrap();

    for caller in ["ambiguous_caller", "wildcard_caller"] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty(), "{caller} must fail closed");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in ["ambiguous_caller", "wildcard_caller"] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            persisted.callees.is_empty(),
            "{caller} must fail closed from the persisted index"
        );
    }
}

#[test]
fn traces_rust_nested_out_of_line_module_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_directory = dir.join("api");
    let api_path = api_directory.join("mod.rs");
    let helper_path = api_directory.join("helper.rs");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&api_directory).unwrap();
    fs::write(
        &root_path,
        "mod api;\nfn caller() { api::helper::value(); }\nfn crate_caller() { crate::api::helper::value(); }\n",
    )
    .unwrap();
    fs::write(&api_path, "mod helper;\n").unwrap();
    fs::write(&helper_path, "pub fn value() {}\n").unwrap();

    for caller in ["caller", "crate_caller"] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert_eq!(live.indexed_files, 3);
        assert_eq!(
            live.callees.len(),
            1,
            "{caller} should resolve its nested module call"
        );
        assert_eq!(live.callees[0].symbol_id, "value");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in ["caller", "crate_caller"] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert_eq!(persisted.indexed_files, 3);
        assert_eq!(
            persisted.callees.len(),
            1,
            "{caller} should resolve its nested module call from the persisted index"
        );
        assert_eq!(persisted.callees[0].symbol_id, "value");
    }
}

#[test]
fn traces_rust_raw_identifier_out_of_line_module_calls() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let module_path = dir.join("await.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod r#await;\nfn caller() { r#await::helper(); }\n",
    )
    .unwrap();
    fs::write(&module_path, "pub fn helper() {}\n").unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "helper");
}

#[test]
fn traces_rust_direct_out_of_line_module_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_path = dir.join("api.rs");
    let db_path = dir.join("symbols.db");
    fs::write(&root_path, "mod stale;\n").unwrap();
    fs::write(&api_path, "pub fn helper() {}\n").unwrap();
    let root_overlay = "mod api;\nfn caller() { api::helper(); }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &root_path,
        root_overlay,
        "caller",
        TraceDirection::Callees,
    )
    .unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &root_path,
        root_overlay,
        "caller",
        TraceDirection::Callees,
    )
    .unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "helper");
}

#[test]
fn traces_rust_nested_out_of_line_module_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_directory = dir.join("api");
    let api_path = api_directory.join("mod.rs");
    let helper_path = api_directory.join("helper.rs");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&api_directory).unwrap();
    fs::write(
        &root_path,
        "mod api;\nfn caller() { api::helper::value(); }\n",
    )
    .unwrap();
    fs::write(&api_path, "mod stale;\n").unwrap();
    fs::write(&helper_path, "pub fn value() {}\n").unwrap();
    let api_overlay = "mod helper;\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &api_path,
        api_overlay,
        "caller",
        TraceDirection::Callees,
    )
    .unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "value");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &api_path,
        api_overlay,
        "caller",
        TraceDirection::Callees,
    )
    .unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "value");
}

#[test]
fn does_not_trace_path_semantic_rust_nested_module_calls() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_directory = dir.join("api");
    let api_path = api_directory.join("mod.rs");
    let custom_path = api_directory.join("custom.rs");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&api_directory).unwrap();
    fs::write(
        &root_path,
        "mod api;\nfn caller() { api::helper::value(); }\n",
    )
    .unwrap();
    fs::write(&api_path, "#[path = \"custom.rs\"]\nmod helper;\n").unwrap();
    fs::write(&custom_path, "pub fn value() {}\n").unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn does_not_trace_ambiguous_or_path_semantic_rust_direct_module_calls() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_file_path = dir.join("api.rs");
    let api_module_path = dir.join("api").join("mod.rs");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(api_module_path.parent().unwrap()).unwrap();
    fs::write(
        &root_path,
        "#[path = \"custom.rs\"]\nmod custom;\nmod api;\nfn custom_caller() { custom::helper(); }\nfn api_caller() { api::helper(); }\n",
    )
    .unwrap();
    fs::write(&api_file_path, "pub fn helper() {}\n").unwrap();
    fs::write(&api_module_path, "pub fn helper() {}\n").unwrap();

    for caller in ["custom_caller", "api_caller"] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty(), "{caller} must fail closed");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in ["custom_caller", "api_caller"] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            persisted.callees.is_empty(),
            "{caller} must fail closed from the persisted index"
        );
    }
}

#[test]
fn traces_rust_pub_use_reexport_function_import_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let bridge_path = dir.join("bridge.rs");
    let impl_path = dir.join("impl_mod.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod bridge;\nmod impl_mod;\nuse crate::bridge::function;\nfn caller() { function(); }\n",
    )
    .unwrap();
    fs::write(&bridge_path, "pub use crate::impl_mod::function;\n").unwrap();
    fs::write(&impl_path, "pub fn function() {}\n").unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "function");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "function");
}

#[test]
fn traces_rust_pub_use_reexport_module_qualified_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let bridge_path = dir.join("bridge.rs");
    let impl_path = dir.join("impl_mod.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod bridge;\nmod impl_mod;\nfn caller() { bridge::function(); }\n",
    )
    .unwrap();
    fs::write(&bridge_path, "pub use crate::impl_mod::function;\n").unwrap();
    fs::write(&impl_path, "pub fn function() {}\n").unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "function");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "function");
}

#[test]
fn traces_rust_nested_pub_use_reexport_chains_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let bridge_path = dir.join("bridge.rs");
    let impl_path = dir.join("impl_mod.rs");
    let deeper_path = dir.join("deeper.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod bridge;\nmod impl_mod;\nmod deeper;\nfn caller() { bridge::function(); }\n",
    )
    .unwrap();
    fs::write(&bridge_path, "pub use crate::impl_mod::function;\n").unwrap();
    fs::write(&impl_path, "pub use crate::deeper::function;\n").unwrap();
    fs::write(&deeper_path, "pub fn function() {}\n").unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 4);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "function");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 4);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "function");
}

#[test]
fn traces_rust_pub_use_reexport_alias_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let bridge_path = dir.join("bridge.rs");
    let impl_path = dir.join("impl_mod.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod bridge;\nmod impl_mod;\nfn caller() { bridge::renamed(); }\n",
    )
    .unwrap();
    fs::write(
        &bridge_path,
        "pub use crate::impl_mod::function as renamed;\n",
    )
    .unwrap();
    fs::write(&impl_path, "pub fn function() {}\n").unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "function");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "function");
}

#[test]
fn traces_rust_crate_root_pub_use_reexport_import_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_path = dir.join("api.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api;\npub use api::helper;\nuse crate::helper;\nfn caller() { helper(); }\n",
    )
    .unwrap();
    fs::write(&api_path, "pub fn helper() {}\n").unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "helper");
}

#[test]
fn traces_rust_module_binding_imports_keep_out_of_line_module_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_path = dir.join("api.rs");
    let helpers_path = dir.join("helpers.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api;\nmod helpers;\nuse crate::api;\nuse crate::helpers::{self};\nfn caller() { api::helper(); }\nfn self_caller() { helpers::helper(); }\n",
    )
    .unwrap();
    fs::write(&api_path, "pub fn helper() {}\n").unwrap();
    fs::write(&helpers_path, "pub fn helper() {}\n").unwrap();

    for caller in ["caller", "self_caller"] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert_eq!(live.indexed_files, 3);
        assert_eq!(
            live.callees.len(),
            1,
            "{caller} should keep its qualified out-of-line call"
        );
        assert_eq!(live.callees[0].symbol_id, "helper");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in ["caller", "self_caller"] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert_eq!(persisted.indexed_files, 3);
        assert_eq!(
            persisted.callees.len(),
            1,
            "{caller} should keep its qualified out-of-line call from the persisted index"
        );
        assert_eq!(persisted.callees[0].symbol_id, "helper");
    }
}

#[test]
fn keeps_rust_private_use_reexports_fail_closed_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let bridge_path = dir.join("bridge.rs");
    let impl_path = dir.join("impl_mod.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod bridge;\nmod impl_mod;\nuse crate::bridge::function;\nfn caller() { function(); }\n",
    )
    .unwrap();
    fs::write(&bridge_path, "use crate::impl_mod::function;\n").unwrap();
    fs::write(&impl_path, "pub fn function() {}\n").unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty(), "private use must not re-export");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert!(
        persisted.callees.is_empty(),
        "private use must not re-export from the persisted index"
    );
}

#[test]
fn keeps_rust_ambiguous_pub_use_reexports_fail_closed_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let bridge_path = dir.join("bridge.rs");
    let impl_path = dir.join("impl_mod.rs");
    let other_path = dir.join("other_mod.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod bridge;\nmod impl_mod;\nmod other_mod;\nuse crate::bridge::function;\nfn caller() { function(); }\n",
    )
    .unwrap();
    fs::write(
        &bridge_path,
        "pub use crate::impl_mod::function;\npub use crate::other_mod::function;\n",
    )
    .unwrap();
    fs::write(&impl_path, "pub fn function() {}\n").unwrap();
    fs::write(&other_path, "pub fn function() {}\n").unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "ambiguous re-export must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert!(
        persisted.callees.is_empty(),
        "ambiguous re-export must fail closed from the persisted index"
    );
}

#[test]
fn traces_rust_crate_root_qualified_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_path = dir.join("api.rs");
    let util_path = dir.join("util.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api;\nmod util;\nfn root_helper() {}\npub use util::helper;\nfn root_caller() { crate::root_helper(); }\n",
    )
    .unwrap();
    fs::write(
        &api_path,
        "fn caller() { crate::root_helper(); crate::helper(); }\n",
    )
    .unwrap();
    fs::write(&util_path, "pub fn helper() {}\n").unwrap();

    let root_live = trace_symbol_graph(&dir, "root_caller", TraceDirection::Callees).unwrap();
    assert_eq!(root_live.callees.len(), 1);
    assert_eq!(root_live.callees[0].symbol_id, "root_helper");

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callees.len(), 2);
    assert!(live.callees.iter().any(|c| c.symbol_id == "root_helper"));
    assert!(live.callees.iter().any(|c| c.symbol_id == "helper"));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let root_persisted =
        trace_symbol_graph_from_index(&db_path, "root_caller", TraceDirection::Callees).unwrap();
    assert_eq!(root_persisted.callees.len(), 1);
    assert_eq!(root_persisted.callees[0].symbol_id, "root_helper");
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callees.len(), 2);
    assert!(
        persisted
            .callees
            .iter()
            .any(|c| c.symbol_id == "root_helper")
    );
    assert!(persisted.callees.iter().any(|c| c.symbol_id == "helper"));
}

#[test]
fn keeps_rust_crate_root_qualified_calls_fail_closed_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_path = dir.join("api.rs");
    let db_path = dir.join("symbols.db");
    fs::write(&root_path, "mod api;\nfn root_helper() {}\n").unwrap();
    fs::write(&api_path, "fn caller() { crate::missing(); }\n").unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert!(
        live.callees.is_empty(),
        "missing crate-root call must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert!(
        persisted.callees.is_empty(),
        "missing crate-root call must fail closed from the persisted index"
    );
}
