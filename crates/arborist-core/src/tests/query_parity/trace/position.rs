use super::*;
use crate::{trace_symbol_graph_from_index_with_source, trace_symbol_graph_with_source};

#[test]
fn trace_symbol_graph_at_position_uses_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let renamed_helper = "def renamed_helper(value: int) -> int:\n    return value + 2\n";
    let renamed_caller = "from graph_b import renamed_helper\n\n\ndef orchestrate(value: int) -> int:\n    return renamed_helper(value)\n";
    vfs.open_file(&helper, Some(renamed_helper)).unwrap();
    vfs.open_file(&caller, Some(renamed_caller)).unwrap();

    let result = vfs
        .trace_symbol_graph_at_position(
            &dir,
            &helper,
            &Position { row: 0, column: 5 },
            TraceDirection::Callers,
        )
        .unwrap();
    assert_eq!(result.symbol.semantic_path, "renamed_helper");
    assert_eq!(result.callers.len(), 1);
    assert_eq!(result.callers[0].semantic_path, "orchestrate");
}

#[test]
fn trace_symbol_graph_at_position_with_source_normalizes_path_without_writing_disk() {
    let dir = temporary_dir();
    let nested = dir.join("child");
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let caller_alias = nested.join("..").join("caller.py");

    fs::create_dir_all(&nested).unwrap();
    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let result = trace_symbol_graph_at_position_with_source(
            &dir,
            &caller_alias,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
            &Position { row: 3, column: 5 },
            TraceDirection::Both,
        )
        .unwrap();

    assert!(!caller.exists());
    assert_eq!(result.symbol.semantic_path, "orchestrate");
    assert_eq!(result.symbol.file_path, normalize_path(&caller));
    assert!(
        result
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn traces_symbol_graph_at_position_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("graph_a.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from graph_b import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let position = Position { row: 0, column: 5 };
    let live =
        trace_symbol_graph_at_position(&dir, &helper, &position, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.semantic_path, "helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].semantic_path, "orchestrate");
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_at_position_from_index(
        &db_path,
        &helper,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].semantic_path, "orchestrate");
}

#[test]
fn traces_javascript_symbol_graph_at_position_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper = dir.join("helper.ts");
    let caller = dir.join("caller.ts");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "export function helper(value: number): number { return value + 1; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "import { helper } from \"./helper\";\nexport function caller(value: number): number { return helper(value); }\n",
    )
    .unwrap();

    let position = Position { row: 0, column: 16 };
    let live =
        trace_symbol_graph_at_position(&dir, &helper, &position, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, "helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].semantic_path, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_at_position_from_index(
        &db_path,
        &helper,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].semantic_path, "caller");
}

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
fn traces_java_unqualified_same_type_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Counter.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;\nclass Counter {\n    int helper() { return 1; }\n    int caller() { return helper(); }\n    int first(int value) { return value; }\n    long first(long value) { return value; }\n    long ambiguous() { return first(1L); }\n}\n",
    )
    .unwrap();

    let helper_path = "com::example::Counter::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Counter::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, helper_path);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "com::example::Counter::caller"
    );

    let overloaded_id = format!(
        "{}::com::example::Counter::first#overload[2]",
        normalize_path(&source_path)
    );
    let overloaded =
        trace_symbol_graph_from_index(&db_path, &overloaded_id, TraceDirection::Callers).unwrap();
    assert!(overloaded.callers.is_empty());
}

#[test]
fn traces_java_explicit_this_constructor_initializers_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Counter.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;\nclass Counter {\n    Counter() {}\n    Counter(int value) { this(); }\n    Counter(int... values) {}\n    Counter(boolean first, boolean second) { this(1, 2); }\n}\n",
    )
    .unwrap();

    let target = format!(
        "{}::com::example::Counter::Counter#overload[1]",
        normalize_path(&source_path)
    );
    let delegated_constructor = format!(
        "{}::com::example::Counter::Counter#overload[2]",
        normalize_path(&source_path)
    );
    let params_constructor = format!(
        "{}::com::example::Counter::Counter#overload[3]",
        normalize_path(&source_path)
    );

    let live = trace_symbol_graph(&dir, &target, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, delegated_constructor);
    let params_live =
        trace_symbol_graph(&dir, &params_constructor, TraceDirection::Callers).unwrap();
    assert!(params_live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, &target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, delegated_constructor);
    let params_persisted =
        trace_symbol_graph_from_index(&db_path, &params_constructor, TraceDirection::Callers)
            .unwrap();
    assert!(params_persisted.callers.is_empty());
}

