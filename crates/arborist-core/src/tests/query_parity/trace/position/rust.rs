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

#[test]
fn traces_rust_crate_root_inline_module_function_import_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let sibling_path = dir.join("sibling.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub fn helper() {}\n}\nmod sibling;\nuse crate::api::helper;\nfn root_caller() { helper(); }\n",
    )
    .unwrap();
    fs::write(
        &sibling_path,
        "use crate::api::helper;\nfn caller() { helper(); }\n",
    )
    .unwrap();

    for caller in ["root_caller", "caller"] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert_eq!(
            live.callees.len(),
            1,
            "{caller} should resolve its inline-module import"
        );
        assert_eq!(live.callees[0].symbol_id, "api::helper");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in ["root_caller", "caller"] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert_eq!(
            persisted.callees.len(),
            1,
            "{caller} should resolve its inline-module import from the persisted index"
        );
        assert_eq!(persisted.callees[0].symbol_id, "api::helper");
    }
}

#[test]
fn traces_rust_crate_root_inline_module_qualified_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let sibling_path = dir.join("sibling.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub fn helper() {}\n}\nmod sibling;\n",
    )
    .unwrap();
    fs::write(&sibling_path, "fn caller() { crate::api::helper(); }\n").unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "api::helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "api::helper");
}

#[test]
fn keeps_rust_crate_root_inline_module_imports_fail_closed_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let sibling_path = dir.join("sibling.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub fn helper() {}\n}\nmod sibling;\n",
    )
    .unwrap();
    fs::write(
        &sibling_path,
        "use crate::api::missing;\nuse crate::other::helper;\nfn missing_caller() { missing(); }\nfn other_caller() { helper(); }\n",
    )
    .unwrap();

    for caller in ["missing_caller", "other_caller"] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(
            live.callees.is_empty(),
            "{caller} must fail closed for a missing inline-module import"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in ["missing_caller", "other_caller"] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            persisted.callees.is_empty(),
            "{caller} must fail closed for a missing inline-module import from the persisted index"
        );
    }
}

#[test]
fn traces_rust_static_method_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Counter {}\nimpl Counter {\n    fn new() -> Counter { Counter {} }\n}\nfn caller() { let _ = Counter::new(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "Counter::new");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "Counter::new");
}

#[test]
fn traces_rust_static_method_calls_from_out_of_line_children_in_live_workspace_and_persisted_index()
{
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let caller_path = dir.join("caller_mod.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod caller_mod;\nstruct Counter {}\nimpl Counter {\n    pub fn new() -> Counter { Counter {} }\n}\n",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "use crate::Counter;\nfn caller() { let _ = Counter::new(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "Counter::new");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "Counter::new");
}

#[test]
fn keeps_rust_static_method_calls_fail_closed_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Counter {}\nimpl Counter {\n    fn new() -> Counter { Counter {} }\n}\nfn missing_caller() { let _ = Counter::missing(); }\nfn unknown_caller() { let _ = Unknown::new(); }\n",
    )
    .unwrap();

    for caller in ["missing_caller", "unknown_caller"] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(
            live.callees.is_empty(),
            "{caller} must fail closed for a missing static method"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in ["missing_caller", "unknown_caller"] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            persisted.callees.is_empty(),
            "{caller} must fail closed for a missing static method from the persisted index"
        );
    }
}

#[test]
fn traces_rust_module_binding_inline_module_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let sibling_path = dir.join("sibling.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub fn helper() {}\n}\nmod sibling;\n",
    )
    .unwrap();
    fs::write(
        &sibling_path,
        "use crate::api;\nfn caller() { api::helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "api::helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "api::helper");
}

#[test]
fn traces_rust_module_binding_inline_module_calls_from_crate_root_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub fn helper() {}\n}\nuse crate::api;\nfn caller() { api::helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "api::helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "api::helper");
}

#[test]
fn traces_rust_module_binding_inline_module_alias_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let sibling_path = dir.join("sibling.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub fn helper() {}\n}\nmod sibling;\n",
    )
    .unwrap();
    fs::write(
        &sibling_path,
        "use crate::api as mod_alias;\nfn caller() { mod_alias::helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "api::helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "api::helper");
}

#[test]
fn keeps_rust_module_binding_inline_module_calls_fail_closed_in_live_workspace_and_persisted_index()
{
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let sibling_path = dir.join("sibling.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub fn helper() {}\n}\nmod sibling;\n",
    )
    .unwrap();
    fs::write(
        &sibling_path,
        "use crate::api;\nuse crate::missing;\nfn missing_fn_caller() { api::missing_fn(); }\nfn missing_module_caller() { missing::helper(); }\n",
    )
    .unwrap();

    for caller in ["missing_fn_caller", "missing_module_caller"] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(
            live.callees.is_empty(),
            "{caller} must fail closed for a missing module-binding inline-module call"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in ["missing_fn_caller", "missing_module_caller"] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            persisted.callees.is_empty(),
            "{caller} must fail closed for a missing module-binding inline-module call from the persisted index"
        );
    }
}

#[test]
fn traces_rust_inline_module_in_out_of_line_module_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_path = dir.join("api.rs");
    let sibling_path = dir.join("sibling.rs");
    let db_path = dir.join("symbols.db");
    fs::write(&root_path, "mod api;\nmod sibling;\n").unwrap();
    fs::write(&api_path, "mod inner {\n    pub fn helper() {}\n}\n").unwrap();
    fs::write(
        &sibling_path,
        "fn caller() { crate::api::inner::helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "inner::helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "inner::helper");
}

#[test]
fn traces_rust_inline_module_in_out_of_line_module_imports_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_directory = dir.join("api");
    let api_path = api_directory.join("mod.rs");
    let sibling_path = dir.join("sibling.rs");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&api_directory).unwrap();
    fs::write(&root_path, "mod api;\nmod sibling;\n").unwrap();
    fs::write(&api_path, "mod inner {\n    pub fn helper() {}\n}\n").unwrap();
    fs::write(
        &sibling_path,
        "use crate::api::inner::helper;\nfn caller() { helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "inner::helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "inner::helper");
}

