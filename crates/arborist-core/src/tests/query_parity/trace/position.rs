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
fn traces_java_same_file_multilevel_inherited_methods() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Grand { int helper() { return 1; } }
class Base extends Grand {}
class Child extends Base {
    int bareCaller() { return helper(); }
    int superCaller() { return super.helper(); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Grand::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    assert_eq!(
        live.callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Child::bareCaller",
            "com::example::Child::superCaller"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Child::bareCaller",
            "com::example::Child::superCaller"
        ]
    );
}

#[test]
fn traces_java_same_file_multilevel_inherited_methods_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Grand { int helper() { return 1; } }
class Base extends Grand {}
class Child extends Base { int caller() { return helper(); } }
";
    let helper_symbol = "com::example::Grand::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Child::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "com::example::Child::caller"
    );
}

#[test]
fn ignores_cyclic_java_same_file_inheritance() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class First extends Second {}
class Second extends First {}
class Child extends First { int caller() { return helper(); } }
",
    )
    .unwrap();

    let live =
        trace_symbol_graph(&dir, "com::example::Child::caller", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index(
        &db_path,
        "com::example::Child::caller",
        TraceDirection::Callees,
    )
    .unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn traces_java_same_package_simple_superclasses_across_files() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let grand_path = source_dir.join("Grand.java");
    let base_path = source_dir.join("Base.java");
    let child_path = source_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &grand_path,
        "package com.example; class Grand { int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &base_path,
        "package com.example; class Base extends Grand { Base() {} Base(int value) {} Base(int... values) {} }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.example; class Child extends Base { Child() { super(); } Child(int value) { super(value); } Child(boolean first, boolean second) { super(1, 2); } int bareCaller() { return helper(); } int superCaller() { return super.helper(); } }
",
    )
    .unwrap();

    let base_file_path = normalize_path(&base_path);
    let child_file_path = normalize_path(&child_path);
    let base_zero = format!("{base_file_path}::com::example::Base::Base#overload[1]");
    let base_one = format!("{base_file_path}::com::example::Base::Base#overload[2]");
    let base_params = format!("{base_file_path}::com::example::Base::Base#overload[3]");
    let child_zero = format!("{child_file_path}::com::example::Child::Child#overload[1]");
    let child_one = format!("{child_file_path}::com::example::Child::Child#overload[2]");
    for (target, caller) in [
        (base_zero.as_str(), child_zero.as_str()),
        (base_one.as_str(), child_one.as_str()),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert_eq!(live.callers.len(), 1);
        assert_eq!(live.callers[0].symbol_id, caller);
    }
    let live_params = trace_symbol_graph(&dir, &base_params, TraceDirection::Callers).unwrap();
    assert!(live_params.callers.is_empty());
    let helper_live =
        trace_symbol_graph(&dir, "com::example::Grand::helper", TraceDirection::Callers).unwrap();
    assert_eq!(helper_live.callers.len(), 2);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (target, caller) in [
        (base_zero.as_str(), child_zero.as_str()),
        (base_one.as_str(), child_one.as_str()),
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert_eq!(persisted.callers.len(), 1);
        assert_eq!(persisted.callers[0].symbol_id, caller);
    }
    let persisted_params =
        trace_symbol_graph_from_index(&db_path, &base_params, TraceDirection::Callers).unwrap();
    assert!(persisted_params.callers.is_empty());
    let helper_persisted = trace_symbol_graph_from_index(
        &db_path,
        "com::example::Grand::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(helper_persisted.callers.len(), 2);
}

#[test]
fn traces_java_same_package_simple_superclasses_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let base_path = source_dir.join("Base.java");
    let child_path = source_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &base_path,
        "package com.example; class Base { Base() {} int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example; class Child extends Base { Child() { super(); } int caller() { return helper(); } }
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &child_path,
        overlay,
        "com::example::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Child::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &child_path,
        overlay,
        "com::example::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Child::Child");
}

#[test]
fn traces_java_explicit_imported_outer_superclasses_across_files() {
    let dir = temporary_dir();
    let outer_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let outer_path = outer_dir.join("Outer.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&outer_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &outer_path,
        "package com.base; class Outer { static class Base { Base() {} int helper() { return 1; } } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; import com.base.Outer; class Child extends Outer.Base { Child() { super(); } int caller() { return super.helper(); } }
",
    )
    .unwrap();

    let constructor_live = trace_symbol_graph(
        &dir,
        "com::base::Outer::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(constructor_live.callers.len(), 1);
    assert_eq!(
        constructor_live.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_live = trace_symbol_graph(
        &dir,
        "com::base::Outer::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(helper_live.callers.len(), 1);
    assert_eq!(
        helper_live.callers[0].symbol_id,
        "com::child::Child::caller"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let constructor_persisted = trace_symbol_graph_from_index(
        &db_path,
        "com::base::Outer::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(constructor_persisted.callers.len(), 1);
    assert_eq!(
        constructor_persisted.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_persisted = trace_symbol_graph_from_index(
        &db_path,
        "com::base::Outer::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(helper_persisted.callers.len(), 1);
    assert_eq!(
        helper_persisted.callers[0].symbol_id,
        "com::child::Child::caller"
    );
}

#[test]
fn traces_java_same_package_outer_superclasses_across_files() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let outer_path = source_dir.join("Outer.java");
    let child_path = source_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &outer_path,
        "package com.example; class Outer { static class Base { Base() {} int helper() { return 1; } } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.example; class Child extends Outer.Base { Child() { super(); } int caller() { return super.helper(); } }
",
    )
    .unwrap();

    let constructor_live = trace_symbol_graph(
        &dir,
        "com::example::Outer::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(constructor_live.callers.len(), 1);
    assert_eq!(
        constructor_live.callers[0].symbol_id,
        "com::example::Child::Child"
    );
    let helper_live = trace_symbol_graph(
        &dir,
        "com::example::Outer::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(helper_live.callers.len(), 1);
    assert_eq!(
        helper_live.callers[0].symbol_id,
        "com::example::Child::caller"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let constructor_persisted = trace_symbol_graph_from_index(
        &db_path,
        "com::example::Outer::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(constructor_persisted.callers.len(), 1);
    assert_eq!(
        constructor_persisted.callers[0].symbol_id,
        "com::example::Child::Child"
    );
    let helper_persisted = trace_symbol_graph_from_index(
        &db_path,
        "com::example::Outer::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(helper_persisted.callers.len(), 1);
    assert_eq!(
        helper_persisted.callers[0].symbol_id,
        "com::example::Child::caller"
    );
}

#[test]
fn traces_java_explicit_local_import_simple_generic_superclasses_across_files() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let base_path = base_dir.join("Base.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &base_path,
        "package com.base; class Base<T> { Base() {} int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; import com.base.Base; class Child extends Base<String> { Child() { super(); } int caller() { return super.helper(); } }
",
    )
    .unwrap();

    let constructor_live =
        trace_symbol_graph(&dir, "com::base::Base::Base", TraceDirection::Callers).unwrap();
    assert_eq!(constructor_live.callers.len(), 1);
    assert_eq!(
        constructor_live.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_live =
        trace_symbol_graph(&dir, "com::base::Base::helper", TraceDirection::Callers).unwrap();
    assert_eq!(helper_live.callers.len(), 1);
    assert_eq!(
        helper_live.callers[0].symbol_id,
        "com::child::Child::caller"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let constructor_persisted =
        trace_symbol_graph_from_index(&db_path, "com::base::Base::Base", TraceDirection::Callers)
            .unwrap();
    assert_eq!(constructor_persisted.callers.len(), 1);
    assert_eq!(
        constructor_persisted.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_persisted =
        trace_symbol_graph_from_index(&db_path, "com::base::Base::helper", TraceDirection::Callers)
            .unwrap();
    assert_eq!(helper_persisted.callers.len(), 1);
    assert_eq!(
        helper_persisted.callers[0].symbol_id,
        "com::child::Child::caller"
    );
}

#[test]
fn traces_java_qualified_generic_superclasses_across_files() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let base_path = base_dir.join("Base.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &base_path,
        "package com.base; class Base<T> { Base() {} int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; class Child extends com.base.Base<String> { Child() { super(); } int caller() { return super.helper(); } }
",
    )
    .unwrap();

    let constructor_live =
        trace_symbol_graph(&dir, "com::base::Base::Base", TraceDirection::Callers).unwrap();
    assert_eq!(constructor_live.callers.len(), 1);
    assert_eq!(
        constructor_live.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_live =
        trace_symbol_graph(&dir, "com::base::Base::helper", TraceDirection::Callers).unwrap();
    assert_eq!(helper_live.callers.len(), 1);
    assert_eq!(
        helper_live.callers[0].symbol_id,
        "com::child::Child::caller"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let constructor_persisted =
        trace_symbol_graph_from_index(&db_path, "com::base::Base::Base", TraceDirection::Callers)
            .unwrap();
    assert_eq!(constructor_persisted.callers.len(), 1);
    assert_eq!(
        constructor_persisted.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_persisted =
        trace_symbol_graph_from_index(&db_path, "com::base::Base::helper", TraceDirection::Callers)
            .unwrap();
    assert_eq!(helper_persisted.callers.len(), 1);
    assert_eq!(
        helper_persisted.callers[0].symbol_id,
        "com::child::Child::caller"
    );
}

#[test]
fn traces_java_qualified_simple_superclasses_across_files() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let base_path = base_dir.join("Base.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &base_path,
        "package com.base; class Base { Base() {} int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; class Child extends com.base.Base { Child() { super(); } int caller() { return super.helper(); } }
",
    )
    .unwrap();

    let constructor_live =
        trace_symbol_graph(&dir, "com::base::Base::Base", TraceDirection::Callers).unwrap();
    assert_eq!(constructor_live.callers.len(), 1);
    assert_eq!(
        constructor_live.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_live =
        trace_symbol_graph(&dir, "com::base::Base::helper", TraceDirection::Callers).unwrap();
    assert_eq!(helper_live.callers.len(), 1);
    assert_eq!(
        helper_live.callers[0].symbol_id,
        "com::child::Child::caller"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let constructor_persisted =
        trace_symbol_graph_from_index(&db_path, "com::base::Base::Base", TraceDirection::Callers)
            .unwrap();
    assert_eq!(constructor_persisted.callers.len(), 1);
    assert_eq!(
        constructor_persisted.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_persisted =
        trace_symbol_graph_from_index(&db_path, "com::base::Base::helper", TraceDirection::Callers)
            .unwrap();
    assert_eq!(helper_persisted.callers.len(), 1);
    assert_eq!(
        helper_persisted.callers[0].symbol_id,
        "com::child::Child::caller"
    );
}

#[test]
fn traces_java_explicit_local_import_simple_superclasses_across_files() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let base_path = base_dir.join("Base.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &base_path,
        "package com.base; class Base { Base() {} int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; import com.base.Base; class Child extends Base { Child() { super(); } int caller() { return super.helper(); } }
",
    )
    .unwrap();

    let constructor_live =
        trace_symbol_graph(&dir, "com::base::Base::Base", TraceDirection::Callers).unwrap();
    assert_eq!(constructor_live.callers.len(), 1);
    assert_eq!(
        constructor_live.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_live =
        trace_symbol_graph(&dir, "com::base::Base::helper", TraceDirection::Callers).unwrap();
    assert_eq!(helper_live.callers.len(), 1);
    assert_eq!(
        helper_live.callers[0].symbol_id,
        "com::child::Child::caller"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let constructor_persisted =
        trace_symbol_graph_from_index(&db_path, "com::base::Base::Base", TraceDirection::Callers)
            .unwrap();
    assert_eq!(constructor_persisted.callers.len(), 1);
    assert_eq!(
        constructor_persisted.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_persisted =
        trace_symbol_graph_from_index(&db_path, "com::base::Base::helper", TraceDirection::Callers)
            .unwrap();
    assert_eq!(helper_persisted.callers.len(), 1);
    assert_eq!(
        helper_persisted.callers[0].symbol_id,
        "com::child::Child::caller"
    );
}

#[test]
fn traces_java_explicit_imported_outer_superclasses_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let outer_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let outer_path = outer_dir.join("Outer.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&outer_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &outer_path,
        "package com.base; class Outer { static class Base { Base() {} int helper() { return 1; } } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.child; import com.base.Outer; class Child extends Outer.Base { Child() { super(); } int caller() { return helper(); } }
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &child_path,
        overlay,
        "com::base::Outer::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::child::Child::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &child_path,
        overlay,
        "com::base::Outer::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::child::Child::Child");
}

#[test]
fn traces_java_same_package_outer_superclasses_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let outer_path = source_dir.join("Outer.java");
    let child_path = source_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &outer_path,
        "package com.example; class Outer { static class Base { Base() {} int helper() { return 1; } } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example; class Child extends Outer.Base { Child() { super(); } int caller() { return helper(); } }
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &child_path,
        overlay,
        "com::example::Outer::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Child::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &child_path,
        overlay,
        "com::example::Outer::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Child::Child");
}

#[test]
fn traces_java_explicit_local_import_simple_generic_superclasses_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let base_path = base_dir.join("Base.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &base_path,
        "package com.base; class Base<T> { Base() {} int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.child; import com.base.Base; class Child extends Base<String> { Child() { super(); } int caller() { return helper(); } }
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &child_path,
        overlay,
        "com::base::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::child::Child::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &child_path,
        overlay,
        "com::base::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::child::Child::Child");
}

#[test]
fn traces_java_qualified_generic_superclasses_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let base_path = base_dir.join("Base.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &base_path,
        "package com.base; class Base<T> { Base() {} int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.child; class Child extends com.base.Base<String> { Child() { super(); } int caller() { return helper(); } }
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &child_path,
        overlay,
        "com::base::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::child::Child::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &child_path,
        overlay,
        "com::base::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::child::Child::Child");
}

#[test]
fn traces_java_qualified_simple_superclasses_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let base_path = base_dir.join("Base.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &base_path,
        "package com.base; class Base { Base() {} int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.child; class Child extends com.base.Base { Child() { super(); } int caller() { return helper(); } }
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &child_path,
        overlay,
        "com::base::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::child::Child::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &child_path,
        overlay,
        "com::base::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::child::Child::Child");
}

#[test]
fn traces_java_explicit_local_import_simple_superclasses_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let base_path = base_dir.join("Base.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &base_path,
        "package com.base; class Base { Base() {} int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.child; import com.base.Base; class Child extends Base { Child() { super(); } int caller() { return helper(); } }
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &child_path,
        overlay,
        "com::base::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::child::Child::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &child_path,
        overlay,
        "com::base::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::child::Child::Child");
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

    // A property receiver named `GlobalHelper` of declared type `GlobalHelper`
    // dispatches `GlobalHelper.Instance(1)` to the instance method, while the
    // unbound same-named type interpretation from `SimpleCaller` stays static
    // and the `params` method is never reached through an instance call.
    let instance_live =
        trace_symbol_graph(&dir, "GlobalHelper::Instance", TraceDirection::Callers).unwrap();
    assert_eq!(instance_live.callers.len(), 1);
    assert_eq!(
        instance_live.callers[0].symbol_id,
        "MemberShadowCaller::MemberShadow"
    );
    let instance_persisted =
        trace_symbol_graph_from_index(&db_path, "GlobalHelper::Instance", TraceDirection::Callers)
            .unwrap();
    assert_eq!(instance_persisted.callers.len(), 1);
    assert_eq!(
        instance_persisted.callers[0].symbol_id,
        "MemberShadowCaller::MemberShadow"
    );
    let flexible_live =
        trace_symbol_graph(&dir, "GlobalHelper::Flexible", TraceDirection::Callers).unwrap();
    assert!(flexible_live.callers.is_empty());
    let flexible_persisted =
        trace_symbol_graph_from_index(&db_path, "GlobalHelper::Flexible", TraceDirection::Callers)
            .unwrap();
    assert!(flexible_persisted.callers.is_empty());
}

#[test]
fn traces_csharp_typed_receiver_instance_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
class Base {
    public int BaseHelper(int value) => value;
}
class Counter : Base {
    public int Helper(int value) => value;
    public static int Utility(int value) => value;
    public int Flexible(params int[] values) => values.Length;
}
class Box<T> {
    public T Get() => default;
}
class GlobalHelper {
    public int Instance(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "using Demo.Utility;
namespace Demo;
class Caller {
    int ParameterReceiver(Counter counter) => counter.Helper(1);
    int InheritedReceiver(Counter counter) => counter.BaseHelper(1);
    int LocalReceiver() { Counter local = new Counter(); return local.Helper(1); }
    int StaticReceiver(Counter counter) => counter.Utility(1);
    int ParamsReceiver(Counter counter) => counter.Flexible(1);
    int PrimitiveShadow(int Counter) => Counter.Helper(1);
    int GenericReceiver(Box<int> box) => box.Get();
}
",
    )
    .unwrap();
    fs::write(
        dir.join("FieldCaller.cs"),
        "namespace Demo;
class FieldCaller {
    Counter field = new Counter();
    GlobalHelper GlobalHelper { get; } = new GlobalHelper();
    int FieldReceiver() => field.Helper(1);
    int PropertyReceiver() => GlobalHelper.Instance(1);
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Worker.cs"),
        "namespace Demo.Utility;
class Worker {
    public int Run(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("ImportedCaller.cs"),
        "using Demo.Utility;
namespace Demo.App;
class ImportedCaller {
    int ImportedReceiver(Worker worker) => worker.Run(1);
}
",
    )
    .unwrap();

    for (target, callers) in [
        (
            "Demo::Counter::Helper",
            vec![
                "Demo::Caller::LocalReceiver",
                "Demo::Caller::ParameterReceiver",
                "Demo::FieldCaller::FieldReceiver",
            ],
        ),
        (
            "Demo::Base::BaseHelper",
            vec!["Demo::Caller::InheritedReceiver"],
        ),
        ("Demo::Box::Get", vec!["Demo::Caller::GenericReceiver"]),
        (
            "Demo::GlobalHelper::Instance",
            vec!["Demo::FieldCaller::PropertyReceiver"],
        ),
        (
            "Demo::Utility::Worker::Run",
            vec!["Demo::App::ImportedCaller::ImportedReceiver"],
        ),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert_eq!(
            live.callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            callers,
            "{target}"
        );
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert_eq!(
            persisted
                .callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            callers,
            "{target}"
        );
    }

    for target in ["Demo::Counter::Utility", "Demo::Counter::Flexible"] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty(), "{target}");
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty(), "{target}");
    }
}

#[test]
fn fails_closed_on_csharp_unusable_instance_receiver_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Counter.cs"),
        "namespace Demo;
class Counter {
    public int Helper(int value) => value;
    public static int Utility(int value) => value;
}
class ShadowCaller {
    int PrimitiveShadow(int Counter) => Counter.Helper(1);
    int VarShadow() { var Counter = new Counter(); return Counter.Helper(1); }
    int LambdaShadow() {
        System.Func<Counter, int> f = Counter => Counter.Helper(1);
        return f(new Counter());
    }
    int StaticThroughInstance(Counter counter) => counter.Utility(1);
    int MissingMethod(Counter counter) => counter.Nope(1);
    int FactoryShadow() {
        var Counter = MakeCounter();
        return Counter.Helper(1);
    }
    int MakeCounter() => 1;
}
",
    )
    .unwrap();

    for caller in [
        "Demo::ShadowCaller::PrimitiveShadow",
        "Demo::ShadowCaller::LambdaShadow",
        "Demo::ShadowCaller::StaticThroughInstance",
        "Demo::ShadowCaller::MissingMethod",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty(), "{caller}");
    }
    // `var Counter = new Counter()` now infers the constructed type, so
    // `VarShadow` resolves the instance call; every other shadow scenario
    // stays bound without a usable type and fails closed.
    let helper_live =
        trace_symbol_graph(&dir, "Demo::Counter::Helper", TraceDirection::Callers).unwrap();
    assert_eq!(helper_live.callers.len(), 1);
    assert_eq!(
        helper_live.callers[0].symbol_id,
        "Demo::ShadowCaller::VarShadow"
    );
    let utility_live =
        trace_symbol_graph(&dir, "Demo::Counter::Utility", TraceDirection::Callers).unwrap();
    assert!(utility_live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let helper_persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::Counter::Helper", TraceDirection::Callers)
            .unwrap();
    assert_eq!(helper_persisted.callers.len(), 1);
    assert_eq!(
        helper_persisted.callers[0].symbol_id,
        "Demo::ShadowCaller::VarShadow"
    );
    let utility_persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::Counter::Utility", TraceDirection::Callers)
            .unwrap();
    assert!(utility_persisted.callers.is_empty());
}

#[test]
fn traces_csharp_var_constructor_receiver_instance_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
    public static int Utility(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    int ConstructorReceiver() { var helper = new Helper(); return helper.Run(1); }
    int DottedConstructorReceiver() { var helper = new Demo.Helper(); return helper.Run(1); }
    int FactoryReceiver() { var helper = MakeHelper(); return helper.Run(1); }
    int StaticThroughConstructor() { var helper = new Helper(); return helper.Utility(1); }
    int UnknownConstructorReceiver() { var helper = new NotIndexed(); return helper.Run(1); }
    Helper MakeHelper() => new Helper();
}
",
    )
    .unwrap();

    let run_target = "Demo::Helper::Run";
    let live = trace_symbol_graph(&dir, run_target, TraceDirection::Callers).unwrap();
    assert_eq!(
        live.callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Demo::Caller::ConstructorReceiver",
            "Demo::Caller::DottedConstructorReceiver",
            "Demo::Caller::FactoryReceiver"
        ]
    );
    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, run_target, TraceDirection::Callers).unwrap();
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Demo::Caller::ConstructorReceiver",
            "Demo::Caller::DottedConstructorReceiver",
            "Demo::Caller::FactoryReceiver"
        ]
    );

    // A `var` local from a factory initializer infers its receiver type from
    // the factory's declared return type, so `FactoryReceiver` calls both the
    // same-type factory and the receiver method; the static method and an
    // unknown constructed type still fail closed.
    let utility_live =
        trace_symbol_graph(&dir, "Demo::Helper::Utility", TraceDirection::Callers).unwrap();
    assert!(utility_live.callers.is_empty());
    let factory_live = trace_symbol_graph(
        &dir,
        "Demo::Caller::FactoryReceiver",
        TraceDirection::Callees,
    )
    .unwrap();
    assert_eq!(factory_live.callees.len(), 2);
    assert_eq!(
        factory_live.callees[0].symbol_id,
        "Demo::Caller::MakeHelper"
    );
    assert_eq!(factory_live.callees[1].symbol_id, "Demo::Helper::Run");
    let unknown_live = trace_symbol_graph(
        &dir,
        "Demo::Caller::UnknownConstructorReceiver",
        TraceDirection::Callees,
    )
    .unwrap();
    assert!(unknown_live.callees.is_empty());
}

#[test]
fn traces_csharp_var_constructor_receiver_instance_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper_path = dir.join("Helper.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper_path,
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace Demo; class Stale {}
",
    )
    .unwrap();
    let overlay = "namespace Demo;
class Caller {
    int Call() { var helper = new Helper(); return helper.Run(1); }
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "Demo::Helper::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "Demo::Helper::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::Caller::Call");
}

#[test]
fn traces_csharp_typed_receiver_instance_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper_path = dir.join("Counter.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper_path,
        "namespace Demo;
class Counter {
    public int Helper(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace Demo; class Stale {}
",
    )
    .unwrap();
    let overlay = "namespace Demo;
class Caller {
    int Call(Counter counter) => counter.Helper(1);
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "Demo::Counter::Helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "Demo::Counter::Helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::Caller::Call");
}

#[test]
fn traces_csharp_interface_receiver_instance_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
interface IWorker {
    int Run(int value);
    static int Utility(int value) => value;
}
class Worker : IWorker {
    public int Run(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    int ParameterReceiver(IWorker worker) => worker.Run(1);
    int LocalReceiver() { IWorker worker = new Worker(); return worker.Run(1); }
    IWorker field = new Worker();
    int FieldReceiver() => field.Run(1);
    int ImportedReceiver(Demo.IWorker worker) => worker.Run(1);
    int StaticThroughInterface(IWorker worker) => worker.Utility(1);
}
",
    )
    .unwrap();

    let target = "Demo::IWorker::Run";
    let callers = [
        "Demo::Caller::FieldReceiver",
        "Demo::Caller::ImportedReceiver",
        "Demo::Caller::LocalReceiver",
        "Demo::Caller::ParameterReceiver",
    ];
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(
        live.callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        callers
    );
    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        callers
    );

    // Interface receivers dispatch on the interface method, not the concrete
    // implementation, and a static interface member reached through an
    // instance receiver fails closed.
    let impl_live = trace_symbol_graph(&dir, "Demo::Worker::Run", TraceDirection::Callers).unwrap();
    assert!(impl_live.callers.is_empty());
    let utility_live =
        trace_symbol_graph(&dir, "Demo::IWorker::Utility", TraceDirection::Callers).unwrap();
    assert!(utility_live.callers.is_empty());
}

#[test]
fn traces_csharp_interface_receiver_instance_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let types_path = dir.join("Types.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &types_path,
        "namespace Demo;
interface IWorker {
    int Run(int value);
}
class Worker : IWorker {
    public int Run(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace Demo; class Stale {}
",
    )
    .unwrap();
    let overlay = "namespace Demo;
class Caller {
    int Call(IWorker worker) => worker.Run(1);
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "Demo::IWorker::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "Demo::IWorker::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::Caller::Call");
}

#[test]
fn fails_closed_on_csharp_unresolvable_interface_receiver_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
interface IWorker {
    int Run(int value);
    static int Utility(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    int MissingMethod(IWorker worker) => worker.Nope(1);
    int StaticMethod(IWorker worker) => worker.Utility(1);
    int UnknownInterface(NotIndexed worker) => worker.Run(1);
}
",
    )
    .unwrap();

    for caller in [
        "Demo::Caller::MissingMethod",
        "Demo::Caller::StaticMethod",
        "Demo::Caller::UnknownInterface",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty(), "{caller}");
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(persisted.callees.is_empty(), "{caller}");
    }
}

#[test]
fn traces_csharp_interface_chain_receiver_instance_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
interface IBase {
    int BaseRun(int value);
    static int BaseUtility(int value) => value;
}
interface IMiddle : IBase {
    int MiddleRun(int value);
}
interface IWorker : IMiddle, IBase {
    int Run(int value);
}
class Worker : IWorker {
    public int Run(int value) => value;
    public int MiddleRun(int value) => value;
    public int BaseRun(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    int DirectRun(IWorker worker) => worker.Run(1);
    int InheritedRun(IWorker worker) => worker.MiddleRun(1);
    int DiamondRun(IWorker worker) => worker.BaseRun(1);
    int MiddleTypedRun(IMiddle middle) => middle.BaseRun(1);
    int BaseTypedRun(IBase baseWorker) => baseWorker.BaseRun(1);
    int StaticThroughChain(IMiddle middle) => middle.BaseUtility(1);
}
",
    )
    .unwrap();

    let run_live = trace_symbol_graph(&dir, "Demo::IWorker::Run", TraceDirection::Callers).unwrap();
    assert_eq!(
        run_live
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        ["Demo::Caller::DirectRun"]
    );
    let middle_live =
        trace_symbol_graph(&dir, "Demo::IMiddle::MiddleRun", TraceDirection::Callers).unwrap();
    assert_eq!(
        middle_live
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        ["Demo::Caller::InheritedRun"]
    );
    let base_live =
        trace_symbol_graph(&dir, "Demo::IBase::BaseRun", TraceDirection::Callers).unwrap();
    assert_eq!(
        base_live
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Demo::Caller::BaseTypedRun",
            "Demo::Caller::DiamondRun",
            "Demo::Caller::MiddleTypedRun",
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let run_persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::IWorker::Run", TraceDirection::Callers)
            .unwrap();
    assert_eq!(
        run_persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        ["Demo::Caller::DirectRun"]
    );
    let middle_persisted = trace_symbol_graph_from_index(
        &db_path,
        "Demo::IMiddle::MiddleRun",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(
        middle_persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        ["Demo::Caller::InheritedRun"]
    );
    let base_persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::IBase::BaseRun", TraceDirection::Callers)
            .unwrap();
    assert_eq!(
        base_persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Demo::Caller::BaseTypedRun",
            "Demo::Caller::DiamondRun",
            "Demo::Caller::MiddleTypedRun",
        ]
    );

    // Interface chain dispatch targets the interface declaration, not the
    // concrete implementing class, and a static interface member reached
    // through an inherited interface fails closed.
    let impl_live =
        trace_symbol_graph(&dir, "Demo::Worker::BaseRun", TraceDirection::Callers).unwrap();
    assert!(impl_live.callers.is_empty());
    let utility_live =
        trace_symbol_graph(&dir, "Demo::IBase::BaseUtility", TraceDirection::Callers).unwrap();
    assert!(utility_live.callers.is_empty());
}

#[test]
fn traces_csharp_interface_chain_receiver_instance_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let base_path = dir.join("Base.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &base_path,
        "namespace Demo;
public interface IBase {
    int BaseRun(int value);
}
",
    )
    .unwrap();
    fs::write(
        dir.join("More.cs"),
        "namespace App;
public interface IMiddle : Demo.IBase {
    int MiddleRun(int value);
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace App; class Stale {}
",
    )
    .unwrap();
    let overlay = "namespace App;
class Caller {
    int Call(IMiddle middle) => middle.BaseRun(1);
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "Demo::IBase::BaseRun",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "App::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "Demo::IBase::BaseRun",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "App::Caller::Call");
}

#[test]
fn fails_closed_on_csharp_unresolvable_interface_chain_receiver_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
interface ILeft {
    int Conflict(int value);
}
interface IRight {
    int Conflict(int value);
}
interface IBase {
    int BaseRun(int value);
    static int BaseUtility(int value) => value;
}
interface IMiddle : IBase {
    int BaseRun();
}
interface IBroken : NotIndexed {
}
interface ICyclicA : ICyclicB {
}
interface ICyclicB : ICyclicA {
}
interface IWorker : ILeft, IRight {
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    int MissingMethod(IWorker worker) => worker.Nope(1);
    int CompetingMethods(IWorker worker) => worker.Conflict(1);
    int ArityShadowed(IMiddle middle) => middle.BaseRun(1);
    int StaticThroughChain(IBase baseWorker) => baseWorker.BaseUtility(1);
    int CyclicChain(ICyclicA cyclic) => cyclic.Nope(1);
    int UnresolvableParent(IBroken broken) => broken.Run(1);
    int UnknownInterface(NotIndexed worker) => worker.Run(1);
}
",
    )
    .unwrap();

    for caller in [
        "Demo::Caller::MissingMethod",
        "Demo::Caller::CompetingMethods",
        "Demo::Caller::ArityShadowed",
        "Demo::Caller::StaticThroughChain",
        "Demo::Caller::CyclicChain",
        "Demo::Caller::UnresolvableParent",
        "Demo::Caller::UnknownInterface",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty(), "{caller}");
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(persisted.callees.is_empty(), "{caller}");
    }
}

#[test]
fn traces_csharp_struct_receiver_instance_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
struct Point {
    public int Norm(int value) => value;
    public static int StaticNorm(int value) => value;
}
interface IPoint {
    int ViaInterface(int value);
}
struct Worker : IPoint {
    public int ViaInterface(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    int ParameterReceiver(Point point) => point.Norm(1);
    int LocalReceiver() { Point point = new Point(); return point.Norm(1); }
    Point field = new Point();
    int FieldReceiver() => field.Norm(1);
    int VarConstructorReceiver() { var point = new Point(); return point.Norm(1); }
    int ConstructorReceiver() => new Point().Norm(1);
    int StaticThroughStruct(Point point) => point.StaticNorm(1);
    int StructInterfaceMethod(Worker worker) => worker.ViaInterface(1);
}
",
    )
    .unwrap();

    let target = "Demo::Point::Norm";
    let callers = [
        "Demo::Caller::ConstructorReceiver",
        "Demo::Caller::FieldReceiver",
        "Demo::Caller::LocalReceiver",
        "Demo::Caller::ParameterReceiver",
        "Demo::Caller::VarConstructorReceiver",
    ];
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(
        live.callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        callers
    );
    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        callers
    );

    // A struct implementing an interface dispatches to the struct's own
    // method declaration, and a static struct member reached through an
    // instance receiver fails closed.
    let interface_live =
        trace_symbol_graph(&dir, "Demo::Worker::ViaInterface", TraceDirection::Callers).unwrap();
    assert_eq!(
        interface_live
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        ["Demo::Caller::StructInterfaceMethod"]
    );
    let static_live =
        trace_symbol_graph(&dir, "Demo::Point::StaticNorm", TraceDirection::Callers).unwrap();
    assert!(static_live.callers.is_empty());
}

