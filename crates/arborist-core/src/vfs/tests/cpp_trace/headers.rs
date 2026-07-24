use super::*;

#[test]
fn traces_cpp_header_type_alias_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let header = workspace.join("aliases.hpp");
    let caller = workspace.join("caller.cpp");
    fs::write(
        &header,
        "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { using Alias = api::Counter; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"aliases.hpp\"\nnamespace app { int caller(int value) { return value; } }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &caller,
        Some(
            "#include \"aliases.hpp\"\nnamespace app { int caller(int value) { Alias counter{value}; return value; } }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "app::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn does_not_trace_cpp_header_type_aliases_moved_after_the_caller_in_virtual_source() {
    let workspace = temp_workspace();
    let header = workspace.join("aliases.hpp");
    let caller = workspace.join("caller.cpp");
    fs::write(
        &header,
        "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { using Alias = api::Counter; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"aliases.hpp\"\nnamespace app { int caller(int value) { return value; } }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &caller,
        Some(
            "namespace app { int caller(int value) { Alias counter{value}; return value; } }\n#include \"aliases.hpp\"\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "app::caller", TraceDirection::Both)
        .unwrap();
    assert!(trace.callees.is_empty());
}

#[test]
fn traces_cpp_type_aliases_from_virtual_local_headers() {
    let workspace = temp_workspace();
    let header = workspace.join("aliases.hpp");
    let caller = workspace.join("caller.cpp");
    fs::write(
        &caller,
        "#include \"aliases.hpp\"\nnamespace app { int caller(int value) { Alias counter{value}; return value; } }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &header,
        Some(
            "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { using Alias = api::Counter; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "app::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn traces_cpp_qualified_namespace_aliases_from_virtual_local_headers_in_order() {
    let workspace = temp_workspace();
    let header = workspace.join("imports.hpp");
    let caller = workspace.join("caller.cpp");
    fs::write(
        &caller,
        "#include \"imports.hpp\"\nint caller() { return detail::convert(1); }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &header,
        Some(
            "namespace implementation { int convert(int value) { return value; } }\nnamespace detail = implementation;\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["implementation::convert(int)"]
    );

    vfs.open_file(
        &caller,
        Some("int caller() { return detail::convert(1); }\n#include \"imports.hpp\"\n"),
    )
    .unwrap();
    let trace = vfs
        .trace_symbol_graph(&workspace, "caller", TraceDirection::Both)
        .unwrap();
    assert!(trace.callees.is_empty());
}