#[test]
fn traces_rust_module_binding_inline_module_in_out_of_line_module_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_path = dir.join("api.rs");
    let sibling_path = dir.join("sibling.rs");
    let db_path = dir.join("symbols.db");
    fs::write(&root_path, "mod api;\nmod sibling;\n").unwrap();
    fs::write(&api_path, "mod inner {\n    pub fn helper() {}\n}\n").unwrap();
    fs::write(
        &sibling_path,
        "use crate::api;\nfn caller() { api::inner::helper(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "inner::helper");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "inner::helper");
}

#[test]
fn keeps_rust_inline_module_in_out_of_line_module_calls_fail_closed_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let api_path = dir.join("api.rs");
    let sibling_path = dir.join("sibling.rs");
    let db_path = dir.join("symbols.db");
    fs::write(&root_path, "mod api;\nmod sibling;\n").unwrap();
    fs::write(&api_path, "mod inner {\n    pub fn helper() {}\n}\n").unwrap();
    fs::write(
        &sibling_path,
        "fn missing_fn_caller() { crate::api::inner::missing_fn(); }\nfn missing_module_caller() { crate::api::missing::helper(); }\n",
    )
    .unwrap();

    for caller in ["missing_fn_caller", "missing_module_caller"] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(
            live.callees.is_empty(),
            "{caller} must fail closed for a missing inline-module target"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in ["missing_fn_caller", "missing_module_caller"] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            persisted.callees.is_empty(),
            "{caller} must fail closed for a missing inline-module target from the persisted index"
        );
    }
}

#[test]
fn traces_rust_local_struct_literal_method_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Counter {}\nimpl Counter {\n    fn increment(&self) {}\n}\nfn caller() {\n    let c = Counter {};\n    c.increment();\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "Counter::increment");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "Counter::increment");
}

#[test]
fn traces_rust_direct_struct_literal_method_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Counter {}\nimpl Counter {\n    fn increment(&self) {}\n}\nfn caller() { Counter {}.increment(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "Counter::increment");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "Counter::increment");
}

#[test]
fn keeps_rust_struct_literal_method_calls_fail_closed_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Counter {}\nimpl Counter {\n    fn increment(&self) {}\n}\nfn missing_method_caller() { let c = Counter {}; c.missing(); }\nfn non_struct_binding_caller() { let n = 5; n.increment(); }\nfn shadowed_binding_caller() {\n    let c = Counter {};\n    let c = Other {};\n    c.increment();\n}\nstruct Other {}\nimpl Other { fn increment(&self) {} }\n",
    )
    .unwrap();

    for caller in [
        "missing_method_caller",
        "non_struct_binding_caller",
        "shadowed_binding_caller",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(
            live.callees.is_empty(),
            "{caller} must fail closed for an unknown method receiver"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in [
        "missing_method_caller",
        "non_struct_binding_caller",
        "shadowed_binding_caller",
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            persisted.callees.is_empty(),
            "{caller} must fail closed for an unknown method receiver from the persisted index"
        );
    }
}

#[test]
fn traces_rust_typed_parameter_method_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Counter {}\nimpl Counter {\n    fn increment(&self) {}\n}\nfn caller(c: &Counter, d: Counter, e: &mut Counter) {\n    c.increment();\n    d.increment();\n    e.increment();\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "Counter::increment");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "Counter::increment");
}

#[test]
fn keeps_rust_typed_parameter_method_calls_fail_closed_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Counter {}\nimpl Counter {\n    fn increment(&self) {}\n}\nfn primitive_parameter_caller(value: i32) { value.increment(); }\nfn generic_parameter_caller<T>(value: T) { value.increment(); }\nfn unknown_type_parameter_caller(value: Unknown) { value.increment(); }\nfn shadowed_parameter_caller(value: Counter) {\n    let value = Other {};\n    value.increment();\n}\nfn duplicated_parameter_caller(value: Counter, value: Other) { value.increment(); }\nstruct Other {}\nimpl Other { fn increment(&self) {} }\n",
    )
    .unwrap();

    for caller in [
        "primitive_parameter_caller",
        "generic_parameter_caller",
        "unknown_type_parameter_caller",
        "shadowed_parameter_caller",
        "duplicated_parameter_caller",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(
            live.callees.is_empty(),
            "{caller} must fail closed for an unresolved typed parameter receiver"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in [
        "primitive_parameter_caller",
        "generic_parameter_caller",
        "unknown_type_parameter_caller",
        "shadowed_parameter_caller",
        "duplicated_parameter_caller",
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            persisted.callees.is_empty(),
            "{caller} must fail closed for an unresolved typed parameter receiver from the persisted index"
        );
    }
}

#[test]
fn traces_rust_constructor_call_binding_method_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Counter {}\nimpl Counter {\n    fn new() -> Counter { Counter {} }\n    fn increment(&self) {}\n}\nfn caller() {\n    let c = Counter::new();\n    c.increment();\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 2);
    assert!(
        live.callees
            .iter()
            .any(|callee| callee.symbol_id == "Counter::new"),
        "the constructor static call should trace to Counter::new"
    );
    assert!(
        live.callees
            .iter()
            .any(|callee| callee.symbol_id == "Counter::increment"),
        "the instance call should trace to Counter::increment"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callees.len(), 2);
    assert!(
        persisted
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "Counter::new")
    );
    assert!(
        persisted
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "Counter::increment")
    );
}