#[test]
fn traces_csharp_struct_receiver_instance_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let types_path = dir.join("Types.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &types_path,
        "namespace Demo;
struct Point {
    public int Norm(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace Demo; class Stale {}
",
    )
    .unwrap();
    let overlay = "namespace Demo;
class Caller {
    int Call(Point point) => point.Norm(1);
    int Constructed() => new Point().Norm(1);
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "Demo::Point::Norm",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 2);
    assert_eq!(live.callers[0].symbol_id, "Demo::Caller::Call");
    assert_eq!(live.callers[1].symbol_id, "Demo::Caller::Constructed");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "Demo::Point::Norm",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 2);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::Caller::Call");
    assert_eq!(persisted.callers[1].symbol_id, "Demo::Caller::Constructed");
}

#[test]
fn fails_closed_on_csharp_unresolvable_struct_receiver_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
struct Point {
    public int Norm(int value) => value;
    public static int StaticNorm(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    int MissingMethod(Point point) => point.Nope(1);
    int StaticMethod(Point point) => point.StaticNorm(1);
    int UnknownStruct(NotIndexed point) => point.Norm(1);
}
",
    )
    .unwrap();

    for caller in [
        "Demo::Caller::MissingMethod",
        "Demo::Caller::StaticMethod",
        "Demo::Caller::UnknownStruct",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty(), "{caller}");
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(persisted.callees.is_empty(), "{caller}");
    }
}

#[test]
fn traces_csharp_nested_declared_type_receiver_instance_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
class NestedContainer {
    public class Inner {
        public int Help(int value) => value;
    }
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    int ParameterReceiver(NestedContainer.Inner inner) => inner.Help(1);
    int VarConstructorReceiver() { var inner = new NestedContainer.Inner(); return inner.Help(1); }
    NestedContainer.Inner field = new NestedContainer.Inner();
    int FieldReceiver() => field.Help(1);
    int ConstructorReceiver() => new NestedContainer.Inner().Help(1);
}
",
    )
    .unwrap();

    let target = "Demo::NestedContainer::Inner::Help";
    let callers = [
        "Demo::Caller::ConstructorReceiver",
        "Demo::Caller::FieldReceiver",
        "Demo::Caller::ParameterReceiver",
        "Demo::Caller::VarConstructorReceiver",
    ];
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(
        live.callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        callers
    );
    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        callers
    );
}

#[test]
fn traces_csharp_nested_declared_type_receiver_instance_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let types_path = dir.join("Types.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &types_path,
        "namespace Demo;
class NestedContainer {
    public class Inner {
        public int Help(int value) => value;
    }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace Demo; class Stale {}
",
    )
    .unwrap();
    let overlay = "namespace Demo;
class Caller {
    int Call(NestedContainer.Inner inner) => inner.Help(1);
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "Demo::NestedContainer::Inner::Help",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "Demo::NestedContainer::Inner::Help",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::Caller::Call");
}

#[test]
fn fails_closed_on_csharp_unresolvable_nested_declared_type_receivers() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("OuterA.cs"),
        "namespace Demo;
class Outer {
    public class Inner {
        public int Help(int value) => value;
    }
}
",
    )
    .unwrap();
    fs::write(
        dir.join("OuterB.cs"),
        "namespace Demo;
class Outer {
    public class Inner {
        public int Other(int value) => value;
    }
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    int UnknownReceiver(NotIndexed.Thing thing) => thing.Help(1);
    int MissingNestedReceiver(Outer.Missing thing) => thing.Help(1);
    int AmbiguousNestedReceiver(Outer.Inner thing) => thing.Help(1);
}
",
    )
    .unwrap();

    for caller in [
        "Demo::Caller::UnknownReceiver",
        "Demo::Caller::MissingNestedReceiver",
        "Demo::Caller::AmbiguousNestedReceiver",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty(), "{caller}");
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(persisted.callees.is_empty(), "{caller}");
    }
}

#[test]
fn traces_csharp_member_chain_receiver_instance_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
    public static int Utility(int value) => value;
}
class Holder {
    public Helper helper = new Helper();
    public Helper Property { get; set; }
}
interface IWorker {
    int Run(int value);
}
class Worker : IWorker {
    public int Run(int value) => value;
}
struct Point {
    public int Norm(int value) => value;
}
class Group {
    public Holder holder = new Holder();
    public IWorker worker = new Worker();
    public Point point = new Point();
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    int FieldHop(Group group) => group.holder.helper.Run(1);
    int PropertyHop(Group group) => group.holder.Property.Run(1);
    int InterfaceHop(Group group) => group.worker.Run(1);
    int StructHop(Group group) => group.point.Norm(1);
    int StaticHop(Group group) => group.holder.helper.Utility(1);
}
",
    )
    .unwrap();

    for (target, callers) in [
        (
            "Demo::Helper::Run",
            vec!["Demo::Caller::FieldHop", "Demo::Caller::PropertyHop"],
        ),
        ("Demo::IWorker::Run", vec!["Demo::Caller::InterfaceHop"]),
        ("Demo::Point::Norm", vec!["Demo::Caller::StructHop"]),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert_eq!(
            live.callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            callers,
            "{target}"
        );
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert_eq!(
            persisted
                .callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            callers,
            "{target}"
        );
    }

    // An interface-typed hop dispatches on the interface method, not the
    // concrete implementation, and a static method reached through a chain
    // fails closed.
    let impl_live = trace_symbol_graph(&dir, "Demo::Worker::Run", TraceDirection::Callers).unwrap();
    assert!(impl_live.callers.is_empty());
    let utility_live =
        trace_symbol_graph(&dir, "Demo::Helper::Utility", TraceDirection::Callers).unwrap();
    assert!(utility_live.callers.is_empty());
}

#[test]
fn traces_csharp_member_chain_receiver_instance_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let types_path = dir.join("Types.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &types_path,
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
}
class Group {
    public Helper helper = new Helper();
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace Demo; class Stale {}
",
    )
    .unwrap();
    let overlay = "namespace Demo;
class Caller {
    int Call(Group group) => group.helper.Run(1);
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "Demo::Helper::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "Demo::Helper::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::Caller::Call");
}

#[test]
fn fails_closed_on_csharp_unresolvable_member_chain_receiver_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
    public static int Utility(int value) => value;
}
class Group {
    public Helper helper = new Helper();
    public NotIndexed unknown = new NotIndexed();
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    int MissingHop(Group group) => group.absent.Run(1);
    int UnknownHopType(Group group) => group.unknown.Run(1);
    int MissingFinalMember(Group group) => group.helper.Nope(1);
    int StaticFinalMember(Group group) => group.helper.Utility(1);
}
",
    )
    .unwrap();

    for caller in [
        "Demo::Caller::MissingHop",
        "Demo::Caller::UnknownHopType",
        "Demo::Caller::MissingFinalMember",
        "Demo::Caller::StaticFinalMember",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty(), "{caller}");
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(persisted.callees.is_empty(), "{caller}");
    }
}

#[test]
fn traces_csharp_this_member_chain_receiver_instance_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
    public static int Utility(int value) => value;
}
interface IWorker {
    int Run(int value);
}
class Worker : IWorker {
    public int Run(int value) => value;
}
struct Point {
    public int Norm(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    Holder holder = new Holder();
    IWorker worker = new Worker();
    Point point = new Point();
    int FieldHop() => this.holder.helper.Run(1);
    int PropertyHop() => this.holder.Property.Run(1);
    int InterfaceHop() => this.worker.Run(1);
    int StructHop() => this.point.Norm(1);
    int StaticHop() => this.holder.helper.Utility(1);
}
class Holder {
    public Helper helper = new Helper();
    public Helper Property { get; set; }
}
",
    )
    .unwrap();

    for (target, callers) in [
        (
            "Demo::Helper::Run",
            vec!["Demo::Caller::FieldHop", "Demo::Caller::PropertyHop"],
        ),
        ("Demo::IWorker::Run", vec!["Demo::Caller::InterfaceHop"]),
        ("Demo::Point::Norm", vec!["Demo::Caller::StructHop"]),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert_eq!(
            live.callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            callers,
            "{target}"
        );
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert_eq!(
            persisted
                .callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            callers,
            "{target}"
        );
    }

    // An interface-typed hop dispatches on the interface method, not the
    // concrete implementation, and a static method reached through a chain
    // fails closed.
    let impl_live = trace_symbol_graph(&dir, "Demo::Worker::Run", TraceDirection::Callers).unwrap();
    assert!(impl_live.callers.is_empty());
    let utility_live =
        trace_symbol_graph(&dir, "Demo::Helper::Utility", TraceDirection::Callers).unwrap();
    assert!(utility_live.callers.is_empty());
}

#[test]
fn traces_csharp_this_member_chain_receiver_instance_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let types_path = dir.join("Types.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &types_path,
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace Demo; class Stale {}
",
    )
    .unwrap();
    let overlay = "namespace Demo;
class Caller {
    Helper helper = new Helper();
    int Call() => this.helper.Run(1);
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "Demo::Helper::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "Demo::Helper::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::Caller::Call");
}

#[test]
fn fails_closed_on_csharp_unresolvable_this_member_chain_receiver_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
    public static int Utility(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    Helper helper = new Helper();
    NotIndexed unknown = new NotIndexed();
    int MissingHop() => this.absent.Run(1);
    int UnknownHopType() => this.unknown.Run(1);
    int MissingFinalMember() => this.helper.Nope(1);
    int StaticFinalMember() => this.helper.Utility(1);
}
",
    )
    .unwrap();

    for caller in [
        "Demo::Caller::MissingHop",
        "Demo::Caller::UnknownHopType",
        "Demo::Caller::MissingFinalMember",
        "Demo::Caller::StaticFinalMember",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty(), "{caller}");
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(persisted.callees.is_empty(), "{caller}");
    }
}

#[test]
fn traces_csharp_method_call_hop_receiver_instance_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
    public static int Utility(int value) => value;
}
interface IWorker {
    int Run(int value);
}
class Worker : IWorker {
    public int Run(int value) => value;
}
struct Point {
    public int Norm(int value) => value;
}
class Holder {
    public Helper Make() => new Helper();
}
class Group {
    public Helper Make() => new Helper();
    public static Helper StaticMake() => new Helper();
    public int Tag() => 1;
    public IWorker GetWorker() => new Worker();
    public Point GetPoint() => new Point();
    public Holder holder = new Holder();
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    int MethodHop(Group group) => group.Make().Run(1);
    int FieldThenMethodHop(Group group) => group.holder.Make().Run(1);
    int InterfaceReturnHop(Group group) => group.GetWorker().Run(1);
    int StructReturnHop(Group group) => group.GetPoint().Norm(1);
    int ThisMethodHop() => this.Make().Run(1);
    Helper Make() => new Helper();
}
",
    )
    .unwrap();

    for (target, callers) in [
        (
            "Demo::Helper::Run",
            vec![
                "Demo::Caller::FieldThenMethodHop",
                "Demo::Caller::MethodHop",
                "Demo::Caller::ThisMethodHop",
            ],
        ),
        (
            "Demo::IWorker::Run",
            vec!["Demo::Caller::InterfaceReturnHop"],
        ),
        ("Demo::Point::Norm", vec!["Demo::Caller::StructReturnHop"]),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert_eq!(
            live.callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            callers,
            "{target}"
        );
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert_eq!(
            persisted
                .callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            callers,
            "{target}"
        );
    }

    // An interface-returning hop dispatches on the interface method, not the
    // concrete implementation.
    let impl_live = trace_symbol_graph(&dir, "Demo::Worker::Run", TraceDirection::Callers).unwrap();
    assert!(impl_live.callers.is_empty());
}

#[test]
fn traces_csharp_method_call_hop_receiver_instance_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let types_path = dir.join("Types.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &types_path,
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
}
class Group {
    public Helper Make() => new Helper();
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace Demo; class Stale {}
",
    )
    .unwrap();
    let overlay = "namespace Demo;
class Caller {
    int Call(Group group) => group.Make().Run(1);
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "Demo::Helper::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "Demo::Helper::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::Caller::Call");
}

#[test]
fn fails_closed_on_csharp_unresolvable_method_call_hop_receiver_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
    public static int Utility(int value) => value;
}
class Group {
    public Helper Make() => new Helper();
    public static Helper StaticMake() => new Helper();
    public int Tag() => 1;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    int UnknownHop(Group group) => group.Missing().Run(1);
    int StaticHop(Group group) => group.StaticMake().Run(1);
    int ArityMismatchHop(Group group) => group.Make(1).Run(1);
    int PrimitiveReturnHop(Group group) => group.Tag().Run(1);
    int MissingFinalMember(Group group) => group.Make().Nope(1);
    int StaticFinalMember(Group group) => group.Make().Utility(1);
    int ThisUnknownHop() => this.Missing().Run(1);
}
",
    )
    .unwrap();

    // The chain hop itself must fail closed: no callee is traced for the
    // unresolved final member (`.Run` on a primitive, a missing/static final
    // member, or an arity-mismatched/static/unknown hop). Legitimate
    // intermediate calls such as `group.Tag()` or `group.Make()` still trace
    // as direct callees, as they did before method-call hop support.
    for (caller, expected) in [
        ("Demo::Caller::UnknownHop", Vec::<&str>::new()),
        ("Demo::Caller::StaticHop", Vec::<&str>::new()),
        ("Demo::Caller::ArityMismatchHop", Vec::<&str>::new()),
        ("Demo::Caller::PrimitiveReturnHop", vec!["Demo::Group::Tag"]),
        (
            "Demo::Caller::MissingFinalMember",
            vec!["Demo::Group::Make"],
        ),
        ("Demo::Caller::StaticFinalMember", vec!["Demo::Group::Make"]),
        ("Demo::Caller::ThisUnknownHop", Vec::<&str>::new()),
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert_eq!(
            live.callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            expected,
            "{caller} live"
        );
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert_eq!(
            persisted
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            expected,
            "{caller} persisted"
        );
    }
}

#[test]
fn traces_csharp_constructor_member_chain_receiver_instance_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
    public static int Utility(int value) => value;
}
interface IWorker {
    int Run(int value);
}
class Worker : IWorker {
    public int Run(int value) => value;
}
struct Point {
    public int Norm(int value) => value;
}
class Holder {
    public Helper Make() => new Helper();
}
class Group {
    public Helper Make() => new Helper();
    public Holder holder = new Holder();
    public IWorker GetWorker() => new Worker();
    public Point GetPoint() => new Point();
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    int MethodChain() => new Group().Make().Run(1);
    int FieldThenMethodChain() => new Group().holder.Make().Run(1);
    int InterfaceReturnChain() => new Group().GetWorker().Run(1);
    int StructReturnChain() => new Group().GetPoint().Norm(1);
    int NamespaceQualifiedChain() => new Demo.Group().Make().Run(1);
}
",
    )
    .unwrap();

    for (target, callers) in [
        (
            "Demo::Helper::Run",
            vec![
                "Demo::Caller::FieldThenMethodChain",
                "Demo::Caller::MethodChain",
                "Demo::Caller::NamespaceQualifiedChain",
            ],
        ),
        (
            "Demo::IWorker::Run",
            vec!["Demo::Caller::InterfaceReturnChain"],
        ),
        ("Demo::Point::Norm", vec!["Demo::Caller::StructReturnChain"]),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert_eq!(
            live.callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            callers,
            "{target}"
        );
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert_eq!(
            persisted
                .callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            callers,
            "{target}"
        );
    }

    // An interface-returning hop dispatches on the interface method, not the
    // concrete implementation.
    let impl_live = trace_symbol_graph(&dir, "Demo::Worker::Run", TraceDirection::Callers).unwrap();
    assert!(impl_live.callers.is_empty());
}

#[test]
fn traces_csharp_constructor_member_chain_receiver_instance_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let types_path = dir.join("Types.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &types_path,
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
}
class Group {
    public Helper Make() => new Helper();
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace Demo; class Stale {}
",
    )
    .unwrap();
    let overlay = "namespace Demo;
class Caller {
    int Call() => new Group().Make().Run(1);
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "Demo::Helper::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "Demo::Helper::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::Caller::Call");
}

#[test]
fn fails_closed_on_csharp_unresolvable_constructor_member_chain_receiver_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
    public static int Utility(int value) => value;
}
class Group {
    public Helper Make() => new Helper();
    public static Helper StaticMake() => new Helper();
    public int Tag() => 1;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    int UnknownHop() => new Group().Missing().Run(1);
    int StaticHop() => new Group().StaticMake().Run(1);
    int ArityMismatchHop() => new Group().Make(1).Run(1);
    int PrimitiveReturnHop() => new Group().Tag().Run(1);
    int MissingFinalMember() => new Group().Make().Nope(1);
    int StaticFinalMember() => new Group().Make().Utility(1);
}
",
    )
    .unwrap();

    // The constructor-rooted chain itself must fail closed: no callee is
    // traced for the unresolved final member (`.Run` on a primitive, a
    // missing/static final member, or an arity-mismatched/static/unknown
    // hop). Legitimate intermediate constructor-rooted calls such as
    // `new Group().Tag()` or `new Group().Make()` still trace as direct
    // callees, as they did before constructor-rooted chain support.
    for (caller, expected) in [
        ("Demo::Caller::UnknownHop", Vec::<&str>::new()),
        ("Demo::Caller::StaticHop", Vec::<&str>::new()),
        ("Demo::Caller::ArityMismatchHop", Vec::<&str>::new()),
        ("Demo::Caller::PrimitiveReturnHop", vec!["Demo::Group::Tag"]),
        (
            "Demo::Caller::MissingFinalMember",
            vec!["Demo::Group::Make"],
        ),
        ("Demo::Caller::StaticFinalMember", vec!["Demo::Group::Make"]),
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert_eq!(
            live.callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            expected,
            "{caller} live"
        );
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert_eq!(
            persisted
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            expected,
            "{caller} persisted"
        );
    }
}

#[test]
fn traces_csharp_var_factory_receiver_instance_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
}
class Holder {
    public Helper helper = new Helper();
}
class Factories {
    public static Helper MakeHelper() => new Helper();
}
class Group {
    public Helper Make() => new Helper();
    public Holder holder = new Holder();
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "using static Demo.Factories;
namespace Demo;
class Caller {
    Helper MakeHelper() => new Helper();
    Helper MakeHelper(int value) => new Helper();
    Group MakeGroup() => new Group();
    int SameTypeFactory() { var helper = MakeHelper(); return helper.Run(1); }
    int ArityOneFactory() { var helper = MakeHelper(1); return helper.Run(1); }
    int ThisFactory() { var helper = this.MakeHelper(); return helper.Run(1); }
    int QualifiedFactory() { var helper = Factories.MakeHelper(); return helper.Run(1); }
    int FactoryThenMethodHop() { var group = MakeGroup(); return group.Make().Run(1); }
    int FactoryThenFieldHop() { var group = MakeGroup(); return group.holder.helper.Run(1); }
}
class StaticImportCaller {
    int StaticImportFactory() { var helper = MakeHelper(); return helper.Run(1); }
}
",
    )
    .unwrap();

    for (target, callers) in [
        (
            "Demo::Helper::Run",
            vec![
                "Demo::Caller::ArityOneFactory",
                "Demo::Caller::FactoryThenFieldHop",
                "Demo::Caller::FactoryThenMethodHop",
                "Demo::Caller::QualifiedFactory",
                "Demo::Caller::SameTypeFactory",
                "Demo::Caller::ThisFactory",
                "Demo::StaticImportCaller::StaticImportFactory",
            ],
        ),
        (
            "Demo::Caller::MakeGroup",
            vec![
                "Demo::Caller::FactoryThenFieldHop",
                "Demo::Caller::FactoryThenMethodHop",
            ],
        ),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert_eq!(
            live.callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            callers,
            "{target}"
        );
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert_eq!(
            persisted
                .callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            callers,
            "{target}"
        );
    }
}