#[test]
fn traces_java_explicit_this_constructor_initializers_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Counter.java");
    let db_path = dir.join("symbols.db");
    fs::write(&source_path, "package com.example; class Counter {}\n").unwrap();
    let overlay = "package com.example;\nclass Counter {\n    Counter() {}\n    Counter(int value) { this(); }\n}\n";
    let target = format!(
        "{}::com::example::Counter::Counter#overload[1]",
        normalize_path(&source_path)
    );
    let delegated_constructor = format!(
        "{}::com::example::Counter::Counter#overload[2]",
        normalize_path(&source_path)
    );

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        &target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, delegated_constructor);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        &target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, delegated_constructor);
}
#[test]
fn traces_java_explicit_same_file_super_constructor_initializers() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;\nclass Base {\n    Base() {}\n    Base(int value) {}\n    Base(int... values) {}\n    int helper() { return 1; }\n}\nclass Child extends Base {\n    Child() { super(); }\n    Child(int value) { super(value); }\n    Child(boolean first, boolean second) { super(1, 2); }\n    int inheritedCaller() { return super.helper(); }\n    int inheritedBareCaller() { return helper(); }\n}\n",
    )
    .unwrap();
    let file_path = normalize_path(&source_path);
    let base_zero = format!("{file_path}::com::example::Base::Base#overload[1]");
    let base_one = format!("{file_path}::com::example::Base::Base#overload[2]");
    let child_zero = format!("{file_path}::com::example::Child::Child#overload[1]");
    let child_one = format!("{file_path}::com::example::Child::Child#overload[2]");
    let base_params = format!("{file_path}::com::example::Base::Base#overload[3]");

    let live_zero = trace_symbol_graph(&dir, &base_zero, TraceDirection::Callers).unwrap();
    assert_eq!(live_zero.callers.len(), 1);
    assert_eq!(live_zero.callers[0].symbol_id, child_zero);
    let live_one = trace_symbol_graph(&dir, &base_one, TraceDirection::Callers).unwrap();
    assert_eq!(live_one.callers.len(), 1);
    assert_eq!(live_one.callers[0].symbol_id, child_one);
    let live_params = trace_symbol_graph(&dir, &base_params, TraceDirection::Callers).unwrap();
    assert!(live_params.callers.is_empty());
    let helper_live =
        trace_symbol_graph(&dir, "com::example::Base::helper", TraceDirection::Callers).unwrap();
    assert_eq!(helper_live.callers.len(), 2);
    assert_eq!(
        helper_live.callers[0].symbol_id,
        "com::example::Child::inheritedBareCaller"
    );
    assert_eq!(
        helper_live.callers[1].symbol_id,
        "com::example::Child::inheritedCaller"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_zero =
        trace_symbol_graph_from_index(&db_path, &base_zero, TraceDirection::Callers).unwrap();
    assert_eq!(persisted_zero.callers.len(), 1);
    assert_eq!(persisted_zero.callers[0].symbol_id, child_zero);
    let persisted_one =
        trace_symbol_graph_from_index(&db_path, &base_one, TraceDirection::Callers).unwrap();
    assert_eq!(persisted_one.callers.len(), 1);
    assert_eq!(persisted_one.callers[0].symbol_id, child_one);
    let persisted_params =
        trace_symbol_graph_from_index(&db_path, &base_params, TraceDirection::Callers).unwrap();
    assert!(persisted_params.callers.is_empty());
    let helper_persisted = trace_symbol_graph_from_index(
        &db_path,
        "com::example::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(helper_persisted.callers.len(), 2);
    assert_eq!(
        helper_persisted.callers[0].symbol_id,
        "com::example::Child::inheritedBareCaller"
    );
    assert_eq!(
        helper_persisted.callers[1].symbol_id,
        "com::example::Child::inheritedCaller"
    );
}
#[test]
fn traces_java_explicit_same_file_super_constructor_initializers_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(&source_path, "package com.example; class Stale {}\n").unwrap();
    let overlay = "package com.example;\nclass Base { Base() {} int helper() { return 1; } }\nclass Child extends Base { Child() { super(); } int inheritedCaller() { return super.helper(); }\n    int inheritedBareCaller() { return helper(); } }\n";
    let target = "com::example::Base::Base";
    let child_constructor = "com::example::Child::Child";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, child_constructor);
    let helper_live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        "com::example::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(helper_live.callers.len(), 2);
    assert_eq!(
        helper_live.callers[0].symbol_id,
        "com::example::Child::inheritedBareCaller"
    );
    assert_eq!(
        helper_live.callers[1].symbol_id,
        "com::example::Child::inheritedCaller"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, child_constructor);
    let helper_persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        "com::example::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(helper_persisted.callers.len(), 2);
    assert_eq!(
        helper_persisted.callers[0].symbol_id,
        "com::example::Child::inheritedBareCaller"
    );
    assert_eq!(
        helper_persisted.callers[1].symbol_id,
        "com::example::Child::inheritedCaller"
    );
}
#[test]
fn traces_csharp_conservative_direct_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Counter.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "class GlobalHelper {\n    public static int Utility(int value) => value;\n    public static int Flexible(params int[] values) => values.Length;\n    public int Instance(int value) => value;\n}\nclass Counter {\n    Counter() {}\n    Counter(int value) : this() {}\n    Counter(string value) : base() {}\n    Counter(params int[] values) {}\n    Counter(bool first, bool second) : this(1, 2) {}\n    int Helper() => 1;\n    int Caller() => Helper();\n    int ExplicitThis() => this.Helper();\n    int ExplicitThisParameterShadow(System.Func<int> Helper) => this.Helper();\n    int First(int value) => value;\n    long First(long value) => value;\n    long Ambiguous() => First(1L);\n    int Flexible(params int[] values) => values.Length;\n    int ParamsCaller() => Flexible(1);\n    int GlobalStaticCaller() => global::GlobalHelper.Utility(1);\n    int GlobalInstanceCaller() => global::GlobalHelper.Instance(1);\n    int GlobalParamsCaller() => global::GlobalHelper.Flexible(1);\n}\nclass SimpleCaller {\n    int LocalStaticCaller() => GlobalHelper.Utility(1);\n    int LocalInstanceCaller() => GlobalHelper.Instance(1);\n    int LocalParamsCaller() => GlobalHelper.Flexible(1);\n}\nclass Outer {\n    class Nested {\n        int NestedStaticCaller() => GlobalHelper.Utility(1);\n    }\n}\nclass MemberShadowCaller {\n    GlobalHelper GlobalHelper { get; } = new GlobalHelper();\n    int MemberShadow() => GlobalHelper.Instance(1);\n}\n",
    )
    .unwrap();

    let helper_path = "Counter::Helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 3);
    assert_eq!(
        live.callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Counter::Caller",
            "Counter::ExplicitThis",
            "Counter::ExplicitThisParameterShadow"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, helper_path);
    assert_eq!(persisted.callers.len(), 3);
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Counter::Caller",
            "Counter::ExplicitThis",
            "Counter::ExplicitThisParameterShadow"
        ]
    );

    let overloaded_id = format!(
        "{}::Counter::First#overload[2]",
        normalize_path(&source_path)
    );
    let overloaded =
        trace_symbol_graph_from_index(&db_path, &overloaded_id, TraceDirection::Callers).unwrap();
    assert!(overloaded.callers.is_empty());

    let params_target =
        trace_symbol_graph_from_index(&db_path, "Counter::Flexible", TraceDirection::Callers)
            .unwrap();
    assert!(params_target.callers.is_empty());

    let constructor_target = "Counter::Counter";
    let delegated_constructor_id = format!(
        "{}::Counter::Counter#overload[2]",
        normalize_path(&source_path)
    );
    let constructor_live =
        trace_symbol_graph(&dir, constructor_target, TraceDirection::Callers).unwrap();
    assert_eq!(constructor_live.callers.len(), 1);
    assert_eq!(
        constructor_live.callers[0].symbol_id,
        delegated_constructor_id
    );
    let constructor_persisted =
        trace_symbol_graph_from_index(&db_path, constructor_target, TraceDirection::Callers)
            .unwrap();
    assert_eq!(constructor_persisted.callers.len(), 1);
    assert_eq!(
        constructor_persisted.callers[0].symbol_id,
        delegated_constructor_id
    );

    let params_constructor_id = format!(
        "{}::Counter::Counter#overload[4]",
        normalize_path(&source_path)
    );
    let params_constructor_live =
        trace_symbol_graph(&dir, &params_constructor_id, TraceDirection::Callers).unwrap();
    assert!(params_constructor_live.callers.is_empty());
    let params_constructor_persisted =
        trace_symbol_graph_from_index(&db_path, &params_constructor_id, TraceDirection::Callers)
            .unwrap();
    assert!(params_constructor_persisted.callers.is_empty());

    let static_target = "GlobalHelper::Utility";
    let static_live = trace_symbol_graph(&dir, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(static_live.callers.len(), 3);
    assert_eq!(
        static_live
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Counter::GlobalStaticCaller",
            "Outer::Nested::NestedStaticCaller",
            "SimpleCaller::LocalStaticCaller"
        ]
    );
    let static_persisted =
        trace_symbol_graph_from_index(&db_path, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(static_persisted.callers.len(), 3);
    assert_eq!(
        static_persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Counter::GlobalStaticCaller",
            "Outer::Nested::NestedStaticCaller",
            "SimpleCaller::LocalStaticCaller"
        ]
    );

    for target in ["GlobalHelper::Instance", "GlobalHelper::Flexible"] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn traces_csharp_simple_and_global_base_constructor_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Base.cs"),
        "namespace Demo;
class Base {
    public Base(int value) {}
    public Base(params int[] values) {}
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Derived.cs"),
        "namespace Demo;
class SimpleDerived : Base { SimpleDerived(int value) : base(value) {} }
class GlobalDerived : global::Demo.Base { GlobalDerived(int value) : base(value) {} }
class ParamsDerived : Base { ParamsDerived() : base(1, 2) {} }
",
    )
    .unwrap();

    let target = "Demo::Base::Base";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(
        live.callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Demo::GlobalDerived::GlobalDerived",
            "Demo::SimpleDerived::SimpleDerived"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Demo::GlobalDerived::GlobalDerived",
            "Demo::SimpleDerived::SimpleDerived"
        ]
    );
}

#[test]
fn traces_csharp_direct_base_methods_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Base.cs"),
        "namespace Demo;
class Base {
    public int Ping(int value) => value;
    public int Flexible(params int[] values) => values.Length;
    public static int Static(int value) => value;
    public int First(int value) => value;
    public long First(long value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Derived.cs"),
        "namespace Demo;
class SimpleDerived : Base {
    int Call(int value) => base.Ping(value);
    int ParamsCaller() => base.Flexible(1);
    int StaticCaller() => base.Static(1);
    int Ambiguous() => base.First(1);
}
class GlobalDerived : global::Demo.Base {
    int GlobalCall(int value) => base.Ping(value);
}
",
    )
    .unwrap();

    let target = "Demo::Base::Ping";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(
        live.callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Demo::GlobalDerived::GlobalCall",
            "Demo::SimpleDerived::Call"
        ]
    );
    for caller in [
        "Demo::SimpleDerived::ParamsCaller",
        "Demo::SimpleDerived::StaticCaller",
        "Demo::SimpleDerived::Ambiguous",
    ] {
        let trace = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(trace.callees.is_empty(), "{caller}");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Demo::GlobalDerived::GlobalCall",
            "Demo::SimpleDerived::Call"
        ]
    );
    for caller in [
        "Demo::SimpleDerived::ParamsCaller",
        "Demo::SimpleDerived::StaticCaller",
        "Demo::SimpleDerived::Ambiguous",
    ] {
        let trace =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(trace.callees.is_empty(), "{caller}");
    }
}

#[test]
fn traces_csharp_file_and_namespace_alias_base_members_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Base.cs"),
        "namespace Demo;
class Base {
    public Base(int value) {}
    public int Ping(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("RootAlias.cs"),
        "using RootBase = Demo.Base;
namespace Demo.App;
class RootDerived : RootBase {
    RootDerived(int value) : base(value) {}
    int Call(int value) => base.Ping(value);
}
",
    )
    .unwrap();
    fs::write(
        dir.join("NamespaceAlias.cs"),
        "namespace Demo.Exact {
using ExactBase = Demo.Base;
class ExactDerived : ExactBase {
    ExactDerived(int value) : base(value) {}
    int Call(int value) => base.Ping(value);
}
}
",
    )
    .unwrap();

    for target in ["Demo::Base::Base", "Demo::Base::Ping"] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert_eq!(live.indexed_files, 3);
        assert_eq!(
            live.callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            if target.ends_with("::Base") {
                [
                    "Demo::App::RootDerived::RootDerived",
                    "Demo::Exact::ExactDerived::ExactDerived",
                ]
            } else {
                [
                    "Demo::App::RootDerived::Call",
                    "Demo::Exact::ExactDerived::Call",
                ]
            }
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for target in ["Demo::Base::Base", "Demo::Base::Ping"] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert_eq!(persisted.indexed_files, 3);
        assert_eq!(
            persisted
                .callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            if target.ends_with("::Base") {
                [
                    "Demo::App::RootDerived::RootDerived",
                    "Demo::Exact::ExactDerived::ExactDerived",
                ]
            } else {
                [
                    "Demo::App::RootDerived::Call",
                    "Demo::Exact::ExactDerived::Call",
                ]
            }
        );
    }
}

#[test]
fn traces_csharp_file_and_namespace_import_base_members_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Base.cs"),
        "namespace Demo.Utility;