#[test]
fn traces_rust_constructor_call_binding_method_calls_in_inline_modules_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub struct Counter {}\n    impl Counter {\n        pub fn new() -> Counter { Counter {} }\n        pub fn increment(&self) {}\n    }\n    pub fn caller() {\n        let c = Counter::new();\n        c.increment();\n    }\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "api::caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    let mut actual = live
        .callees
        .iter()
        .map(|callee| callee.symbol_id.as_str())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(actual, ["api::Counter::increment", "api::Counter::new"]);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "api::caller", TraceDirection::Callees).unwrap();
    let mut actual = persisted
        .callees
        .iter()
        .map(|callee| callee.symbol_id.as_str())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(actual, ["api::Counter::increment", "api::Counter::new"]);
}

#[test]
fn keeps_rust_constructor_call_binding_method_calls_fail_closed_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub struct Counter {}\n    impl Counter {\n        pub fn new() -> Counter { Counter {} }\n        pub fn increment(&self) {}\n    }\n}\nstruct RootCounter {}\nimpl RootCounter {\n    fn new() -> RootCounter { RootCounter {} }\n    fn increment(&self) {}\n}\nfn path_constructor_caller() {\n    let c = crate::api::Counter::new();\n    c.increment();\n}\nfn turbofish_constructor_caller() {\n    let c = RootCounter::<u8>::new();\n    c.increment();\n}\nfn unknown_type_constructor_caller() {\n    let c = Unknown::new();\n    c.increment();\n}\nfn shadowed_constructor_caller() {\n    let c = RootCounter::new();\n    let c = Other {};\n    c.increment();\n}\nfn missing_method_constructor_caller() {\n    let c = RootCounter::new();\n    c.missing();\n}\nstruct Other {}\nimpl Other { fn increment(&self) {} }\n",
    )
    .unwrap();

    let forbidden_targets = [
        ("path_constructor_caller", "api::Counter::increment"),
        ("turbofish_constructor_caller", "RootCounter::increment"),
        ("unknown_type_constructor_caller", "Unknown::increment"),
        ("shadowed_constructor_caller", "RootCounter::increment"),
        ("shadowed_constructor_caller", "Other::increment"),
        ("missing_method_constructor_caller", "RootCounter::missing"),
    ];
    for (caller, forbidden) in forbidden_targets {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(
            !live
                .callees
                .iter()
                .any(|callee| callee.symbol_id == forbidden),
            "{caller} must not trace {forbidden} for an unresolved constructor-call binding"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, forbidden) in forbidden_targets {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            !persisted
                .callees
                .iter()
                .any(|callee| callee.symbol_id == forbidden),
            "{caller} must not trace {forbidden} for an unresolved constructor-call binding from the persisted index"
        );
    }
}

#[test]
fn keeps_rust_out_of_line_module_constructor_calls_fail_closed_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let module_path = dir.join("Counter.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod Counter;\nfn caller() {\n    let c = Counter::new();\n    c.increment();\n}\n",
    )
    .unwrap();
    fs::write(
        &module_path,
        "pub fn new() -> i32 { 0 }\npub fn increment() {}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert!(
        live.callees.iter().any(|callee| callee.symbol_id == "new"),
        "the module-qualified static call should still resolve"
    );
    assert!(
        !live
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "increment"),
        "an out-of-line module name must not be treated as a constructor binding type"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert!(
        persisted
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "new")
    );
    assert!(
        !persisted
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "increment")
    );
}

#[test]
fn traces_rust_self_method_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Counter {\n    value: u32,\n}\nimpl Counter {\n    fn new() -> Counter { Counter { value: 0 } }\n    fn increment(&mut self) {}\n    fn twice(&self) -> u32 {\n        self.increment();\n        self.increment();\n        self.value\n    }\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::twice", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "Counter::increment");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::twice", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "Counter::increment");
}

#[test]
fn traces_rust_self_method_calls_in_inline_modules_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub struct Counter {\n        pub value: u32,\n    }\n    impl Counter {\n        pub fn new() -> Counter { Counter { value: 0 } }\n        pub fn increment(&mut self) {}\n        pub fn twice(&self) -> u32 {\n            self.increment();\n            self.value\n        }\n    }\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "api::Counter::twice", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "api::Counter::increment");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "api::Counter::twice", TraceDirection::Callees)
            .unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "api::Counter::increment");
}

#[test]
fn traces_rust_member_chain_method_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Inner {}\nimpl Inner {\n    fn increment(&self) {}\n}\nstruct Outer {\n    inner: Inner,\n}\nfn caller(outer: Outer) {\n    outer.inner.increment();\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "Inner::increment");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "Inner::increment");
}

#[test]
fn traces_rust_member_chain_method_calls_from_struct_literal_bindings_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Inner {}\nimpl Inner {\n    fn increment(&self) {}\n}\nstruct Outer {\n    inner: Inner,\n}\nfn caller() {\n    let outer = Outer { inner: Inner {} };\n    outer.inner.increment();\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "Inner::increment");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "Inner::increment");
}

#[test]
fn traces_rust_member_chain_method_calls_in_inline_modules_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub struct Inner {}\n    impl Inner {\n        pub fn increment(&self) {}\n    }\n    pub struct Outer {\n        pub inner: Inner,\n    }\n    pub fn caller(outer: Outer) {\n        outer.inner.increment();\n    }\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "api::caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "api::Inner::increment");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "api::caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "api::Inner::increment");
}

#[test]
fn traces_rust_multi_hop_and_self_field_member_chain_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Leaf {}\nimpl Leaf {\n    fn run(&self) {}\n}\nstruct Middle {\n    leaf: Leaf,\n}\nstruct Root {\n    middle: Middle,\n}\nfn caller(root: Root) {\n    root.middle.leaf.run();\n}\nimpl Root {\n    fn go(&self) {\n        self.middle.leaf.run();\n    }\n}\n",
    )
    .unwrap();

    for caller_path in ["caller", "Root::go"] {
        let live = trace_symbol_graph(&dir, caller_path, TraceDirection::Callees).unwrap();
        assert_eq!(live.indexed_files, 1);
        assert_eq!(live.callees.len(), 1);
        assert_eq!(live.callees[0].symbol_id, "Leaf::run");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller_path in ["caller", "Root::go"] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller_path, TraceDirection::Callees).unwrap();
        assert_eq!(persisted.indexed_files, 1);
        assert_eq!(persisted.callees.len(), 1);
        assert_eq!(persisted.callees[0].symbol_id, "Leaf::run");
    }
}

