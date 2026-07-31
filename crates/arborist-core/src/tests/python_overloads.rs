use super::*;

const OVERLOADED_SOURCE: &str = r#"from typing import overload

class LokDB:
    @overload
    def get(self, key: str) -> str: ...

    @overload
    def get(self, key: int) -> int: ...

    def get(self, key):
        return key
"#;

const ALIASED_OVERLOAD_SOURCE: &str = r#"from typing import overload as typing_overload
from typing_extensions import overload as extensions_overload

class AliasDB:
    @typing_overload
    def get(self, key: str) -> str: ...

    @extensions_overload
    def get(self, key: int) -> int: ...

    def get(self, key):
        return key
"#;

#[test]
fn typing_overload_import_aliases_keep_overload_ids_consistent() {
    let dir = temporary_dir();
    let source_path = dir.join("aliases.py");
    let db_path = dir.join("symbols.db");
    fs::write(&source_path, ALIASED_OVERLOAD_SOURCE).unwrap();

    let source_anchor = source_path.to_string_lossy().replace('\\', "/");
    let expected = vec![
        format!("{source_anchor}::AliasDB.get#implementation"),
        format!("{source_anchor}::AliasDB.get#overload[1]"),
        format!("{source_anchor}::AliasDB.get#overload[2]"),
    ];
    let skeleton = get_semantic_skeleton(&source_path, ALIASED_OVERLOAD_SOURCE, 2, &[]).unwrap();
    let mut skeleton_ids = skeleton
        .available_symbols
        .iter()
        .filter(|symbol| symbol.semantic_path == "AliasDB.get")
        .map(|symbol| symbol.symbol_id.clone())
        .collect::<Vec<_>>();
    skeleton_ids.sort();
    assert_eq!(skeleton_ids, expected);

    let mut live_ids = list_symbols(&dir, 20)
        .unwrap()
        .symbols
        .into_iter()
        .filter(|symbol| symbol.semantic_path == "AliasDB.get")
        .map(|symbol| symbol.symbol_id)
        .collect::<Vec<_>>();
    live_ids.sort();
    assert_eq!(live_ids, expected);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let mut persisted_ids = list_symbols_from_index(&db_path, 20)
        .unwrap()
        .symbols
        .into_iter()
        .filter(|symbol| symbol.semantic_path == "AliasDB.get")
        .map(|symbol| symbol.symbol_id)
        .collect::<Vec<_>>();
    persisted_ids.sort();
    assert_eq!(persisted_ids, expected);
}

#[test]
fn decorator_argument_text_does_not_mark_a_definition_as_an_overload() {
    let source = r#"class Store:
    @decorator("""
@overload
""")
    def get(self, key: str) -> str: ...

    def get(self, key):
        return key
"#;

    let skeleton = get_semantic_skeleton(Path::new("sample.py"), source, 2, &[]).unwrap();
    let mut ids = skeleton
        .available_symbols
        .iter()
        .filter(|symbol| symbol.semantic_path == "Store.get")
        .map(|symbol| symbol.symbol_id.clone())
        .collect::<Vec<_>>();
    ids.sort();

    let source_anchor = std::env::current_dir()
        .unwrap()
        .join("sample.py")
        .to_string_lossy()
        .replace('\\', "/");
    assert_eq!(
        ids,
        [
            format!("{source_anchor}::Store.get#definition[1]"),
            format!("{source_anchor}::Store.get#definition[2]"),
        ]
    );
}

fn inline_overload_id(suffix: &str) -> String {
    let file_path = std::env::current_dir()
        .unwrap()
        .join("lokdb.py")
        .to_string_lossy()
        .replace('\\', "/");
    format!("{file_path}::LokDB.get#{suffix}")
}