#[test]
fn traces_csharp_var_factory_receiver_instance_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let types_path = dir.join("Types.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &types_path,
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace Demo; class Stale {}
",
    )
    .unwrap();
    let overlay = "namespace Demo;
class Caller {
    Helper MakeHelper() => new Helper();
    int Call() { var helper = MakeHelper(); return helper.Run(1); }
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "Demo::Helper::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "Demo::Helper::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::Caller::Call");
}

#[test]
fn fails_closed_on_csharp_unresolvable_var_factory_receiver_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
}
class Factories {
    public static Helper MakeHelper() => new Helper();
    public static Helper MakeAmbiguous() => new Helper();
}
class OtherFactories {
    public static Helper MakeHelper() => new Helper();
    public static Helper MakeAmbiguous() => new Helper();
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "using static Demo.Factories;
using static Demo.OtherFactories;
namespace Demo;
class Caller {
    Helper MakeHelper() => new Helper();
    void MakeVoid() {}
    int MakeInt() => 1;
    int UnknownFactory() { var helper = Missing(); return helper.Run(1); }
    int ArityMismatchFactory() { var helper = MakeHelper(1, 2); return helper.Run(1); }
    int VoidFactory() { var helper = MakeVoid(); return helper.Run(1); }
    int PrimitiveFactory() { var helper = MakeInt(); return helper.Run(1); }
    int AmbiguousFactory() { var helper = MakeAmbiguous(); return helper.Run(1); }
}
",
    )
    .unwrap();

    // The factory-inferred receiver itself must fail closed: no callee is
    // traced for `.Run` when the factory is unknown, arity-mismatched,
    // ambiguous, or has no usable (void/primitive) declared return type.
    // Legitimate bare factory calls still trace as direct callees when they
    // resolve independently of the receiver binding.
    for (caller, expected) in [
        ("Demo::Caller::UnknownFactory", Vec::<&str>::new()),
        ("Demo::Caller::ArityMismatchFactory", Vec::<&str>::new()),
        ("Demo::Caller::VoidFactory", vec!["Demo::Caller::MakeVoid"]),
        (
            "Demo::Caller::PrimitiveFactory",
            vec!["Demo::Caller::MakeInt"],
        ),
        ("Demo::Caller::AmbiguousFactory", Vec::<&str>::new()),
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert_eq!(
            live.callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            expected,
            "{caller} live"
        );
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert_eq!(
            persisted
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            expected,
            "{caller} persisted"
        );
    }
}

#[test]
fn traces_csharp_var_field_receiver_instance_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
}
class Inner {
    public Helper helper = new Helper();
}
class Holder {
    public Helper helper = new Helper();
    public Inner GetInner() => new Inner();
}
class Caller {
    Helper helper = new Helper();
    Holder holder = new Holder();
    int BareField() { var v = helper; return v.Run(1); }
    int ThisField() { var v = this.helper; return v.Run(1); }
    int ThisChain() { var v = this.holder.helper; return v.Run(1); }
    int BoundChain() { var v = holder.helper; return v.Run(1); }
    int ConstructorRooted() { var v = new Holder().helper; return v.Run(1); }
    int MethodHopChain() { var v = holder.GetInner().helper; return v.Run(1); }
}
",
    )
    .unwrap();

    let target = "Demo::Helper::Run";
    let expected = [
        "Demo::Caller::BareField",
        "Demo::Caller::BoundChain",
        "Demo::Caller::ConstructorRooted",
        "Demo::Caller::MethodHopChain",
        "Demo::Caller::ThisChain",
        "Demo::Caller::ThisField",
    ];
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(
        live.callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        expected
    );
    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn traces_csharp_var_field_receiver_instance_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let holder_path = dir.join("Holder.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &holder_path,
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
}
class Holder {
    public Helper helper = new Helper();
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace Demo; class Stale {}
",
    )
    .unwrap();
    let overlay = "namespace Demo;
class Caller {
    Holder holder = new Holder();
    int Call() { var v = holder.helper; return v.Run(1); }
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "Demo::Helper::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "Demo::Helper::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::Caller::Call");
}

#[test]
fn fails_closed_on_csharp_unresolvable_var_field_receiver_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
}
class Holder {
    public Helper helper = new Helper();
    public int Count = 1;
}
class Caller {
    Holder holder = new Holder();
    int count = 1;
    int UnknownBare() { var v = missing; return v.Run(1); }
    int PrimitiveBare() { var v = count; return v.Run(1); }
    int UnknownHop() { var v = holder.missing; return v.Run(1); }
    int PrimitiveHop() { var v = holder.Count; return v.Run(1); }
    int FactoryInferredRoot() { var x = MakeHelper(); var v = x; return v.Run(1); }
    int VarFromVar() { var h = holder.helper; var v = h; return v.Run(1); }
    Helper MakeHelper() => new Helper();
}
",
    )
    .unwrap();

    // A `var` local initialized from a field/property-access chain pins its
    // receiver only when every root and hop resolves to a usable declared
    // type; unknown or primitive fields, factory-inferred roots, and
    // chain-marked `var` roots fail closed, while legitimate factory calls
    // still trace as direct callees.
    for (caller, expected) in [
        ("Demo::Caller::UnknownBare", Vec::<&str>::new()),
        ("Demo::Caller::PrimitiveBare", Vec::<&str>::new()),
        ("Demo::Caller::UnknownHop", Vec::<&str>::new()),
        ("Demo::Caller::PrimitiveHop", Vec::<&str>::new()),
        ("Demo::Caller::VarFromVar", Vec::<&str>::new()),
        (
            "Demo::Caller::FactoryInferredRoot",
            vec!["Demo::Caller::MakeHelper"],
        ),
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert_eq!(
            live.callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            expected,
            "{caller} live"
        );
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert_eq!(
            persisted
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            expected,
            "{caller} persisted"
        );
    }
}

#[test]
fn traces_csharp_base_member_chain_receiver_instance_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Helper.cs"),
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
    public static int Utility(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Base.cs"),
        "namespace Demo;
class Holder {
    public Helper helper = new Helper();
}
class Base {
    public Helper Make() => new Helper();
    public Holder holder = new Holder();
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller : Base {
    int MethodHop() => base.Make().Run(1);
    int FieldThenMethodHop() => base.holder.helper.Run(1);
}
",
    )
    .unwrap();

    for (target, callers) in [
        (
            "Demo::Helper::Run",
            vec![
                "Demo::Caller::FieldThenMethodHop",
                "Demo::Caller::MethodHop",
            ],
        ),
        ("Demo::Base::Make", vec!["Demo::Caller::MethodHop"]),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert_eq!(
            live.callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            callers,
            "{target}"
        );
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert_eq!(
            persisted
                .callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            callers,
            "{target}"
        );
    }
}

#[test]
fn traces_csharp_base_member_chain_receiver_instance_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let base_path = dir.join("Base.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &base_path,
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
}
class Base {
    public Helper Make() => new Helper();
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace Demo; class Stale {}
",
    )
    .unwrap();
    let overlay = "namespace Demo;
class Caller : Base {
    int Call() => base.Make().Run(1);
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "Demo::Helper::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "Demo::Helper::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::Caller::Call");
}

#[test]
fn fails_closed_on_csharp_unresolvable_base_member_chain_receiver_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
    public static int Utility(int value) => value;
}
class Base {
    public Helper Make() => new Helper();
    public static Helper StaticMake() => new Helper();
    public int Tag() => 1;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller : Base {
    int UnknownHop() => base.Missing().Run(1);
    int StaticHop() => base.StaticMake().Run(1);
    int PrimitiveReturnHop() => base.Tag().Run(1);
    int MissingFinalMember() => base.Make().Nope(1);
    int StaticFinalMember() => base.Make().Utility(1);
}
",
    )
    .unwrap();

    // The base-rooted chain itself must fail closed: no callee is traced for
    // the unresolved final member (`.Run` on a primitive, a missing/static
    // final member, or a static/unknown hop). Legitimate intermediate
    // base-rooted calls such as `base.Tag()` or `base.Make()` still trace as
    // direct callees, as they did before base-rooted chain support.
    for (caller, expected) in [
        ("Demo::Caller::UnknownHop", Vec::<&str>::new()),
        ("Demo::Caller::StaticHop", Vec::<&str>::new()),
        ("Demo::Caller::PrimitiveReturnHop", vec!["Demo::Base::Tag"]),
        ("Demo::Caller::MissingFinalMember", vec!["Demo::Base::Make"]),
        ("Demo::Caller::StaticFinalMember", vec!["Demo::Base::Make"]),
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert_eq!(
            live.callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            expected,
            "{caller} live"
        );
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert_eq!(
            persisted
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            expected,
            "{caller} persisted"
        );
    }
}

#[test]
fn traces_csharp_constructor_receiver_instance_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
    public static int Utility(int value) => value;
}
class NestedContainer {
    public class Inner {
        public int Help(int value) => value;
    }
}
class Box<T> {
    public T Get() => default;
}
class Base {
    public int Ping(int value) => value;
}
class Derived : Base {
    public int Pong(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    int DirectConstructorReceiver() => new Helper().Run(1);
    int NamespaceConstructorReceiver() => new Demo.Helper().Run(1);
    int NestedConstructorReceiver() => new NestedContainer.Inner().Help(1);
    int GenericConstructorReceiver() => new Box<int>().Get();
    int InheritedConstructorReceiver() => new Derived().Ping(1);
}
",
    )
    .unwrap();

    for (target, callers) in [
        (
            "Demo::Helper::Run",
            vec![
                "Demo::Caller::DirectConstructorReceiver",
                "Demo::Caller::NamespaceConstructorReceiver",
            ],
        ),
        (
            "Demo::NestedContainer::Inner::Help",
            vec!["Demo::Caller::NestedConstructorReceiver"],
        ),
        (
            "Demo::Box::Get",
            vec!["Demo::Caller::GenericConstructorReceiver"],
        ),
        (
            "Demo::Base::Ping",
            vec!["Demo::Caller::InheritedConstructorReceiver"],
        ),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert_eq!(
            live.callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            callers,
            "{target}"
        );
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert_eq!(
            persisted
                .callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            callers,
            "{target}"
        );
    }

    // A static method reached through a fresh constructed instance and an
    // unknown constructed type fail closed in both live and persisted paths.
    let utility_live =
        trace_symbol_graph(&dir, "Demo::Helper::Utility", TraceDirection::Callers).unwrap();
    assert!(utility_live.callers.is_empty());
    rebuild_symbol_index(&dir, &db_path).unwrap();
    let utility_persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::Helper::Utility", TraceDirection::Callers)
            .unwrap();
    assert!(utility_persisted.callers.is_empty());
}

#[test]
fn traces_csharp_constructor_receiver_instance_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper_path = dir.join("Helper.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper_path,
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace Demo; class Stale {}
",
    )
    .unwrap();
    let overlay = "namespace Demo;
class Caller {
    int Call() => new Helper().Run(1);
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "Demo::Helper::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "Demo::Helper::Run",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::Caller::Call");
}

#[test]
fn fails_closed_on_csharp_unresolvable_constructor_receiver_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Types.cs"),
        "namespace Demo;
class Helper {
    public int Run(int value) => value;
    public static int Utility(int value) => value;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    int StaticThroughConstructor() => new Helper().Utility(1);
    int UnknownConstructorReceiver() => new NotIndexed().Run(1);
    int MissingMember() => new Helper().Nope(1);
    int ChainedConstructorReceiver() => new Helper().Other().Run(1);
    Helper Other() => new Helper();
}
",
    )
    .unwrap();

    for caller in [
        "Demo::Caller::StaticThroughConstructor",
        "Demo::Caller::UnknownConstructorReceiver",
        "Demo::Caller::MissingMember",
        "Demo::Caller::ChainedConstructorReceiver",
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        assert!(live.callees.is_empty(), "{caller}");
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        assert!(persisted.callees.is_empty(), "{caller}");
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
fn traces_csharp_inherited_bare_and_this_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Base.cs"),
        "namespace Demo;
class Base {
    public int Ping(int value) => value;
    public int Flexible(params int[] values) => values.Length;
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Derived.cs"),
        "namespace Demo;
class Derived : Base {
    int Bare(int value) => Ping(value);
    int Explicit(int value) => this.Ping(value);
    int Params(int value) => Flexible(value);
    static int StaticCall(int value) => Ping(value);
}
class HidingDerived : Base {
    int Ping(string value) => value.Length;
    int Hidden(int value) => Ping(value);
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
        ["Demo::Derived::Bare", "Demo::Derived::Explicit"]
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
        ["Demo::Derived::Bare", "Demo::Derived::Explicit"]
    );

    let target = "Demo::Base::Flexible";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(live.callers.is_empty(), "{target}");
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty(), "{target}");
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
fn traces_csharp_generic_static_type_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Outer.cs"),
        "namespace Demo;
class Outer<T> {
    public static int Direct(int value) => value;
    class Helper<U> {
        public static int Utility(int value) => value;
        public static int Flexible(params int[] values) => values.Length;
        public int Instance(int value) => value;
    }
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo;
class Caller {
    int DirectCall() => Outer<int>.Direct(1);
    int NestedCall() => Outer<int>.Helper<string>.Utility(1);
    int GlobalNestedCall() => global::Demo.Outer<int>.Helper<string>.Utility(1);
    int InstanceCall() => Outer<int>.Helper<string>.Instance(1);
    int ParamsCall() => Outer<int>.Helper<string>.Flexible(1);
}
",
    )
    .unwrap();

    for (target, expected_callers) in [
        ("Demo::Outer::Direct", vec!["Demo::Caller::DirectCall"]),
        (
            "Demo::Outer::Helper::Utility",
            vec!["Demo::Caller::GlobalNestedCall", "Demo::Caller::NestedCall"],
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
        ("Demo::Outer::Direct", vec!["Demo::Caller::DirectCall"]),
        (
            "Demo::Outer::Helper::Utility",
            vec!["Demo::Caller::GlobalNestedCall", "Demo::Caller::NestedCall"],
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

    for target in [
        "Demo::Outer::Helper::Instance",
        "Demo::Outer::Helper::Flexible",
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty(), "{target}");
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty(), "{target}");
    }
}

#[test]
fn traces_csharp_generic_alias_and_static_import_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Targets.cs"),
        "namespace Demo.Utility;
class LocalAliasTarget<T> { public static int FromLocalAlias(int value) => value; }
class LocalStaticTarget<T> { public static int FromLocalStatic(int value) => value; }
class GlobalAliasTarget<T> { public static int FromGlobalAlias(int value) => value; }
class GlobalStaticTarget<T> { public static int FromGlobalStatic(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("GlobalUsings.cs"),
        "global using GlobalAlias = Demo.Utility.GlobalAliasTarget<int>;
global using static Demo.Utility.GlobalStaticTarget<int>;
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "using LocalAlias = Demo.Utility.LocalAliasTarget<int>;
using static Demo.Utility.LocalStaticTarget<int>;
namespace Demo.App;
class Caller {
    int LocalAliasCall() => LocalAlias.FromLocalAlias(1);
    int LocalStaticCall() => FromLocalStatic(1);
    int GlobalAliasCall() => GlobalAlias.FromGlobalAlias(1);
    int GlobalStaticCall() => FromGlobalStatic(1);
}
",
    )
    .unwrap();

    for (target, expected_caller) in [
        (
            "Demo::Utility::LocalAliasTarget::FromLocalAlias",
            "Demo::App::Caller::LocalAliasCall",
        ),
        (
            "Demo::Utility::LocalStaticTarget::FromLocalStatic",
            "Demo::App::Caller::LocalStaticCall",
        ),
        (
            "Demo::Utility::GlobalAliasTarget::FromGlobalAlias",
            "Demo::App::Caller::GlobalAliasCall",
        ),
        (
            "Demo::Utility::GlobalStaticTarget::FromGlobalStatic",
            "Demo::App::Caller::GlobalStaticCall",
        ),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert_eq!(live.indexed_files, 3);
        assert_eq!(live.callers.len(), 1, "{target}");
        assert_eq!(live.callers[0].symbol_id, expected_caller, "{target}");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (target, expected_caller) in [
        (
            "Demo::Utility::LocalAliasTarget::FromLocalAlias",
            "Demo::App::Caller::LocalAliasCall",
        ),
        (
            "Demo::Utility::LocalStaticTarget::FromLocalStatic",
            "Demo::App::Caller::LocalStaticCall",
        ),
        (
            "Demo::Utility::GlobalAliasTarget::FromGlobalAlias",
            "Demo::App::Caller::GlobalAliasCall",
        ),
        (
            "Demo::Utility::GlobalStaticTarget::FromGlobalStatic",
            "Demo::App::Caller::GlobalStaticCall",
        ),
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert_eq!(persisted.indexed_files, 3);
        assert_eq!(persisted.callers.len(), 1, "{target}");
        assert_eq!(persisted.callers[0].symbol_id, expected_caller, "{target}");
    }
}

#[test]
fn traces_csharp_nested_type_static_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Outer.cs"),
        "namespace Demo;
class Outer {
    class Helper {
        public static int Utility(int value) => value;
        public static int Flexible(params int[] values) => values.Length;
        public int Instance(int value) => value;
    }
}
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo.App;
class Caller {
    int Call() => Outer.Helper.Utility(1);
    int InstanceCall() => Outer.Helper.Instance(1);
    int ParamsCall() => Outer.Helper.Flexible(1);
}
",
    )
    .unwrap();

    let target = "Demo::Outer::Helper::Utility";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::App::Caller::Call");

    for target in [
        "Demo::Outer::Helper::Instance",
        "Demo::Outer::Helper::Flexible",
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty(), "{target}");
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty(), "{target}");
    }
}

#[test]
fn traces_csharp_nested_type_static_calls_through_namespace_imports() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Outer.cs"),
        "namespace Demo.Utility;
class Outer { class Helper { public static int Utility(int value) => value; } }
",
    )
    .unwrap();
    fs::write(
        dir.join("LocalCaller.cs"),
        "using Demo.Utility;
namespace Demo.App; class LocalCaller { int Call() => Outer.Helper.Utility(1); }
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
        dir.join("GlobalCaller.cs"),
        "namespace Demo.Other; class GlobalCaller { int Call() => Outer.Helper.Utility(1); }
",
    )
    .unwrap();

    let target = "Demo::Utility::Outer::Helper::Utility";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 4);
    assert_eq!(
        live.callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Demo::App::LocalCaller::Call",
            "Demo::Other::GlobalCaller::Call"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 4);
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Demo::App::LocalCaller::Call",
            "Demo::Other::GlobalCaller::Call"
        ]
    );
}

#[test]
fn does_not_trace_ambiguous_csharp_namespace_imported_nested_type_static_calls() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    for (name, namespace) in [("First.cs", "Demo.First"), ("Second.cs", "Demo.Second")] {
        fs::write(
            dir.join(name),
            format!(
                "namespace {namespace}; class Outer {{ class Helper {{ public static int Utility(int value) => value; }} }}\n"
            ),
        )
        .unwrap();
    }
    fs::write(
        dir.join("Caller.cs"),
        "using Demo.First;
using Demo.Second;
namespace Demo.App; class Caller { int Call() => Outer.Helper.Utility(1); }
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
fn traces_csharp_nested_type_static_calls_through_local_and_global_aliases() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Outer.cs"),
        "namespace Demo;
class Outer { class Helper { public static int Utility(int value) => value; } }
",
    )
    .unwrap();
    fs::write(
        dir.join("LocalCaller.cs"),
        "using LocalOuter = Demo.Outer;
namespace Demo.App; class LocalCaller { int Call() => LocalOuter.Helper.Utility(1); }
",
    )
    .unwrap();
    fs::write(
        dir.join("GlobalUsings.cs"),
        "global using GlobalOuter = Demo.Outer;
",
    )
    .unwrap();
    fs::write(
        dir.join("GlobalCaller.cs"),
        "namespace Demo.Other; class GlobalCaller { int Call() => GlobalOuter.Helper.Utility(1); }
",
    )
    .unwrap();

    let target = "Demo::Outer::Helper::Utility";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 4);
    assert_eq!(
        live.callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Demo::App::LocalCaller::Call",
            "Demo::Other::GlobalCaller::Call"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 4);
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "Demo::App::LocalCaller::Call",
            "Demo::Other::GlobalCaller::Call"
        ]
    );
}

#[test]
fn does_not_trace_csharp_nested_type_static_calls_past_a_nearer_type() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("Outer.cs"),
        "namespace Demo; class Outer { class Helper { public static int Utility(int value) => value; } }
",
    )
    .unwrap();
    fs::write(
        dir.join("Nearer.cs"),
        "namespace Demo.App; class Outer { class Helper { public int Utility(int value) => value; } }
",
    )
    .unwrap();
    fs::write(
        dir.join("NearerCaller.cs"),
        "namespace Demo.App.Tools; class NearerCaller { int Call() => Outer.Helper.Utility(1); }
",
    )
    .unwrap();

    let target = "Demo::Outer::Helper::Utility";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_var_arity_method_hop_field_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Group {
    Entry entry = new Entry();
    Group inner() { return this; }
    Group inner(int value) { return this; }
    Group makeFoo(int value) { return new Group(); }
}
class Util {
    static Group make(int value) { return new Group(); }
}
class Caller {
    Group group = new Group();
    Group makeFoo(int value) { return new Group(); }
    int bareArityFactoryHop() {
        var v = makeFoo(1).entry;
        return v.helper(1);
    }
    int boundArityFactoryHop() {
        var v = group.makeFoo(1).entry;
        return v.helper(1);
    }
    int staticArityFactoryHop() {
        var v = Util.make(1).entry;
        return v.helper(1);
    }
    int arityChainHop() {
        var v = group.makeFoo(1).inner(0).entry;
        return v.helper(1);
    }
    int directArityHop() {
        return group.makeFoo(1).entry.helper(1);
    }
    int arityMemberHop() {
        return group.inner(1).entry.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 6);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::arityChainHop",
            "com::example::Caller::arityMemberHop",
            "com::example::Caller::bareArityFactoryHop",
            "com::example::Caller::boundArityFactoryHop",
            "com::example::Caller::directArityHop",
            "com::example::Caller::staticArityFactoryHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 6);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::arityChainHop",
            "com::example::Caller::arityMemberHop",
            "com::example::Caller::bareArityFactoryHop",
            "com::example::Caller::boundArityFactoryHop",
            "com::example::Caller::directArityHop",
            "com::example::Caller::staticArityFactoryHop"
        ]
    );
}