#[test]
fn keeps_rust_member_chain_method_calls_fail_closed_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Inner {}\nimpl Inner {\n    fn increment(&self) {}\n}\nstruct Outer {\n    inner: Inner,\n    items: Vec<Inner>,\n}\nfn unknown_field_caller(outer: Outer) {\n    outer.missing.increment();\n}\nfn generic_field_caller(outer: Outer) {\n    outer.items.increment();\n}\nfn unknown_base_caller() {\n    unknown.inner.increment();\n}\nfn missing_method_caller(outer: Outer) {\n    outer.inner.missing();\n}\nfn primitive_receiver_caller() {\n    let n = 5;\n    n.to_string();\n}\n",
    )
    .unwrap();

    let forbidden_targets = [
        ("unknown_field_caller", "Inner::increment"),
        ("generic_field_caller", "Inner::increment"),
        ("unknown_base_caller", "Inner::increment"),
        ("missing_method_caller", "Inner::missing"),
        ("primitive_receiver_caller", "i32::to_string"),
    ];
    for (caller, forbidden) in forbidden_targets {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(
            !live
                .callees
                .iter()
                .any(|callee| callee.symbol_id == forbidden),
            "{caller} must not trace {forbidden} for an unresolved member chain"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, forbidden) in forbidden_targets {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            !persisted
                .callees
                .iter()
                .any(|callee| callee.symbol_id == forbidden),
            "{caller} must not trace {forbidden} for an unresolved member chain from the persisted index"
        );
    }
}

#[test]
fn traces_rust_member_chain_method_call_hops_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Inner {}\nimpl Inner {\n    fn increment(&self) {}\n}\nstruct Outer {}\nimpl Outer {\n    fn get_inner(&self) -> Inner { Inner {} }\n}\nfn caller(outer: Outer) {\n    outer.get_inner().increment();\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 2);
    assert!(
        live.callees
            .iter()
            .any(|callee| callee.symbol_id == "Outer::get_inner")
    );
    assert!(
        live.callees
            .iter()
            .any(|callee| callee.symbol_id == "Inner::increment")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 2);
    assert!(
        persisted
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "Outer::get_inner")
    );
    assert!(
        persisted
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "Inner::increment")
    );
}

#[test]
fn traces_rust_member_chain_method_call_hops_from_self_and_struct_literal_bindings_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Inner {}\nimpl Inner {\n    fn run(&self) {}\n}\nstruct Root {}\nimpl Root {\n    fn get_inner(&self) -> Inner { Inner {} }\n    fn go(&self) {\n        self.get_inner().run();\n    }\n}\nfn caller() {\n    let root = Root {};\n    root.get_inner().run();\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Root::go", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 2);
    assert!(
        live.callees
            .iter()
            .any(|callee| callee.symbol_id == "Root::get_inner")
    );
    assert!(
        live.callees
            .iter()
            .any(|callee| callee.symbol_id == "Inner::run")
    );
    let live_caller = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live_caller.callees.len(), 2);
    assert!(
        live_caller
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "Root::get_inner")
    );
    assert!(
        live_caller
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "Inner::run")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Root::go", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 2);
    assert!(
        persisted
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "Root::get_inner")
    );
    assert!(
        persisted
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "Inner::run")
    );
    let persisted_caller =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted_caller.callees.len(), 2);
    assert!(
        persisted_caller
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "Root::get_inner")
    );
    assert!(
        persisted_caller
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "Inner::run")
    );
}

#[test]
fn traces_rust_multi_hop_member_chain_method_calls_and_bare_function_hops_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Leaf {}\nimpl Leaf {\n    fn run(&self) {}\n}\nstruct Middle {}\nimpl Middle {\n    fn leaf(&self) -> Leaf { Leaf {} }\n}\nstruct Root {}\nimpl Root {\n    fn middle(&self) -> Middle { Middle {} }\n}\nfn make_root() -> Root { Root {} }\nfn caller() {\n    make_root().middle().leaf().run();\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 4);
    for expected in ["make_root", "Root::middle", "Middle::leaf", "Leaf::run"] {
        assert!(
            live.callees
                .iter()
                .any(|callee| callee.symbol_id == expected),
            "{expected} must be traced"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 4);
    for expected in ["make_root", "Root::middle", "Middle::leaf", "Leaf::run"] {
        assert!(
            persisted
                .callees
                .iter()
                .any(|callee| callee.symbol_id == expected),
            "{expected} must be traced from the persisted index"
        );
    }
}

#[test]
fn traces_rust_member_chain_method_call_hops_in_inline_modules_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub struct Inner {}\n    impl Inner {\n        pub fn increment(&self) {}\n    }\n    pub struct Outer {}\n    impl Outer {\n        pub fn get_inner(&self) -> Inner { Inner {} }\n    }\n    pub fn caller(outer: Outer) {\n        outer.get_inner().increment();\n    }\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "api::caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 2);
    assert!(
        live.callees
            .iter()
            .any(|callee| callee.symbol_id == "api::Outer::get_inner")
    );
    assert!(
        live.callees
            .iter()
            .any(|callee| callee.symbol_id == "api::Inner::increment")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "api::caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 2);
    assert!(
        persisted
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "api::Outer::get_inner")
    );
    assert!(
        persisted
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "api::Inner::increment")
    );
}