#[test]
fn assigns_unique_python_overload_ids_across_skeleton_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source_path = dir.join("lokdb.py");
    let db_path = dir.join("symbols.db");
    fs::write(&source_path, OVERLOADED_SOURCE).unwrap();

    let skeleton = get_semantic_skeleton(&source_path, OVERLOADED_SOURCE, 2, &[]).unwrap();
    let source_anchor = source_path.to_string_lossy().replace('\\', "/");
    let expected = vec![
        format!("{source_anchor}::LokDB.get#implementation"),
        format!("{source_anchor}::LokDB.get#overload[1]"),
        format!("{source_anchor}::LokDB.get#overload[2]"),
    ];
    let mut skeleton_ids = skeleton
        .available_symbols
        .iter()
        .filter(|symbol| symbol.semantic_path == "LokDB.get")
        .map(|symbol| symbol.symbol_id.clone())
        .collect::<Vec<_>>();
    skeleton_ids.sort();
    assert_eq!(skeleton_ids, expected);

    let mut live_ids = list_symbols(&dir, 20)
        .unwrap()
        .symbols
        .into_iter()
        .filter(|symbol| symbol.semantic_path == "LokDB.get")
        .map(|symbol| symbol.symbol_id)
        .collect::<Vec<_>>();
    live_ids.sort();
    assert_eq!(live_ids, expected);

    let live_error = read_symbol(&dir, "LokDB.get")
        .expect_err("an overload-set semantic path must not silently select a declaration");
    assert!(
        live_error
            .to_string()
            .contains("ambiguous Python semantic path")
    );
    assert!(live_error.to_string().contains(&expected[0]));
    assert!(
        read_symbol(&dir, &expected[0])
            .unwrap()
            .source
            .contains("return key")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let mut persisted_ids = list_symbols_from_index(&db_path, 20)
        .unwrap()
        .symbols
        .into_iter()
        .filter(|symbol| symbol.semantic_path == "LokDB.get")
        .map(|symbol| symbol.symbol_id)
        .collect::<Vec<_>>();
    persisted_ids.sort();
    assert_eq!(persisted_ids, expected);
    assert!(
        read_symbol_from_index(&db_path, "LokDB.get")
            .expect_err("persisted overload-set paths must also be rejected")
            .to_string()
            .contains("ambiguous Python semantic path")
    );

    let live_trace_error = trace_symbol_graph(&dir, "LokDB.get", TraceDirection::Both)
        .expect_err("live trace must reject an overload-set semantic path");
    assert!(
        live_trace_error
            .to_string()
            .contains("ambiguous Python semantic path")
    );
    let live_trace = trace_symbol_graph(&dir, &expected[0], TraceDirection::Both).unwrap();
    assert_eq!(live_trace.symbol.symbol_id, expected[0]);

    let persisted_trace_error =
        trace_symbol_graph_from_index(&db_path, "LokDB.get", TraceDirection::Both)
            .expect_err("persisted trace must reject an overload-set semantic path");
    assert!(
        persisted_trace_error
            .to_string()
            .contains("ambiguous Python semantic path")
    );
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, &expected[0], TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.symbol.symbol_id, expected[0]);
}

#[test]
fn python_overload_ids_are_unique_across_files_and_plain_paths_remain_ambiguous() {
    let dir = temporary_dir();
    let first = dir.join("first.py");
    let second = dir.join("second.py");
    let singleton = dir.join("singleton.py");
    fs::write(&first, OVERLOADED_SOURCE).unwrap();
    fs::write(&second, OVERLOADED_SOURCE).unwrap();
    fs::write(
        &singleton,
        "class LokDB:\n    def get(self, key):\n        return key\n",
    )
    .unwrap();

    let overload_symbols = list_symbols(&dir, 20)
        .unwrap()
        .symbols
        .into_iter()
        .filter(|symbol| {
            symbol.semantic_path == "LokDB.get"
                && (symbol.symbol_id.contains("#overload[")
                    || symbol.symbol_id.contains("#implementation"))
        })
        .collect::<Vec<_>>();
    assert_eq!(overload_symbols.len(), 6);
    assert_eq!(
        overload_symbols
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        6
    );

    for path in [&first, &second] {
        let anchor = path.to_string_lossy().replace('\\', "/");
        let symbol_id = format!("{anchor}::LokDB.get#implementation");
        let read = read_symbol(&dir, &symbol_id).unwrap();
        assert_eq!(read.symbol.symbol_id, symbol_id);
        assert_eq!(read.symbol.file_path, anchor);
    }
    let singleton_anchor = singleton.to_string_lossy().replace('\\', "/");
    let singleton_id = format!("{singleton_anchor}::LokDB.get");
    let singleton_read = read_symbol(&dir, &singleton_id).unwrap();
    assert_eq!(singleton_read.symbol.symbol_id, singleton_id);
    assert_eq!(singleton_read.symbol.file_path, singleton_anchor);

    let error = read_symbol(&dir, "LokDB.get")
        .expect_err("a singleton exact ID must not mask overload candidates in other files");
    assert!(error.to_string().contains("ambiguous Python semantic path"));
    assert!(
        error
            .to_string()
            .contains("first.py::LokDB.get#implementation")
    );
    assert!(
        error
            .to_string()
            .contains("second.py::LokDB.get#implementation")
    );
}

#[test]
fn file_qualified_singleton_python_ids_can_patch_the_selected_file() {
    let source = "class LokDB:\n    def get(self, key):\n        return key\n";
    let file_path = std::env::current_dir().unwrap().join("singleton.py");
    let normalized = file_path.to_string_lossy().replace('\\', "/");
    let symbol_id = format!("{normalized}::LokDB.get");

    let result = patch_ast_node(
        Path::new("singleton.py"),
        source,
        &symbol_id,
        "def get(self, key):\n    return \"selected\"\n",
        None,
    )
    .unwrap();
    assert!(result.applied, "{:#?}", result.validation);
    assert_eq!(result.resolved_symbol_id, symbol_id);
    assert!(result.updated_source.contains("return \"selected\""));
}