#[test]
fn traces_java_var_arity_method_hop_field_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Entry { int helper(int value) { return value; } }
class Group { Entry entry = new Entry(); }
class Caller {
    Group makeFoo(int value) { return new Group(); }
    int run() {
        var v = makeFoo(1).entry;
        return v.helper(1);
    }
}
";
    let helper_symbol = "com::example::Entry::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_arity_method_hop_field_receiver_calls_across_files_with_static_import() {
    let dir = temporary_dir();
    let factory_dir = dir.join("src").join("pkg").join("factory");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let factory_path = factory_dir.join("Util.java");
    let caller_path = caller_dir.join("Caller.java");
    let helper_path = helper_dir.join("Helper.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&factory_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::create_dir_all(&helper_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Helper { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &factory_path,
        "package pkg.factory;
import pkg.helper.Helper;
public class Util {
    public static Holder make(int value) { return new Holder(); }
    public static class Holder {
        public Helper entry = new Helper();
        public static Holder nestedMake(int value) { return new Holder(); }
    }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import static pkg.factory.Util.make;
import static pkg.factory.Util.Holder.nestedMake;
public class Caller {
    public int importedArityFactoryHop() {
        var v = make(1).entry;
        return v.helper(1);
    }
    public int importedNestedArityFactoryHop() {
        var v = nestedMake(1).entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::helper::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "pkg::caller::Caller::importedArityFactoryHop",
            "pkg::caller::Caller::importedNestedArityFactoryHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "pkg::caller::Caller::importedArityFactoryHop",
            "pkg::caller::Caller::importedNestedArityFactoryHop"
        ]
    );
}

#[test]
fn java_var_arity_method_hop_field_receiver_calls_fail_closed_for_unsupported_references() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Group { Entry entry = new Entry(); }
class Util {
    static Group make(int value) { return new Group(); }
}
class Caller {
    Group make(int value) { return new Group(); }
    Group makeTwo(int a, int b) { return new Group(); }
    void makeVoid() { }
    int primitive() { return 0; }
    int arityMismatchLow() {
        var v = make().entry;
        return v.helper(1);
    }
    int arityMismatchHigh() {
        var v = make(1, 2).entry;
        return v.helper(1);
    }
    int multiParameterMismatch() {
        var v = makeTwo(1).entry;
        return v.helper(1);
    }
    int staticArityMismatch() {
        var v = Util.make(1, 2).entry;
        return v.helper(1);
    }
    int unknownFactory() {
        var v = missing(1).entry;
        return v.helper(1);
    }
    int voidFactory() {
        var v = makeVoid().entry;
        return v.helper(1);
    }
    int primitiveFactory() {
        var v = primitive().entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "arity-mismatched, unknown, void-returning, and primitive-returning method-call hops must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_csharp_enclosing_namespace_static_calls_in_live_workspace_and_persisted_index() {
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
        "namespace Demo.App.Tools;
class Caller {
    int Call() => Helper.Utility(1);
    int InstanceCall() => Helper.Instance(1);
    int ParamsCall() => Helper.Flexible(1);
}
",
    )
    .unwrap();

    let static_target = "Demo::Helper::Utility";
    let live = trace_symbol_graph(&dir, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::App::Tools::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, static_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "Demo::App::Tools::Caller::Call"
    );

    for target in ["Demo::Helper::Instance", "Demo::Helper::Flexible"] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn does_not_trace_csharp_enclosing_namespace_static_calls_past_a_nearer_type() {
    let dir = temporary_dir();
    let db_path = dir.join("symbols.db");
    fs::write(
        dir.join("OuterHelper.cs"),
        "namespace Demo; class Helper { public static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("NearerHelper.cs"),
        "namespace Demo.App; class Helper { public int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        dir.join("Caller.cs"),
        "namespace Demo.App.Tools; class Caller { int Call() => Helper.Utility(1); }
",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Demo::Helper::Utility", TraceDirection::Callers).unwrap();
    assert!(live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Demo::Helper::Utility", TraceDirection::Callers)
            .unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_csharp_same_namespace_static_interface_calls_across_files() {
    let dir = temporary_dir();
    let interface_path = dir.join("Tools.cs");
    let caller_path = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &interface_path,
        "namespace Demo; interface Tools { static int Utility(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "namespace Demo; class Caller { int Call() => Tools.Utility(1); }
",
    )
    .unwrap();

    let target = "Demo::Tools::Utility";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Demo::Caller::Call");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Demo::Caller::Call");
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
fn traces_java_same_file_static_type_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper {
    static int utility(int value) { return value; }
    static int flexible(int... values) { return values.length; }
    int instance(int value) { return value; }
}
class Main {
    int caller() { return Helper.utility(1); }
    int parameterShadowed(Helper Helper) { return Helper.utility(1); }
    int localTypeShadowed() { class Helper {} return Helper.utility(1); }
    int nonStatic() { return Helper.instance(1); }
    int varargs() { return Helper.flexible(1); }
}
class FieldShadowing {
    private Helper Helper;
    int fieldShadowed() { return Helper.utility(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::utility";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, helper_symbol);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, helper_symbol);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");

    for target in [
        "com::example::Helper::instance",
        "com::example::Helper::flexible",
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn traces_java_same_file_static_type_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    let overlay = "class Helper { static int utility(int value) { return value; } }
class Main { int caller() { return Helper.utility(1); } }
";
    fs::write(
        &source_path,
        "class Stale {}
",
    )
    .unwrap();

    let helper_symbol = "Helper::utility";
    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Main::caller");
}

#[test]
fn traces_java_same_package_outer_static_type_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let outer_path = source_dir.join("Outer.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example; class Main { int caller() { return Outer.Helper.utility(1); } int shadowed(Outer Outer) { return Outer.Helper.utility(1); } }
",
    )
    .unwrap();
    fs::write(
        &outer_path,
        "package com.example; class Outer { static class Helper { static int utility(int value) { return value; } int instance(int value) { return value; } } }
",
    )
    .unwrap();

    let helper_symbol = "com::example::Outer::Helper::utility";
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

    let instance_symbol = "com::example::Outer::Helper::instance";
    let live_instance = trace_symbol_graph(&dir, instance_symbol, TraceDirection::Callers).unwrap();
    assert!(live_instance.callers.is_empty());
    let persisted_instance =
        trace_symbol_graph_from_index(&db_path, instance_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted_instance.callers.is_empty());
}

#[test]
fn traces_java_outer_static_type_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let outer_path = source_dir.join("Outer.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &outer_path,
        "package com.example; class Outer { static class Helper { static int utility(int value) { return value; } } }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay =
        "package com.example; class Main { int caller() { return Outer.Helper.utility(1); } }
";
    let helper_symbol = "com::example::Outer::Helper::utility";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");
}

#[test]
fn traces_java_explicit_imported_outer_static_type_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let outer_dir = dir.join("src").join("com").join("base");
    let caller_dir = dir.join("src").join("com").join("child");
    let caller_path = caller_dir.join("Main.java");
    let outer_path = outer_dir.join("Outer.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&outer_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.child; import com.base.Outer; class Main { int caller() { return Outer.Helper.utility(1); } int shadowed(Outer Outer) { return Outer.Helper.utility(1); } }
",
    )
    .unwrap();
    fs::write(
        &outer_path,
        "package com.base; class Outer { static class Helper { static int utility(int value) { return value; } int instance(int value) { return value; } } }
",
    )
    .unwrap();

    let helper_symbol = "com::base::Outer::Helper::utility";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, helper_symbol);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::child::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, helper_symbol);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::child::Main::caller");
}

#[test]
fn traces_java_same_package_static_interface_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let interface_path = source_dir.join("Tools.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example; class Main { int caller() { return Tools.utility(1); } int shadowed(Tools Tools) { return Tools.utility(1); } }
",
    )
    .unwrap();
    fs::write(
        &interface_path,
        "package com.example; interface Tools { static int utility(int value) { return value; } }
",
    )
    .unwrap();

    let target = "com::example::Tools::utility";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");
}

#[test]
fn traces_java_static_interface_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Tools.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example; interface Tools { static int utility(int value) { return value; } } class Main { int caller() { return Tools.utility(1); } }
";
    let target = "com::example::Tools::utility";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

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
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");
}

#[test]
fn traces_java_same_package_default_interface_methods_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let interface_path = source_dir.join("Defaults.java");
    let abstract_interface_path = source_dir.join("Abstracts.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example;
class Main implements Defaults { int caller() { return helper(1); } int thisCaller() { return this.helper(1); } }
class AbstractMain implements Abstracts { int caller() { return helper(1); } }
",
    )
    .unwrap();
    fs::write(
        &interface_path,
        "package com.example; interface Defaults { default int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &abstract_interface_path,
        "package com.example; interface Abstracts { int helper(int value); }
",
    )
    .unwrap();

    let default_target = "com::example::Defaults::helper";
    let abstract_target = "com::example::Abstracts::helper";
    let live = trace_symbol_graph(&dir, default_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callers.len(), 2);
    assert_eq!(
        live.callers
            .iter()
            .map(|caller| caller.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Main::caller",
            "com::example::Main::thisCaller"
        ]
    );
    assert!(
        trace_symbol_graph(&dir, abstract_target, TraceDirection::Callers)
            .unwrap()
            .callers
            .is_empty()
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, default_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callers.len(), 2);
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|caller| caller.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Main::caller",
            "com::example::Main::thisCaller"
        ]
    );
    assert!(
        trace_symbol_graph_from_index(&db_path, abstract_target, TraceDirection::Callers)
            .unwrap()
            .callers
            .is_empty()
    );
}

#[test]
fn traces_java_unambiguous_default_methods_across_multiple_direct_interfaces() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let primary_path = source_dir.join("Primary.java");
    let empty_path = source_dir.join("Empty.java");
    let abstract_path = source_dir.join("Abstracts.java");
    let secondary_path = source_dir.join("Secondary.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example; class Main implements Primary, Empty { int caller() { return helper(1); } int thisCaller() { return this.helper(1); } } class AbstractBlocked implements Primary, Abstracts { int caller() { return helper(1); } } class DefaultBlocked implements Primary, Secondary { int caller() { return helper(1); } }
",
    )
    .unwrap();
    fs::write(
        &primary_path,
        "package com.example; interface Primary { default int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &empty_path,
        "package com.example; interface Empty {}
",
    )
    .unwrap();
    fs::write(
        &abstract_path,
        "package com.example; interface Abstracts { int helper(int value); }
",
    )
    .unwrap();
    fs::write(
        &secondary_path,
        "package com.example; interface Secondary { default int helper(int value) { return value; } }
",
    )
    .unwrap();

    let target = "com::example::Primary::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 5);
    assert_eq!(
        live.callers
            .iter()
            .map(|caller| caller.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Main::caller",
            "com::example::Main::thisCaller"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 5);
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|caller| caller.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Main::caller",
            "com::example::Main::thisCaller"
        ]
    );
}

#[test]
fn traces_java_unique_default_interface_inheritance_chains_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let child_path = source_dir.join("Child.java");
    let root_path = source_dir.join("Root.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example; class Main implements Child { int caller() { return helper(1); } int thisCaller() { return this.helper(1); } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.example; interface Child extends Root {}
",
    )
    .unwrap();
    fs::write(
        &root_path,
        "package com.example; interface Root { default int helper(int value) { return value; } }
",
    )
    .unwrap();

    let target = "com::example::Root::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(
        live.callers
            .iter()
            .map(|caller| caller.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Main::caller",
            "com::example::Main::thisCaller"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|caller| caller.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Main::caller",
            "com::example::Main::thisCaller"
        ]
    );
}

#[test]
fn traces_java_same_package_outer_default_interface_inheritance_chains() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let outer_path = source_dir.join("Outer.java");
    let root_path = source_dir.join("Root.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example; class Main implements Outer.Child { int caller() { return helper(1); } }
",
    )
    .unwrap();
    fs::write(
        &outer_path,
        "package com.example; class Outer { interface Child extends Root {} }
",
    )
    .unwrap();
    fs::write(
        &root_path,
        "package com.example; interface Root { default int helper(int value) { return value; } }
",
    )
    .unwrap();

    let target = "com::example::Root::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");
}

#[test]
fn traces_java_explicit_imported_outer_default_interface_inheritance_chains() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let caller_dir = dir.join("src").join("com").join("child");
    let outer_path = base_dir.join("Outer.java");
    let root_path = base_dir.join("Root.java");
    let caller_path = caller_dir.join("Main.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &outer_path,
        "package com.base; class Outer { interface Child extends Root {} }
",
    )
    .unwrap();
    fs::write(
        &root_path,
        "package com.base; interface Root { default int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package com.child; import com.base.Outer; class Main implements Outer.Child { int caller() { return helper(1); } }
",
    )
    .unwrap();

    let target = "com::base::Root::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::child::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::child::Main::caller");
}

#[test]
fn traces_java_explicit_imported_default_interface_inheritance_chains() {
    let dir = temporary_dir();
    let root_dir = dir.join("src").join("com").join("root");
    let middle_dir = dir.join("src").join("com").join("middle");
    let caller_dir = dir.join("src").join("com").join("child");
    let root_path = root_dir.join("Root.java");
    let child_path = middle_dir.join("Child.java");
    let caller_path = caller_dir.join("Main.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&root_dir).unwrap();
    fs::create_dir_all(&middle_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &root_path,
        "package com.root; interface Root { default int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.middle; import com.root.Root; interface Child extends Root {}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package com.child; import com.middle.Child; class Main implements Child { int caller() { return helper(1); } }
",
    )
    .unwrap();

    let target = "com::root::Root::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::child::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::child::Main::caller");

    let caller_overlay = "package com.child; import com.middle.Child; class Main implements Child { int caller() { return this.helper(1); } }
";
    let live_overlay = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live_overlay.callers.len(), 1);
    assert_eq!(
        live_overlay.callers[0].symbol_id,
        "com::child::Main::caller"
    );

    let persisted_overlay = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted_overlay.callers.len(), 1);
    assert_eq!(
        persisted_overlay.callers[0].symbol_id,
        "com::child::Main::caller"
    );
}

#[test]
fn traces_java_default_interface_methods_through_unique_empty_superclass_chains() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let base_path = source_dir.join("Base.java");
    let blocking_base_path = source_dir.join("BlockingBase.java");
    let interface_path = source_dir.join("Defaults.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example; class Main extends Base implements Defaults { int caller() { return helper(1); } int thisCaller() { return this.helper(1); } } class Blocked extends BlockingBase implements Defaults { int caller() { return helper(1); } int thisCaller() { return this.helper(1); } }
",
    )
    .unwrap();
    fs::write(
        &base_path,
        "package com.example; class Base {}
",
    )
    .unwrap();
    fs::write(
        &blocking_base_path,
        "package com.example; class BlockingBase { int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &interface_path,
        "package com.example; interface Defaults { default int helper(int value) { return value; } }
",
    )
    .unwrap();

    let target = "com::example::Defaults::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 4);
    assert_eq!(
        live.callers
            .iter()
            .map(|caller| caller.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Main::caller",
            "com::example::Main::thisCaller"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 4);
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|caller| caller.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Main::caller",
            "com::example::Main::thisCaller"
        ]
    );
}

#[test]
fn traces_java_default_interface_inheritance_chains_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Root.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example; class Base {} interface Root { default int helper(int value) { return value; } } interface Child extends Root {} class Main extends Base implements Child { int caller() { return this.helper(1); } }
";
    let target = "com::example::Root::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

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
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");
}

#[test]
fn traces_java_default_interface_methods_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Defaults.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example; interface Defaults { default int helper(int value) { return value; } } interface Empty {} class Main implements Defaults, Empty { int caller() { return this.helper(1); } }
";
    let target = "com::example::Defaults::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

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
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");
}

#[test]
fn traces_java_explicit_imported_default_interface_methods_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let interface_dir = dir.join("src").join("com").join("base");
    let caller_dir = dir.join("src").join("com").join("child");
    let caller_path = caller_dir.join("Main.java");
    let interface_path = interface_dir.join("Defaults.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&interface_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.child; import com.base.Defaults; class Main implements Defaults { int caller() { return helper(1); } }
",
    )
    .unwrap();
    fs::write(
        &interface_path,
        "package com.base; interface Defaults { default int helper(int value) { return value; } }
",
    )
    .unwrap();

    let target = "com::base::Defaults::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::child::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::child::Main::caller");
}

#[test]
fn traces_java_same_package_static_type_calls_across_files_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let helper_path = source_dir.join("Helper.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example;
class Main {
    int caller() { return Helper.utility(1); }
    int parameterShadowed(Helper Helper) { return Helper.utility(1); }
    int nonStatic() { return Helper.instance(1); }
    int varargs() { return Helper.flexible(1); }
}
",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package com.example;
class Helper {
    static int utility(int value) { return value; }
    static int flexible(int... values) { return values.length; }
    int instance(int value) { return value; }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::utility";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");

    for target in [
        "com::example::Helper::instance",
        "com::example::Helper::flexible",
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn ignores_ambiguous_java_same_package_static_type_calls_across_files() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("Main.java"),
        "package com.example; class Main { int caller() { return Helper.utility(1); } }
",
    )
    .unwrap();
    fs::write(
        source_dir.join("First.java"),
        "package com.example; class Helper { static int utility(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        source_dir.join("Second.java"),
        "package com.example; class Helper { static int utility(int value) { return value; } }
",
    )
    .unwrap();

    let live =
        trace_symbol_graph(&dir, "com::example::Main::caller", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index(
        &db_path,
        "com::example::Main::caller",
        TraceDirection::Callees,
    )
    .unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn traces_java_same_package_static_type_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let helper_path = source_dir.join("Helper.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package com.example; class Helper { static int utility(int value) { return value; } }
",
    )
    .unwrap();
    let overlay = "package com.example; class Main { int caller() { return Helper.utility(1); } }
";
    let helper_symbol = "com::example::Helper::utility";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");
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

#[test]
fn traces_kotlin_same_file_top_level_function_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Counter.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nfun helper(value: Int): Int = value\n\nfun caller(): Int = helper(1)\n",
    )
    .unwrap();

    let helper_path = "com::example::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, helper_path);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_cross_file_same_package_top_level_function_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let main_path = dir.join("Main.kt");
    let helper_path_file = dir.join("Helper.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &main_path,
        "package com.example\n\nfun caller(): Int = helper(1)\n",
    )
    .unwrap();
    fs::write(
        &helper_path_file,
        "package com.example\n\nfun helper(value: Int): Int = value\n",
    )
    .unwrap();

    let helper_path = "com::example::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, helper_path);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_enclosing_type_members_and_package_functions_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Counter.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nfun helper(value: Int): Int = value\n\nclass Counter {\n    fun own(): Int = 1\n    fun caller(): Int = own() + helper(2)\n}\n",
    )
    .unwrap();

    let helper_path = "com::example::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Counter::caller");

    let own_path = "com::example::Counter::own";
    let own_live = trace_symbol_graph(&dir, own_path, TraceDirection::Callers).unwrap();
    assert_eq!(own_live.callers.len(), 1);
    assert_eq!(
        own_live.callers[0].symbol_id,
        "com::example::Counter::caller"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "com::example::Counter::caller"
    );
    let own_persisted =
        trace_symbol_graph_from_index(&db_path, own_path, TraceDirection::Callers).unwrap();
    assert_eq!(own_persisted.callers.len(), 1);
    assert_eq!(
        own_persisted.callers[0].symbol_id,
        "com::example::Counter::caller"
    );
}

#[test]
fn handles_kotlin_ambiguous_package_names_and_qualified_receiver_calls() {
    let dir = temporary_dir();
    let first = dir.join("First.kt");
    let second = dir.join("Second.kt");
    let holder = dir.join("Holder.kt");
    fs::write(
        &first,
        "package com.example\n\nfun helper(value: Int): Int = value\n",
    )
    .unwrap();
    fs::write(
        &second,
        "package com.example\n\nfun helper(value: Int): Int = value + 1\n",
    )
    .unwrap();
    fs::write(
        &holder,
        "package com.example\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\nclass Holder {\n    fun run(): Int {\n        val other = Other()\n        return other.helper(1)\n    }\n}\n",
    )
    .unwrap();

    // Ambiguous package-level name: two same-package top-level functions fail closed.
    let trace = trace_symbol_graph(&dir, "com::example::helper", TraceDirection::Callers).unwrap();
    assert!(trace.callers.is_empty());

    // Qualified receiver calls resolve when the receiver type is pinned to a local class.
    let other_helper =
        trace_symbol_graph(&dir, "com::example::Other::helper", TraceDirection::Callers).unwrap();
    assert_eq!(other_helper.callers.len(), 1);
    assert_eq!(
        other_helper.callers[0].symbol_id,
        "com::example::Holder::run"
    );
}
#[test]
fn traces_kotlin_cross_package_imported_top_level_function_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let caller_path = dir.join("Caller.kt");
    let helper_path = dir.join("Helper.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package com.example\n\nimport org.util.helper\n\nfun caller(): Int = helper(1)\n",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package org.util\n\nfun helper(value: Int): Int = value\n",
    )
    .unwrap();

    let helper_semantic_path = "org::util::helper";
    let live = trace_symbol_graph(&dir, helper_semantic_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, helper_semantic_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_semantic_path, TraceDirection::Callers)
            .unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, helper_semantic_path);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_aliased_imported_top_level_function_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let caller_path = dir.join("Caller.kt");
    let helper_path = dir.join("Helper.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package com.example\n\nimport org.util.helper as h\n\nfun caller(): Int = h(1)\n",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package org.util\n\nfun helper(value: Int): Int = value\n",
    )
    .unwrap();

    let helper_semantic_path = "org::util::helper";
    let live = trace_symbol_graph(&dir, helper_semantic_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_semantic_path, TraceDirection::Callers)
            .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn does_not_trace_kotlin_wildcard_or_competing_imported_function_calls() {
    let dir = temporary_dir();
    let wildcard_caller = dir.join("WildcardCaller.kt");
    let competing_caller = dir.join("CompetingCaller.kt");
    let wildcard_helper = dir.join("WildcardHelper.kt");
    let first_helper = dir.join("FirstHelper.kt");
    let second_helper = dir.join("SecondHelper.kt");
    fs::write(
        &wildcard_caller,
        "package com.example\n\nimport org.util.*\n\nfun wildcardCaller(): Int = helper(1)\n",
    )
    .unwrap();
    fs::write(
        &competing_caller,
        "package com.example\n\nimport org.first.helper\nimport org.second.helper\n\nfun competingCaller(): Int = helper(1)\n",
    )
    .unwrap();
    fs::write(
        &wildcard_helper,
        "package org.util\n\nfun helper(value: Int): Int = value\n",
    )
    .unwrap();
    fs::write(
        &first_helper,
        "package org.first\n\nfun helper(value: Int): Int = value\n",
    )
    .unwrap();
    fs::write(
        &second_helper,
        "package org.second\n\nfun helper(value: Int): Int = value\n",
    )
    .unwrap();

    // Wildcard imports do not produce a unique binding.
    let wildcard = trace_symbol_graph(&dir, "org::util::helper", TraceDirection::Callers).unwrap();
    assert!(wildcard.callers.is_empty());

    // Competing explicit imports of the same simple name fail closed.
    let first = trace_symbol_graph(&dir, "org::first::helper", TraceDirection::Callers).unwrap();
    assert!(first.callers.is_empty());
    let second = trace_symbol_graph(&dir, "org::second::helper", TraceDirection::Callers).unwrap();
    assert!(second.callers.is_empty());
}
#[test]
fn traces_kotlin_qualified_receiver_calls_via_local_constructor_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let holder = dir.join("Holder.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &holder,
        "package com.example\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\nclass Holder {\n    fun run(): Int {\n        val other = Other()\n        return other.helper(1)\n    }\n}\n",
    )
    .unwrap();

    let helper_path = "com::example::Other::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Holder::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Holder::run");
}

#[test]
fn traces_kotlin_qualified_receiver_calls_via_parameter_and_class_property_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Counters.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Counter {\n    fun increment(value: Int): Int = value\n}\n\nclass Holder {\n    val counter = Counter()\n    fun viaProperty(): Int = counter.increment(1)\n}\n\nfun viaParameter(counter: Counter): Int = counter.increment(2)\n",
    )
    .unwrap();

    let increment_path = "com::example::Counter::increment";
    let live = trace_symbol_graph(&dir, increment_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut caller_ids = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    caller_ids.sort_unstable();
    assert_eq!(
        caller_ids,
        vec![
            "com::example::Holder::viaProperty",
            "com::example::viaParameter"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, increment_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
}

#[test]
fn does_not_trace_kotlin_qualified_receiver_calls_with_unknown_or_ambiguous_receiver_types() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    fs::write(
        &source_path,
        "package com.example\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\nclass Third {\n    fun helper(value: Int): Int = value\n}\n\nfun ambiguousReceiver(): Int {\n    val other = if (true) Other() else Third()\n    return other.helper(1)\n}\n",
    )
    .unwrap();

    let helper_path = "com::example::Other::helper";
    let trace = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert!(trace.callers.is_empty());
}
#[test]
fn traces_kotlin_same_file_extension_function_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Caller.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Other {\n    fun member(value: Int): Int = value\n}\n\nfun Other.helper(value: Int): Int = value\n\nclass Holder {\n    fun run(): Int {\n        val other = Other()\n        return other.helper(1)\n    }\n}\n",
    )
    .unwrap();

    let helper_path = "com::example::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Holder::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Holder::run");
}

#[test]
fn traces_kotlin_cross_file_same_package_extension_function_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let caller_path = dir.join("Caller.kt");
    let extension_path = dir.join("Extensions.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package com.example\n\nclass Other\n\nclass Holder {\n    fun run(): Int {\n        val other = Other()\n        return other.helper(1)\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        &extension_path,
        "package com.example\n\nfun Other.helper(value: Int): Int = value\n",
    )
    .unwrap();

    let helper_path = "com::example::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Holder::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Holder::run");
}

#[test]
fn traces_kotlin_cross_package_imported_extension_function_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let caller_path = dir.join("Caller.kt");
    let extension_path = dir.join("Extensions.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package com.example\n\nimport org.util.Other\nimport org.util.helper\n\nclass Holder {\n    fun run(other: Other): Int = other.helper(1)\n}\n",
    )
    .unwrap();
    fs::write(
        &extension_path,
        "package org.util\n\nclass Other\n\nfun Other.helper(value: Int): Int = value\n",
    )
    .unwrap();

    let helper_path = "org::util::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Holder::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Holder::run");
}

#[test]
fn does_not_trace_kotlin_extension_calls_when_member_or_ambiguous_targets_exist() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    fs::write(
        &source_path,
        "package com.example\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\nfun Other.helper(value: Int): Int = value + 1\n\nclass Holder {\n    fun run(): Int {\n        val other = Other()\n        return other.helper(1)\n    }\n}\n",
    )
    .unwrap();

    // A member function shadows the extension and resolves to the member, not the extension.
    let member_path = "com::example::Other::helper";
    let member_trace = trace_symbol_graph(&dir, member_path, TraceDirection::Callers).unwrap();
    assert_eq!(member_trace.callers.len(), 1);
    assert_eq!(
        member_trace.callers[0].symbol_id,
        "com::example::Holder::run"
    );
    let extension_path = "com::example::helper";
    let extension_trace =
        trace_symbol_graph(&dir, extension_path, TraceDirection::Callers).unwrap();
    assert!(extension_trace.callers.is_empty());
}

#[test]
fn does_not_trace_kotlin_extension_calls_with_ambiguous_or_unknown_receivers() {
    let dir = temporary_dir();
    let caller_path = dir.join("Caller.kt");
    let first_extension = dir.join("FirstExtensions.kt");
    let second_extension = dir.join("SecondExtensions.kt");
    fs::write(
        &caller_path,
        "package com.example\n\nclass Other\n\nfun unknownReceiver(): Int {\n    val other = makeOther()\n    return other.helper(1)\n}\n\nfun makeOther(): Other = Other()\n\nclass Holder {\n    fun run(): Int {\n        val other = Other()\n        return other.helper(1)\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        &first_extension,
        "package com.example\n\nfun Other.helper(value: Int): Int = value + 1\n",
    )
    .unwrap();
    fs::write(
        &second_extension,
        "package com.example\n\nfun Other.helper(value: Int): Int = value + 2\n",
    )
    .unwrap();

    // An unknown receiver binding (function-return initializer) fails closed.
    let unknown_trace =
        trace_symbol_graph(&dir, "com::example::helper", TraceDirection::Callers).unwrap();
    assert!(unknown_trace.callers.is_empty());

    // Two same-package extension declarations of the same name and arity for the same
    // receiver type make the extension lookup ambiguous and fail closed; the local
    // `Other()` constructor call still resolves to the class.
    let ambiguous_trace =
        trace_symbol_graph(&dir, "com::example::Holder::run", TraceDirection::Callees).unwrap();
    assert!(
        ambiguous_trace
            .callees
            .iter()
            .all(|callee| callee.symbol_id != "com::example::helper"),
        "ambiguous extension must not resolve"
    );
    assert!(
        ambiguous_trace
            .callees
            .iter()
            .any(|callee| callee.symbol_id == "com::example::Other")
    );
}