#[test]
fn keeps_rust_member_chain_method_call_hops_fail_closed_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Inner {}\nimpl Inner {\n    fn increment(&self) {}\n}\nstruct Outer {}\nimpl Outer {\n    fn get_inner(&self) -> Inner { Inner {} }\n    fn get_inners(&self) -> Vec<Inner> { Vec::new() }\n    fn get_inner_with(&self, value: u8) -> Inner { Inner {} }\n    fn no_return(&self) {}\n}\nfn arg_hop_caller(outer: Outer) {\n    outer.get_inner_with(1).increment();\n}\nfn generic_hop_caller(outer: Outer) {\n    outer.get_inners().increment();\n}\nfn unknown_hop_caller(outer: Outer) {\n    outer.missing().increment();\n}\nfn unknown_base_hop_caller() {\n    unknown.get_inner().increment();\n}\nfn no_return_hop_caller(outer: Outer) {\n    outer.no_return().increment();\n}\nfn primitive_hop_caller() {\n    let n = 5;\n    n.to_string().len();\n}\n",
    )
    .unwrap();

    let forbidden_targets = [
        ("arg_hop_caller", "Inner::increment"),
        ("generic_hop_caller", "Inner::increment"),
        ("unknown_hop_caller", "Inner::increment"),
        ("unknown_base_hop_caller", "Inner::increment"),
        ("no_return_hop_caller", "Inner::increment"),
        ("primitive_hop_caller", "usize::len"),
    ];
    for (caller, forbidden) in forbidden_targets {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(
            !live
                .callees
                .iter()
                .any(|callee| callee.symbol_id == forbidden),
            "{caller} must not trace {forbidden} for an unresolved call hop"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, forbidden) in forbidden_targets {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            !persisted
                .callees
                .iter()
                .any(|callee| callee.symbol_id == forbidden),
            "{caller} must not trace {forbidden} for an unresolved call hop from the persisted index"
        );
    }
}

#[test]
fn traces_rust_method_call_hop_let_bindings_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Inner {}\nimpl Inner {\n    fn increment(&self) {}\n}\nstruct Outer {}\nimpl Outer {\n    fn get_inner(&self) -> Inner { Inner {} }\n}\nfn caller(outer: Outer) {\n    let inner = outer.get_inner();\n    inner.increment();\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 2);
    assert!(
        live.callees
            .iter()
            .any(|callee| callee.symbol_id == "Outer::get_inner")
    );
    assert!(
        live.callees
            .iter()
            .any(|callee| callee.symbol_id == "Inner::increment")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 2);
    assert!(
        persisted
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "Outer::get_inner")
    );
    assert!(
        persisted
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "Inner::increment")
    );
}

#[test]
fn traces_rust_bare_function_call_hop_let_bindings_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Leaf {}\nimpl Leaf {\n    fn run(&self) {}\n}\nstruct Middle {}\nimpl Middle {\n    fn leaf(&self) -> Leaf { Leaf {} }\n}\nstruct Root {}\nimpl Root {\n    fn middle(&self) -> Middle { Middle {} }\n}\nfn make_root() -> Root { Root {} }\nfn caller() {\n    let root = make_root();\n    let middle = root.middle();\n    let leaf = middle.leaf();\n    leaf.run();\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 4);
    for expected in ["make_root", "Root::middle", "Middle::leaf", "Leaf::run"] {
        assert!(
            live.callees
                .iter()
                .any(|callee| callee.symbol_id == expected),
            "{expected} must be traced"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 4);
    for expected in ["make_root", "Root::middle", "Middle::leaf", "Leaf::run"] {
        assert!(
            persisted
                .callees
                .iter()
                .any(|callee| callee.symbol_id == expected),
            "{expected} must be traced from the persisted index"
        );
    }
}

#[test]
fn traces_rust_self_and_inline_module_call_hop_let_bindings_in_live_workspace_and_persisted_index()
{
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub struct Inner {}\n    impl Inner {\n        pub fn increment(&self) {}\n    }\n    pub struct Outer {}\n    impl Outer {\n        pub fn get_inner(&self) -> Inner { Inner {} }\n    }\n    pub fn caller(outer: Outer) {\n        let inner = outer.get_inner();\n        inner.increment();\n    }\n}\nstruct Root {}\nimpl Root {\n    fn get_inner(&self) -> Inner { Inner {} }\n    fn go(&self) {\n        let inner = self.get_inner();\n        inner.increment();\n    }\n}\nstruct Inner {}\nimpl Inner {\n    fn increment(&self) {}\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "api::caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 2);
    assert!(
        live.callees
            .iter()
            .any(|callee| callee.symbol_id == "api::Outer::get_inner")
    );
    assert!(
        live.callees
            .iter()
            .any(|callee| callee.symbol_id == "api::Inner::increment")
    );
    let live_self = trace_symbol_graph(&dir, "Root::go", TraceDirection::Callees).unwrap();
    assert_eq!(live_self.callees.len(), 2);
    assert!(
        live_self
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "Root::get_inner")
    );
    assert!(
        live_self
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "Inner::increment")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "api::caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 2);
    assert!(
        persisted
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "api::Outer::get_inner")
    );
    assert!(
        persisted
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "api::Inner::increment")
    );
    let persisted_self =
        trace_symbol_graph_from_index(&db_path, "Root::go", TraceDirection::Callees).unwrap();
    assert_eq!(persisted_self.callees.len(), 2);
    assert!(
        persisted_self
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "Root::get_inner")
    );
    assert!(
        persisted_self
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "Inner::increment")
    );
}