class Base {
    public Base(int value) {}
    public int Ping(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("RootImport.cs"),
        "using Demo.Utility;
namespace Demo.App;
class RootDerived : Base {
    RootDerived(int value) : base(value) {}
    int Call(int value) => base.Ping(value);
}
",
    )
    .unwrap();
    fs::write(
        dir.join("NamespaceImport.cs"),
        "namespace Demo.Exact {
using Demo.Utility;
class ExactDerived : Base {
    ExactDerived(int value) : base(value) {}
    int Call(int value) => base.Ping(value);
}
}
",
    )
    .unwrap();

    for target in ["Demo::Utility::Base::Base", "Demo::Utility::Base::Ping"] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert_eq!(live.indexed_files, 3);
        assert_eq!(
            live.callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            if target.ends_with("::Base") {
                [
                    "Demo::App::RootDerived::RootDerived",
                    "Demo::Exact::ExactDerived::ExactDerived",
                ]
            } else {
                [
                    "Demo::App::RootDerived::Call",
                    "Demo::Exact::ExactDerived::Call",
                ]
            }
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for target in ["Demo::Utility::Base::Base", "Demo::Utility::Base::Ping"] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert_eq!(persisted.indexed_files, 3);
        assert_eq!(
            persisted
                .callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            if target.ends_with("::Base") {
                [
                    "Demo::App::RootDerived::RootDerived",
                    "Demo::Exact::ExactDerived::ExactDerived",
                ]
            } else {
                [
                    "Demo::App::RootDerived::Call",
                    "Demo::Exact::ExactDerived::Call",
                ]
            }
        );
    }
}

#[test]
fn traces_csharp_global_alias_and_namespace_import_base_members_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Base.cs"),
        "namespace Demo.Utility;
class Base {
    public Base(int value) {}
    public int Ping(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("GlobalAlias.cs"),
        "global using BaseAlias = Demo.Utility.Base;
namespace Demo.Alias;
class AliasDerived : BaseAlias {
    AliasDerived(int value) : base(value) {}
    int Call(int value) => base.Ping(value);
}
",
    )
    .unwrap();
    fs::write(
        dir.join("GlobalImport.cs"),
        "global using Demo.Utility;
namespace Demo.Import;
class ImportDerived : Base {
    ImportDerived(int value) : base(value) {}
    int Call(int value) => base.Ping(value);
}
",
    )
    .unwrap();

    for target in ["Demo::Utility::Base::Base", "Demo::Utility::Base::Ping"] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert_eq!(live.indexed_files, 3);
        assert_eq!(
            live.callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            if target.ends_with("::Base") {
                [
                    "Demo::Alias::AliasDerived::AliasDerived",
                    "Demo::Import::ImportDerived::ImportDerived",
                ]
            } else {
                [
                    "Demo::Alias::AliasDerived::Call",
                    "Demo::Import::ImportDerived::Call",
                ]
            }
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for target in ["Demo::Utility::Base::Base", "Demo::Utility::Base::Ping"] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert_eq!(persisted.indexed_files, 3);
        assert_eq!(
            persisted
                .callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            if target.ends_with("::Base") {
                [
                    "Demo::Alias::AliasDerived::AliasDerived",
                    "Demo::Import::ImportDerived::ImportDerived",
                ]
            } else {
                [
                    "Demo::Alias::AliasDerived::Call",
                    "Demo::Import::ImportDerived::Call",
                ]
            }
        );
    }
}

#[test]
fn does_not_trace_ambiguous_csharp_local_or_global_base_import_alias_members() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Bases.cs"),
        "namespace First { class Base { public Base(int value) {} public int Ping(int value) => value; } }
namespace Second { class Base { public Base(int value) {} public int Ping(int value) => value; } }
",
    )
    .unwrap();
    fs::write(
        dir.join("Ambiguous.cs"),
        "using First;
using Second;
namespace Demo.App;
class AmbiguousDerived : Base {
    AmbiguousDerived(int value) : base(value) {}
    int Call(int value) => base.Ping(value);
}
",
    )
    .unwrap();
    fs::write(
        dir.join("GlobalAliasOne.cs"),
        "global using BaseAlias = First.Base;
namespace Demo.GlobalAlias;
class GlobalAliasDerived : BaseAlias {
    GlobalAliasDerived(int value) : base(value) {}
    int Call(int value) => base.Ping(value);
}
",
    )
    .unwrap();
    fs::write(
        dir.join("GlobalAliasTwo.cs"),
        "global using BaseAlias = Second.Base;
",
    )
    .unwrap();
    for caller in [
        "Demo::App::AmbiguousDerived::AmbiguousDerived",
        "Demo::App::AmbiguousDerived::Call",
        "Demo::GlobalAlias::GlobalAliasDerived::GlobalAliasDerived",
        "Demo::GlobalAlias::GlobalAliasDerived::Call",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty(), "{caller}");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in [
        "Demo::App::AmbiguousDerived::AmbiguousDerived",
        "Demo::App::AmbiguousDerived::Call",
        "Demo::GlobalAlias::GlobalAliasDerived::GlobalAliasDerived",
        "Demo::GlobalAlias::GlobalAliasDerived::Call",
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(persisted.callees.is_empty(), "{caller}");
    }
}

#[test]
fn does_not_trace_ambiguous_or_colliding_csharp_base_alias_members() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Base.cs"),
        "namespace Demo { class Base { public Base(int value) {} public int Ping(int value) => value; } }
namespace Other { class Base { public Base(int value) {} public int Ping(int value) => value; } }
",
    )
    .unwrap();
    fs::write(
        dir.join("Ambiguous.cs"),
        "using BaseAlias = Demo.Base;
using BaseAlias = Other.Base;
namespace Demo.App;
class AmbiguousDerived : BaseAlias {
    AmbiguousDerived(int value) : base(value) {}
    int Call(int value) => base.Ping(value);
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Collision.cs"),
        "using BaseAlias = Demo.Base;
namespace Demo.Collision;
class BaseAlias {}
class CollisionDerived : BaseAlias {
    CollisionDerived() : base() {}
    int Call(int value) => base.Ping(value);
}
",
    )
    .unwrap();
    for caller in [
        "Demo::App::AmbiguousDerived::AmbiguousDerived",
        "Demo::App::AmbiguousDerived::Call",
        "Demo::Collision::CollisionDerived::CollisionDerived",
        "Demo::Collision::CollisionDerived::Call",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty(), "{caller}");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in [
        "Demo::App::AmbiguousDerived::AmbiguousDerived",
        "Demo::App::AmbiguousDerived::Call",
        "Demo::Collision::CollisionDerived::CollisionDerived",
        "Demo::Collision::CollisionDerived::Call",
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(persisted.callees.is_empty(), "{caller}");
    }
}

#[test]
fn traces_csharp_unshadowed_qualified_base_members_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Base.cs"),
        "namespace Demo;
class Base {
    public Base(int value) {}
    public int Ping(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Derived.cs"),
        "namespace Other;
class Derived : Demo.Base {
    Derived(int value) : base(value) {}
    int Call(int value) => base.Ping(value);
}
",
    )
    .unwrap();

    for target in ["Demo::Base::Base", "Demo::Base::Ping"] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert_eq!(live.indexed_files, 2);
        assert_eq!(
            live.callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            if target.ends_with("::Base") {
                ["Other::Derived::Derived"]
            } else {
                ["Other::Derived::Call"]
            }
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for target in ["Demo::Base::Base", "Demo::Base::Ping"] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert_eq!(persisted.indexed_files, 2);
        assert_eq!(
            persisted
                .callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            if target.ends_with("::Base") {
                ["Other::Derived::Derived"]
            } else {
                ["Other::Derived::Call"]
            }
        );
    }
}

#[test]
fn traces_csharp_ancestor_base_methods_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Grand.cs"),
        "namespace Demo.Utility;
class Grand {
    public int Ping(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Parent.cs"),
        "using GrandAlias = Demo.Utility.Grand;
namespace Demo.Middle;
class Parent : GrandAlias {}
",
    )
    .unwrap();
    fs::write(
        dir.join("Derived.cs"),
        "using Demo.Middle;
namespace Demo.App;
class Derived : Parent {
    int Call(int value) => base.Ping(value);
}
",
    )
    .unwrap();