#[test]
fn file_qualified_singleton_python_ids_expand_the_selected_symbol() {
    let source = "class LokDB:\n    def get(self, key):\n        return key\n";
    let file_path = std::env::current_dir().unwrap().join("singleton.py");
    let normalized = file_path.to_string_lossy().replace('\\', "/");
    let symbol_id = format!("{normalized}::LokDB.get");

    let expanded = get_semantic_skeleton(
        Path::new("singleton.py"),
        source,
        1,
        std::slice::from_ref(&symbol_id),
    )
    .unwrap();
    assert!(expanded.skeleton.contains("return key"));
}

#[test]
fn incremental_refresh_rewrites_unchanged_files_when_python_ids_change() {
    let dir = temporary_dir();
    let first = dir.join("first.py");
    let second = dir.join("second.py");
    let db_path = dir.join("symbols.db");
    let singleton_source = "class LokDB:\n    def get(self, key):\n        return key\n";
    fs::write(&first, singleton_source).unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    fs::write(&second, singleton_source).unwrap();
    refresh_symbol_index_for_file(&dir, &db_path, &second).unwrap();
    let mut collided_ids = list_symbols_from_index(&db_path, 20)
        .unwrap()
        .symbols
        .into_iter()
        .filter(|symbol| symbol.semantic_path == "LokDB.get")
        .map(|symbol| symbol.symbol_id)
        .collect::<Vec<_>>();
    collided_ids.sort();
    let first_anchor = first.to_string_lossy().replace('\\', "/");
    let second_anchor = second.to_string_lossy().replace('\\', "/");
    assert_eq!(
        collided_ids,
        [
            format!("{first_anchor}::LokDB.get"),
            format!("{second_anchor}::LokDB.get"),
        ]
    );

    fs::remove_file(&second).unwrap();
    refresh_symbol_index_for_file(&dir, &db_path, &second).unwrap();
    let remaining = list_symbols_from_index(&db_path, 20)
        .unwrap()
        .symbols
        .into_iter()
        .filter(|symbol| symbol.semantic_path == "LokDB.get")
        .collect::<Vec<_>>();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].symbol_id, "LokDB.get");
    assert_eq!(remaining[0].file_path, first_anchor);
}

#[test]
fn python_overload_skeleton_expansion_requires_an_exact_symbol_id() {
    let ambiguous = get_semantic_skeleton(
        Path::new("lokdb.py"),
        OVERLOADED_SOURCE,
        1,
        &["LokDB.get".to_string()],
    )
    .expect_err("ambiguous overload-set expansion must be rejected");
    assert!(
        ambiguous
            .to_string()
            .contains("ambiguous Python semantic path")
    );
    let overload_two = inline_overload_id("overload[2]");
    assert!(ambiguous.to_string().contains(&overload_two));

    let expanded = get_semantic_skeleton(
        Path::new("lokdb.py"),
        OVERLOADED_SOURCE,
        1,
        std::slice::from_ref(&overload_two),
    )
    .unwrap();
    assert!(
        expanded
            .skeleton
            .contains("def get(self, key: int) -> int: ...")
    );
    assert!(
        expanded
            .available_symbols
            .iter()
            .any(|symbol| symbol.symbol_id == overload_two)
    );
}

#[test]
fn python_overload_position_patch_resolves_the_exact_implementation() {
    let result = patch_ast_node_at_position(
        Path::new("lokdb.py"),
        OVERLOADED_SOURCE,
        &Position { row: 9, column: 8 },
        "def get(self, key):\n    return \"position\"\n",
        None,
    )
    .unwrap();

    assert!(result.applied, "{:#?}", result.validation);
    assert_eq!(
        result.resolved_symbol_id,
        inline_overload_id("implementation")
    );
    assert!(result.updated_source.contains("return \"position\""));
    assert_eq!(result.updated_source.matches("@overload").count(), 2);
}

#[test]
fn python_overload_patches_require_an_exact_symbol_id() {
    let ambiguous = patch_ast_node(
        Path::new("lokdb.py"),
        OVERLOADED_SOURCE,
        "LokDB.get",
        "def get(self, key):\n    return \"updated\"\n",
        None,
    )
    .expect_err("ambiguous overload-set patches must be rejected before editing");
    assert!(
        ambiguous
            .to_string()
            .contains("ambiguous Python semantic path")
    );
    assert!(
        ambiguous
            .to_string()
            .contains("lokdb.py::LokDB.get#overload[1]")
    );
    assert!(
        ambiguous
            .to_string()
            .contains("lokdb.py::LokDB.get#implementation")
    );

    let result = patch_ast_node(
        Path::new("lokdb.py"),
        OVERLOADED_SOURCE,
        &inline_overload_id("implementation"),
        "def get(self, key):\n    return \"updated\"\n",
        None,
    )
    .unwrap();
    assert!(result.applied, "{:#?}", result.validation);
    assert_eq!(
        result.resolved_symbol_id,
        inline_overload_id("implementation")
    );
    assert_eq!(result.resolved_path, "LokDB.get");
    assert_eq!(result.updated_source.matches("@overload").count(), 2);
    assert!(result.updated_source.contains("return \"updated\""));
}