#[test]
fn keeps_rust_call_hop_let_bindings_fail_closed_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Inner {}\nimpl Inner {\n    fn increment(&self) {}\n}\nstruct Outer {}\nimpl Outer {\n    fn get_inner(&self) -> Inner { Inner {} }\n    fn get_inners(&self) -> Vec<Inner> { Vec::new() }\n    fn get_inner_with(&self, value: u8) -> Inner { Inner {} }\n}\nfn arg_hop_caller(outer: Outer) {\n    let inner = outer.get_inner_with(1);\n    inner.increment();\n}\nfn generic_hop_caller(outer: Outer) {\n    let inners = outer.get_inners();\n    inners.increment();\n}\nfn unknown_hop_caller(outer: Outer) {\n    let inner = outer.missing();\n    inner.increment();\n}\nfn unknown_base_hop_caller() {\n    let inner = unknown.get_inner();\n    inner.increment();\n}\nfn make_root() -> Root { Root {} }\nfn shadowed_function_caller() {\n    let make_root = || Root {};\n    let root = make_root();\n    root.middle().leaf().run();\n}\nstruct Root {}\nimpl Root { fn middle(&self) -> Middle { Middle {} } }\nstruct Middle {}\nimpl Middle { fn leaf(&self) -> Leaf { Leaf {} } }\nstruct Leaf {}\nimpl Leaf { fn run(&self) {} }\n",
    )
    .unwrap();

    let forbidden_targets = [
        ("arg_hop_caller", "Inner::increment"),
        ("generic_hop_caller", "Inner::increment"),
        ("unknown_hop_caller", "Inner::increment"),
        ("unknown_base_hop_caller", "Inner::increment"),
        ("shadowed_function_caller", "Leaf::run"),
    ];
    for (caller, forbidden) in forbidden_targets {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(
            !live
                .callees
                .iter()
                .any(|callee| callee.symbol_id == forbidden),
            "{caller} must not trace {forbidden} for an unresolved call-hop let binding"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, forbidden) in forbidden_targets {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            !persisted
                .callees
                .iter()
                .any(|callee| callee.symbol_id == forbidden),
            "{caller} must not trace {forbidden} for an unresolved call-hop let binding from the persisted index"
        );
    }
}

#[test]
fn traces_rust_field_access_let_bindings_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Inner {}\nimpl Inner {\n    fn increment(&self) {}\n}\nstruct Outer {\n    inner: Inner,\n}\nfn caller(outer: Outer) {\n    let inner = outer.inner;\n    inner.increment();\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "Inner::increment");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "Inner::increment");
}

#[test]
fn traces_rust_multi_hop_field_access_let_bindings_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Leaf {}\nimpl Leaf {\n    fn run(&self) {}\n}\nstruct Middle {\n    leaf: Leaf,\n}\nstruct Root {\n    middle: Middle,\n}\nfn caller(root: Root) {\n    let leaf = root.middle.leaf;\n    leaf.run();\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "Leaf::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "Leaf::run");
}

#[test]
fn traces_rust_struct_literal_and_self_field_access_let_bindings_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Inner {}\nimpl Inner {\n    fn increment(&self) {}\n}\nstruct Outer {\n    inner: Inner,\n}\nfn caller() {\n    let inner = Outer { inner: Inner {} }.inner;\n    inner.increment();\n}\nimpl Outer {\n    fn go(&self) {\n        let inner = self.inner;\n        inner.increment();\n    }\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "Inner::increment");
    let live_self = trace_symbol_graph(&dir, "Outer::go", TraceDirection::Callees).unwrap();
    assert_eq!(live_self.callees.len(), 1);
    assert_eq!(live_self.callees[0].symbol_id, "Inner::increment");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "Inner::increment");
    let persisted_self =
        trace_symbol_graph_from_index(&db_path, "Outer::go", TraceDirection::Callees).unwrap();
    assert_eq!(persisted_self.callees.len(), 1);
    assert_eq!(persisted_self.callees[0].symbol_id, "Inner::increment");
}

#[test]
fn traces_rust_field_access_let_bindings_in_inline_modules_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub struct Inner {}\n    impl Inner {\n        pub fn increment(&self) {}\n    }\n    pub struct Outer {\n        pub inner: Inner,\n    }\n    pub fn caller(outer: Outer) {\n        let inner = outer.inner;\n        inner.increment();\n    }\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "api::caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "api::Inner::increment");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "api::caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "api::Inner::increment");
}

#[test]
fn keeps_rust_field_access_let_bindings_fail_closed_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Inner {}\nimpl Inner {\n    fn increment(&self) {}\n}\nstruct Outer {\n    inner: Inner,\n    items: Vec<Inner>,\n}\nfn unknown_field_caller(outer: Outer) {\n    let inner = outer.missing;\n    inner.increment();\n}\nfn generic_field_caller(outer: Outer) {\n    let items = outer.items;\n    items.increment();\n}\nfn unknown_base_caller() {\n    let inner = unknown.inner;\n    inner.increment();\n}\n",
    )
    .unwrap();

    let forbidden_targets = [
        ("unknown_field_caller", "Inner::increment"),
        ("generic_field_caller", "Inner::increment"),
        ("unknown_base_caller", "Inner::increment"),
    ];
    for (caller, forbidden) in forbidden_targets {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(
            !live
                .callees
                .iter()
                .any(|callee| callee.symbol_id == forbidden),
            "{caller} must not trace {forbidden} for an unresolved field-access let binding"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, forbidden) in forbidden_targets {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            !persisted
                .callees
                .iter()
                .any(|callee| callee.symbol_id == forbidden),
            "{caller} must not trace {forbidden} for an unresolved field-access let binding from the persisted index"
        );
    }
}

#[test]
fn traces_rust_self_prefixed_static_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Counter {}\nimpl Counter {\n    fn new() -> Counter { Counter {} }\n    fn create() -> Counter {\n        Self::new()\n    }\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::create", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "Counter::new");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::create", TraceDirection::Callees)
            .unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "Counter::new");
}

#[test]
fn traces_rust_self_prefixed_static_calls_in_inline_modules_in_live_workspace_and_persisted_index()
{
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub struct Counter {}\n    impl Counter {\n        pub fn new() -> Counter { Counter {} }\n        pub fn create() -> Counter {\n            Self::new()\n        }\n    }\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "api::Counter::create", TraceDirection::Callees).unwrap();
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "api::Counter::new");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "api::Counter::create", TraceDirection::Callees)
            .unwrap();
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "api::Counter::new");
}