    let live =
        trace_symbol_graph(&dir, "Demo::Utility::Grand::Ping", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::Derived::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index(
        &db_path,
        "Demo::Utility::Grand::Ping",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::App::Derived::Call");
}

#[test]
fn traces_csharp_generic_base_members_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Base.cs"),
        "namespace Demo.Utility;
class Base<T> {
    public Base(int value) {}
    public int Ping(int value) => value;
}
class Parent<T> : Base<T> {}
",
    )
    .unwrap();
    fs::write(
        dir.join("Derived.cs"),
        "using Demo.Utility;
namespace Demo.App;
class ImportedDerived : Base<int> {
    ImportedDerived(int value) : base(value) {}
    int Call(int value) => base.Ping(value);
}
class AncestorDerived : Parent<string> {
    int Call(int value) => base.Ping(value);
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Qualified.cs"),
        "namespace Other;
class QualifiedDerived : global::Demo.Utility.Base<long> {
    QualifiedDerived(int value) : base(value) {}
    int Call(int value) => base.Ping(value);
}
",
    )
    .unwrap();

    for (target, expected_callers) in [
        (
            "Demo::Utility::Base::Base",
            vec![
                "Demo::App::ImportedDerived::ImportedDerived",
                "Other::QualifiedDerived::QualifiedDerived",
            ],
        ),
        (
            "Demo::Utility::Base::Ping",
            vec![
                "Demo::App::AncestorDerived::Call",
                "Demo::App::ImportedDerived::Call",
                "Other::QualifiedDerived::Call",
            ],
        ),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert_eq!(live.indexed_files, 3);
        assert_eq!(
            live.callers
                .iter()
                .map(|symbol| symbol.symbol_id.clone())
                .collect::<Vec<_>>(),
            expected_callers,
            "{target}"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (target, expected_callers) in [
        (
            "Demo::Utility::Base::Base",
            vec![
                "Demo::App::ImportedDerived::ImportedDerived",
                "Other::QualifiedDerived::QualifiedDerived",
            ],
        ),
        (
            "Demo::Utility::Base::Ping",
            vec![
                "Demo::App::AncestorDerived::Call",
                "Demo::App::ImportedDerived::Call",
                "Other::QualifiedDerived::Call",
            ],
        ),
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert_eq!(persisted.indexed_files, 3);
        assert_eq!(
            persisted
                .callers
                .iter()
                .map(|symbol| symbol.symbol_id.clone())
                .collect::<Vec<_>>(),
            expected_callers,
            "{target}"
        );
    }
}

#[test]
fn does_not_trace_ambiguous_csharp_generic_base_members() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Bases.cs"),
        "namespace Demo;
class Base<T> { public int Ping(int value) => value; }
class Base<TFirst, TSecond> { public int Ping(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Derived.cs"),
        "namespace Demo;
class Derived : Base<int> { int Call(int value) => base.Ping(value); }
",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Demo::Derived::Call", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::Derived::Call", TraceDirection::Callees)
            .unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn does_not_trace_ambiguous_cyclic_or_shadowed_csharp_ancestor_base_methods() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Shadowed.cs"),
        "namespace Demo.Shadowed;
class Grand { public int Ping(int value) => value; }
class Parent : Grand { public int Ping() => 0; }
class Derived : Parent { int Call(int value) => base.Ping(value); }
",
    )
    .unwrap();
    fs::write(
        dir.join("Cycle.cs"),
        "namespace Demo.Cycle;
class First : Second {}
class Second : First {}
class Derived : First { int Call(int value) => base.Ping(value); }
",
    )
    .unwrap();
    fs::write(
        dir.join("AmbiguousFirst.cs"),
        "namespace Demo.Ambiguous; class Grand { public int Ping(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("AmbiguousSecond.cs"),
        "namespace Demo.Ambiguous; class Grand { public int Ping(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("AmbiguousDerived.cs"),
        "namespace Demo.Ambiguous;
class Parent : Grand {}
class Derived : Parent { int Call(int value) => base.Ping(value); }
",
    )
    .unwrap();

    for caller in [
        "Demo::Shadowed::Derived::Call",
        "Demo::Cycle::Derived::Call",
        "Demo::Ambiguous::Derived::Call",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty(), "{caller}");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in [
        "Demo::Shadowed::Derived::Call",
        "Demo::Cycle::Derived::Call",
        "Demo::Ambiguous::Derived::Call",
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(persisted.callees.is_empty(), "{caller}");
    }
}

#[test]
fn does_not_trace_shadowed_csharp_qualified_base_members() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Bases.cs"),
        "namespace Demo { class Base { public Base(int value) {} public int Ping(int value) => value; } }
namespace Other { class Base { public Base(int value) {} public int Ping(int value) => value; } }
namespace App { namespace Demo { class Base { public Base(int value) {} public int Ping(int value) => value; } } }
",
    )
    .unwrap();
    fs::write(
        dir.join("Alias.cs"),
        "using Demo = Other;
namespace App;
class AliasDerived : Demo.Base {
    AliasDerived(int value) : base(value) {}
    int Call(int value) => base.Ping(value);
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Relative.cs"),
        "namespace App;
class RelativeDerived : Demo.Base {
    RelativeDerived(int value) : base(value) {}
    int Call(int value) => base.Ping(value);
}
",
    )
    .unwrap();

    for caller in [
        "App::AliasDerived::AliasDerived",
        "App::AliasDerived::Call",
        "App::RelativeDerived::RelativeDerived",
        "App::RelativeDerived::Call",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty(), "{caller}");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in [
        "App::AliasDerived::AliasDerived",
        "App::AliasDerived::Call",
        "App::RelativeDerived::RelativeDerived",
        "App::RelativeDerived::Call",
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(persisted.callees.is_empty(), "{caller}");
    }
}

#[test]
fn does_not_trace_ambiguous_or_qualified_csharp_base_member_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("First.cs"),
        "namespace Demo; class Base { public Base(int value) {} public int Ping(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Second.cs"),
        "namespace Demo; class Base { public Base(int value) {} public int Ping(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Derived.cs"),
        "namespace Demo; class Derived : Base { Derived(int value) : base(value) {} int Call(int value) => base.Ping(value); }
",
    )
    .unwrap();
    fs::write(
        dir.join("Qualified.cs"),
        "namespace Other; class QualifiedDerived : Demo.Base { QualifiedDerived(int value) : base(value) {} int Call(int value) => base.Ping(value); }
",
    )
    .unwrap();

    for caller in [
        "Demo::Derived::Derived",
        "Demo::Derived::Call",
        "Other::QualifiedDerived::QualifiedDerived",
        "Other::QualifiedDerived::Call",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty());
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in [
        "Demo::Derived::Derived",
        "Demo::Derived::Call",
        "Other::QualifiedDerived::QualifiedDerived",
        "Other::QualifiedDerived::Call",
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(persisted.callees.is_empty());
    }
}