#[test]
fn traces_kotlin_same_file_property_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\nclass Group {\n    val member: Other = Other()\n}\n\nclass Holder {\n    fun run(): Int {\n        val group = Group()\n        return group.member.helper(1)\n    }\n}\n",
    )
    .unwrap();

    let helper_path = "com::example::Other::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Holder::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Holder::run");
}

#[test]
fn traces_kotlin_cross_file_property_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let caller_path = dir.join("Caller.kt");
    let helper_path = dir.join("Helper.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package com.example\n\nclass Group {\n    val member: Other = Other()\n}\n\nclass Holder {\n    fun run(): Int {\n        val group = Group()\n        return group.member.memberHelper(1)\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package com.example\n\nclass Other {\n    fun memberHelper(value: Int): Int = value\n}\n",
    )
    .unwrap();

    let member_helper = "com::example::Other::memberHelper";
    let live = trace_symbol_graph(&dir, member_helper, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, member_helper);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Holder::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, member_helper, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Holder::run");
}

#[test]
fn traces_kotlin_property_chain_receiver_calls_to_extension_functions_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Other\n\nfun Other.helper(value: Int): Int = value\n\nclass Group {\n    val member: Other = Other()\n}\n\nclass Holder {\n    fun run(): Int {\n        val group = Group()\n        return group.member.helper(1)\n    }\n}\n",
    )
    .unwrap();

    let helper_path = "com::example::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Holder::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Holder::run");
}

#[test]
fn traces_kotlin_constructor_inferred_property_chain_receiver_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\nclass Group {\n    val inferred = Other()\n}\n\nfun unknownReceiver(): Int {\n    val group = Group()\n    return group.inferred.helper(1)\n}\n",
    )
    .unwrap();

    let helper_path = "com::example::Other::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::unknownReceiver");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "com::example::unknownReceiver"
    );
}

#[test]
fn does_not_trace_kotlin_property_chain_receiver_calls_with_missing_properties_or_undeclared_factories()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    fs::write(
        &source_path,
        "package com.example\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\nclass Group {\n    val derived = makeOther()\n}\n\nfun missingProperty(): Int {\n    val group = Group()\n    return group.absent.helper(1)\n}\n\nfun undeclaredReturn(): Int {\n    val group = Group()\n    return group.derived.helper(1)\n}\n\nfun makeOther() = Other()\n",
    )
    .unwrap();

    // Missing properties and factory calls without a declared return type
    // fail closed instead of guessing a chain target. Only bare constructor
    // initializers and factories with a declared return type pin a receiver.
    let helper_path = "com::example::Other::helper";
    let trace = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert!(trace.callers.is_empty());
}

#[test]
fn traces_kotlin_function_return_type_property_chain_receiver_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\nclass Group {\n    val derived = makeOther()\n}\n\nfun caller(): Int {\n    val group = Group()\n    return group.derived.helper(1)\n}\n\nfun makeOther(): Other = Other()\n",
    )
    .unwrap();

    // A function-call property initializer pins the receiver through the
    // function's declared return type.
    let helper_path = "com::example::Other::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_cross_file_same_package_factory_property_chain_receiver_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let caller_path = dir.join("Caller.kt");
    let factories_path = dir.join("Factories.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package com.example\n\nclass Group {\n    val derived = makeOther()\n}\n\nfun caller(): Int {\n    val group = Group()\n    return group.derived.helper(1)\n}\n",
    )
    .unwrap();
    fs::write(
        &factories_path,
        "package com.example\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\nfun makeOther(): Other = Other()\n",
    )
    .unwrap();

    let helper_path = "com::example::Other::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_imported_factory_property_chain_receiver_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let caller_path = dir
        .join("src")
        .join("com")
        .join("example")
        .join("Caller.kt");
    let factory_path = dir
        .join("src")
        .join("org")
        .join("util")
        .join("Factories.kt");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(factory_path.parent().unwrap()).unwrap();
    fs::write(
        &caller_path,
        "package com.example\n\nimport org.util.makeOther\n\nclass Group {\n    val derived = makeOther()\n}\n\nfun caller(): Int {\n    val group = Group()\n    return group.derived.helper(1)\n}\n",
    )
    .unwrap();
    fs::write(
        &factory_path,
        "package org.util\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\nfun makeOther(): Other = Other()\n",
    )
    .unwrap();

    let helper_path = "org::util::Other::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_interface_receiver_member_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\ninterface Renderer {\n    fun render(value: Int): Int = value\n}\n\nclass Screen : Renderer\n\nfun caller(): Int {\n    val renderer: Renderer = Screen()\n    return renderer.render(1)\n}\n",
    )
    .unwrap();

    let render_path = "com::example::Renderer::render";
    let live = trace_symbol_graph(&dir, render_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, render_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, render_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_interface_property_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\ninterface Renderer {\n    fun render(value: Int): Int = value\n}\n\nclass Screen : Renderer\n\nclass Group {\n    val renderer: Renderer = Screen()\n}\n\nfun caller(): Int {\n    val group = Group()\n    return group.renderer.render(1)\n}\n",
    )
    .unwrap();

    let render_path = "com::example::Renderer::render";
    let live = trace_symbol_graph(&dir, render_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, render_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, render_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_cross_file_imported_interface_receiver_member_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let caller_path = dir.join("Caller.kt");
    let interface_path = dir.join("Renderer.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package com.example\n\nimport org.util.Renderer\n\nfun caller(): Int {\n    val renderer: Renderer = makeRenderer()\n    return renderer.render(1)\n}\n",
    )
    .unwrap();
    fs::write(
        &interface_path,
        "package org.util\n\ninterface Renderer {\n    fun render(value: Int): Int = value\n}\n",
    )
    .unwrap();

    let render_path = "org::util::Renderer::render";
    let live = trace_symbol_graph(&dir, render_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, render_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_typealias_receiver_member_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\ntypealias Helper = Other\n\nfun caller(): Int {\n    val helper: Helper = Other()\n    return helper.helper(1)\n}\n",
    )
    .unwrap();

    let helper_path = "com::example::Other::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_typealias_property_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\ntypealias Helper = Other\n\nclass Group {\n    val member: Helper = Other()\n}\n\nfun caller(): Int {\n    val group = Group()\n    return group.member.helper(1)\n}\n",
    )
    .unwrap();

    let helper_path = "com::example::Other::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn does_not_trace_kotlin_typealias_receiver_calls_with_generic_or_cyclic_targets() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    fs::write(
        &source_path,
        "package com.example\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\ntypealias Box<T> = List<T>\n\ntypealias First = Second\ntypealias Second = First\n\nfun genericTarget(): Int {\n    val box: Box<Other> = emptyList()\n    return box.helper(1)\n}\n\nfun cyclicTarget(): Int {\n    val first: First = Other()\n    return first.helper(1)\n}\n",
    )
    .unwrap();

    // Generic alias targets and cyclic alias chains fail closed instead of
    // guessing a receiver or looping forever.
    let helper_path = "com::example::Other::helper";
    let trace = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert!(trace.callers.is_empty());
}

#[test]
fn traces_kotlin_companion_object_member_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Config {\n    companion object {\n        fun helper(value: Int): Int = value\n    }\n}\n\nfun caller(): Int = Config.helper(1)\n",
    )
    .unwrap();

    let helper_path = "com::example::Config::Companion::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_unqualified_companion_member_calls_from_enclosing_class_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Config {\n    companion object {\n        fun helper(value: Int): Int = value\n    }\n    fun run(): Int = helper(1)\n}\n",
    )
    .unwrap();

    let helper_path = "com::example::Config::Companion::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Config::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Config::run");
}

#[test]
fn does_not_trace_kotlin_instance_or_unknown_member_calls_via_class_name() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    fs::write(
        &source_path,
        "package com.example\n\nclass Config {\n    fun instance(value: Int): Int = value\n    companion object {\n        fun helper(value: Int): Int = value\n    }\n}\n\nfun instanceCall(): Int = Config.instance(1)\n\nfun missingCompanionCall(): Int = Config.missing(1)\n",
    )
    .unwrap();

    // A class-name receiver only dispatches to companion members; instance
    // members and unknown companion members fail closed.
    let instance_path = "com::example::Config::instance";
    let instance_trace = trace_symbol_graph(&dir, instance_path, TraceDirection::Callers).unwrap();
    assert!(instance_trace.callers.is_empty());

    let helper_path = "com::example::Config::Companion::helper";
    let helper_trace = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert!(helper_trace.callers.is_empty());
}

#[test]
fn traces_kotlin_explicit_companion_chain_member_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Config {\n    companion object {\n        fun helper(value: Int): Int = value\n    }\n}\n\nfun caller(): Int = Config.Companion.helper(1)\n",
    )
    .unwrap();

    let helper_path = "com::example::Config::Companion::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_companion_property_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Holder {\n    fun run(value: Int): Int = value\n}\n\nclass Config {\n    companion object {\n        val holder = Holder()\n    }\n}\n\nfun caller(): Int = Config.Companion.holder.run(1)\n",
    )
    .unwrap();

    let run_path = "com::example::Holder::run";
    let live = trace_symbol_graph(&dir, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, run_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn does_not_trace_kotlin_instance_or_unknown_members_via_companion_chains() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    fs::write(
        &source_path,
        "package com.example\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\nclass Config {\n    val holder = Other()\n    fun instance(value: Int): Int = value\n    companion object {\n        fun helper(value: Int): Int = value\n    }\n}\n\nfun instanceChain(): Int = Config.Companion.instance(1)\n\nfun unknownCompanionChain(): Int = Config.Companion.missing(1)\n\nfun instancePropertyChain(): Int = Config.holder.helper(1)\n",
    )
    .unwrap();

    // Companion chains never dispatch to instance members, unknown companion
    // members, or instance properties; all fail closed.
    let instance_path = "com::example::Config::instance";
    let instance_trace = trace_symbol_graph(&dir, instance_path, TraceDirection::Callers).unwrap();
    assert!(instance_trace.callers.is_empty());

    let helper_path = "com::example::Other::helper";
    let helper_trace = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert!(helper_trace.callers.is_empty());
}

#[test]
fn traces_kotlin_named_companion_receiver_member_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Config {\n    companion object Factory {\n        fun helper(value: Int): Int = value\n    }\n}\n\nfun canonicalCaller(): Int = Config.Companion.helper(1)\n\nfun namedCaller(): Int = Config.Factory.helper(1)\n",
    )
    .unwrap();

    // Both the canonical `Companion` spelling and the declared companion name
    // resolve to the same canonical companion-member ID.
    let helper_path = "com::example::Config::Companion::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 2);
    let mut caller_ids = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    caller_ids.sort_unstable();
    assert_eq!(
        caller_ids,
        vec!["com::example::canonicalCaller", "com::example::namedCaller"]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut persisted_ids = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    persisted_ids.sort_unstable();
    assert_eq!(
        persisted_ids,
        vec!["com::example::canonicalCaller", "com::example::namedCaller"]
    );
}

#[test]
fn traces_kotlin_named_companion_property_chain_receiver_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Holder {\n    fun run(value: Int): Int = value\n}\n\nclass Config {\n    companion object Factory {\n        val holder = Holder()\n    }\n}\n\nfun caller(): Int = Config.Factory.holder.run(1)\n",
    )
    .unwrap();

    let run_path = "com::example::Holder::run";
    let live = trace_symbol_graph(&dir, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, run_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn does_not_trace_kotlin_unknown_or_instance_members_via_named_companion_chains() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    fs::write(
        &source_path,
        "package com.example\n\nclass Config {\n    fun instance(value: Int): Int = value\n    companion object Factory {\n        fun helper(value: Int): Int = value\n    }\n}\n\nobject Registry {\n    companion object F {\n        fun helper(value: Int): Int = value\n    }\n}\n\nfun unknownCompanionName(): Int = Config.Missing.helper(1)\n\nfun instanceViaNamedCompanion(): Int = Config.Factory.instance(1)\n\nfun objectCompanionChain(): Int = Registry.F.helper(1)\n",
    )
    .unwrap();

    // An unknown companion name, an instance member reached through the
    // companion, and a companion chain rooted at an object declaration all fail
    // closed instead of guessing a target.
    let helper_path = "com::example::Config::Companion::helper";
    let helper_trace = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert!(helper_trace.callers.is_empty());

    let instance_path = "com::example::Config::instance";
    let instance_trace = trace_symbol_graph(&dir, instance_path, TraceDirection::Callers).unwrap();
    assert!(instance_trace.callers.is_empty());

    let object_helper_path = "com::example::Registry::Companion::helper";
    let object_trace =
        trace_symbol_graph(&dir, object_helper_path, TraceDirection::Callers).unwrap();
    assert!(object_trace.callers.is_empty());
}

#[test]
fn traces_kotlin_factory_inferred_local_binding_receiver_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\nfun makeOther(): Other = Other()\n\nfun caller(): Int {\n    val other = makeOther()\n    return other.helper(1)\n}\n",
    )
    .unwrap();

    // A function-return local binding pins the receiver through the factory's
    // declared return type.
    let helper_path = "com::example::Other::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_factory_inferred_binding_property_chain_receiver_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\nclass Group {\n    val derived = Other()\n}\n\nfun makeGroup(): Group = Group()\n\nfun caller(): Int {\n    val group = makeGroup()\n    return group.derived.helper(1)\n}\n",
    )
    .unwrap();

    let helper_path = "com::example::Other::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_factory_inferred_nested_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\nfun makeInner(): Outer.Inner = Outer.Inner()\n\nfun caller(): Int {\n    val inner = makeInner()\n    return inner.helper(1)\n}\n",
    )
    .unwrap();

    // A dotted factory return type pins the receiver through the same nested
    // type-path rules as a directly declared nested type.
    let helper_path = "com::example::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_cross_file_factory_inferred_nested_receiver_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let caller_path = dir.join("Caller.kt");
    let factory_path = dir.join("Factory.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package com.example\n\nfun caller(): Int {\n    val inner = makeInner()\n    return inner.helper(1)\n}\n",
    )
    .unwrap();
    fs::write(
        &factory_path,
        "package com.example\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\nfun makeInner(): Outer.Inner = Outer.Inner()\n",
    )
    .unwrap();

    let helper_path = "com::example::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_imported_factory_inferred_nested_receiver_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let caller_path = dir.join("Caller.kt");
    let factory_path = dir.join("Factory.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package com.example\n\nimport org.util.makeInner\n\nfun caller(): Int {\n    val inner = makeInner()\n    return inner.helper(1)\n}\n",
    )
    .unwrap();
    fs::write(
        &factory_path,
        "package org.util\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\nfun makeInner(): Outer.Inner = Outer.Inner()\n",
    )
    .unwrap();

    let helper_path = "org::util::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_factory_inferred_nested_binding_property_chain_receiver_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Holder {\n    fun run(value: Int): Int = value\n}\n\nclass Outer {\n    class Inner {\n        val holder: Holder = Holder()\n    }\n}\n\nfun makeInner(): Outer.Inner = Outer.Inner()\n\nfun caller(): Int {\n    val inner = makeInner()\n    return inner.holder.run(1)\n}\n",
    )
    .unwrap();

    let run_path = "com::example::Holder::run";
    let live = trace_symbol_graph(&dir, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, run_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn does_not_trace_kotlin_factory_inferred_nested_receiver_calls_with_missing_or_undeclared_returns()
{
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    fs::write(
        &source_path,
        "package com.example\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\nfun makeMissing(): Outer.Absent = Outer.Absent()\n\nfun makeExpressionBody() = Outer.Inner()\n\nfun missingNestedReturn(): Int {\n    val inner = makeMissing()\n    return inner.helper(1)\n}\n\nfun expressionBodyReturn(): Int {\n    val inner = makeExpressionBody()\n    return inner.helper(1)\n}\n\nfun unknownFactory(): Int {\n    val inner = missingFactory()\n    return inner.helper(1)\n}\n",
    )
    .unwrap();

    // A dotted factory return type that names a missing nested type, a factory
    // without a declared return type, and an unknown factory all fail closed
    // instead of guessing a receiver target.
    let helper_path = "com::example::Outer::Inner::helper";
    let trace = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert!(trace.callers.is_empty());
}

#[test]
fn traces_kotlin_dotted_alias_constructor_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\ntypealias Helper = Outer.Inner\n\nfun caller(): Int {\n    val inner = Helper()\n    return inner.helper(1)\n}\n",
    )
    .unwrap();

    // A constructor call through a dotted alias target pins the nested class
    // receiver exactly like the qualified `Outer.Inner()` spelling.
    let helper_path = "com::example::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_dotted_alias_property_type_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Holder {\n    fun run(value: Int): Int = value\n}\n\nclass Outer {\n    class Inner {\n        val holder: Holder = Holder()\n    }\n}\n\ntypealias Helper = Outer.Inner\n\nclass Group {\n    val inner: Helper = Helper()\n}\n\nfun caller(): Int {\n    val group = Group()\n    return group.inner.holder.run(1)\n}\n",
    )
    .unwrap();

    // A declared property type spelled through a dotted alias resolves to the
    // nested type before the property chain dispatches the terminal member.
    let run_path = "com::example::Holder::run";
    let live = trace_symbol_graph(&dir, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, run_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_dotted_alias_companion_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Outer {\n    class Inner {\n        companion object {\n            fun helper(value: Int): Int = value\n        }\n    }\n}\n\ntypealias InnerAlias = Outer.Inner\ntypealias OuterAlias = Outer\n\nfun dottedTargetCaller(): Int = InnerAlias.helper(1)\n\nfun aliasHopCaller(): Int = OuterAlias.Inner.helper(1)\n",
    )
    .unwrap();

    // A dotted alias target reaches the nested companion directly, and an alias
    // first hop reaches it through the nested-companion chain.
    let helper_path = "com::example::Outer::Inner::Companion::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 2);
    let mut caller_ids = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    caller_ids.sort_unstable();
    assert_eq!(
        caller_ids,
        vec![
            "com::example::aliasHopCaller",
            "com::example::dottedTargetCaller"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut persisted_ids = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    persisted_ids.sort_unstable();
    assert_eq!(
        persisted_ids,
        vec![
            "com::example::aliasHopCaller",
            "com::example::dottedTargetCaller"
        ]
    );
}

#[test]
fn does_not_trace_kotlin_dotted_alias_receiver_calls_with_missing_or_cyclic_targets() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    fs::write(
        &source_path,
        "package com.example\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\ntypealias MissingAlias = Outer.Absent\n\ntypealias A = B.C\ntypealias B = A\n\nfun missingTarget(): Int {\n    val inner = MissingAlias()\n    return inner.helper(1)\n}\n\nfun cyclicTarget(): Int {\n    val value = A()\n    return 1\n}\n",
    )
    .unwrap();

    // A dotted alias target naming a missing nested type and a cyclic dotted
    // alias chain both fail closed instead of guessing a receiver or looping.
    let helper_path = "com::example::Outer::Inner::helper";
    let trace = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert!(trace.callers.is_empty());
}

#[test]
fn traces_kotlin_nested_parameter_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\nfun caller(inner: Outer.Inner): Int {\n    return inner.helper(1)\n}\n",
    )
    .unwrap();

    // A dotted parameter type pins the nested receiver exactly like a local
    // constructor binding.
    let helper_path = "com::example::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_nested_enclosing_property_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\nclass Group {\n    val inner: Outer.Inner = Outer.Inner()\n    fun invoke(): Int {\n        return inner.helper(1)\n    }\n}\n",
    )
    .unwrap();

    // An enclosing-class property with a dotted explicit type pins the nested
    // receiver for unqualified member calls inside the class.
    let helper_path = "com::example::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Group::invoke");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "com::example::Group::invoke"
    );
}

#[test]
fn traces_kotlin_imported_nested_parameter_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let caller_path = dir
        .join("src")
        .join("com")
        .join("example")
        .join("Caller.kt");
    let outer_path = dir.join("src").join("org").join("util").join("Outer.kt");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(outer_path.parent().unwrap()).unwrap();
    fs::write(
        &caller_path,
        "package com.example\n\nimport org.util.Outer\n\nfun caller(inner: Outer.Inner): Int {\n    return inner.helper(1)\n}\n",
    )
    .unwrap();
    fs::write(
        &outer_path,
        "package org.util\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n",
    )
    .unwrap();

    let helper_path = "org::util::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn does_not_trace_kotlin_nested_parameter_receiver_calls_with_missing_or_generic_types() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    fs::write(
        &source_path,
        "package com.example\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\nfun missingNested(inner: Outer.Absent): Int {\n    return inner.helper(1)\n}\n\nfun genericNested(inner: List<Outer.Inner>): Int {\n    return inner.helper(1)\n}\n",
    )
    .unwrap();

    // A dotted parameter type naming a missing nested type and a generic
    // parameter type both fail closed instead of guessing a receiver.
    let helper_path = "com::example::Outer::Inner::helper";
    let trace = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert!(trace.callers.is_empty());
}

#[test]
fn traces_kotlin_object_receiver_member_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nobject Config {\n    fun helper(value: Int): Int = value\n}\n\nfun caller(): Int = Config.helper(1)\n",
    )
    .unwrap();

    let helper_path = "com::example::Config::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_cross_file_object_receiver_member_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let caller_path = dir.join("Caller.kt");
    let config_path = dir.join("Config.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package com.example\n\nfun caller(): Int = Config.helper(1)\n",
    )
    .unwrap();
    fs::write(
        &config_path,
        "package com.example\n\nobject Config {\n    fun helper(value: Int): Int = value\n}\n",
    )
    .unwrap();

    let helper_path = "com::example::Config::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_imported_object_receiver_member_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let caller_path = dir.join("Caller.kt");
    let config_path = dir.join("Config.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package com.example\n\nimport org.util.Config\n\nfun caller(): Int = Config.helper(1)\n",
    )
    .unwrap();
    fs::write(
        &config_path,
        "package org.util\n\nobject Config {\n    fun helper(value: Int): Int = value\n}\n",
    )
    .unwrap();

    let helper_path = "org::util::Config::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn does_not_trace_kotlin_object_receiver_calls_with_unknown_shadowed_or_conflicting_names() {
    let dir = temporary_dir();
    let caller_path = dir.join("Caller.kt");
    let config_path = dir.join("Config.kt");
    let imported_path = dir.join("Imported.kt");
    fs::write(
        &caller_path,
        "package com.example\n\nimport org.util.Config\n\nclass Other\n\nfun unknownObject(): Int = Missing.helper(1)\n\nfun shadowedObject(): Int {\n    val Config = Other()\n    return Config.helper(1)\n}\n\nfun conflictingObject(): Int = Config.helper(1)\n",
    )
    .unwrap();
    fs::write(
        &config_path,
        "package com.example\n\nobject Config {\n    fun helper(value: Int): Int = value\n}\n",
    )
    .unwrap();
    fs::write(
        &imported_path,
        "package org.util\n\nobject Config {\n    fun helper(value: Int): Int = value\n}\n",
    )
    .unwrap();

    // Unknown object names, local shadowing of the object name, and a same-package
    // object that conflicts with an explicit import all fail closed.
    for helper_path in ["com::example::Config::helper", "org::util::Config::helper"] {
        let trace = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
        assert!(
            trace.callers.is_empty(),
            "expected no callers for {helper_path}"
        );
    }
}