#[test]
fn keeps_rust_self_prefixed_static_calls_fail_closed_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Counter {}\nimpl Counter {\n    fn new() -> Counter { Counter {} }\n    fn missing_static_caller() {\n        Self::missing();\n    }\n}\nfn outside_impl_caller() {\n    Self::new();\n}\n",
    )
    .unwrap();

    for (caller, forbidden) in [
        ("Counter::missing_static_caller", "Counter::missing"),
        ("outside_impl_caller", "Counter::new"),
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(
            !live
                .callees
                .iter()
                .any(|callee| callee.symbol_id == forbidden),
            "{caller} must not trace {forbidden} for an unresolved Self-prefixed static call"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, forbidden) in [
        ("Counter::missing_static_caller", "Counter::missing"),
        ("outside_impl_caller", "Counter::new"),
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            !persisted
                .callees
                .iter()
                .any(|callee| callee.symbol_id == forbidden),
            "{caller} must not trace {forbidden} for an unresolved Self-prefixed static call from the persisted index"
        );
    }
}

#[test]
fn traces_rust_tuple_struct_receiver_method_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Counter(u32);\nimpl Counter {\n    fn increment(&self) {}\n}\nfn caller() { Counter(1).increment(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "Counter::increment");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "Counter::increment");
}

#[test]
fn traces_rust_unit_struct_receiver_method_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Unit;\nimpl Unit {\n    fn run(&self) {}\n}\nfn caller() { Unit.run(); }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "Unit::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "Unit::run");
}

#[test]
fn traces_rust_tuple_and_unit_struct_let_binding_receivers_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Counter(u32);\nimpl Counter {\n    fn increment(&self) {}\n}\nstruct Unit;\nimpl Unit {\n    fn run(&self) {}\n}\nfn caller() {\n    let c = Counter(1);\n    c.increment();\n    let u = Unit;\n    u.run();\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    let actual = live
        .callees
        .iter()
        .map(|callee| callee.symbol_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(actual.len(), 2);
    assert!(actual.contains(&"Counter::increment"));
    assert!(actual.contains(&"Unit::run"));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    let actual = persisted
        .callees
        .iter()
        .map(|callee| callee.symbol_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(actual.len(), 2);
    assert!(actual.contains(&"Counter::increment"));
    assert!(actual.contains(&"Unit::run"));
}

#[test]
fn traces_rust_inline_module_tuple_and_unit_struct_receivers_in_live_workspace_and_persisted_index()
{
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub struct Counter(u32);\n    impl Counter {\n        pub fn increment(&self) {}\n    }\n    pub struct Unit;\n    impl Unit {\n        pub fn run(&self) {}\n    }\n    pub fn caller() {\n        Counter(1).increment();\n        Unit.run();\n        let c = Counter(1);\n        c.increment();\n        let u = Unit;\n        u.run();\n    }\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "api::caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    let actual = live
        .callees
        .iter()
        .map(|callee| callee.symbol_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(actual.len(), 2);
    assert!(actual.contains(&"api::Counter::increment"));
    assert!(actual.contains(&"api::Unit::run"));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "api::caller", TraceDirection::Callees).unwrap();
    let actual = persisted
        .callees
        .iter()
        .map(|callee| callee.symbol_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(actual.len(), 2);
    assert!(actual.contains(&"api::Counter::increment"));
    assert!(actual.contains(&"api::Unit::run"));
}

#[test]
fn keeps_rust_tuple_and_unit_struct_receivers_fail_closed_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "struct Counter(u32);\nimpl Counter {\n    fn increment(&self) {}\n}\nstruct Unit;\nimpl Unit {\n    fn run(&self) {}\n}\nfn missing_method_caller() { Counter(1).missing(); }\nfn missing_unit_method_caller() { Unit.missing(); }\nfn non_struct_receiver_caller() { let n = 5; n.run(); }\nfn shadowed_unit_binding_caller() {\n    let Unit = 1;\n    Unit.run();\n}\n",
    )
    .unwrap();

    for caller in [
        "missing_method_caller",
        "missing_unit_method_caller",
        "non_struct_receiver_caller",
        "shadowed_unit_binding_caller",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(
            live.callees.is_empty(),
            "{caller} must fail closed for an unknown or shadowed tuple/unit struct receiver"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in [
        "missing_method_caller",
        "missing_unit_method_caller",
        "non_struct_receiver_caller",
        "shadowed_unit_binding_caller",
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            persisted.callees.is_empty(),
            "{caller} must fail closed for an unknown or shadowed tuple/unit struct receiver from the persisted index"
        );
    }
}

#[test]
fn traces_rust_module_qualified_typed_parameter_method_calls_in_live_workspace_and_persisted_index()
{
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub struct Counter {}\n    impl Counter {\n        pub fn increment(&self) {}\n    }\n}\nfn caller(c: &api::Counter, d: &mut api::Counter) {\n    c.increment();\n    d.increment();\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "api::Counter::increment");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "api::Counter::increment");
}