#[test]
fn traces_csharp_same_namespace_static_calls_across_files_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper_path = dir.join("Helper.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper_path,
        "namespace Demo;
class Helper {
    public static int Utility(int value) => value;
    public static int Flexible(params int[] values) => values.Length;
    public int Instance(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace Demo;
class Caller {
    int Call() => Helper.Utility(1);
    int InstanceCall() => Helper.Instance(1);
    int ParamsCall() => Helper.Flexible(1);
    int Shadowed(int Helper) => Helper.Utility(1);
}
",
    )
    .unwrap();

    let static_target = "Demo::Helper::Utility";
    let live = trace_symbol_graph(&dir, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::Caller::Call");

    for target in ["Demo::Helper::Instance", "Demo::Helper::Flexible"] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn does_not_trace_ambiguous_csharp_same_namespace_static_calls_across_files() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("First.cs"),
        "namespace Demo; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Second.cs"),
        "namespace Demo; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo; class Caller { int Call() => Helper.Utility(1); }
",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Demo::Caller::Call", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn traces_csharp_global_static_calls_across_files_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper_path = dir.join("GlobalHelper.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper_path,
        "namespace Demo;
class GlobalHelper {
    public static int Utility(int value) => value;
    public static int Flexible(params int[] values) => values.Length;
    public int Instance(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace Demo;
class Caller {
    int Call() => global::Demo.GlobalHelper.Utility(1);
    int InstanceCall() => global::Demo.GlobalHelper.Instance(1);
    int ParamsCall() => global::Demo.GlobalHelper.Flexible(1);
}
",
    )
    .unwrap();

    let static_target = "Demo::GlobalHelper::Utility";
    let live = trace_symbol_graph(&dir, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::Caller::Call");

    for target in [
        "Demo::GlobalHelper::Instance",
        "Demo::GlobalHelper::Flexible",
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn does_not_trace_ambiguous_csharp_global_static_calls_across_files() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("First.cs"),
        "namespace Demo; class GlobalHelper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Second.cs"),
        "namespace Demo; class GlobalHelper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo; class Caller { int Call() => global::Demo.GlobalHelper.Utility(1); }
",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Demo::Caller::Call", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn traces_csharp_file_type_alias_static_calls_across_files_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper_path = dir.join("Helper.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper_path,
        "namespace Demo.Utility;
class Helper {
    public static int Utility(int value) => value;
    public static int Flexible(params int[] values) => values.Length;
    public int Instance(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "using HelperAlias = Demo.Utility.Helper;
namespace Demo.App;
class Caller {
    int Call() => HelperAlias.Utility(1);
    int InstanceCall() => HelperAlias.Instance(1);
    int ParamsCall() => HelperAlias.Flexible(1);
    int Shadowed(int HelperAlias) => HelperAlias.Utility(1);
}
",
    )
    .unwrap();

    let static_target = "Demo::Utility::Helper::Utility";
    let live = trace_symbol_graph(&dir, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::App::Caller::Call");

    for target in [
        "Demo::Utility::Helper::Instance",
        "Demo::Utility::Helper::Flexible",
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn does_not_trace_ambiguous_or_colliding_csharp_file_type_alias_static_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("First.cs"),
        "namespace Demo.Utility; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Second.cs"),
        "namespace Demo.Utility; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("AmbiguousAlias.cs"),
        "using HelperAlias = Demo.Utility.Helper;
using HelperAlias = Demo.Utility.Other;
namespace Demo.App; class AmbiguousAlias { int Call() => HelperAlias.Utility(1); }
",
    )
    .unwrap();
    fs::write(
        dir.join("CollidingAlias.cs"),
        "using HelperAlias = Demo.Utility.Helper;
namespace Demo.App;
class HelperAlias { public static int Utility(int value) => value; }
class CollidingAlias { int Call() => HelperAlias.Utility(1); }
",
    )
    .unwrap();

    for caller in [
        "Demo::App::AmbiguousAlias::Call",
        "Demo::App::CollidingAlias::Call",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty(), "{caller}: {:?}", live.callees);
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in [
        "Demo::App::AmbiguousAlias::Call",
        "Demo::App::CollidingAlias::Call",
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(
            persisted.callees.is_empty(),
            "{caller}: {:?}",
            persisted.callees
        );
    }
}

#[test]
fn traces_csharp_namespace_scoped_type_alias_static_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper_path = dir.join("Helper.cs");
    let block_caller_path = dir.join("BlockCaller.cs");
    let file_caller_path = dir.join("FileCaller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper_path,
        "namespace Demo.Utility { class Helper { public static int Utility(int value) => value; } }
namespace Demo.Other { class Helper { public static int Utility(int value) => value; } }
",
    )
    .unwrap();
    fs::write(
        &block_caller_path,
        "using HelperAlias = Demo.Utility.Helper;
namespace Demo.App {
    using HelperAlias = Demo.Other.Helper;
    class BlockCaller { int Call() => HelperAlias.Utility(1); }
}
",
    )
    .unwrap();
    fs::write(
        &file_caller_path,
        "namespace Demo.File;
using HelperAlias = Demo.Utility.Helper;
class FileCaller { int Call() => HelperAlias.Utility(1); }
",
    )
    .unwrap();

    let other_target = "Demo::Other::Helper::Utility";
    let live = trace_symbol_graph(&dir, other_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::BlockCaller::Call");

    let utility_target = "Demo::Utility::Helper::Utility";
    let live = trace_symbol_graph(&dir, utility_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::File::FileCaller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, other_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "Demo::App::BlockCaller::Call"
    );

    let persisted =
        trace_symbol_graph_from_index(&db_path, utility_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "Demo::File::FileCaller::Call"
    );
}

#[test]
fn does_not_trace_ambiguous_csharp_namespace_scoped_type_alias_static_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Helper.cs"),
        "namespace Demo.Utility; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo.App {
    using HelperAlias = Demo.Utility.Helper;
    using HelperAlias = Demo.Other.Helper;
    class Caller { int Call() => HelperAlias.Utility(1); }
}
",
    )
    .unwrap();

    let live =
        trace_symbol_graph(&dir, "Demo::App::Caller::Call", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::App::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn traces_csharp_outer_namespace_imports_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Helpers.cs"),
        "namespace Demo.Utility {
    class Helper { public static int Utility(int value) => value; }
    class Base { public int Ping(int value) => value; }
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Callers.cs"),
        "namespace Demo {
    using HelperAlias = Demo.Utility.Helper;
    using BaseAlias = Demo.Utility.Base;
    using static Demo.Utility.Helper;
    using Demo.Utility;

    namespace App {
        class AliasCaller { int Call(int value) => HelperAlias.Utility(value); }
        class StaticCaller { int Call(int value) => Utility(value); }
        class NamespaceCaller { int Call(int value) => Helper.Utility(value); }
        class Derived : BaseAlias { int Call(int value) => base.Ping(value); }
    }
}
",
    )
    .unwrap();

    for (target, expected_callers) in [
        (
            "Demo::Utility::Helper::Utility",
            vec![
                "Demo::App::AliasCaller::Call",
                "Demo::App::NamespaceCaller::Call",
                "Demo::App::StaticCaller::Call",
            ],
        ),
        (
            "Demo::Utility::Base::Ping",
            vec!["Demo::App::Derived::Call"],
        ),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert_eq!(live.indexed_files, 2);
        assert_eq!(
            live.callers
                .iter()
                .map(|symbol| symbol.symbol_id.clone())
                .collect::<Vec<_>>(),
            expected_callers,
            "{target}"
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (target, expected_callers) in [
        (
            "Demo::Utility::Helper::Utility",
            vec![
                "Demo::App::AliasCaller::Call",
                "Demo::App::NamespaceCaller::Call",
                "Demo::App::StaticCaller::Call",
            ],
        ),
        (
            "Demo::Utility::Base::Ping",
            vec!["Demo::App::Derived::Call"],
        ),
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert_eq!(persisted.indexed_files, 2);
        assert_eq!(
            persisted
                .callers
                .iter()
                .map(|symbol| symbol.symbol_id.clone())
                .collect::<Vec<_>>(),
            expected_callers,
            "{target}"
        );
    }
}

#[test]
fn does_not_trace_csharp_inner_ambiguous_alias_through_outer_namespace() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Helpers.cs"),
        "namespace Demo.Utility {
    class First { public static int Utility(int value) => value; }
    class Second { public static int Utility(int value) => value; }
    class Third { public static int Utility(int value) => value; }
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo {
    using HelperAlias = Demo.Utility.First;
    namespace App {
        using HelperAlias = Demo.Utility.Second;
        using HelperAlias = Demo.Utility.Third;
        namespace Feature {
            class Caller { int Call(int value) => HelperAlias.Utility(value); }
        }
    }
}
",
    )
    .unwrap();

    let live = trace_symbol_graph(
        &dir,
        "Demo::App::Feature::Caller::Call",
        TraceDirection::Callees,
    )
    .unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index(
        &db_path,
        "Demo::App::Feature::Caller::Call",
        TraceDirection::Callees,
    )
    .unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn traces_csharp_namespace_scoped_static_import_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper_path = dir.join("Helper.cs");
    let block_caller_path = dir.join("BlockCaller.cs");
    let file_caller_path = dir.join("FileCaller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper_path,
        "namespace Demo.Utility {
    class BlockHelpers { public static int Utility(int value) => value; }
    class FileHelpers { public static int Utility(int value) => value; }
}
",
    )
    .unwrap();
    fs::write(
        &block_caller_path,
        "namespace Demo.App {
    using static Demo.Utility.BlockHelpers;
    class BlockCaller { int Call() => Utility(1); }
}
namespace Demo.Other {
    class OutOfScopeCaller { int Call() => Utility(1); }
}
",
    )
    .unwrap();
    fs::write(
        &file_caller_path,
        "namespace Demo.File;
using static Demo.Utility.FileHelpers;
class FileCaller { int Call() => Utility(1); }
",
    )
    .unwrap();

    let block_target = "Demo::Utility::BlockHelpers::Utility";
    let live = trace_symbol_graph(&dir, block_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::BlockCaller::Call");

    let file_target = "Demo::Utility::FileHelpers::Utility";
    let live = trace_symbol_graph(&dir, file_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::File::FileCaller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, block_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "Demo::App::BlockCaller::Call"
    );

    let persisted =
        trace_symbol_graph_from_index(&db_path, file_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "Demo::File::FileCaller::Call"
    );
}

#[test]
fn does_not_trace_ambiguous_csharp_namespace_scoped_static_import_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Helpers.cs"),
        "namespace Demo.Utility {
    class First { public static int Utility(int value) => value; }
    class Second { public static int Utility(int value) => value; }
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo.App {
    using static Demo.Utility.First;
    using static Demo.Utility.Second;
    class Caller { int Call() => Utility(1); }
}
",
    )
    .unwrap();

    let live =
        trace_symbol_graph(&dir, "Demo::App::Caller::Call", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::App::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn traces_csharp_global_static_import_calls_from_directive_only_file_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Helper.cs"),
        "namespace Demo.Utility;
class Helper {
    public static int Utility(int value) => value;
    public static int Flexible(params int[] values) => values.Length;
    public int Instance(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("GlobalUsings.cs"),
        "global using static Demo.Utility.Helper;
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo.App;
class Caller {
    int Call() => Utility(1);
    int InstanceCall() => Instance(1);
    int ParamsCall() => Flexible(1);
}
",
    )
    .unwrap();

    let static_target = "Demo::Utility::Helper::Utility";
    let live = trace_symbol_graph(&dir, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::App::Caller::Call");

    for target in [
        "Demo::Utility::Helper::Instance",
        "Demo::Utility::Helper::Flexible",
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn does_not_trace_ambiguous_csharp_global_static_import_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("First.cs"),
        "namespace Demo.Utility; class First { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Second.cs"),
        "namespace Demo.Utility; class Second { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("FirstGlobal.cs"),
        "global using static Demo.Utility.First;
",
    )
    .unwrap();
    fs::write(
        dir.join("SecondGlobal.cs"),
        "global using static Demo.Utility.Second;
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo.App; class Caller { int Call() => Utility(1); }
",
    )
    .unwrap();

    let live =
        trace_symbol_graph(&dir, "Demo::App::Caller::Call", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::App::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn traces_csharp_global_namespace_import_calls_from_directive_only_file_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Helper.cs"),
        "namespace Demo.Utility;
class Helper {
    public static int Utility(int value) => value;
    public static int Flexible(params int[] values) => values.Length;
    public int Instance(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("GlobalUsings.cs"),
        "global using Demo.Utility;
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo.App;
class Caller {
    int Call() => Helper.Utility(1);
    int InstanceCall() => Helper.Instance(1);
    int ParamsCall() => Helper.Flexible(1);
}
",
    )
    .unwrap();

    let static_target = "Demo::Utility::Helper::Utility";
    let live = trace_symbol_graph(&dir, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::App::Caller::Call");

    for target in [
        "Demo::Utility::Helper::Instance",
        "Demo::Utility::Helper::Flexible",
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn does_not_trace_ambiguous_csharp_global_namespace_import_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("First.cs"),
        "namespace Demo.First; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Second.cs"),
        "namespace Demo.Second; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("FirstGlobal.cs"),
        "global using Demo.First;
",
    )
    .unwrap();
    fs::write(
        dir.join("SecondGlobal.cs"),
        "global using Demo.Second;
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo.App; class Caller { int Call() => Helper.Utility(1); }
",
    )
    .unwrap();

    let live =
        trace_symbol_graph(&dir, "Demo::App::Caller::Call", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::App::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn traces_csharp_global_type_alias_calls_from_directive_only_file_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Helper.cs"),
        "namespace Demo.Utility;
class Helper {
    public static int Utility(int value) => value;
    public static int Flexible(params int[] values) => values.Length;
    public int Instance(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("GlobalUsings.cs"),
        "global using HelperAlias = Demo.Utility.Helper;
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo.App;
class Caller {
    int Call() => HelperAlias.Utility(1);
    int InstanceCall() => HelperAlias.Instance(1);
    int ParamsCall() => HelperAlias.Flexible(1);
}
",
    )
    .unwrap();

    let static_target = "Demo::Utility::Helper::Utility";
    let live = trace_symbol_graph(&dir, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::App::Caller::Call");

    for target in [
        "Demo::Utility::Helper::Instance",
        "Demo::Utility::Helper::Flexible",
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn does_not_trace_ambiguous_csharp_global_type_alias_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("First.cs"),
        "namespace Demo.Utility; class First { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Second.cs"),
        "namespace Demo.Utility; class Second { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("FirstGlobal.cs"),
        "global using HelperAlias = Demo.Utility.First;
",
    )
    .unwrap();
    fs::write(
        dir.join("SecondGlobal.cs"),
        "global using HelperAlias = Demo.Utility.Second;
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo.App; class Caller { int Call() => HelperAlias.Utility(1); }
",
    )
    .unwrap();

    let live =
        trace_symbol_graph(&dir, "Demo::App::Caller::Call", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::App::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn traces_csharp_file_static_import_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper_path = dir.join("Helper.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper_path,
        "namespace Demo.Utility;
class Helper {
    public static int Utility(int value) => value;
    public static int Flexible(params int[] values) => values.Length;
    public int Instance(int value) => value;
}
class Unrelated { public static int Other(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "using static Demo.Utility.Helper;
using static Demo.Utility.Unrelated;
namespace Demo.App;
class Caller {
    int Call() => Utility(1);
    int InstanceCall() => Instance(1);
    int ParamsCall() => Flexible(1);
}
class LocalNameBlocksImport {
    int Utility() => 1;
    int Call() => Utility(1);
}
",
    )
    .unwrap();

    let static_target = "Demo::Utility::Helper::Utility";
    let live = trace_symbol_graph(&dir, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::App::Caller::Call");

    for target in [
        "Demo::Utility::Helper::Instance",
        "Demo::Utility::Helper::Flexible",
        "Demo::App::LocalNameBlocksImport::Utility",
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn does_not_trace_ambiguous_csharp_file_static_import_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("First.cs"),
        "namespace Demo.Utility; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Second.cs"),
        "namespace Demo.Utility; class Helper { public static int Utility(int value) => value; }
class Other { public static int Utility(int value) => value; }
class UniqueHelper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("AmbiguousType.cs"),
        "using static Demo.Utility.Helper;
namespace Demo.App; class AmbiguousType { int Call() => Utility(1); }
",
    )
    .unwrap();
    fs::write(
        dir.join("MultipleImports.cs"),
        "using static Demo.Utility.Other;
using static Demo.Utility.UniqueHelper;
namespace Demo.App; class MultipleImports { int Call() => Utility(1); }
",
    )
    .unwrap();
    fs::write(
        dir.join("DuplicateImport.cs"),
        "using static Demo.Utility.UniqueHelper;
using static Demo.Utility.UniqueHelper;
namespace Demo.App; class DuplicateImport { int Call() => Utility(1); }
",
    )
    .unwrap();

    for caller in [
        "Demo::App::AmbiguousType::Call",
        "Demo::App::MultipleImports::Call",
        "Demo::App::DuplicateImport::Call",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty());
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in [
        "Demo::App::AmbiguousType::Call",
        "Demo::App::MultipleImports::Call",
        "Demo::App::DuplicateImport::Call",
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(persisted.callees.is_empty());
    }
}

#[test]
fn traces_csharp_namespace_scoped_namespace_import_static_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let helper_path = dir.join("Helper.cs");
    let block_caller_path = dir.join("BlockCaller.cs");
    let file_caller_path = dir.join("FileCaller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper_path,
        "namespace Demo.Utility {
    class BlockHelper { public static int Utility(int value) => value; }
    class FileHelper { public static int Utility(int value) => value; }
}
",
    )
    .unwrap();
    fs::write(
        &block_caller_path,
        "namespace Demo.App {
    using Demo.Utility;
    class BlockCaller { int Call() => BlockHelper.Utility(1); }
}
namespace Demo.Other {
    class OutOfScopeCaller { int Call() => BlockHelper.Utility(1); }
}
",
    )
    .unwrap();
    fs::write(
        &file_caller_path,
        "namespace Demo.File;
using Demo.Utility;
class FileCaller { int Call() => FileHelper.Utility(1); }
",
    )
    .unwrap();

    let block_target = "Demo::Utility::BlockHelper::Utility";
    let live = trace_symbol_graph(&dir, block_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::BlockCaller::Call");

    let file_target = "Demo::Utility::FileHelper::Utility";
    let live = trace_symbol_graph(&dir, file_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::File::FileCaller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, block_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "Demo::App::BlockCaller::Call"
    );

    let persisted =
        trace_symbol_graph_from_index(&db_path, file_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "Demo::File::FileCaller::Call"
    );
}

#[test]
fn does_not_trace_ambiguous_csharp_namespace_scoped_namespace_import_static_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("First.cs"),
        "namespace Demo.First; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Second.cs"),
        "namespace Demo.Second; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo.App {
    using Demo.First;
    using Demo.Second;
    class Caller { int Call() => Helper.Utility(1); }
}
",
    )
    .unwrap();

    let live =
        trace_symbol_graph(&dir, "Demo::App::Caller::Call", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::App::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn traces_csharp_file_namespace_import_static_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper_path = dir.join("Helper.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper_path,
        "namespace Demo.Utility;
class Helper {
    public static int Utility(int value) => value;
    public static int Flexible(params int[] values) => values.Length;
    public int Instance(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "using Demo.Utility;
namespace Demo.App;
class Caller {
    int Call() => Helper.Utility(1);
    int InstanceCall() => Helper.Instance(1);
    int ParamsCall() => Helper.Flexible(1);
    int Shadowed(int Helper) => Helper.Utility(1);
}
",
    )
    .unwrap();

    let static_target = "Demo::Utility::Helper::Utility";
    let live = trace_symbol_graph(&dir, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::App::Caller::Call");

    for target in [
        "Demo::Utility::Helper::Instance",
        "Demo::Utility::Helper::Flexible",
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn handles_ambiguous_and_same_namespace_csharp_file_namespace_import_static_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Utility.cs"),
        "namespace Demo.Utility; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Shared.cs"),
        "namespace Demo.Shared; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Local.cs"),
        "namespace Demo.App; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("FirstDuplicate.cs"),
        "namespace Demo.Duplicate; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("SecondDuplicate.cs"),
        "namespace Demo.Duplicate; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("MultipleImports.cs"),
        "using Demo.Utility;
using Demo.Shared;
namespace Demo.Client; class MultipleImports { int Call() => Helper.Utility(1); }
",
    )
    .unwrap();
    fs::write(
        dir.join("DuplicateImport.cs"),
        "using Demo.Utility;
using Demo.Utility;
namespace Demo.Client; class DuplicateImport { int Call() => Helper.Utility(1); }
",
    )
    .unwrap();
    fs::write(
        dir.join("SameNamespace.cs"),
        "using Demo.Utility;
namespace Demo.App; class SameNamespace { int Call() => Helper.Utility(1); }
",
    )
    .unwrap();
    fs::write(
        dir.join("DuplicateType.cs"),
        "using Demo.Duplicate;
namespace Demo.Client; class DuplicateType { int Call() => Helper.Utility(1); }
",
    )
    .unwrap();

    for caller in [
        "Demo::Client::MultipleImports::Call",
        "Demo::Client::DuplicateImport::Call",
        "Demo::Client::DuplicateType::Call",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty());
    }

    let same_namespace_target = "Demo::App::Helper::Utility";
    let live = trace_symbol_graph(&dir, same_namespace_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::SameNamespace::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in [
        "Demo::Client::MultipleImports::Call",
        "Demo::Client::DuplicateImport::Call",
        "Demo::Client::DuplicateType::Call",
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(persisted.callees.is_empty());
    }

    let persisted =
        trace_symbol_graph_from_index(&db_path, same_namespace_target, TraceDirection::Callers)
            .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "Demo::App::SameNamespace::Call"
    );
}

#[test]
fn traces_java_explicit_this_method_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Counter.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;\nclass Counter {\n    int helper() { return 1; }\n    int caller() { return this.helper(); }\n}\n",
    )
    .unwrap();

    let helper_symbol = "com::example::Counter::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, helper_symbol);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Counter::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, helper_symbol);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "com::example::Counter::caller"
    );
}

