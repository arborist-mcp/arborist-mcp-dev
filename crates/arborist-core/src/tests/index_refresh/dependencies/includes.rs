use super::*;

#[test]
fn refreshes_c_include_dependents_for_header_change() {
    let dir = temporary_dir();
    let alpha_header = dir.join("alpha.h");
    let alpha_source = dir.join("alpha.c");
    let zeta_header = dir.join("zeta.h");
    let zeta_source = dir.join("zeta.c");
    let wrapper_header = dir.join("wrapper.h");
    let caller = dir.join("caller.c");
    let db_path = dir.join("symbols.db");

    fs::write(&alpha_header, "int helper(int value);\n").unwrap();
    fs::write(
        &alpha_source,
        "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(&zeta_header, "int helper(int value);\n").unwrap();
    fs::write(
        &zeta_source,
        "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
    )
    .unwrap();
    fs::write(&wrapper_header, "#include \"alpha.h\"\n").unwrap();
    fs::write(
        &caller,
        "#include \"wrapper.h\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(initial_trace.callees.len(), 1);
    assert_eq!(
        initial_trace.callees[0].file_path,
        alpha_source.to_string_lossy().replace('\\', "/")
    );

    fs::write(&wrapper_header, "#include \"zeta.h\"\n").unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &wrapper_header).unwrap();
    assert_eq!(stats.indexed_files, 6);
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 4);

    let updated_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(updated_trace.callees.len(), 1);
    assert_eq!(
        updated_trace.callees[0].file_path,
        zeta_source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn refreshes_c_include_dependents_for_parent_relative_header() {
    let dir = temporary_dir();
    let include_dir = dir.join("include");
    let source_dir = dir.join("src");
    let alpha_header = include_dir.join("alpha.h");
    let alpha_source = include_dir.join("alpha.c");
    let zeta_header = include_dir.join("zeta.h");
    let zeta_source = include_dir.join("zeta.c");
    let wrapper_header = include_dir.join("wrapper.h");
    let caller = source_dir.join("caller.c");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&include_dir).unwrap();
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(&alpha_header, "int helper(int value);\n").unwrap();
    fs::write(
        &alpha_source,
        "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(&zeta_header, "int helper(int value);\n").unwrap();
    fs::write(
        &zeta_source,
        "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
    )
    .unwrap();
    fs::write(&wrapper_header, "#include \"alpha.h\"\n").unwrap();
    fs::write(
            &caller,
            "#include \"../include/wrapper.h\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
        )
        .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(initial_trace.callees.len(), 1);
    assert_eq!(
        initial_trace.callees[0].file_path,
        alpha_source.to_string_lossy().replace('\\', "/")
    );

    fs::write(&wrapper_header, "#include \"zeta.h\"\n").unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &wrapper_header).unwrap();
    assert_eq!(stats.indexed_files, 6);
    assert_eq!(stats.rebuilt_files, 2);

    let updated_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(updated_trace.callees.len(), 1);
    assert_eq!(
        updated_trace.callees[0].file_path,
        zeta_source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn refreshes_c_include_dependents_for_hpp_header_change() {
    let dir = temporary_dir();
    let alpha_header = dir.join("alpha.HPP");
    let alpha_source = dir.join("alpha.c");
    let zeta_header = dir.join("zeta.HPP");
    let zeta_source = dir.join("zeta.c");
    let wrapper_header = dir.join("wrapper.hpp");
    let caller = dir.join("caller.c");
    let db_path = dir.join("symbols.db");

    fs::write(&alpha_header, "int helper(int value);\n").unwrap();
    fs::write(
        &alpha_source,
        "#include \"alpha.HPP\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(&zeta_header, "int helper(int value);\n").unwrap();
    fs::write(
        &zeta_source,
        "#include \"zeta.HPP\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
    )
    .unwrap();
    fs::write(&wrapper_header, "#include \"alpha.HPP\"\n").unwrap();
    fs::write(
        &caller,
        "#include \"wrapper.hpp\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(initial_trace.callees.len(), 1);
    assert_eq!(
        initial_trace.callees[0].file_path,
        alpha_source.to_string_lossy().replace('\\', "/")
    );

    fs::write(&wrapper_header, "#include \"zeta.HPP\"\n").unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &wrapper_header).unwrap();
    assert_eq!(stats.indexed_files, 6);
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 4);

    let updated_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(updated_trace.callees.len(), 1);
    assert_eq!(
        updated_trace.callees[0].file_path,
        zeta_source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn refreshes_c_include_dependents_for_deleted_header() {
    let dir = temporary_dir();
    let alpha_header = dir.join("alpha.h");
    let alpha_source = dir.join("alpha.c");
    let zeta_header = dir.join("zeta.h");
    let zeta_source = dir.join("zeta.c");
    let wrapper_header = dir.join("wrapper.h");
    let caller = dir.join("caller.c");
    let db_path = dir.join("symbols.db");

    fs::write(&alpha_header, "int helper(int value);\n").unwrap();
    fs::write(
        &alpha_source,
        "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(&zeta_header, "int helper(int value);\n").unwrap();
    fs::write(
        &zeta_source,
        "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
    )
    .unwrap();
    fs::write(&wrapper_header, "#include \"alpha.h\"\n").unwrap();
    fs::write(
        &caller,
        "#include \"wrapper.h\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(initial_trace.callees.len(), 1);
    assert_eq!(
        initial_trace.callees[0].file_path,
        alpha_source.to_string_lossy().replace('\\', "/")
    );

    fs::remove_file(&wrapper_header).unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &wrapper_header).unwrap();
    assert_eq!(stats.indexed_files, 5);
    assert_eq!(stats.rebuilt_files, 2);

    let updated_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(updated_trace.callees.len(), 1);
    assert_eq!(
        updated_trace.callees[0].file_path,
        zeta_source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn does_not_refresh_dependents_for_missing_system_include() {
    let dir = temporary_dir();
    let helper_header = dir.join("helper.h");
    let helper_source = dir.join("helper.c");
    let caller = dir.join("caller.c");
    let db_path = dir.join("symbols.db");

    fs::write(&helper_header, "int helper(int value);\n").unwrap();
    fs::write(
        &helper_source,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "#include <stdio.h>\n#include \"helper.h\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
        )
        .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();

    let missing_system_header = dir.join("stdio.h");
    let stats = refresh_symbol_index_for_file(&dir, &db_path, &missing_system_header).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 0);
    assert_eq!(stats.reused_files, 3);
}

#[test]
fn refreshes_index_when_symbol_becomes_resolvable() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def assist(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "def orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();

    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(initial_trace.callees.is_empty());

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 1);

    let updated_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        updated_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn refreshes_index_when_symbol_becomes_unresolvable() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "def orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        initial_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );

    fs::write(
        &helper,
        "def assist(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 1);

    let updated_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(updated_trace.callees.is_empty());
}

#[test]
fn refreshes_index_when_symbol_file_is_deleted() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "def orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        initial_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );

    fs::remove_file(&helper).unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
    assert_eq!(stats.indexed_files, 1);
    assert_eq!(stats.rebuilt_files, 1);

    let updated_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(updated_trace.callees.is_empty());
    assert!(trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).is_err());
}