#[test]
fn traces_rust_module_qualified_constructor_binding_method_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub struct Counter {}\n    impl Counter {\n        pub fn new() -> Counter { Counter {} }\n        pub fn increment(&self) {}\n    }\n}\nmod outer {\n    pub mod inner {\n        pub struct Unit {}\n        impl Unit {\n            pub fn new() -> Unit { Unit {} }\n            pub fn run(&self) {}\n        }\n    }\n}\nfn caller() {\n    let c = api::Counter::new();\n    c.increment();\n    let u = outer::inner::Unit::new();\n    u.run();\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    let mut actual = live
        .callees
        .iter()
        .map(|callee| callee.symbol_id.as_str())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(
        actual,
        [
            "api::Counter::increment",
            "api::Counter::new",
            "outer::inner::Unit::new",
            "outer::inner::Unit::run",
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    let mut actual = persisted
        .callees
        .iter()
        .map(|callee| callee.symbol_id.as_str())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(
        actual,
        [
            "api::Counter::increment",
            "api::Counter::new",
            "outer::inner::Unit::new",
            "outer::inner::Unit::run",
        ]
    );
}

#[test]
fn traces_rust_module_qualified_tuple_unit_struct_and_struct_literal_receivers_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub struct Counter(u32);\n    impl Counter {\n        pub fn increment(&self) {}\n    }\n    pub struct Unit;\n    impl Unit {\n        pub fn run(&self) {}\n    }\n    pub struct Plain {}\n    impl Plain {\n        pub fn step(&self) {}\n    }\n}\nfn caller() {\n    api::Counter(1).increment();\n    let c = api::Counter(1);\n    c.increment();\n    api::Unit.run();\n    let u = api::Unit;\n    u.run();\n    api::Plain {}.step();\n    let p = api::Plain {};\n    p.step();\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    let mut actual = live
        .callees
        .iter()
        .map(|callee| callee.symbol_id.as_str())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(
        actual,
        [
            "api::Counter::increment",
            "api::Plain::step",
            "api::Unit::run"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    let mut actual = persisted
        .callees
        .iter()
        .map(|callee| callee.symbol_id.as_str())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(
        actual,
        [
            "api::Counter::increment",
            "api::Plain::step",
            "api::Unit::run"
        ]
    );
}

#[test]
fn keeps_rust_module_qualified_receiver_types_fail_closed_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub struct Counter {}\n    impl Counter {\n        pub fn new() -> Counter { Counter {} }\n        pub fn increment(&self) {}\n    }\n}\nfn unknown_module_parameter_caller(c: &outside::Counter) { c.increment(); }\nfn crate_qualified_parameter_caller(c: &crate::api::Counter) { c.increment(); }\nfn unknown_struct_constructor_caller() {\n    let c = api::Unknown::new();\n    c.increment();\n}\nfn crate_qualified_constructor_caller() {\n    let c = crate::api::Counter::new();\n    c.increment();\n}\nfn shadowed_module_constructor_caller(api: &Other) {\n    let c = api::Counter::new();\n    c.increment();\n}\nfn shadowed_module_tuple_caller() {\n    let api = Other {};\n    api::Counter(1).increment();\n}\nstruct Other {}\nimpl Other { fn increment(&self) {} }\n",
    )
    .unwrap();

    let forbidden_targets = [
        ("unknown_module_parameter_caller", "api::Counter::increment"),
        (
            "crate_qualified_parameter_caller",
            "api::Counter::increment",
        ),
        (
            "unknown_struct_constructor_caller",
            "api::Counter::increment",
        ),
        (
            "crate_qualified_constructor_caller",
            "api::Counter::increment",
        ),
        (
            "shadowed_module_constructor_caller",
            "api::Counter::increment",
        ),
        ("shadowed_module_constructor_caller", "Other::increment"),
        ("shadowed_module_tuple_caller", "api::Counter::increment"),
    ];
    for (caller, forbidden) in forbidden_targets {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(
            !live
                .callees
                .iter()
                .any(|callee| callee.symbol_id == forbidden),
            "{caller} must not trace {forbidden} for an unresolvable module-qualified receiver type"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, forbidden) in forbidden_targets {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            !persisted
                .callees
                .iter()
                .any(|callee| callee.symbol_id == forbidden),
            "{caller} must not trace {forbidden} for an unresolvable module-qualified receiver type from the persisted index"
        );
    }
}

#[test]
fn traces_rust_inline_module_static_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub struct Counter {}\n    impl Counter {\n        pub fn new() -> Counter { Counter {} }\n    }\n    pub fn caller() {\n        let _ = Counter::new();\n    }\n}\nmod outer {\n    pub mod inner {\n        pub struct Unit {}\n        impl Unit {\n            pub fn run() {}\n        }\n    }\n}\nfn caller() {\n    let _ = api::Counter::new();\n    let _ = outer::inner::Unit::run();\n}\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    let mut actual = live
        .callees
        .iter()
        .map(|callee| callee.symbol_id.as_str())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(actual, ["api::Counter::new", "outer::inner::Unit::run"]);

    let live = trace_symbol_graph(&dir, "api::caller", TraceDirection::Callees).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callees.len(), 1);
    assert_eq!(live.callees[0].symbol_id, "api::Counter::new");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Callees).unwrap();
    let mut actual = persisted
        .callees
        .iter()
        .map(|callee| callee.symbol_id.as_str())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(actual, ["api::Counter::new", "outer::inner::Unit::run"]);

    let persisted =
        trace_symbol_graph_from_index(&db_path, "api::caller", TraceDirection::Callees).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callees.len(), 1);
    assert_eq!(persisted.callees[0].symbol_id, "api::Counter::new");
}

#[test]
fn keeps_rust_inline_module_static_calls_fail_closed_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let root_path = dir.join("lib.rs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &root_path,
        "mod api {\n    pub struct Counter {}\n    impl Counter {\n        pub fn new() -> Counter { Counter {} }\n    }\n}\nfn unknown_module_caller() { api_missing::Type::new(); }\nfn unknown_type_caller() { api::Missing::new(); }\nfn non_type_tail_caller() { api::helper::thing(); }\nfn shadowed_module_caller(api: &Other) { api::Counter::new(); }\nfn shadowed_module_binding_caller() {\n    let api = Other {};\n    api::Counter::new();\n}\nstruct Other {}\nimpl Other { fn new() -> Other { Other {} } }\n",
    )
    .unwrap();

    for caller in [
        "unknown_module_caller",
        "unknown_type_caller",
        "non_type_tail_caller",
        "shadowed_module_caller",
        "shadowed_module_binding_caller",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(
            live.callees.is_empty(),
            "{caller} must fail closed for an unresolvable inline-module static call"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in [
        "unknown_module_caller",
        "unknown_type_caller",
        "non_type_tail_caller",
        "shadowed_module_caller",
        "shadowed_module_binding_caller",
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            persisted.callees.is_empty(),
            "{caller} must fail closed for an unresolvable inline-module static call from the persisted index"
        );
    }
}