#[test]
fn traces_java_explicit_local_static_import_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let helper_path = source_dir.join("Helper.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example;\nimport static com.example.Helper.utility;\nimport static com.example.Helper.instance;\nclass Main {\n    int caller() { return utility(1); }\n    int nonStatic() { return instance(1); }\n}\nclass Competing {\n    int utility(long value) { return (int) value; }\n    int caller() { return utility(1); }\n}\n",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package com.example;\nclass Helper {\n    static int utility(int value) { return value; }\n    int instance(int value) { return value; }\n}\n",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::utility";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, helper_symbol);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, helper_symbol);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");

    let instance_symbol = "com::example::Helper::instance";
    let live_instance = trace_symbol_graph(&dir, instance_symbol, TraceDirection::Callers).unwrap();
    assert!(live_instance.callers.is_empty());
    let persisted_instance =
        trace_symbol_graph_from_index(&db_path, instance_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted_instance.callers.is_empty());
}

#[test]
fn ignores_ambiguous_java_explicit_static_import_calls_in_live_and_persisted_indexes() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let first_path = source_dir.join("First.java");
    let second_path = source_dir.join("Second.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example;\nimport static com.example.First.utility;\nimport static com.example.Second.utility;\nclass Main { int caller() { return utility(1); } }\n",
    )
    .unwrap();
    fs::write(
        &first_path,
        "package com.example; class First { static int utility(int value) { return value; } }\n",
    )
    .unwrap();
    fs::write(
        &second_path,
        "package com.example; class Second { static int utility(int value) { return value; } }\n",
    )
    .unwrap();

    let first_symbol = "com::example::First::utility";
    let live = trace_symbol_graph(&dir, first_symbol, TraceDirection::Callers).unwrap();
    assert!(live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, first_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_explicit_local_import_static_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let helper_path = source_dir.join("Helper.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example;\nimport com.example.Helper;\nclass Main {\n    int caller() { return Helper.utility(1); }\n    int shadowed(Helper Helper) { return Helper.utility(1); }\n    int nonStatic() { return Helper.instance(1); }\n}\n",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package com.example;\nclass Helper {\n    static int utility(int value) { return value; }\n    int instance(int value) { return value; }\n}\n",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::utility";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, helper_symbol);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, helper_symbol);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");

    let instance_symbol = "com::example::Helper::instance";
    let live_instance = trace_symbol_graph(&dir, instance_symbol, TraceDirection::Callers).unwrap();
    assert!(live_instance.callers.is_empty());
    let persisted_instance =
        trace_symbol_graph_from_index(&db_path, instance_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted_instance.callers.is_empty());
}