#[test]
fn traces_kotlin_constructor_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Other\n\ndata class Record(val id: Int)\n\nfun caller(): Other {\n    Other()\n    Record(1)\n    return Other()\n}\n",
    )
    .unwrap();

    let other_path = "com::example::Other";
    let live = trace_symbol_graph(&dir, other_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, other_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, other_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");

    // Data classes are constructible and trace through the same path.
    let record_path = "com::example::Record";
    let record_live = trace_symbol_graph(&dir, record_path, TraceDirection::Callers).unwrap();
    assert_eq!(record_live.callers.len(), 1);
    assert_eq!(record_live.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_cross_file_and_imported_constructor_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let caller_path = dir.join("Caller.kt");
    let same_path = dir.join("SamePackage.kt");
    let imported_path = dir.join("Imported.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package com.example\n\nimport org.util.Imported\n\nfun caller(): Int {\n    SamePackage()\n    Imported()\n    return 0\n}\n",
    )
    .unwrap();
    fs::write(&same_path, "package com.example\n\nclass SamePackage\n").unwrap();
    fs::write(&imported_path, "package org.util\n\nclass Imported\n").unwrap();

    let same_package_path = "com::example::SamePackage";
    let live = trace_symbol_graph(&dir, same_package_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    let imported_class_path = "org::util::Imported";
    let imported_live =
        trace_symbol_graph(&dir, imported_class_path, TraceDirection::Callers).unwrap();
    assert_eq!(imported_live.callers.len(), 1);
    assert_eq!(imported_live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, imported_class_path, TraceDirection::Callers)
            .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn does_not_trace_kotlin_constructor_calls_for_non_constructible_or_ambiguous_classes() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let imported_path = dir.join("Imported.kt");
    fs::write(
        &source_path,
        "package com.example\n\nimport org.util.Other\n\ninterface Service\n\nenum class Color { RED }\n\nsealed class Shape\n\nabstract class Base\n\nannotation class Marker\n\nfun interface Listener\n\nclass Other\n\nfun caller(): Int {\n    Service()\n    Color.RED\n    Shape()\n    Base()\n    Marker()\n    Listener()\n    Other()\n    return 0\n}\n",
    )
    .unwrap();
    fs::write(&imported_path, "package org.util\n\nclass Other\n").unwrap();

    // Interfaces, enums, sealed/abstract/annotation classes, fun interfaces, and a
    // same-package class that conflicts with an explicit import all fail closed.
    for target_path in [
        "com::example::Service",
        "com::example::Color",
        "com::example::Shape",
        "com::example::Base",
        "com::example::Marker",
        "com::example::Listener",
        "com::example::Other",
        "org::util::Other",
    ] {
        let trace = trace_symbol_graph(&dir, target_path, TraceDirection::Callers).unwrap();
        assert!(
            trace.callers.is_empty(),
            "expected no constructor callers for {target_path}"
        );
    }
}

#[test]
fn traces_kotlin_object_property_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Holder {\n    fun run(value: Int): Int = value\n}\n\nobject Config {\n    val holder: Holder = Holder()\n}\n\nfun caller(): Int = Config.holder.run(1)\n",
    )
    .unwrap();

    let run_path = "com::example::Holder::run";
    let live = trace_symbol_graph(&dir, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, run_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_imported_object_property_chain_receiver_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let caller_path = dir.join("Caller.kt");
    let config_path = dir.join("Config.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package com.example\n\nimport org.util.Config\nimport org.util.Holder\n\nfun caller(): Int = Config.holder.run(1)\n",
    )
    .unwrap();
    fs::write(
        &config_path,
        "package org.util\n\nclass Holder {\n    fun run(value: Int): Int = value\n}\n\nobject Config {\n    val holder: Holder = Holder()\n}\n",
    )
    .unwrap();

    let run_path = "org::util::Holder::run";
    let live = trace_symbol_graph(&dir, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_nested_class_constructor_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\nfun caller(): Int {\n    val inner = Outer.Inner()\n    return inner.helper(1)\n}\n",
    )
    .unwrap();

    // A qualified constructor initializer pins the nested class receiver.
    let helper_path = "com::example::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_nested_class_constructor_bare_call_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Outer {\n    class Inner\n}\n\nfun caller(): Int {\n    val inner = Outer.Inner()\n    return 1\n}\n",
    )
    .unwrap();

    let inner_path = "com::example::Outer::Inner";
    let live = trace_symbol_graph(&dir, inner_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, inner_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, inner_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_nested_class_property_chain_receiver_calls_in_live_workspace_and_persisted_index()
{
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\nclass Group {\n    val inner = Outer.Inner()\n}\n\nfun caller(): Int {\n    val group = Group()\n    return group.inner.helper(1)\n}\n",
    )
    .unwrap();

    let helper_path = "com::example::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_qualified_nested_property_type_receiver_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\nclass Group {\n    val inner: Outer.Inner = Outer.Inner()\n}\n\nfun caller(): Int {\n    val group = Group()\n    return group.inner.helper(1)\n}\n",
    )
    .unwrap();

    let helper_path = "com::example::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_imported_nested_class_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let caller_path = dir
        .join("src")
        .join("com")
        .join("example")
        .join("Caller.kt");
    let outer_path = dir.join("src").join("org").join("util").join("Outer.kt");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(outer_path.parent().unwrap()).unwrap();
    fs::write(
        &caller_path,
        "package com.example\n\nimport org.util.Outer\n\nfun caller(): Int {\n    val inner = Outer.Inner()\n    return inner.helper(1)\n}\n",
    )
    .unwrap();
    fs::write(
        &outer_path,
        "package org.util\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n",
    )
    .unwrap();

    let helper_path = "org::util::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn does_not_trace_kotlin_qualified_receiver_calls_with_missing_nested_types() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    fs::write(
        &source_path,
        "package com.example\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\nfun missingNestedChain(): Int {\n    val value = Outer.Absent()\n    return value.helper(1)\n}\n\nfun missingNestedBare(): Int {\n    Outer.Absent()\n    return 1\n}\n",
    )
    .unwrap();

    // An unknown nested type fails closed for both initializer chains and bare
    // constructor calls instead of guessing a target.
    let helper_path = "com::example::Outer::Inner::helper";
    let helper_trace = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert!(helper_trace.callers.is_empty());

    let inner_path = "com::example::Outer::Inner";
    let inner_trace = trace_symbol_graph(&dir, inner_path, TraceDirection::Callers).unwrap();
    assert!(inner_trace.callers.is_empty());
}

#[test]
fn traces_kotlin_nested_object_receiver_member_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Outer {\n    object Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\nfun caller(): Int = Outer.Inner.helper(1)\n",
    )
    .unwrap();

    let helper_path = "com::example::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_nested_object_property_chain_receiver_calls_in_live_workspace_and_persisted_index()
{
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Holder {\n    fun run(value: Int): Int = value\n}\n\nclass Outer {\n    object Inner {\n        val holder: Holder = Holder()\n    }\n}\n\nfun caller(): Int = Outer.Inner.holder.run(1)\n",
    )
    .unwrap();

    let run_path = "com::example::Holder::run";
    let live = trace_symbol_graph(&dir, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, run_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_imported_nested_object_receiver_member_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let caller_path = dir
        .join("src")
        .join("com")
        .join("example")
        .join("Caller.kt");
    let outer_path = dir.join("src").join("org").join("util").join("Outer.kt");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(outer_path.parent().unwrap()).unwrap();
    fs::write(
        &caller_path,
        "package com.example\n\nimport org.util.Outer\n\nfun caller(): Int = Outer.Inner.helper(1)\n",
    )
    .unwrap();
    fs::write(
        &outer_path,
        "package org.util\n\nclass Outer {\n    object Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n",
    )
    .unwrap();

    let helper_path = "org::util::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_object_rooted_nested_object_receiver_member_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nobject Config {\n    object Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\nfun caller(): Int = Config.Inner.helper(1)\n",
    )
    .unwrap();

    let helper_path = "com::example::Config::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn does_not_trace_kotlin_nested_object_receiver_calls_with_unknown_or_conflicting_names() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    fs::write(
        &source_path,
        "package com.example\n\nclass Holder {\n    fun run(value: Int): Int = value\n}\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n    object Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\nclass Outer2 {\n    class Inner2 {\n        fun helper(value: Int): Int = value\n    }\n}\n\nfun unknownNestedObject(): Int = Outer.Absent.helper(1)\n\nfun conflictingNestedObject(): Int = Outer.Inner.helper(1)\n\nfun nestedClassReceiver(): Int = Outer2.Inner2.helper(1)\n\nfun shadowedNestedObject(): Int {\n    val Outer = Holder()\n    return Outer.Inner.helper(1)\n}\n",
    )
    .unwrap();

    // An unknown nested object name, a nested class that conflicts with a
    // same-named nested object, an instance-member call through a nested class
    // name, and a local binding that shadows the outer class all fail closed
    // instead of guessing a chain target.
    for helper_path in [
        "com::example::Outer::Inner::helper",
        "com::example::Outer2::Inner2::helper",
        "com::example::Holder::run",
    ] {
        let trace = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
        assert!(
            trace.callers.is_empty(),
            "expected no callers for {helper_path}"
        );
    }
}

#[test]
fn traces_kotlin_nested_companion_receiver_member_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Outer {\n    class Inner {\n        companion object {\n            fun helper(value: Int): Int = value\n        }\n    }\n    interface Service {\n        companion object {\n            fun serve(value: Int): Int = value\n        }\n    }\n}\n\nfun classCaller(): Int = Outer.Inner.helper(1)\n\nfun interfaceCaller(): Int = Outer.Service.serve(1)\n",
    )
    .unwrap();

    // A nested class or interface inside `Outer` may host the companion, so
    // the class-name receiver dispatches to the nested type's companion scope.
    let helper_path = "com::example::Outer::Inner::Companion::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::classCaller");

    let serve_path = "com::example::Outer::Service::Companion::serve";
    let serve_trace = trace_symbol_graph(&dir, serve_path, TraceDirection::Callers).unwrap();
    assert_eq!(serve_trace.callers.len(), 1);
    assert_eq!(
        serve_trace.callers[0].symbol_id,
        "com::example::interfaceCaller"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::classCaller");
    let persisted_serve =
        trace_symbol_graph_from_index(&db_path, serve_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted_serve.callers.len(), 1);
    assert_eq!(
        persisted_serve.callers[0].symbol_id,
        "com::example::interfaceCaller"
    );
}

#[test]
fn traces_kotlin_nested_companion_explicit_chain_receiver_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Holder {\n    fun run(value: Int): Int = value\n}\n\nclass Outer {\n    class Inner {\n        companion object {\n            fun helper(value: Int): Int = value\n            val holder = Holder()\n        }\n    }\n}\n\nfun companionCaller(): Int = Outer.Inner.Companion.helper(1)\n\nfun chainCaller(): Int = Outer.Inner.Companion.holder.run(1)\n",
    )
    .unwrap();

    let helper_path = "com::example::Outer::Inner::Companion::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::companionCaller");

    let run_path = "com::example::Holder::run";
    let run_trace = trace_symbol_graph(&dir, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(run_trace.callers.len(), 1);
    assert_eq!(run_trace.callers[0].symbol_id, "com::example::chainCaller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_helper =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted_helper.callers.len(), 1);
    assert_eq!(
        persisted_helper.callers[0].symbol_id,
        "com::example::companionCaller"
    );
    let persisted_run =
        trace_symbol_graph_from_index(&db_path, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted_run.callers.len(), 1);
    assert_eq!(
        persisted_run.callers[0].symbol_id,
        "com::example::chainCaller"
    );
}

#[test]
fn traces_kotlin_named_nested_companion_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Outer {\n    class Inner {\n        companion object Factory {\n            fun helper(value: Int): Int = value\n        }\n    }\n}\n\nfun canonicalCaller(): Int = Outer.Inner.Companion.helper(1)\n\nfun namedCaller(): Int = Outer.Inner.Factory.helper(1)\n",
    )
    .unwrap();

    // Both the canonical `Companion` spelling and the declared nested companion
    // name resolve to the same canonical companion-member ID.
    let helper_path = "com::example::Outer::Inner::Companion::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 2);
    let mut caller_ids = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    caller_ids.sort_unstable();
    assert_eq!(
        caller_ids,
        vec!["com::example::canonicalCaller", "com::example::namedCaller"]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut persisted_ids = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    persisted_ids.sort_unstable();
    assert_eq!(
        persisted_ids,
        vec!["com::example::canonicalCaller", "com::example::namedCaller"]
    );
}

#[test]
fn traces_kotlin_imported_nested_companion_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let caller_path = dir
        .join("src")
        .join("com")
        .join("example")
        .join("Caller.kt");
    let outer_path = dir.join("src").join("org").join("util").join("Outer.kt");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(outer_path.parent().unwrap()).unwrap();
    fs::write(
        &caller_path,
        "package com.example\n\nimport org.util.Outer\n\nfun caller(): Int = Outer.Inner.helper(1)\n",
    )
    .unwrap();
    fs::write(
        &outer_path,
        "package org.util\n\nclass Outer {\n    class Inner {\n        companion object {\n            fun helper(value: Int): Int = value\n        }\n    }\n}\n",
    )
    .unwrap();

    let helper_path = "org::util::Outer::Inner::Companion::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn does_not_trace_kotlin_nested_companion_receiver_calls_without_companions_or_with_unknown_names()
{
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    fs::write(
        &source_path,
        "package com.example\n\nclass Holder {\n    fun run(value: Int): Int = value\n}\n\nclass Outer {\n    class Plain {\n        fun helper(value: Int): Int = value\n    }\n    companion object {\n        fun helper(value: Int): Int = value\n    }\n}\n\nclass Outer2 {\n    class Inner {\n        companion object {\n            fun helper(value: Int): Int = value\n        }\n    }\n}\n\nfun nestedClassWithoutCompanion(): Int = Outer.Plain.helper(1)\n\nfun unknownNestedType(): Int = Outer.Inner.helper(1)\n\nfun shadowedNestedCompanion(): Int {\n    val Outer2 = Holder()\n    return Outer2.Inner.helper(1)\n}\n",
    )
    .unwrap();

    // An instance member called through a nested class name, an unknown nested
    // type, and a local binding that shadows the outer class name all fail
    // closed instead of dispatching to the outer or nested companion scope.
    for helper_path in [
        "com::example::Outer::Plain::helper",
        "com::example::Outer::Companion::helper",
        "com::example::Outer2::Inner::Companion::helper",
    ] {
        let trace = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
        assert!(
            trace.callers.is_empty(),
            "expected no callers for {helper_path}"
        );
    }
}

#[test]
fn does_not_trace_kotlin_object_property_chains_with_unknown_or_shadowed_objects() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    fs::write(
        &source_path,
        "package com.example\n\nclass Holder {\n    fun run(value: Int): Int = value\n}\n\nobject Config {\n    val holder: Holder = Holder()\n}\n\nfun unknownObject(): Int = Missing.holder.run(1)\n\nfun shadowedObject(): Int {\n    val Config = Holder()\n    return Config.holder.run(1)\n}\n",
    )
    .unwrap();

    // An unknown object name and a local binding that shadows the object name
    // both fail closed instead of guessing a chain target.
    let run_path = "com::example::Holder::run";
    let trace = trace_symbol_graph(&dir, run_path, TraceDirection::Callers).unwrap();
    assert!(trace.callers.is_empty());
}

#[test]
fn traces_kotlin_constructor_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\nclass Group {\n    val member: Outer.Inner = Outer.Inner()\n    fun helper(value: Int): Int = value\n}\n\nfun caller(): Int {\n    val a = Outer.Inner().helper(1)\n    val b = Group().member.helper(2)\n    val c = Group().helper(3)\n    return a + b + c\n}\n",
    )
    .unwrap();

    // A constructor-call receiver chain dispatches like any other instance
    // receiver: the constructed type path pins the receiver and each
    // intermediate property hop resolves its declared type.
    for helper_path in [
        "com::example::Outer::Inner::helper",
        "com::example::Group::helper",
    ] {
        let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
        assert_eq!(live.symbol.symbol_id, helper_path);
        assert_eq!(live.callers.len(), 1);
        assert_eq!(live.callers[0].symbol_id, "com::example::caller");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for helper_path in [
        "com::example::Outer::Inner::helper",
        "com::example::Group::helper",
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
        assert_eq!(persisted.callers.len(), 1);
        assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
    }
}

#[test]
fn traces_kotlin_imported_constructor_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let caller_path = dir
        .join("src")
        .join("com")
        .join("example")
        .join("Caller.kt");
    let outer_path = dir.join("src").join("org").join("util").join("Outer.kt");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(outer_path.parent().unwrap()).unwrap();
    fs::write(
        &caller_path,
        "package com.example\n\nimport org.util.Outer\nimport org.util.Group\n\nfun caller(): Int {\n    val a = Outer.Inner().helper(1)\n    val b = Group().member.helper(2)\n    val c = Group().helper(3)\n    return a + b + c\n}\n",
    )
    .unwrap();
    fs::write(
        &outer_path,
        "package org.util\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\nclass Group {\n    val member: Outer.Inner = Outer.Inner()\n    fun helper(value: Int): Int = value\n}\n",
    )
    .unwrap();

    // Explicitly imported constructible types pin constructor-chain receivers
    // exactly like same-package classes.
    for helper_path in [
        "org::util::Outer::Inner::helper",
        "org::util::Group::helper",
    ] {
        let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
        assert_eq!(live.indexed_files, 2);
        assert_eq!(live.symbol.symbol_id, helper_path);
        assert_eq!(live.callers.len(), 1);
        assert_eq!(live.callers[0].symbol_id, "com::example::caller");
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for helper_path in [
        "org::util::Outer::Inner::helper",
        "org::util::Group::helper",
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
        assert_eq!(persisted.indexed_files, 2);
        assert_eq!(persisted.callers.len(), 1);
        assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
    }
}

#[test]
fn does_not_trace_kotlin_constructor_chain_receiver_calls_with_function_call_or_non_constructible_bases()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    fs::write(
        &source_path,
        "package com.example\n\nclass Other {\n    fun helper(value: Int): Int = value\n}\n\nfun makeOther(): Other = Other()\n\ninterface Shape {\n    fun draw(): Int\n}\n\nabstract class AbstractBase {\n    fun run(value: Int): Int = value\n}\n\nclass Outer {\n    class Inner {\n        fun helper(value: Int): Int = value\n    }\n}\n\nfun caller(): Int {\n    val a = makeOther().helper(1)\n    val b = Unknown().helper(2)\n    val c = Outer.Missing().helper(3)\n    val d = Shape().draw(4)\n    val e = AbstractBase().run(5)\n    return a + b + c + d + e\n}\n",
    )
    .unwrap();

    // A function-call base, an unknown type, a missing nested type, and
    // non-constructible bases (interface, abstract class) all fail closed
    // instead of guessing a chain target.
    for helper_path in [
        "com::example::Other::helper",
        "com::example::Outer::Inner::helper",
        "com::example::Shape::draw",
        "com::example::AbstractBase::run",
    ] {
        let trace = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
        assert!(
            trace.callers.is_empty(),
            "expected no callers for {helper_path}"
        );
    }
}

#[test]
fn traces_kotlin_enum_companion_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nenum class Color {\n    companion object Factory {\n        fun helper(value: Int): Int = value\n    }\n    fun memberHelper(value: Int): Int = value\n}\n\nfun caller(): Int {\n    Color.Factory.helper(1)\n    Color.Companion.helper(2)\n    Color.memberHelper(3)\n    return 0\n}\n",
    )
    .unwrap();

    // Enums are class declarations that can host companion objects, so both
    // the declared-name and canonical `Companion` spellings dispatch to the
    // companion scope; an instance member reached through the class name fails
    // closed.
    let helper_path = "com::example::Color::Companion::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");

    let member_path = "com::example::Color::memberHelper";
    let trace = trace_symbol_graph(&dir, member_path, TraceDirection::Callers).unwrap();
    assert!(trace.callers.is_empty());
}