#[test]
fn traces_go_unshadowed_same_file_direct_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\nfunc caller() int { return helper() }\nfunc helper() int { return 1 }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "helper", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, "helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    let position = Position { row: 3, column: 5 };
    let live_at_position =
        trace_symbol_graph_at_position(&dir, &source_path, &position, TraceDirection::Callers)
            .unwrap();
    assert_eq!(live_at_position.symbol.symbol_id, "helper");
    assert_eq!(live_at_position.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");

    let persisted_at_position = trace_symbol_graph_at_position_from_index(
        &db_path,
        &source_path,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted_at_position.symbol.symbol_id, "helper");
    assert_eq!(persisted_at_position.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_unshadowed_named_receiver_method_calls() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc (counter Counter) caller() int { return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Counter::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Counter::caller");
}

#[test]
fn traces_go_direct_pointer_and_generic_composite_literal_method_calls() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\ntype Box[T any] struct{}\nfunc (Counter) Value() int { return 1 }\nfunc (Box[T]) Value() int { return 2 }\nfunc pointerCaller() int { return (&Counter{}).Value() }\nfunc genericCaller() int { return Box[int]{}.Value() }\n",
    )
    .unwrap();

    let pointer_live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(pointer_live.callers.len(), 1);
    assert_eq!(pointer_live.callers[0].symbol_id, "pointerCaller");
    let generic_live = trace_symbol_graph(&dir, "Box::Value", TraceDirection::Callers).unwrap();
    assert_eq!(generic_live.callers.len(), 1);
    assert_eq!(generic_live.callers[0].symbol_id, "genericCaller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let pointer_persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(pointer_persisted.callers.len(), 1);
    assert_eq!(pointer_persisted.callers[0].symbol_id, "pointerCaller");
    let generic_persisted =
        trace_symbol_graph_from_index(&db_path, "Box::Value", TraceDirection::Callers).unwrap();
    assert_eq!(generic_persisted.callers.len(), 1);
    assert_eq!(generic_persisted.callers[0].symbol_id, "genericCaller");
}

#[test]
fn traces_go_explicit_type_conversion_method_receiver_forms() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Scalar int\ntype Box[T ~int] int\nfunc (Scalar) Value() int { return 1 }\nfunc (Box[T]) Value() int { return 2 }\nfunc pointerCaller(value *Scalar) int { return (*Scalar)(value).Value() }\nfunc parenthesizedCaller(value int) int { return (Scalar)(value).Value() }\nfunc genericCaller(value int) int { return Box[int](value).Value() }\n",
    )
    .unwrap();

    let scalar_live = trace_symbol_graph(&dir, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert_eq!(scalar_live.callers.len(), 2);
    assert_eq!(scalar_live.callers[0].symbol_id, "parenthesizedCaller");
    assert_eq!(scalar_live.callers[1].symbol_id, "pointerCaller");
    let box_live = trace_symbol_graph(&dir, "Box::Value", TraceDirection::Callers).unwrap();
    assert_eq!(box_live.callers.len(), 1);
    assert_eq!(box_live.callers[0].symbol_id, "genericCaller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let scalar_persisted =
        trace_symbol_graph_from_index(&db_path, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert_eq!(scalar_persisted.callers.len(), 2);
    assert_eq!(scalar_persisted.callers[0].symbol_id, "parenthesizedCaller");
    assert_eq!(scalar_persisted.callers[1].symbol_id, "pointerCaller");
    let box_persisted =
        trace_symbol_graph_from_index(&db_path, "Box::Value", TraceDirection::Callers).unwrap();
    assert_eq!(box_persisted.callers.len(), 1);
    assert_eq!(box_persisted.callers[0].symbol_id, "genericCaller");
}

#[test]
fn traces_go_explicit_type_conversion_method_receivers_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("scalar.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Scalar) Value() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Scalar int\nfunc caller(value *Scalar) int { return (*Scalar)(value).Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Scalar::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Scalar::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_type_conversion_methods_and_preserves_factory_call_edges() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Scalar int\ntype Alias = Scalar\ntype Chained = Alias\ntype LoopA = LoopB\ntype LoopB = LoopA\ntype Broken = Missing\ntype Result struct{}\nfunc (Scalar) Value() int { return 1 }\nfunc (Result) Value() int { return 2 }\nfunc Broken(value int) Result { return Result{} }\nfunc Factory(value int) Result { return Result{} }\nfunc aliasConversionCaller(value int) int { return Alias(value).Value() }\nfunc chainedConversionCaller(value int) int { return Chained(value).Value() }\nfunc conversionCaller(value int) int { return Scalar(value).Value() }\nfunc factoryCaller(value int) int { return Factory(value).Value() }\nfunc loopConversionCaller(value int) int { return LoopA(value).Value() }\nfunc brokenConversionCaller(value int) int { return Broken(value).Value() }\nfunc parenthesizedFactoryCaller(value int) int { return (Factory)(value).Value() }\n",
    )
    .unwrap();

    let conversion_live =
        trace_symbol_graph(&dir, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert_eq!(conversion_live.callers.len(), 3);
    assert_eq!(
        conversion_live.callers[0].symbol_id,
        "aliasConversionCaller"
    );
    assert_eq!(
        conversion_live.callers[1].symbol_id,
        "chainedConversionCaller"
    );
    assert_eq!(conversion_live.callers[2].symbol_id, "conversionCaller");
    let factory_live = trace_symbol_graph(&dir, "Factory", TraceDirection::Callers).unwrap();
    assert_eq!(factory_live.callers.len(), 2);
    assert_eq!(factory_live.callers[0].symbol_id, "factoryCaller");
    assert_eq!(
        factory_live.callers[1].symbol_id,
        "parenthesizedFactoryCaller"
    );
    let broken_live = trace_symbol_graph(&dir, "Broken", TraceDirection::Callers).unwrap();
    assert!(broken_live.callers.is_empty());
    let result_method_live =
        trace_symbol_graph(&dir, "Result::Value", TraceDirection::Callers).unwrap();
    assert!(result_method_live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let conversion_persisted =
        trace_symbol_graph_from_index(&db_path, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert_eq!(conversion_persisted.callers.len(), 3);
    assert_eq!(
        conversion_persisted.callers[0].symbol_id,
        "aliasConversionCaller"
    );
    assert_eq!(
        conversion_persisted.callers[1].symbol_id,
        "chainedConversionCaller"
    );
    assert_eq!(
        conversion_persisted.callers[2].symbol_id,
        "conversionCaller"
    );
    let factory_persisted =
        trace_symbol_graph_from_index(&db_path, "Factory", TraceDirection::Callers).unwrap();
    assert_eq!(factory_persisted.callers.len(), 2);
    assert_eq!(factory_persisted.callers[0].symbol_id, "factoryCaller");
    assert_eq!(
        factory_persisted.callers[1].symbol_id,
        "parenthesizedFactoryCaller"
    );
    let broken_persisted =
        trace_symbol_graph_from_index(&db_path, "Broken", TraceDirection::Callers).unwrap();
    assert!(broken_persisted.callers.is_empty());
    let result_method_persisted =
        trace_symbol_graph_from_index(&db_path, "Result::Value", TraceDirection::Callers).unwrap();
    assert!(result_method_persisted.callers.is_empty());
}

#[test]
fn traces_go_simple_type_alias_method_calls() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\ntype Alias = Counter\ntype Chained = Alias\ntype LoopA = LoopB\ntype LoopB = LoopA\nfunc (Counter) Value() int { return 1 }\nfunc compositeCaller() int { return Alias{}.Value() }\nfunc parameterCaller(value Alias) int { return value.Value() }\nfunc localCaller() int { value := Alias{}; return value.Value() }\nfunc chainedCaller() int { return Chained{}.Value() }\nfunc loopCaller() int { return LoopA{}.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 4);
    assert_eq!(live.callers[0].symbol_id, "chainedCaller");
    assert_eq!(live.callers[1].symbol_id, "compositeCaller");
    assert_eq!(live.callers[2].symbol_id, "localCaller");
    assert_eq!(live.callers[3].symbol_id, "parameterCaller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 4);
    assert_eq!(persisted.callers[0].symbol_id, "chainedCaller");
    assert_eq!(persisted.callers[1].symbol_id, "compositeCaller");
    assert_eq!(persisted.callers[2].symbol_id, "localCaller");
    assert_eq!(persisted.callers[3].symbol_id, "parameterCaller");
}

#[test]
fn traces_go_simple_type_alias_method_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("counter.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Counter) Value() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Counter struct{}\ntype Alias = Counter\ntype Chained = Alias\nfunc caller() int { return Chained{}.Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_type_assertion_methods_and_does_not_fallback_to_functions() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Scalar int\ntype Alias = Scalar\ntype Chained = Alias\ntype LoopA = LoopB\ntype LoopB = LoopA\ntype Result struct{}\nfunc (Scalar) Value() int { return 1 }\nfunc (Result) Value() int { return 2 }\nfunc Factory(value int) Result { return Result{} }\nfunc aliasAssertionCaller(value any) int { return value.(Alias).Value() }\nfunc assertionCaller(value any) int { return value.(Scalar).Value() }\nfunc chainedAssertionCaller(value any) int { return value.(Chained).Value() }\nfunc invalidFactoryAssertion(value any) int { return value.(Factory).Value() }\nfunc loopAssertionCaller(value any) int { return value.(LoopA).Value() }\n",
    )
    .unwrap();

    let assertion_live =
        trace_symbol_graph(&dir, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert_eq!(assertion_live.callers.len(), 3);
    assert_eq!(assertion_live.callers[0].symbol_id, "aliasAssertionCaller");
    assert_eq!(assertion_live.callers[1].symbol_id, "assertionCaller");
    assert_eq!(
        assertion_live.callers[2].symbol_id,
        "chainedAssertionCaller"
    );
    let factory_live = trace_symbol_graph(&dir, "Factory", TraceDirection::Callers).unwrap();
    assert!(factory_live.callers.is_empty());
    let result_method_live =
        trace_symbol_graph(&dir, "Result::Value", TraceDirection::Callers).unwrap();
    assert!(result_method_live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let assertion_persisted =
        trace_symbol_graph_from_index(&db_path, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert_eq!(assertion_persisted.callers.len(), 3);
    assert_eq!(
        assertion_persisted.callers[0].symbol_id,
        "aliasAssertionCaller"
    );
    assert_eq!(assertion_persisted.callers[1].symbol_id, "assertionCaller");
    assert_eq!(
        assertion_persisted.callers[2].symbol_id,
        "chainedAssertionCaller"
    );
    let factory_persisted =
        trace_symbol_graph_from_index(&db_path, "Factory", TraceDirection::Callers).unwrap();
    assert!(factory_persisted.callers.is_empty());
    let result_method_persisted =
        trace_symbol_graph_from_index(&db_path, "Result::Value", TraceDirection::Callers).unwrap();
    assert!(result_method_persisted.callers.is_empty());
}

#[test]
fn traces_go_type_assertion_methods_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("scalar.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Scalar) Value() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Scalar int\ntype Alias = Scalar\ntype Chained = Alias\nfunc caller(value any) int { return value.(Chained).Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Scalar::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Scalar::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn fails_closed_for_go_type_conversion_methods_with_ambiguous_type_declarations() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let duplicate_type_path = dir.join("duplicate.go");
    let method_path = dir.join("scalar.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\ntype Scalar int\nfunc caller(value int) int { return Scalar(value).Value() }\nfunc assertionCaller(value any) int { return value.(Scalar).Value() }\n",
    )
    .unwrap();
    fs::write(&duplicate_type_path, "package metrics\n\ntype Scalar int\n").unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Scalar) Value() int { return 1 }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert!(live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_go_type_conversion_methods_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("scalar.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Scalar) Value() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Scalar int\ntype Alias = Scalar\ntype Chained = Alias\nfunc caller(value int) int { return Chained(value).Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Scalar::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Scalar::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_unshadowed_function_body_local_variable_method_calls() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc caller() int { counter := Counter{}; return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_local_variable_method_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("counter.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Counter) Value() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Counter struct{}\nfunc caller() int { counter := Counter{}; return counter.Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_unshadowed_named_parameter_method_calls() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc caller(counter *Counter) int { return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_same_file_direct_composite_literal_method_calls() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc caller() int { return Counter{}.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, "Counter::Value");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, "Counter::Value");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_same_package_direct_composite_literal_method_calls_across_source_files() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("counter.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\ntype Counter struct{}\nfunc caller() int { return Counter{}.Value() }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Counter) Value() int { return 1 }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, "Counter::Value");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, "Counter::Value");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_same_package_composite_literal_method_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("counter.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Counter) Value() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Counter struct{}\nfunc caller() int { return Counter{}.Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_local_package_imported_function_calls_in_live_and_persisted_indexes() {
    let dir = temporary_dir();
    let caller_path = dir.join("cmd").join("main.go");
    let service_path = dir.join("internal").join("service").join("service.go");
    let utility_path = dir.join("internal").join("utility").join("utility.go");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(service_path.parent().unwrap()).unwrap();
    fs::create_dir_all(utility_path.parent().unwrap()).unwrap();
    fs::write(dir.join("go.mod"), "module example.com/project\n").unwrap();
    fs::write(
        &caller_path,
        "package main\n\nimport (\n    \"example.com/project/internal/service\"\n    utility_alias \"example.com/project/internal/utility\"\n)\n\ntype local struct{}\nfunc (local) Value() int { return 0 }\nfunc caller() int { return service.Value() + utility_alias.Other() }\nfunc shadowed() int { service := local{}; return service.Value() }\n",
    )
    .unwrap();
    fs::write(
        &service_path,
        "package service\n\nfunc Value() int { return 1 }\n",
    )
    .unwrap();
    fs::write(
        &utility_path,
        "package utility\n\nfunc Other() int { return 2 }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.symbol.symbol_id, "Value");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.symbol.symbol_id, "Value");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");

    let position = Position { row: 2, column: 5 };
    let persisted_at_position = trace_symbol_graph_at_position_from_index(
        &db_path,
        &service_path,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted_at_position.symbol.symbol_id, "Value");
    assert_eq!(persisted_at_position.callers.len(), 1);
    assert_eq!(persisted_at_position.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_same_package_direct_calls_across_source_files() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let helper_path = dir.join("helper.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc caller() int { return helper() }\n",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package metrics\n\nfunc helper() int { return 1 }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "helper", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_same_package_direct_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let helper_path = dir.join("helper.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package metrics\n\nfunc helper() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\nfunc caller() int { return helper() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn does_not_trace_go_same_directory_calls_across_different_or_test_packages() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let helper_path = dir.join("helper_test.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc caller() int { return helper() }\n",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package metrics_test\n\nfunc helper() int { return 1 }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "helper", TraceDirection::Callers).unwrap();
    assert!(live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}