#[test]
fn traces_kotlin_deep_constructor_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Inner {\n    fun helper(value: Int): Int = value\n}\n\nclass Mid {\n    val inner: Inner = Inner()\n}\n\nclass Group {\n    val mid: Mid = Mid()\n}\n\nfun caller(): Int {\n    return Group().mid.inner.helper(1)\n}\n",
    )
    .unwrap();

    // A constructor-call receiver followed by multiple property hops resolves
    // each intermediate property's declared type before the final dispatch.
    let helper_path = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_kotlin_deep_object_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Callers.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example\n\nclass Member {\n    fun run(value: Int): Int = value\n}\n\nclass Holder {\n    val member: Member = Member()\n}\n\nobject Outer {\n    object Inner {\n        val holder: Holder = Holder()\n    }\n}\n\nfun caller(): Int {\n    return Outer.Inner.holder.member.run(2)\n}\n",
    )
    .unwrap();

    // An object-rooted nested-object chain resolves each intermediate
    // property's declared type before dispatching the terminal member.
    let run_path = "com::example::Member::run";
    let live = trace_symbol_graph(&dir, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, run_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn traces_java_typed_parameter_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int run(Helper helper) { return helper.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_symbol);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_typed_parameter_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int run(Helper helper) { return helper.helper(1); }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_typed_local_and_field_receiver_calls() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    private Helper fieldHelper = new Helper();
    int run() {
        Helper local = new Helper();
        return local.helper(1) + fieldHelper.helper(2);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_typed_receiver_calls_across_files_with_explicit_import() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.helper.Foo;
class Bar {
    int run(Foo foo) { return foo.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::helper::Foo::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_typed_receiver_inherited_methods() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Grand { int helper(int value) { return value; } }
class Base extends Grand {}
class Caller {
    int run(Base base) { return base.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Grand::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_typed_receiver_calls_fail_closed_for_unknown_types_and_shadowed_static_calls() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { static int run(int value) { return value; } }
class Caller {
    int unknownType(Unknown value) { return value.helper(1); }
    int memberChain(Helper helper) { return helper.inner.helper(1); }
    int lambdaShadowed() {
        java.util.function.IntFunction<Integer> function = Helper -> Helper.run(1);
        return function.apply(0);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Helper::run";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "bound but unresolvable receivers must not fall through to static type calls"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_var_constructor_inferred_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int run() {
        var helper = new Helper();
        return helper.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_constructor_inferred_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int run() {
        var helper = new Helper();
        return helper.helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_constructor_inferred_nested_receiver_types() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Outer {
    static class Inner { int helper(int value) { return value; } }
}
class Caller {
    int run() {
        var inner = new Outer.Inner();
        return inner.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_constructor_inferred_receiver_inherited_methods() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Grand { int helper(int value) { return value; } }
class Base extends Grand {}
class Helper extends Base {}
class Caller {
    int run() {
        var helper = new Helper();
        return helper.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Grand::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_var_constructor_receiver_calls_fail_closed_without_constructor_initializers() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { static int run(int value) { return value; } }
class Caller {
    int run() {
        var factory = makeHelper();
        var missing = new Missing();
        return factory.run(1) + missing.run(2);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Helper::run";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "unknown-factory and missing-constructor var initializers must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_constructor_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int run() { return new Helper().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_constructor_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int run() { return new Helper().helper(1); }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_constructor_receiver_nested_types() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Outer {
    static class Inner { int helper(int value) { return value; } }
}
class Caller {
    int run() { return new Outer.Inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_constructor_receiver_inherited_methods() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Grand { int helper(int value) { return value; } }
class Base extends Grand {}
class Helper extends Base {}
class Caller {
    int run() { return new Helper().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Grand::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_constructor_receiver_calls_fail_closed_for_unknown_and_anonymous_types() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int run(int value) { return value; } }
class Caller {
    int missing() { return new Missing().run(1); }
    int overridden() {
        return new Helper() { int run(int value) { return value + 1; } }.run(2);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Helper::run";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "unknown constructor types and anonymous bodies that declare the invoked member must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_anonymous_constructor_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int direct() { return new Helper() { }.helper(1); }
    int directWithBody() {
        return new Helper() { int other() { return 0; } }.helper(2);
    }
    int varInitializer() {
        var v = new Helper() { };
        return v.helper(3);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 3);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::direct",
            "com::example::Caller::directWithBody",
            "com::example::Caller::varInitializer"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 3);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::direct",
            "com::example::Caller::directWithBody",
            "com::example::Caller::varInitializer"
        ]
    );
}

#[test]
fn traces_java_anonymous_constructor_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int run() {
        return new Helper() { }.helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_anonymous_constructor_receiver_calls_fail_closed_for_unsupported_references() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int overridden() {
        return new Helper() { int helper(int value) { return value + 1; } }.helper(1);
    }
    int arityOverride() {
        return new Helper() { int helper() { return 0; } }.helper(1);
    }
    int missingType() {
        return new Missing() { }.helper(1);
    }
    int chained() {
        return new Helper() { }.inner().helper(1);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "anonymous receivers with overriding bodies, unknown constructed types, and chains with unknown hops must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_anonymous_constructor_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Group {
    Helper inner = new Helper();
    Group inner2() { return this; }
    Group inner2(int value) { return this; }
}
class Outer { static class Inner { int helper(int value) { return value; } } }
class Caller {
    int fieldChain() { return new Group() { }.inner.helper(1); }
    int methodHopChain() { return new Group() { }.inner2().inner.helper(2); }
    int methodHopArgChain() { return new Group() { }.inner2(1).inner.helper(5); }
    int bodyUnrelated() {
        return new Group() { int other() { return 0; } }.inner.helper(3);
    }
    int nested() { return new Outer.Inner() { }.helper(4); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 4);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::bodyUnrelated",
            "com::example::Caller::fieldChain",
            "com::example::Caller::methodHopArgChain",
            "com::example::Caller::methodHopChain"
        ]
    );

    let nested_symbol = "com::example::Outer::Inner::helper";
    let nested_live = trace_symbol_graph(&dir, nested_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(nested_live.callers.len(), 1);
    assert_eq!(
        nested_live.callers[0].symbol_id,
        "com::example::Caller::nested"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 4);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::bodyUnrelated",
            "com::example::Caller::fieldChain",
            "com::example::Caller::methodHopArgChain",
            "com::example::Caller::methodHopChain"
        ]
    );
    let nested_persisted =
        trace_symbol_graph_from_index(&db_path, nested_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(nested_persisted.callers.len(), 1);
    assert_eq!(
        nested_persisted.callers[0].symbol_id,
        "com::example::Caller::nested"
    );
}

#[test]
fn traces_java_anonymous_constructor_chain_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Group { Helper inner = new Helper(); }
class Caller {
    int run() {
        return new Group() { }.inner.helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_anonymous_constructor_chain_receiver_calls_fail_closed_for_unsupported_references() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Other { int helper(int value) { return value + 10; } }
class Group {
    Helper inner = new Helper();
    Group inner2() { return this; }
}
class Caller {
    int overrideFinal() {
        return new Group() { int helper(int value) { return value + 1; } }.inner.helper(1);
    }
    int overrideHop() {
        return new Group() { int inner2() { return this; } }.inner2().inner.helper(2);
    }
    int overrideArgHop() {
        return new Group() { Group inner2(int value) { return this; } }.inner2(1).inner.helper(2);
    }
    int arityMismatch() {
        return new Group() { }.inner2(1).inner.helper(2);
    }
    int fieldShadow() {
        return new Group() { Other inner = new Other(); }.inner.helper(2);
    }
    int missingType() {
        return new Missing() { }.inner.helper(1);
    }
    int unknownHop() {
        return new Group() { }.missing().inner.helper(1);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "anonymous-rooted chains with overriding or field-shadowing bodies, arity-mismatched hops, unknown constructed types, and unknown hops must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_var_anonymous_field_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Group {
    Helper entry = new Helper();
    Group entry2() { return this; }
    Group entry2(int value) { return this; }
    Holder holder = new Holder();
}
class Holder { Helper entry = new Helper(); }
class Outer { static class Inner { Helper entry = new Helper(); } }
class Caller {
    int varField() {
        var v = new Group() { }.entry;
        return v.helper(1);
    }
    int varFieldWithBody() {
        var v = new Group() { int other() { return 0; } }.entry;
        return v.helper(2);
    }
    int varFieldNested() {
        var v = new Outer.Inner() { }.entry;
        return v.helper(3);
    }
    int varFieldWithArgHop() {
        var v = new Group() { }.entry2(1).entry;
        return v.helper(6);
    }
    int varFieldWithHop() {
        var v = new Group() { }.entry2().entry;
        return v.helper(4);
    }
    int varFieldWithFieldHop() {
        var v = new Group() { }.holder.entry;
        return v.helper(5);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 6);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::varField",
            "com::example::Caller::varFieldNested",
            "com::example::Caller::varFieldWithArgHop",
            "com::example::Caller::varFieldWithBody",
            "com::example::Caller::varFieldWithFieldHop",
            "com::example::Caller::varFieldWithHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 6);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::varField",
            "com::example::Caller::varFieldNested",
            "com::example::Caller::varFieldWithArgHop",
            "com::example::Caller::varFieldWithBody",
            "com::example::Caller::varFieldWithFieldHop",
            "com::example::Caller::varFieldWithHop"
        ]
    );
}

#[test]
fn traces_java_var_anonymous_field_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Group {
    Helper entry = new Helper();
    Group entry2() { return this; }
    Group entry2(int value) { return this; }
}
class Caller {
    int run() {
        var v = new Group() { }.entry;
        return v.helper(1);
    }
    int runHop() {
        var v = new Group() { }.entry2().entry;
        return v.helper(2);
    }
    int runArgHop() {
        var v = new Group() { }.entry2(1).entry;
        return v.helper(3);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 3);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::run",
            "com::example::Caller::runArgHop",
            "com::example::Caller::runHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 3);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::run",
            "com::example::Caller::runArgHop",
            "com::example::Caller::runHop"
        ]
    );
}

#[test]
fn java_var_anonymous_field_receiver_calls_fail_closed_for_unsupported_references() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Group {
    Helper entry = new Helper();
    Group entry2() { return this; }
}
class Caller {
    int shadowedField() {
        var v = new Group() { Helper entry = new Helper(); }.entry;
        return v.helper(1);
    }
    int shadowedHop() {
        var v = new Group() { Group entry2() { return this; } }.entry2().entry;
        return v.helper(1);
    }
    int shadowedArgHop() {
        var v = new Group() { Group entry2(int value) { return this; } }.entry2(1).entry;
        return v.helper(1);
    }
    int missingType() {
        var v = new Missing() { }.entry;
        return v.helper(1);
    }
    int unknownChain() {
        var v = new Group() { }.missing.entry;
        return v.helper(1);
    }
    int argHop() {
        var v = new Group() { }.entry2(1).entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "anonymous var field-initializer chains with shadowing bodies, unknown constructed types, unknown chains, and arity-mismatched or shadowed argument hops must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_interface_typed_parameter_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Helper { int helper(int value); }
class Caller {
    int run(Helper helper) { return helper.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_interface_typed_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
interface Helper { int helper(int value); }
class Caller {
    int run(Helper helper) { return helper.helper(1); }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_interface_typed_receiver_default_methods() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Helper { default int helper(int value) { return value; } }
class Caller {
    int run(Helper helper) { return helper.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_interface_typed_receiver_calls_across_files_with_explicit_import() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public interface Foo { int helper(int value); }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.helper.Foo;
public class Bar {
    public int run(Foo foo) { return foo.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::helper::Foo::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn java_interface_typed_receiver_calls_fail_closed_when_interface_lacks_declaration() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Helper {}
class Impl implements Helper { int run(int value) { return value; } }
class Caller {
    int run(Helper helper) { return helper.run(1); }
}
",
    )
    .unwrap();

    let target = "com::example::Impl::run";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "receivers typed as an interface must not guess implementation methods"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_interface_inherited_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Base { int helper(int value); }
interface Mid extends Base {}
class Caller {
    int run(Mid mid) { return mid.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Base::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_interface_inherited_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
interface Base { int helper(int value); }
interface Mid extends Base {}
class Caller {
    int run(Mid mid) { return mid.helper(1); }
}
";
    let helper_symbol = "com::example::Base::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_interface_inherited_receiver_default_methods() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Base { default int helper(int value) { return value; } }
interface Mid extends Base {}
class Caller {
    int run(Mid mid) { return mid.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Base::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_interface_inherited_receiver_calls_across_files_with_imports() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("pkg").join("base");
    let mid_dir = dir.join("src").join("pkg").join("mid");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let base_path = base_dir.join("Base.java");
    let mid_path = mid_dir.join("Mid.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&mid_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &base_path,
        "package pkg.base;
public interface Base { int helper(int value); }
",
    )
    .unwrap();
    fs::write(
        &mid_path,
        "package pkg.mid;
import pkg.base.Base;
public interface Mid extends Base {}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.mid.Mid;
public class Bar {
    public int run(Mid mid) { return mid.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::base::Base::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_interface_inherited_receiver_calls_through_member_chains() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Base { int helper(int value); }
interface Mid extends Base {}
class Group { Mid mid; }
class Caller {
    int run(Group group) { return group.mid.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Base::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_interface_inherited_receiver_calls_fail_closed_for_unresolvable_chains() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Base { static int run(int value) { return value; } }
interface Other { static int run(int value) { return value; } }
interface Branching extends Base, Other {}
interface Missing extends Unknown {}
class Impl implements Base {}
class Caller {
    int branching(Branching branching) { return branching.run(1); }
    int missingParent(Missing missing) { return missing.run(1); }
    int classReceiver(Impl impl) { return impl.run(1); }
}
",
    )
    .unwrap();

    let target = "com::example::Base::run";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "branching or unresolved interface chains, static interface members, and class receivers must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_class_receiver_interface_default_methods_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Helper { default int helper(int value) { return value; } }
class Impl implements Helper {}
class Caller {
    int run(Impl impl) { return impl.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_class_receiver_interface_default_methods_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
interface Helper { default int helper(int value) { return value; } }
class Impl implements Helper {}
class Caller {
    int run(Impl impl) { return impl.helper(1); }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_class_receiver_interface_default_methods_across_files_with_explicit_import() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let impl_dir = dir.join("src").join("pkg").join("impl");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Helper.java");
    let impl_path = impl_dir.join("Impl.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&impl_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public interface Helper { default int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &impl_path,
        "package pkg.impl;
import pkg.helper.Helper;
public class Impl implements Helper {}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.impl.Impl;
public class Bar {
    public int run(Impl impl) { return impl.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::helper::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_class_receiver_interface_default_methods_through_shared_receiver_paths() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Helper { default int helper(int value) { return value; } }
class Impl implements Helper {}
class Group { Impl impl; }
class Caller {
    int newCall() { return new Impl().helper(1); }
    int varCall() { var x = new Impl(); return x.helper(1); }
    int chainCall(Group group) { return group.impl.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 3);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::chainCall",
            "com::example::Caller::newCall",
            "com::example::Caller::varCall"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 3);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::chainCall",
            "com::example::Caller::newCall",
            "com::example::Caller::varCall"
        ]
    );
}

#[test]
fn java_class_receiver_interface_default_methods_fail_closed_for_nearer_class_declarations() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Helper { default int helper(int value) { return value; } }
class StaticImpl implements Helper { static int helper(int value) { return value; } }
class ArityImpl implements Helper { int helper() { return 0; } }
class Caller {
    int staticCall(StaticImpl impl) { return impl.helper(1); }
    int arityCall(ArityImpl impl) { return impl.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "same-name methods nearer in the receiver class hierarchy must suppress interface default dispatch"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn java_class_receiver_interface_default_methods_fail_closed_for_competing_or_unresolved_chains() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Helper { default int helper(int value) { return value; } }
interface Other { default int helper(int value) { return value; } }
class Competing implements Helper, Other {}
class Missing implements Helper, Unknown {}
interface StaticHelper { static int helper(int value) { return value; } }
class StaticInterface implements StaticHelper {}
class Caller {
    int competing(Competing impl) { return impl.helper(1); }
    int missing(Missing impl) { return impl.helper(1); }
    int staticInterface(StaticInterface impl) { return impl.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "competing defaults, unresolved interfaces, and static-only interface members must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_interface_inherited_receiver_calls_through_branching_chains_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Base { int helper(int value); }
interface Marker {}
interface Mid extends Base, Marker {}
class Caller {
    int run(Mid mid) { return mid.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Base::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_interface_inherited_receiver_default_methods_through_branching_chains() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Base { default int helper(int value) { return value; } }
interface Marker {}
interface Mid extends Base, Marker {}
class Caller {
    int run(Mid mid) { return mid.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Base::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_interface_inherited_receiver_calls_through_branching_chains_from_dirty_vfs_overrides()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
interface Base { default int helper(int value) { return value; } }
interface Marker {}
interface Mid extends Base, Marker {}
class Caller {
    int run(Mid mid) { return mid.helper(1); }
}
";
    let helper_symbol = "com::example::Base::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_interface_inherited_receiver_calls_through_branching_chains_across_files_with_imports()
 {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("pkg").join("base");
    let marker_dir = dir.join("src").join("pkg").join("marker");
    let mid_dir = dir.join("src").join("pkg").join("mid");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let base_path = base_dir.join("Base.java");
    let marker_path = marker_dir.join("Marker.java");
    let mid_path = mid_dir.join("Mid.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&marker_dir).unwrap();
    fs::create_dir_all(&mid_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &base_path,
        "package pkg.base;
public interface Base { int helper(int value); }
",
    )
    .unwrap();
    fs::write(
        &marker_path,
        "package pkg.marker;
public interface Marker {}
",
    )
    .unwrap();
    fs::write(
        &mid_path,
        "package pkg.mid;
import pkg.base.Base;
import pkg.marker.Marker;
public interface Mid extends Base, Marker {}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.mid.Mid;
public class Bar {
    public int run(Mid mid) { return mid.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::base::Base::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_interface_inherited_receiver_calls_through_diamond_chains_resolve_once() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Root { int helper(int value); }
interface Left extends Root {}
interface Right extends Root {}
interface Mid extends Left, Right {}
class Caller {
    int run(Mid mid) { return mid.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Root::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_class_receiver_interface_default_methods_through_branching_chains() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Base { default int helper(int value) { return value; } }
interface Marker {}
interface Helper extends Base, Marker {}
class Impl implements Helper {}
class Caller {
    int run(Impl impl) { return impl.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Base::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_interface_inherited_receiver_calls_fail_closed_for_competing_or_unresolvable_branches() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Base { int helper(int value); }
interface Other { int helper(int value); }
interface Competing extends Base, Other {}
interface DefaultOther { default int helper(int value) { return value; } }
interface CompetingDefaults extends Base, DefaultOther {}
interface Missing extends Base, Unknown {}
interface StaticBranch { static int helper(int value) { return value; } }
interface StaticOnly extends Base, StaticBranch {}
interface Root { int helper(int value); }
interface CycleA extends CycleB {}
interface CycleB extends CycleA {}
interface Cyclic extends Root, CycleA {}
class Caller {
    int competing(Competing value) { return value.helper(1); }
    int competingDefaults(CompetingDefaults value) { return value.helper(1); }
    int missing(Missing value) { return value.helper(1); }
    int staticOnly(StaticOnly value) { return value.helper(1); }
    int cyclic(Cyclic value) { return value.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Base::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "competing, unresolved, static-only, and cyclic branches must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_generic_receiver_parameter_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Box<T> { int helper(int value) { return value; } }
class Caller {
    int run(Box<String> box) { return box.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Box::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_generic_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Box<T> { int helper(int value) { return value; } }
class Caller {
    int run(Box<String> box) { return box.helper(1); }
}
";
    let helper_symbol = "com::example::Box::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_generic_receiver_calls_through_member_chains_and_constructors() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Box<T> { int helper(int value) { return value; } }
class Group { Box<String> box; }
class Caller {
    int newCall() { return new Box<String>().helper(1); }
    int varCall() { var box = new Box<String>(); return box.helper(1); }
    int chainCall(Group group) { return group.box.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Box::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 3);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::chainCall",
            "com::example::Caller::newCall",
            "com::example::Caller::varCall"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 3);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::chainCall",
            "com::example::Caller::newCall",
            "com::example::Caller::varCall"
        ]
    );
}

#[test]
fn traces_java_generic_receiver_calls_through_factory_initializers() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Box<T> { int helper(int value) { return value; } }
class Caller {
    Box<String> makeBox() { return new Box<String>(); }
    int run() { var box = makeBox(); return box.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Box::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_generic_receiver_calls_fail_closed_for_array_types() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Box<T> { int helper(int value) { return value; } }
class Caller {
    int run(Box<String>[] boxes) { return boxes.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Box::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "array-typed receivers must fail closed instead of guessing a raw element type"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_member_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner inner = new Inner(); }
class Caller {
    int run(Group group) { return group.inner.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_member_chain_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner inner = new Inner(); }
class Caller {
    int run(Group group) { return group.inner.helper(1); }
}
";
    let helper_symbol = "com::example::Inner::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_member_chain_receiver_calls_across_files_with_imports() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let group_dir = dir.join("src").join("pkg").join("group");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let group_path = group_dir.join("Group.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&group_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &group_path,
        "package pkg.group;
import pkg.helper.Foo;
public class Group { public Foo inner = new Foo(); }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.group.Group;
public class Bar {
    public int run(Group group) { return group.inner.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::helper::Foo::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_member_chain_receiver_calls_through_var_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner inner = new Inner(); }
class Caller {
    int varReceiver() {
        var group = new Group();
        return group.inner.helper(1);
    }
    int constructorReceiver() { return new Group().inner.helper(2); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::constructorReceiver",
            "com::example::Caller::varReceiver"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::constructorReceiver",
            "com::example::Caller::varReceiver"
        ]
    );
}

#[test]
fn traces_java_constructor_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner inner = new Inner(); }
class Caller {
    int run() { return new Group().inner.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_constructor_chain_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner inner = new Inner(); }
class Caller {
    int run() { return new Group().inner.helper(1); }
}
";
    let helper_symbol = "com::example::Inner::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_constructor_chain_receiver_calls_across_files_with_imports() {
    let dir = temporary_dir();
    let inner_dir = dir.join("src").join("pkg").join("inner");
    let group_dir = dir.join("src").join("pkg").join("group");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let inner_path = inner_dir.join("Foo.java");
    let group_path = group_dir.join("Group.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&inner_dir).unwrap();
    fs::create_dir_all(&group_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &inner_path,
        "package pkg.inner;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &group_path,
        "package pkg.group;
import pkg.inner.Foo;
public class Group { public Foo inner = new Foo(); }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.group.Group;
public class Bar {
    public int run() { return new Group().inner.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::inner::Foo::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_constructor_chain_receiver_calls_through_deep_chains() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Holder { Helper helper = new Helper(); }
class Group { Holder holder = new Holder(); }
class Caller {
    int run() { return new Group().holder.helper.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_constructor_chain_receiver_calls_fail_closed_for_unresolvable_bases() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { static int run(int value) { return value; } }
class Group { Inner inner = new Inner(); }
class Caller {
    static Group makeGroup() { return new Group(); }
    int functionCallBase() { return makeGroup().inner.run(1); }
    int unknownHop() { return new Group().missing.run(1); }
    int staticMember() { return new Group().inner.run(1); }
}
",
    )
    .unwrap();

    let target = "com::example::Inner::run";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "function-call bases, unknown chain hops, and static final members must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn java_member_chain_receiver_calls_fail_closed_for_unknown_hops() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { static int run(int value) { return value; } }
class Group {}
class Caller {
    int missingField(Group group) { return group.missing.run(1); }
    int unknownHopType(Group group) { return group.inner.run(1); }
}
",
    )
    .unwrap();

    let target = "com::example::Inner::run";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "unknown chain hops and static final members must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}
#[test]
fn traces_java_factory_inferred_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    static Helper makeHelper() { return new Helper(); }
    int run() {
        var factory = makeHelper();
        return factory.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_factory_inferred_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    static Helper makeHelper() { return new Helper(); }
    int run() {
        var factory = makeHelper();
        return factory.helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_factory_inferred_receiver_calls_across_files_with_static_import() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let factory_dir = dir.join("src").join("pkg").join("factory");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let factory_path = factory_dir.join("Fact.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&factory_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &factory_path,
        "package pkg.factory;
import pkg.helper.Foo;
public class Fact {
    public static Foo makeFoo() { return new Foo(); }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import static pkg.factory.Fact.makeFoo;
public class Bar {
    public int run() {
        var foo = makeFoo();
        return foo.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::helper::Foo::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_factory_inferred_nested_receiver_types() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Outer {
    static class Inner { int helper(int value) { return value; } }
}
class Caller {
    static Outer.Inner makeInner() { return new Outer.Inner(); }
    int run() {
        var inner = makeInner();
        return inner.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_factory_inferred_receiver_inherited_methods() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Grand { int helper(int value) { return value; } }
class Base extends Grand {}
class Caller {
    static Base makeBase() { return new Base(); }
    int run() {
        var base = makeBase();
        return base.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Grand::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_factory_inferred_receiver_calls_fail_closed_for_unresolvable_factories() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { static int run(int value) { return value; } }
class Util {
    static Helper makeHelper(int value) { return new Helper(); }
}
class Caller {
    static void makeVoid() {}
    static int makeInt() { return 1; }
    static Helper makeHelper() { return new Helper(); }
    static Helper makeHelper(int value) { return new Helper(); }
    static Helper makeHelper(String value) { return new Helper(); }
    int unknownFactory() {
        var factory = makeUnknown();
        return factory.run(1);
    }
    int qualifiedInitializer() {
        var factory = Util.makeHelper(1);
        return factory.run(1);
    }
    int voidFactory() {
        var factory = makeVoid();
        return factory.run(1);
    }
    int primitiveFactory() {
        var factory = makeInt();
        return factory.run(1);
    }
    int arityMismatch() {
        var factory = makeHelper(1, 2);
        return factory.run(1);
    }
    int ambiguousOverload() {
        var factory = makeHelper(1);
        return factory.run(1);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Helper::run";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "unknown, qualified, void/primitive-return, arity-mismatched, and ambiguous factory initializers must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_method_hop_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner inner() { return new Inner(); } }
class Caller {
    int run(Group group) { return group.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_method_hop_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner inner() { return new Inner(); } }
class Caller {
    int run(Group group) { return group.inner().helper(1); }
}
";
    let helper_symbol = "com::example::Inner::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_method_hop_receiver_calls_through_shared_paths() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Deep { int helper(int value) { return value; } }
class Inner { Deep deeper() { return new Deep(); } }
class Group { Inner inner() { return new Inner(); } }
class Holder { Group group; }
class Caller {
    int newCall() { return new Group().inner().deeper().helper(1); }
    int varCall() { var group = new Group(); return group.inner().deeper().helper(1); }
    int paramCall(Group group) { return group.inner().deeper().helper(1); }
    int fieldHopCall(Holder holder) { return holder.group.inner().deeper().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Deep::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 4);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::fieldHopCall",
            "com::example::Caller::newCall",
            "com::example::Caller::paramCall",
            "com::example::Caller::varCall"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 4);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::fieldHopCall",
            "com::example::Caller::newCall",
            "com::example::Caller::paramCall",
            "com::example::Caller::varCall"
        ]
    );
}

#[test]
fn traces_java_method_hop_receiver_calls_across_files_with_imports() {
    let dir = temporary_dir();
    let inner_dir = dir.join("src").join("pkg").join("inner");
    let group_dir = dir.join("src").join("pkg").join("group");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let inner_path = inner_dir.join("Foo.java");
    let group_path = group_dir.join("Group.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&inner_dir).unwrap();
    fs::create_dir_all(&group_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &inner_path,
        "package pkg.inner;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &group_path,
        "package pkg.group;
import pkg.inner.Foo;
public class Group { public Foo inner() { return new Foo(); } }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.group.Group;
public class Bar {
    public int run(Group group) { return group.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::inner::Foo::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_method_hop_receiver_calls_through_interface_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
interface IGroup { Inner inner(); }
class Caller {
    int run(IGroup group) { return group.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_method_hop_receiver_calls_through_generic_return_types() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Box<T> { int helper(int value) { return value; } }
class Group { Box<String> inner() { return new Box<String>(); } }
class Caller {
    int run(Group group) { return group.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Box::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_method_hop_receiver_calls_fail_closed_for_unsupported_hops() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Group {
    Inner inner() { return new Inner(); }
    Inner inner(int value) { return new Inner(); }
    int tag() { return 1; }
    void reset() {}
    static Inner make() { return new Inner(); }
}
class Caller {
    int argMismatchHop(Group group) { return group.inner(1, 2).helper(1); }
    int unknownHop(Group group) { return group.unknown().helper(1); }
    int primitiveHop(Group group) { return group.tag().helper(1); }
    int voidHop(Group group) { return group.reset().helper(1); }
    int staticHop(Group group) { return group.make().helper(1); }
    int unboundHop() { return unknown.inner().helper(1); }
    int unknownThisHop() { return this.inner().helper(1); }
}
",
    )
    .unwrap();

    let target = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "arity-mismatched, unknown, primitive/void-return, static, unbound, and unknown `this`-rooted method-call hops must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_this_rooted_member_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Caller {
    Inner inner = new Inner();
    int run() { return this.inner.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_this_rooted_method_hop_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Caller {
    Inner inner() { return new Inner(); }
    int run() { return this.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_this_rooted_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Inner { int helper(int value) { return value; } }
class Caller {
    Inner inner = new Inner();
    Inner makeInner() { return new Inner(); }
    int fieldCall() { return this.inner.helper(1); }
    int hopCall() { return this.makeInner().helper(1); }
}
";
    let helper_symbol = "com::example::Inner::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::fieldCall",
            "com::example::Caller::hopCall"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::fieldCall",
            "com::example::Caller::hopCall"
        ]
    );
}

#[test]
fn traces_java_this_rooted_receiver_calls_through_shared_paths() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Deep { int helper(int value) { return value; } }
class Inner { Deep deeper() { return new Deep(); } }
class Holder { Inner inner() { return new Inner(); } }
class Caller {
    Holder holder = new Holder();
    Inner inner() { return new Inner(); }
    int thisHop() { return this.inner().deeper().helper(1); }
    int thisFieldHop() { return this.holder.inner().deeper().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Deep::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::thisFieldHop",
            "com::example::Caller::thisHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::thisFieldHop",
            "com::example::Caller::thisHop"
        ]
    );
}

#[test]
fn traces_java_this_rooted_receiver_calls_across_files_with_imports() {
    let dir = temporary_dir();
    let inner_dir = dir.join("src").join("pkg").join("inner");
    let group_dir = dir.join("src").join("pkg").join("group");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let inner_path = inner_dir.join("Foo.java");
    let group_path = group_dir.join("Group.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&inner_dir).unwrap();
    fs::create_dir_all(&group_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &inner_path,
        "package pkg.inner;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &group_path,
        "package pkg.group;
import pkg.inner.Foo;
public class Group { public Foo foo = new Foo(); }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.group.Group;
import pkg.inner.Foo;
public class Bar {
    Group group = new Group();
    Foo makeFoo() { return new Foo(); }
    public int fieldCall() { return this.group.foo.helper(1); }
    public int hopCall() { return this.makeFoo().helper(1); }
}
",
    )
    .unwrap();

    let foo_helper_symbol = "pkg::inner::Foo::helper";
    let live = trace_symbol_graph(&dir, foo_helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        ["pkg::caller::Bar::fieldCall", "pkg::caller::Bar::hopCall"]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, foo_helper_symbol, TraceDirection::Callers)
            .unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        ["pkg::caller::Bar::fieldCall", "pkg::caller::Bar::hopCall"]
    );
}

#[test]
fn java_this_rooted_receiver_calls_fail_closed_for_unsupported_hops() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Caller {
    int tag() { return 1; }
    void reset() {}
    static Inner make() { return new Inner(); }
    int unknownHop() { return this.unknown().helper(1); }
    int unknownFieldHop() { return this.missing.helper(1); }
    int primitiveHop() { return this.tag().helper(1); }
    int voidHop() { return this.reset().helper(1); }
    int staticHop() { return this.make().helper(1); }
}
",
    )
    .unwrap();

    let target = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "unknown, missing-field, primitive/void-return, and static `this`-rooted chain hops must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_super_rooted_method_hop_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Base { Inner inner() { return new Inner(); } }
class Child extends Base {
    int run() { return super.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Child::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Child::run");
}

#[test]
fn traces_java_super_rooted_member_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Base { Inner member = new Inner(); }
class Child extends Base {
    int run() { return super.member.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Child::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Child::run");
}

#[test]
fn traces_java_super_rooted_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Inner { int helper(int value) { return value; } }
class Base { Inner inner() { return new Inner(); } Inner member = new Inner(); }
class Child extends Base {
    int hopCall() { return super.inner().helper(1); }
    int fieldCall() { return super.member.helper(1); }
}
";
    let helper_symbol = "com::example::Inner::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Child::fieldCall",
            "com::example::Child::hopCall"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Child::fieldCall",
            "com::example::Child::hopCall"
        ]
    );
}

#[test]
fn traces_java_super_rooted_receiver_calls_through_shared_paths() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Deep { int helper(int value) { return value; } }
class Inner { Deep deeper() { return new Deep(); } }
class Base { Inner inner() { return new Inner(); } }
class Mid extends Base { Inner member = new Inner(); }
class Child extends Mid {
    int hopCall() { return super.inner().deeper().helper(1); }
    int fieldHopCall() { return super.member.deeper().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Deep::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Child::fieldHopCall",
            "com::example::Child::hopCall"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Child::fieldHopCall",
            "com::example::Child::hopCall"
        ]
    );
}

#[test]
fn traces_java_super_rooted_receiver_calls_across_files_with_imports() {
    let dir = temporary_dir();
    let inner_dir = dir.join("src").join("pkg").join("inner");
    let base_dir = dir.join("src").join("pkg").join("base");
    let child_dir = dir.join("src").join("pkg").join("child");
    let inner_path = inner_dir.join("Foo.java");
    let base_path = base_dir.join("Base.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&inner_dir).unwrap();
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &inner_path,
        "package pkg.inner;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &base_path,
        "package pkg.base;
import pkg.inner.Foo;
public class Base { public Foo inner() { return new Foo(); } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package pkg.child;
import pkg.base.Base;
public class Child extends Base {
    public int run() { return super.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::inner::Foo::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::child::Child::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::child::Child::run");
}

#[test]
fn java_super_rooted_receiver_calls_fail_closed_for_unsupported_hops() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Base {
    int tag() { return 1; }
    void reset() {}
    static Inner make() { return new Inner(); }
}
class Child extends Base {
    int argHop() { return super.inner(1).helper(1); }
    int unknownHop() { return super.unknown().helper(1); }
    int unknownFieldHop() { return super.missing.helper(1); }
    int primitiveHop() { return super.tag().helper(1); }
    int voidHop() { return super.reset().helper(1); }
    int staticHop() { return super.make().helper(1); }
}
class Solo {
    int noSuperHop() { return super.helper(1); }
}
",
    )
    .unwrap();

    let target = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "argument-taking, unknown, missing-field, primitive/void-return, static, and no-superclass `super`-rooted chain hops must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_qualified_initializer_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner makeInner() { return new Inner(); } }
class Caller {
    int run(Group g) {
        var v = g.makeInner();
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_qualified_initializer_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner makeInner() { return new Inner(); } }
class Caller {
    int run(Group g) {
        var v = g.makeInner();
        return v.helper(1);
    }
}
";
    let helper_symbol = "com::example::Inner::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_qualified_initializer_receiver_calls_through_shared_paths() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Group {
    Inner makeInner() { return new Inner(); }
    Group inner() { return new Group(); }
}
class Base { Inner makeInner() { return new Inner(); } }
class Child extends Base {
    int thisInitializer() {
        var v = this.makeInner();
        return v.helper(1);
    }
    int superInitializer() {
        var v = super.makeInner();
        return v.helper(1);
    }
}
class Caller {
    Group holder = new Group();
    int constructorInitializer() {
        var v = new Group().makeInner();
        return v.helper(1);
    }
    int fieldInitializer() {
        var v = holder.makeInner();
        return v.helper(1);
    }
    int hopInitializer(Group g) {
        var v = g.inner().makeInner();
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 5);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::constructorInitializer",
            "com::example::Caller::fieldInitializer",
            "com::example::Caller::hopInitializer",
            "com::example::Child::superInitializer",
            "com::example::Child::thisInitializer"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 5);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::constructorInitializer",
            "com::example::Caller::fieldInitializer",
            "com::example::Caller::hopInitializer",
            "com::example::Child::superInitializer",
            "com::example::Child::thisInitializer"
        ]
    );
}

#[test]
fn traces_java_qualified_initializer_receiver_calls_across_files_with_imports() {
    let dir = temporary_dir();
    let inner_dir = dir.join("src").join("pkg").join("inner");
    let group_dir = dir.join("src").join("pkg").join("group");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let inner_path = inner_dir.join("Foo.java");
    let group_path = group_dir.join("Group.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&inner_dir).unwrap();
    fs::create_dir_all(&group_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &inner_path,
        "package pkg.inner;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &group_path,
        "package pkg.group;
import pkg.inner.Foo;
public class Group { public Foo makeFoo() { return new Foo(); } }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.group.Group;
public class Bar {
    public int run(Group g) {
        var v = g.makeFoo();
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let foo_helper_symbol = "pkg::inner::Foo::helper";
    let live = trace_symbol_graph(&dir, foo_helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, foo_helper_symbol, TraceDirection::Callers)
            .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_qualified_initializer_receiver_calls_fail_closed_for_unsupported_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner makeInner(int value) { return new Inner(); } }
class Util { static Inner make() { return new Inner(); } }
class Caller {
    Group group = new Group();
    int unboundReceiver() {
        var v = unknown.makeInner();
        return v.helper(1);
    }
    int factoryInferredHop() {
        var a = makeA();
        var b = a.make();
        return b.helper(1);
    }
    int arityMismatch() {
        var v = group.makeInner();
        return v.helper(1);
    }
    int unknownThisCallee() {
        var v = this.missing();
        return v.helper(1);
    }
    int unknownSuperCallee() {
        var v = super.missing();
        return v.helper(1);
    }
    Inner makeA() { return new Inner(); }
}
",
    )
    .unwrap();

    let target = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "unbound, factory-inferred, arity-mismatched, and unknown `this`/`super` qualified initializer receivers must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_method_hop_receiver_calls_through_interface_inheritance() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
interface IFactory { Inner inner(); }
interface IGroup extends IFactory {}
class Caller {
    int run(IGroup group) { return group.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_method_hop_receiver_calls_through_interface_defaults() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
interface IGroup {
    default Inner inner() { return new Inner(); }
}
class Caller {
    int run(IGroup group) { return group.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_method_hop_receiver_calls_through_inherited_interface_defaults() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
interface IFactory {
    default Inner inner() { return new Inner(); }
}
interface IGroup extends IFactory {}
class Caller {
    int run(IGroup group) { return group.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_method_hop_receiver_calls_through_class_receiver_interface_defaults() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
interface IGroup {
    default Inner inner() { return new Inner(); }
}
class Group implements IGroup {}
class Caller {
    int run(Group group) { return group.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_method_hop_receiver_calls_fail_closed_for_ambiguous_or_static_interface_hops() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
interface IGroupA { Inner inner(); }
interface IGroupB { Inner inner(); }
interface IGroup extends IGroupA, IGroupB {}
interface IStatic {
    static Inner inner() { return new Inner(); }
}
class Caller {
    int ambiguousHop(IGroup group) { return group.inner().helper(1); }
    int staticHop(IStatic group) { return group.inner().helper(1); }
}
",
    )
    .unwrap();

    let target = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "branching and static interface method-call hops must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_receiver_calls_with_nested_type_imports_across_files() {
    let dir = temporary_dir();
    let outer_dir = dir.join("src").join("pkg").join("outer");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let outer_path = outer_dir.join("Outer.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&outer_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &outer_path,
        "package pkg.outer;
public class Outer {
    public static class Inner { public int helper(int value) { return value; } }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.outer.Outer.Inner;
public class Bar {
    public int run(Inner inner) { return inner.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::outer::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_bare_calls_with_nested_static_member_imports_across_files() {
    let dir = temporary_dir();
    let outer_dir = dir.join("src").join("pkg").join("outer");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let outer_path = outer_dir.join("Outer.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&outer_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &outer_path,
        "package pkg.outer;
public class Outer {
    public static class Inner {
        public static int helper(int value) { return value; }
    }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import static pkg.outer.Outer.Inner.helper;
public class Bar {
    public int run() { return helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::outer::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_receiver_calls_with_nested_type_imports_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let outer_dir = dir.join("src").join("pkg").join("outer");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let outer_path = outer_dir.join("Outer.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&outer_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &outer_path,
        "package pkg.outer;
public class Outer {
    public static class Inner { public int helper(int value) { return value; } }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller; class Stale {}
",
    )
    .unwrap();
    let overlay = "package pkg.caller;
import pkg.outer.Outer.Inner;
public class Bar {
    public int run(Inner inner) { return inner.helper(1); }
}
";
    let helper_symbol = "pkg::outer::Outer::Inner::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn java_nested_type_imports_fail_closed_for_missing_nested_targets() {
    let dir = temporary_dir();
    let outer_dir = dir.join("src").join("pkg").join("outer");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let outer_path = outer_dir.join("Outer.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&outer_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &outer_path,
        "package pkg.outer;
public class Outer { public static class Other { public int helper(int value) { return value; } } }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.outer.Outer.Missing;
public class Bar {
    public int run(Missing inner) { return inner.helper(1); }
}
",
    )
    .unwrap();

    let target = "pkg::outer::Outer::Other::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "nested type imports naming a missing nested target must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_var_field_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    Helper helper = new Helper();
    int thisField() {
        var v = this.helper;
        return v.helper(1);
    }
    int bareField() {
        var v = helper;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::bareField",
            "com::example::Caller::thisField"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::bareField",
            "com::example::Caller::thisField"
        ]
    );
}

#[test]
fn traces_java_var_static_field_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Util { static Helper STATIC_HELPER = new Helper(); }
class Caller {
    int run() {
        var v = Util.STATIC_HELPER;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_static_imported_field_receiver_calls_across_files() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let util_dir = dir.join("src").join("pkg").join("util");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let util_path = util_dir.join("Util.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&util_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &util_path,
        "package pkg.util;
import pkg.helper.Foo;
public class Util { public static Foo STATIC_HELPER = new Foo(); }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import static pkg.util.Util.STATIC_HELPER;
public class Bar {
    public int run() {
        var v = STATIC_HELPER;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let foo_helper_symbol = "pkg::helper::Foo::helper";
    let live = trace_symbol_graph(&dir, foo_helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, foo_helper_symbol, TraceDirection::Callers)
            .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_var_field_receiver_calls_through_shared_paths() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Holder { Entry entry = new Entry(); }
class Base { Holder holder = new Holder(); }
class Child extends Base {
    int thisChain() {
        var v = this.holder.entry;
        return v.helper(1);
    }
    int superField() {
        var v = super.holder.entry;
        return v.helper(1);
    }
    int bareChain() {
        var v = holder.entry;
        return v.helper(1);
    }
    int bareField() {
        var v = holder;
        return v.entry.helper(1);
    }
}
class Util {
    static Holder REGISTRY = new Holder();
    static class Inner { static Entry STATIC_ENTRY = new Entry(); }
}
class Caller {
    int staticChain() {
        var v = Util.REGISTRY.entry;
        return v.helper(1);
    }
    int nestedStatic() {
        var v = Util.Inner.STATIC_ENTRY;
        return v.helper(1);
    }
    int typedLocal() {
        Holder local = new Holder();
        var v = local;
        return v.entry.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 7);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::nestedStatic",
            "com::example::Caller::staticChain",
            "com::example::Caller::typedLocal",
            "com::example::Child::bareChain",
            "com::example::Child::bareField",
            "com::example::Child::superField",
            "com::example::Child::thisChain"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 7);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::nestedStatic",
            "com::example::Caller::staticChain",
            "com::example::Caller::typedLocal",
            "com::example::Child::bareChain",
            "com::example::Child::bareField",
            "com::example::Child::superField",
            "com::example::Child::thisChain"
        ]
    );
}

#[test]
fn traces_java_var_field_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    Helper helper = new Helper();
    int run() {
        var v = this.helper;
        return v.helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_var_field_receiver_calls_fail_closed_for_unsupported_references() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Util { Helper INSTANCE_HELPER = new Helper(); }
class Caller {
    Helper helper = new Helper();
    int factoryInferredCopy() {
        var local = makeHelper();
        var v = local;
        return v.helper(1);
    }
    int nonStaticTypeField() {
        var v = Util.INSTANCE_HELPER;
        return v.helper(1);
    }
    int unknownField() {
        var v = missing;
        return v.helper(1);
    }
    int unknownTypeField() {
        var v = Missing.STATIC;
        return v.helper(1);
    }
    int unknownThisField() {
        var v = this.missing;
        return v.helper(1);
    }
    int unknownSuperField() {
        var v = super.missing;
        return v.helper(1);
    }
    int boundReceiverField() {
        var v = helper.other;
        return v.helper(1);
    }
    Helper makeHelper() { return new Helper(); }
}
",
    )
    .unwrap();

    let target = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "factory-inferred copies, non-static type fields, unknown fields, unknown types, unknown `this`/`super` fields, and bound-receiver field chains with unknown fields must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_var_field_receiver_calls_through_method_hops() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Group {
    Entry entry = new Entry();
    Group inner() { return this; }
}
class Holder {
    Group group = new Group();
    Group inner() { return group; }
}
class Base { Holder holder = new Holder(); }
class Child extends Base {
    Group inner() { return holder.group; }
    int thisHop() {
        var v = this.holder.inner().entry;
        return v.helper(1);
    }
    int thisDirectHop() {
        var v = this.inner().entry;
        return v.helper(1);
    }
    int superHop() {
        var v = super.holder.inner().entry;
        return v.helper(1);
    }
    int bareHop() {
        var v = holder.inner().entry;
        return v.helper(1);
    }
    int bareFieldHop() {
        var v = holder.group.inner().entry;
        return v.helper(1);
    }
}
class Util { static Holder REGISTRY = new Holder(); }
class Caller {
    int staticHop() {
        var v = Util.REGISTRY.inner().entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 6);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::staticHop",
            "com::example::Child::bareFieldHop",
            "com::example::Child::bareHop",
            "com::example::Child::superHop",
            "com::example::Child::thisDirectHop",
            "com::example::Child::thisHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 6);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::staticHop",
            "com::example::Child::bareFieldHop",
            "com::example::Child::bareHop",
            "com::example::Child::superHop",
            "com::example::Child::thisDirectHop",
            "com::example::Child::thisHop"
        ]
    );
}

#[test]
fn traces_java_var_field_receiver_calls_through_method_hops_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Entry { int helper(int value) { return value; } }
class Group { Entry entry = new Entry(); Group inner() { return this; } }
class Base { Group group = new Group(); }
class Caller extends Base {
    int run() {
        var v = this.group.inner().entry;
        return v.helper(1);
    }
    int runBare() {
        var v = group.inner().entry;
        return v.helper(1);
    }
}
";
    let helper_symbol = "com::example::Entry::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        ["com::example::Caller::run", "com::example::Caller::runBare"]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        ["com::example::Caller::run", "com::example::Caller::runBare"]
    );
}

#[test]
fn traces_java_var_field_receiver_calls_through_bound_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Group {
    Entry entry = new Entry();
    Group inner() { return this; }
}
class Holder {
    Group group = new Group();
    Group inner() { return group; }
}
class Caller {
    Holder holder = new Holder();
    int fieldChain() {
        var v = holder.group.entry;
        return v.helper(1);
    }
    int fieldHop() {
        var v = holder.inner().entry;
        return v.helper(1);
    }
    int paramChain(Holder local) {
        var v = local.group.entry;
        return v.helper(1);
    }
    int paramHop(Holder local) {
        var v = local.inner().entry;
        return v.helper(1);
    }
    int declaredLocal() {
        Holder local = new Holder();
        var v = local.group.entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 5);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::declaredLocal",
            "com::example::Caller::fieldChain",
            "com::example::Caller::fieldHop",
            "com::example::Caller::paramChain",
            "com::example::Caller::paramHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 5);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::declaredLocal",
            "com::example::Caller::fieldChain",
            "com::example::Caller::fieldHop",
            "com::example::Caller::paramChain",
            "com::example::Caller::paramHop"
        ]
    );
}

#[test]
fn traces_java_var_field_receiver_calls_through_bound_receivers_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Entry { int helper(int value) { return value; } }
class Group { Entry entry = new Entry(); Group inner() { return this; } }
class Caller {
    Group group = new Group();
    int fieldChain() {
        var v = group.entry;
        return v.helper(1);
    }
    int paramChain(Group g) {
        var v = g.entry;
        return v.helper(1);
    }
}
";
    let helper_symbol = "com::example::Entry::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::fieldChain",
            "com::example::Caller::paramChain"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::fieldChain",
            "com::example::Caller::paramChain"
        ]
    );
}

#[test]
fn traces_java_var_static_imported_field_chain_receiver_calls_across_files() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let util_dir = dir.join("src").join("pkg").join("util");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let util_path = util_dir.join("Util.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&util_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &util_path,
        "package pkg.util;
import pkg.helper.Foo;
class Holder { public Foo foo = new Foo(); }
public class Util { public static Holder HOLDER = new Holder(); }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import static pkg.util.Util.HOLDER;
public class Bar {
    public int run() {
        var v = HOLDER.foo;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let foo_helper_symbol = "pkg::helper::Foo::helper";
    let live = trace_symbol_graph(&dir, foo_helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, foo_helper_symbol, TraceDirection::Callers)
            .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_var_field_receiver_calls_through_constructor_roots() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Group {
    Entry entry = new Entry();
    Group inner() { return this; }
}
class Holder {
    Group group = new Group();
    Group inner() { return group; }
}
class Caller {
    int constructorChain() {
        var v = new Holder().group.entry;
        return v.helper(1);
    }
    int constructorHop() {
        var v = new Holder().inner().entry;
        return v.helper(1);
    }
    int constructorDirect() {
        var v = new Group().entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 3);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::constructorChain",
            "com::example::Caller::constructorDirect",
            "com::example::Caller::constructorHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 3);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::constructorChain",
            "com::example::Caller::constructorDirect",
            "com::example::Caller::constructorHop"
        ]
    );
}

#[test]
fn traces_java_var_field_receiver_calls_through_constructor_roots_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Entry { int helper(int value) { return value; } }
class Group { Entry entry = new Entry(); Group inner() { return this; } }
class Holder { Group group = new Group(); }
class Caller {
    int run() {
        var v = new Holder().group.entry;
        return v.helper(1);
    }
    int runDirect() {
        var v = new Group().entry;
        return v.helper(1);
    }
}
";
    let helper_symbol = "com::example::Entry::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::run",
            "com::example::Caller::runDirect"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::run",
            "com::example::Caller::runDirect"
        ]
    );
}

#[test]
fn traces_java_var_static_type_factory_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Util {
    static Helper make() { return new Helper(); }
    static Helper make(int value) { return new Helper(); }
    static class Nested {
        static Helper nestedMake() { return new Helper(); }
    }
}
interface Factory {
    static Helper make() { return new Helper(); }
}
class Caller {
    int simpleFactory() {
        var v = Util.make();
        return v.helper(1);
    }
    int arityFactory() {
        var v = Util.make(2);
        return v.helper(1);
    }
    int nestedFactory() {
        var v = Util.Nested.nestedMake();
        return v.helper(1);
    }
    int interfaceFactory() {
        var v = Factory.make();
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 4);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::arityFactory",
            "com::example::Caller::interfaceFactory",
            "com::example::Caller::nestedFactory",
            "com::example::Caller::simpleFactory"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 4);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::arityFactory",
            "com::example::Caller::interfaceFactory",
            "com::example::Caller::nestedFactory",
            "com::example::Caller::simpleFactory"
        ]
    );
}

#[test]
fn traces_java_var_static_type_factory_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Util { static Helper make() { return new Helper(); } }
class Caller {
    int run() {
        var v = Util.make();
        return v.helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_static_type_factory_receiver_calls_across_files_with_imports() {
    let dir = temporary_dir();
    let factory_dir = dir.join("src").join("pkg").join("factory");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let factory_path = factory_dir.join("Util.java");
    let caller_path = caller_dir.join("Caller.java");
    let helper_path = helper_dir.join("Helper.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&factory_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::create_dir_all(&helper_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Helper { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &factory_path,
        "package pkg.factory;
import pkg.helper.Helper;
public class Util {
    public static Helper make() { return new Helper(); }
    public static class Nested {
        public static Helper nestedMake() { return new Helper(); }
    }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.factory.Util;
public class Caller {
    public int importedFactory() {
        var v = Util.make();
        return v.helper(1);
    }
    public int importedNestedFactory() {
        var v = Util.Nested.nestedMake();
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::helper::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "pkg::caller::Caller::importedFactory",
            "pkg::caller::Caller::importedNestedFactory"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "pkg::caller::Caller::importedFactory",
            "pkg::caller::Caller::importedNestedFactory"
        ]
    );
}

#[test]
fn java_var_static_type_factory_receiver_calls_fail_closed_for_unsupported_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Util {
    static Helper make(int value) { return new Helper(); }
    static Helper varargs(int... values) { return new Helper(); }
}
class Caller {
    int arityMismatch() {
        var v = Util.make();
        return v.helper(1);
    }
    int varargsFactory() {
        var v = Util.varargs(1);
        return v.helper(1);
    }
    int unknownType() {
        var v = Missing.make();
        return v.helper(1);
    }
}
class ShadowingCaller {
    Helper Util;
    int shadowedByField() {
        var v = Util.make();
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "arity-mismatched static factories, varargs factories, unknown types, and bound-name shadowing of static type receivers must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_var_static_factory_method_hop_field_receiver_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Group {
    Entry entry = new Entry();
    Group inner() { return this; }
}
class Util {
    static Group factory() { return new Group(); }
    static class Nested {
        static Group nestedFactory() { return new Group(); }
    }
}
class Caller {
    int staticFactoryHop() {
        var v = Util.factory().entry;
        return v.helper(1);
    }
    int nestedFactoryHop() {
        var v = Util.Nested.nestedFactory().entry;
        return v.helper(1);
    }
    int staticFactoryInstanceHop() {
        var v = Util.factory().inner().entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 3);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::nestedFactoryHop",
            "com::example::Caller::staticFactoryHop",
            "com::example::Caller::staticFactoryInstanceHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 3);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::nestedFactoryHop",
            "com::example::Caller::staticFactoryHop",
            "com::example::Caller::staticFactoryInstanceHop"
        ]
    );
}

#[test]
fn traces_java_var_static_factory_method_hop_field_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Entry { int helper(int value) { return value; } }
class Group { Entry entry = new Entry(); }
class Util { static Group factory() { return new Group(); } }
class Caller {
    int run() {
        var v = Util.factory().entry;
        return v.helper(1);
    }
}
";
    let helper_symbol = "com::example::Entry::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_static_factory_method_hop_field_receiver_calls_across_files_with_imports() {
    let dir = temporary_dir();
    let factory_dir = dir.join("src").join("pkg").join("factory");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let factory_path = factory_dir.join("Util.java");
    let caller_path = caller_dir.join("Caller.java");
    let helper_path = helper_dir.join("Helper.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&factory_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::create_dir_all(&helper_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Helper { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &factory_path,
        "package pkg.factory;
import pkg.helper.Helper;
public class Util {
    public static Holder factory() { return new Holder(); }
    public static class Holder {
        public Helper entry = new Helper();
        public static Holder nestedFactory() { return new Holder(); }
    }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.factory.Util;
public class Caller {
    public int importedFactoryHop() {
        var v = Util.factory().entry;
        return v.helper(1);
    }
    public int importedNestedFactoryHop() {
        var v = Util.Holder.nestedFactory().entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::helper::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "pkg::caller::Caller::importedFactoryHop",
            "pkg::caller::Caller::importedNestedFactoryHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "pkg::caller::Caller::importedFactoryHop",
            "pkg::caller::Caller::importedNestedFactoryHop"
        ]
    );
}

#[test]
fn java_var_static_factory_method_hop_field_receiver_calls_fail_closed_for_unsupported_references()
{
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Group {
    Entry entry = new Entry();
}
class Util {
    static Group factory(int value) { return new Group(); }
    static Group varargs(int... values) { return new Group(); }
    Group instanceFactory() { return new Group(); }
    static int primitive() { return 0; }
    static Missing missingReturn() { return null; }
}
class Caller {
    int instanceFactoryHop() {
        var v = Util.instanceFactory().entry;
        return v.helper(1);
    }
    int arityMismatch() {
        var v = Util.factory().entry;
        return v.helper(1);
    }
    int varargsFactory() {
        var v = Util.varargs(1).entry;
        return v.helper(1);
    }
    int unknownFactory() {
        var v = Util.missing().entry;
        return v.helper(1);
    }
    int primitiveReturn() {
        var v = Util.primitive().entry;
        return v.helper(1);
    }
    int unknownReturnType() {
        var v = Util.missingReturn().entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "non-static factories, arity-mismatched factories, varargs factories, unknown factories, and primitive or unknown factory return types must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_var_factory_method_hop_field_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Group {
    Entry entry = new Entry();
    Group inner() { return this; }
    Group makeFoo() { return new Group(); }
}
class Util {
    static Group make() { return new Group(); }
}
class Caller {
    Group group = new Group();
    Group makeFoo() { return new Group(); }
    int bareFactoryHop() {
        var v = makeFoo().entry;
        return v.helper(1);
    }
    int bareFactoryInstanceHop() {
        var v = makeFoo().inner().entry;
        return v.helper(1);
    }
    int staticTypeFactoryHop() {
        var v = Util.make().entry;
        return v.helper(1);
    }
    int boundFactoryHop() {
        var v = group.makeFoo().entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 4);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::bareFactoryHop",
            "com::example::Caller::bareFactoryInstanceHop",
            "com::example::Caller::boundFactoryHop",
            "com::example::Caller::staticTypeFactoryHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 4);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::bareFactoryHop",
            "com::example::Caller::bareFactoryInstanceHop",
            "com::example::Caller::boundFactoryHop",
            "com::example::Caller::staticTypeFactoryHop"
        ]
    );
}

#[test]
fn traces_java_var_factory_method_hop_field_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Entry { int helper(int value) { return value; } }
class Group { Entry entry = new Entry(); }
class Caller {
    Group makeFoo() { return new Group(); }
    int run() {
        var v = makeFoo().entry;
        return v.helper(1);
    }
}
";
    let helper_symbol = "com::example::Entry::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_factory_method_hop_field_receiver_calls_across_files_with_static_import() {
    let dir = temporary_dir();
    let factory_dir = dir.join("src").join("pkg").join("factory");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let factory_path = factory_dir.join("Util.java");
    let caller_path = caller_dir.join("Caller.java");
    let helper_path = helper_dir.join("Helper.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&factory_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::create_dir_all(&helper_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Helper { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &factory_path,
        "package pkg.factory;
import pkg.helper.Helper;
public class Util {
    public static Holder make() { return new Holder(); }
    public static class Holder {
        public Helper entry = new Helper();
        public static Holder nestedMake() { return new Holder(); }
    }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import static pkg.factory.Util.make;
import static pkg.factory.Util.Holder.nestedMake;
public class Caller {
    public int importedFactoryHop() {
        var v = make().entry;
        return v.helper(1);
    }
    public int importedNestedFactoryHop() {
        var v = nestedMake().entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::helper::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "pkg::caller::Caller::importedFactoryHop",
            "pkg::caller::Caller::importedNestedFactoryHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "pkg::caller::Caller::importedFactoryHop",
            "pkg::caller::Caller::importedNestedFactoryHop"
        ]
    );
}

#[test]
fn java_var_factory_method_hop_field_receiver_calls_fail_closed_for_unsupported_references() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Group { Entry entry = new Entry(); }
class Util {
    static Group make(int value) { return new Group(); }
}
class Caller {
    Group make(int value) { return new Group(); }
    void makeVoid() { }
    int primitive() { return 0; }
    int arityMismatch() {
        var v = make().entry;
        return v.helper(1);
    }
    int staticArityMismatch() {
        var v = Util.make().entry;
        return v.helper(1);
    }
    int unknownFactory() {
        var v = missing().entry;
        return v.helper(1);
    }
    int voidFactory() {
        var v = makeVoid().entry;
        return v.helper(1);
    }
    int primitiveFactory() {
        var v = primitive().entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "arity-mismatched, unknown, void-returning, and primitive-returning factory method-call hops must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}
